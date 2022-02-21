use chrono::prelude::*;
use lupus::*;
use std::fs;
use std::process::Command;
use std::time::SystemTime;
use sysinfo::{System, SystemExt};

pub async fn backup(
    args: Option<Vec<String>>,
    keep_time: usize,
    backup_dir: String,
    backup_store: String,
    btime: usize,
) -> String {
    // if either the directory we are attempting to backup, or the resulting location does not
    // exists then we can just return
    if !check_exist(&backup_dir.to_owned()) || !check_exist(&backup_store.to_owned()) {
        eprintln!(
            "*error: please check your config! either backup directory or backup store is invalid!"
        );
        return "*error: backup directory does not exist".to_string();
    }

    if args.is_some() {
        let args = args.to_owned().unwrap();

        // if the argument for listing the backups exists, then we can create a new vector to store
        // the names and iterate through the files in the backup directory
        //
        // it's quite nice to have the backup size in the folder name so we add that as an element
        if args.to_owned().contains(&"ls".to_owned()) {
            let mut backupn: Vec<u64> = Vec::new();

            let mut youngest = keep_time as u64;

            for i in fs::read_dir(&backup_store).expect("failed to read backup dir!") {
                let i = i.ok().unwrap().path();

                if i.is_file() {
                    let file_name = i.file_name().unwrap().to_string_lossy().into_owned();

                    if file_name.contains("_") && file_name.contains("-") {
                        let file_name = file_name.replace("_", "").replace("-", "");

                        if file_name[10..22].parse::<u64>().is_ok() {
                            backupn.push(file_name[10..22].parse().ok().unwrap());
                        }
                    }

                    // to determine when the next backup will be we find the age of the youngest,
                    // that was the latest backup and then we will know when the next backup will
                    // be
                    if let Ok(time) = &i.metadata().ok().unwrap().created() {
                        if SystemTime::now()
                            .duration_since(time.to_owned())
                            .ok()
                            .unwrap()
                            .as_secs()
                            < youngest
                        {
                            youngest = SystemTime::now()
                                .duration_since(time.to_owned())
                                .ok()
                                .unwrap()
                                .as_secs();
                        }
                    }
                }
            }

            let mut backups: String = String::new();
            backupn.sort_unstable();
            for i in &backupn {
                let i = i.to_string().to_owned();

                let entry = format!(
                    "{}-{}-{}_{}_{}.tar.gz\n",
                    &i[..4],
                    &i[4..6],
                    &i[6..8],
                    &i[8..10],
                    &i[10..12]
                );

                backups.push_str(&entry);
            }

            return format!(
                "```Backups are displayed in UTC time: (YYYY/MM/DD/HH/mm/ss)
{}
Last Backup: {}s ago, Next Backup in: {}s```",
                backups,
                &youngest,
                btime - youngest as usize,
            );
        }
    }

    if args.is_some() {
        let args = args.unwrap();

        if args.to_owned().contains(&"rm".to_owned()) {
            let args = args.to_owned();

            let mut index = 0;

            // find which arg is the rm and check if there is a file name specified after it, if
            // there is none then we know to return
            for (i, e) in args.iter().enumerate() {
                if e == &"rm" {
                    index = i;
                }
            }

            // if the args are not long enough or do not match the conditions then we can just skip
            // them
            if args.len() < index + 1 || args.len() < 3 {
                return "*error: invalid argument".to_string();
            }

            // find the absolute path of the file that we need to delete
            let path = format!("{}/lupus-{}", backup_store, args[index + 1]);

            // check if it exists, if it doesn't then we must say that otherwise we can continue
            if !check_exist(&path.to_owned()) {
                return "*error: location does not exist on file system".to_string();
            }

            /*
            // now we can attempt to remove the file, if it is successful then we can say that,
            // otherwise we can notify through discord that the backup failed
            match fs::remove_file(path) {
                return "removed backup successfully".to_string();
            };*/
        }

        // schedule the creation of a new backup
        if args.to_owned().contains(&"new".to_owned()) {
            return new(
                backup_store.to_owned(),
                backup_dir.to_owned(),
                keep_time.try_into().unwrap(),
            );
        }

        // the lock file in the /tmp directory controls if we can execute backup commands, this is
        // to ensure that there aren't any issues with the backups getting spammed too fast for the
        // file system to handle, sometimes the lock will remain there though and we will have to
        // manually clear it
        if args.to_owned().contains(&"unlock".to_owned()) {
            if !check_exist("/tmp/HypnosCore-Backup.lock") {
                return "lock file does not exist".to_string();
            }

            fs::remove_file("/tmp/HypnosCore-Backup.lock")
                .expect("*error: failed to delete lock file!");

            return "lock file removed".to_string();
        }

        if args.to_owned().contains(&"lock".to_owned()) {
            if check_exist("/tmp/lupus-backup.lock") {
                return "lock file already exist".to_string();
            }

            fs::File::create("/tmp/HypnosCore-Backup.lock")
                .expect("*error: failed to delete lock file!");

            return "lock file created".to_string();
        }

        return "*error: expected additional argument".to_string();
    }
    // remove any zombie processes that were created during the process
    reap();

    return new(
        backup_store.to_owned(),
        backup_dir.to_owned(),
        keep_time.try_into().unwrap(),
    );
}

// big function to handle all the backup processes
fn new(backup_store: String, backup_dir: String, keep_time: u64) -> String {
    // if it is not from the command, we know it's scheduled and therefore we can create a new
    // archive and store it
    let mut sys = System::new_all();
    sys.refresh_disks_list();
    sys.refresh_disks();

    // check the disk to ensure that a backup is safe
    let (u, t, i) = check_disk(&sys);

    // if there is an issue then we can just skip and continue
    if (i as u16 * 100) == 10 {
        eprintln!(
            "*error: backup skipped due to low disk space on index: {}",
            i
        );
        return "Disk space is low! Skipping backup".to_string();
    }

    // if the lock file exists then something could be wrong, we will skip if this happens
    if check_exist("/tmp/HypnosCore-Backup.lock") {
        eprintln!("*warn: backup lock file exists! skipping backup");
        return "Lock file exists, skipping backup".to_string();
    }

    // create the lock file while we perform backups
    fs::File::create("/tmp/HypnosCore-Backup.lock").expect("failed to create lock file");

    // so we can keep track of how it takes to backup, we will measure it
    let btime = std::time::Instant::now();

    let root_res = format!("{}/root_copy/", &backup_store);

    // if the folder doesn't exists for the root copy, create it
    if !check_exist(&root_res.to_owned()) {
        println!("*warn: failed to find root copy directory, creating it instead");
        fs::create_dir_all(root_res.to_owned()).expect("*error: failed to create root_cpy dir");
    }

    // copy the directory into the backup directory, it is called the 'root copy' because it is a
    // direct copy of the folder, we use the option -u to only copy the latest updates and ensure
    // that backing up is speedy
    let root_cpy = Command::new("cp")
        .args(["-ur", &backup_dir, &root_res])
        .status()
        .expect("*error: failed to update root copy");

    // notify of the output of the backup
    if root_cpy.success() {
        println!("*info: updated root copy in {}", &backup_store);
    } else {
        eprintln!("*error: failed to update root copy");
        return "failed to update root copy".to_string();
    }

    // name the new backup based on the time it was taken
    let backup_name = format!(
        "{}/HypnosCore-{}.tar.gz",
        backup_store,
        &Utc::now().to_string().replace(":", "_").replace(" ", "_")[..16]
    );

    let root_backup = format!("{}/root_copy", &backup_store);

    // create a tar archive of the backup
    let backupcmd = Command::new("tar")
        .args(["-czf", &backup_name, &root_backup])
        .status()
        .expect("failed to create backup");

    // if it was a success, we can remove copies older than the specified time, otherwise we can
    // continue
    if backupcmd.success() {
        println!(
            "*info: creating new backup: {}, storage space usage: {:.2}%",
            backup_name,
            (u / t) * 100.0
        );
        println!(
            "*info: finished creating backup in time: {:#?}",
            btime.elapsed()
        );

        // iterate through all the backups we currently have, if any were created the specified
        // amount of seconds ago or more then we need to remove them so that the drive doesn't fill
        // up
        for i in fs::read_dir(&backup_store).expect("failed to read backup dir!") {
            let i = i.ok().unwrap().path();
            if i.is_file() {
                if let Ok(time) = &i.metadata().ok().unwrap().created() {
                    if SystemTime::now()
                        .duration_since(time.to_owned())
                        .ok()
                        .unwrap()
                        .as_secs()
                        >= keep_time
                    {
                        fs::remove_file(i).expect("*error: could not remove old backup");
                    }
                }
            }
        }
        println!("*info: backup cycle complete");
    } else {
        eprintln!("*error: failed to create backup!");
        eprintln!("*info: due to this, skipping deletion of stored backups");
    }

    // remove the now uneeded lock file
    fs::remove_file("/tmp/HypnosCore-Backup.lock").expect("failed to remove backup lock file!");
    return "Successfully created new backup".to_string();
}
