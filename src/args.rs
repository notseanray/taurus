use crate::{error, exit, info, warn, ws::PATH, Config, Session};
use std::fs::{read_dir, remove_file};

pub(crate) fn parse_args(args: Vec<String>) {
    if args.is_empty() {
        return;
    }
    let parseable = args.iter().skip(1);
    for (e, arg) in parseable.enumerate() {
        match arg.as_str() {
            "help" => {
                println!(
                    "valid arguments
    check       | check config
    backup      | operate on backups
    help        | this menu"
                );
                exit!();
            }
            "check" => {
                let _: Config = Config::load_config(PATH.clone());
                let _: Vec<Session> = Config::load_sessions(PATH.to_owned());
                info!("check successful, exiting");
                exit!();
            }
            "backup" => {
                if args.len() == 2 || args.len() > 3 {
                    println!(
                        "invalid usage
example usage:
    taurus backup ls         | list backups
    taurus backup rm file    | remove certain backup
    taurus backup rm all     | remove all backups"
                    );
                    exit!();
                }

                let config = Config::load_config(PATH.clone());
                match args[e + 1].as_str() {
                    "ls" => {
                        let mut backups = "backups: ".to_string();
                        for i in read_dir(config.backup_location).unwrap() {
                            let i = i.unwrap();
                            backups.push_str(&format!("\t{}", i.file_name().to_string_lossy()));
                        }
                        println!("{backups}");
                    }
                    "rm" => {
                        if args.len() < 3 {
                            error!("invalid args! please specify file to operate on");
                            exit!();
                        }

                        if &args[e + 2] == "all" {
                            let mut files = 0;
                            for i in read_dir(config.backup_location).unwrap() {
                                let i = match i {
                                    Ok(v) => v,
                                    Err(e) => {
                                        error!(e);
                                        error!("failed to remove file");
                                        exit!();
                                    }
                                };
                                match remove_file(i.path()) {
                                    Ok(_) => {
                                        info!(format!("successfully removed {:#?}", i.file_name()));
                                        files += 1;
                                    }
                                    Err(e) => {
                                        error!(format!("failed to remove file due to: {e}"));
                                    }
                                };
                            }
                            info!(format!("*info: removed {files} files, exiting now"));
                            exit!();
                        }

                        match remove_file(config.backup_location + &args[e + 2]) {
                            Ok(_) => {}
                            Err(e) => {
                                error!(format!("failed to remove file due to: {e}"));
                                exit!();
                            }
                        };
                    }
                    _ => {}
                };
                exit!();
            }
            _ => {
                warn!(format!("invalid argument -> {}", arg));
                warn!("skipping argument");
                continue;
            }
        };
    }
}
