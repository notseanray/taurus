use crate::{bridge::Session, utils::Sys, ws::CONFIG};
use chrono::{DateTime, Datelike, Local, Timelike};
use serde_derive::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::SystemTime};
use tokio::{
    fs::{create_dir_all, remove_file},
    process::Command,
};

// seconds, must be less than 3600
const SLOTTED_BACKUP_EPSILON: u64 = 1800;

// options for a session running a server that contains a chat bridge
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Game {
    file_path: Option<String>,
    pub backup_path: Option<String>,
    pub backup_interval: Option<u64>,
    pub backup_keep: Option<u64>,
    // TODO make mutally exclusive from above
    pub hourly_slots: Option<u64>,
    pub daily_slots: Option<u64>,
    pub weekly_slots: Option<u64>,
    pub monthly_slots: Option<u64>,
    in_game_cmd: Option<bool>,
    pub chat_bridge: Option<bool>,
}

#[derive(Clone)]
pub(crate) struct BackupSlot {
    pub name: String,
    // time backup has existed since cycle
    pub elapsed_time: u64,
}

impl PartialEq for BackupSlot {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

macro_rules! create_backup {
    ($self:expr, $sys:expr, $name:expr, $clock:expr, $interval:expr, $backup_type:expr) => {{
        let timing = $backup_type.is_some() && $clock % $interval == 0;
        if timing {
            let _ = $self
                .backup(
                    $sys,
                    $name.to_string(),
                    CONFIG.read().await.backup_location.clone(),
                )
                .await;
        };
        timing
    }};
}

impl Game {
    /* given a list of backup timestamps, new backup every hour
     * if the amount of hourly backups exceed the amount of slots, save one for weekly
     * if there are two slots for weekly
     *
     */
    pub(crate) async fn delete_slotted_backups(
        &self,
        name: &str,
        time: u64,
        backup_location: &str,
    ) {
        let dir = PathBuf::from(backup_location);
        if !dir.exists() {
            return;
        }
        let mut backups = match dir.read_dir() {
            Ok(v) => v
                .flatten()
                .filter_map(|x| {
                    if let Ok(v) = x.metadata() {
                        if let Ok(v) = v.created() {
                            Some((
                                x,
                                SystemTime::now().duration_since(v).unwrap().as_secs()
                                    / SLOTTED_BACKUP_EPSILON,
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>(),
            Err(_) => return,
        };
        // sort by least recent to most recent
        backups.sort_by(|l, r| r.1.partial_cmp(&l.1).unwrap());
        // backup time is duration since backup / SLOTTED_BACKUP_EPSILON
        // 6 hourly backup slots
        // 2 daily slots
        // if the last 6 hourly backup slots are full
        // keep the oldest until its age is over a day, consider it a daily
        let mut monthy = Vec::with_capacity(self.monthly_slots.unwrap_or(0) as usize);
        let mut weekly = Vec::with_capacity(self.weekly_slots.unwrap_or(0) as usize);
        let mut daily = Vec::with_capacity(self.daily_slots.unwrap_or(0) as usize);
        let mut hourly = Vec::with_capacity(self.hourly_slots.unwrap_or(0) as usize);
        for (backup, backup_time) in backups {
            let fname = backup.file_name().to_string_lossy().to_string();
            if name != "_" && !(fname.len() > name.len() && fname.starts_with(name)) {
                continue;
            }
            let backup_time = backup_time / SLOTTED_BACKUP_EPSILON;
            if let Some(v) = self.monthly_slots {
                if backup_time % (3600 * 24 * 7 * 30) / SLOTTED_BACKUP_EPSILON == 0 {
                    monthy.push(BackupSlot {
                        name: fname.clone(),
                        elapsed_time: backup_time,
                    });
                    let v = v as usize;
                    if monthy.len() > v {
                        for slot in &monthy.as_slice()[0..v] {
                            println!("delete {}", slot.name);
                            if true {
                                continue;
                            }
                            let _ = remove_file(
                                PathBuf::from(&CONFIG.read().await.backup_location)
                                    .join(slot.name.clone()),
                            )
                            .await;
                        }
                        monthy = monthy.as_slice()[v..].to_vec();
                    }
                }
            }
            if let Some(v) = self.weekly_slots {
                if backup_time % (3600 * 24 * 7) / SLOTTED_BACKUP_EPSILON == 0
                    && backup_time < (3600 * 24 * 7 * 30) / SLOTTED_BACKUP_EPSILON
                {
                    weekly.push(BackupSlot {
                        name: fname.clone(),
                        elapsed_time: backup_time,
                    });
                    let v = v as usize;
                    if weekly.len() > v {
                        for slot in &weekly.as_slice()[0..v] {
                            if monthy.contains(slot) {
                                continue;
                            }
                            if let Some(x) = weekly.last() {
                                if backup_time as isize
                                    == x.elapsed_time as isize
                                        - (3600 * 24 * 7 * 30 / SLOTTED_BACKUP_EPSILON as isize)
                                {
                                    continue;
                                }
                            }
                            if monthy.capacity() > 0 && daily.is_empty() {
                                continue;
                            }
                            println!("delete {}", slot.name);
                            if true {
                                continue;
                            }
                            let _ = remove_file(
                                PathBuf::from(&CONFIG.read().await.backup_location)
                                    .join(slot.name.clone()),
                            )
                            .await;
                        }
                        weekly = weekly.as_slice()[v..].to_vec();
                    }
                }
            }
            if let Some(v) = self.daily_slots {
                if backup_time % (3600 * 24) / SLOTTED_BACKUP_EPSILON == 0
                    && backup_time < (3600 * 24 * 7) / SLOTTED_BACKUP_EPSILON
                {
                    daily.push(BackupSlot {
                        name: fname.clone(),
                        elapsed_time: backup_time,
                    });
                    let v = v as usize;
                    if daily.len() > v {
                        for slot in &daily.as_slice()[0..v] {
                            if weekly.contains(slot) {
                                continue;
                            }
                            if let Some(x) = weekly.last() {
                                if backup_time as isize
                                    == x.elapsed_time as isize
                                        - (3600 * 24 * 7 / SLOTTED_BACKUP_EPSILON as isize)
                                {
                                    continue;
                                }
                            }
                            if weekly.capacity() > 0 && daily.is_empty() {
                                continue;
                            }
                            println!("delete {}", slot.name);
                            if true {
                                continue;
                            }
                            let _ = remove_file(
                                PathBuf::from(&CONFIG.read().await.backup_location)
                                    .join(slot.name.clone()),
                            )
                            .await;
                        }
                        daily = daily.as_slice()[v..].to_vec();
                    }
                }
            }
            if let Some(v) = self.hourly_slots {
                if backup_time % 3600 / SLOTTED_BACKUP_EPSILON == 0
                    && backup_time < (3600 * 24) / SLOTTED_BACKUP_EPSILON
                {
                    hourly.push(BackupSlot {
                        name: fname.clone(),
                        elapsed_time: backup_time,
                    });
                    let v = v as usize;

                    if hourly.len() > v {
                        for slot in &hourly.as_slice()[0..v] {
                            if daily.contains(slot) {
                                continue;
                            }
                            if let Some(x) = daily.last() {
                                if backup_time as isize
                                    == x.elapsed_time as isize
                                        - (3600 * 24 / SLOTTED_BACKUP_EPSILON as isize)
                                {
                                    continue;
                                }
                            }
                            if daily.capacity() > 0 && daily.is_empty() {
                                continue;
                            }
                            println!("delete {}", slot.name);
                            if true {
                                continue;
                            }
                            let _ = remove_file(
                                PathBuf::from(&CONFIG.read().await.backup_location)
                                    .join(slot.name.clone()),
                            )
                            .await;
                        }
                        hourly = hourly.as_slice()[v..].to_vec();
                    }
                }
            }
        }
    }

    fn is_slotted_backups(&self) -> bool {
        self.hourly_slots.is_some()
            || self.daily_slots.is_some()
            || self.weekly_slots.is_some()
            || self.monthly_slots.is_some()
    }

    pub(crate) async fn perform_slotted_backups(&self, clock: u64, sys: &Sys, name: &str) {
        // monthy
        if create_backup!(
            self,
            sys,
            name,
            clock,
            3600 * 24 * 7 * 30,
            self.monthly_slots
        ) {
            return;
        }
        // weekly
        if create_backup!(self, sys, name, clock, 3600 * 24 * 7, self.monthly_slots) {
            return;
        }
        // daily
        if create_backup!(self, sys, name, clock, 3600 * 24, self.monthly_slots) {
            return;
        }
        // hour
        let _ = create_backup!(self, sys, name, clock, 3600, self.monthly_slots);
    }

    pub(crate) async fn perform_scheduled_backups(&self, name: &str, time: u64, sys: &Sys) {
        let default_backup_location = &CONFIG.read().await.backup_location;
        let backup_location = self.backup_path.as_ref().unwrap_or(default_backup_location);
        if self.is_slotted_backups() {
            self.perform_slotted_backups(time, sys, name).await;
            self.delete_slotted_backups(name, time, backup_location)
                .await;
            return;
        }
        if self.backup_interval.is_none() {
            return;
        }
        if time % self.backup_interval.unwrap() == 0 {
            let _ = self
                .backup(
                    sys,
                    name.to_string(),
                    CONFIG.read().await.backup_location.clone(),
                )
                .await;
            if let Some(v) = self.backup_keep {
                delete_backups_older_than(name, v, backup_location).await;
            }
        }
    }

    pub(crate) async fn copy_region(&self, dim: &str, x: i32, y: i32) -> String {
        if self.file_path.is_none()
            || CONFIG.read().await.webserver_location.is_none()
            || CONFIG.read().await.webserver_prefix.is_none()
        {
            return "webserver not configured".to_owned();
        }
        if let Some(ws_l) = &CONFIG.read().await.webserver_location {
            let webserver_location = PathBuf::from(ws_l).join(PathBuf::from("region"));
            if !webserver_location.exists() && fs::create_dir_all(&webserver_location).is_err() {
                return "Unable to create region folder".to_owned();
            }
            let dim_folder = match dim {
                "OW" => "/region",
                "NETHER" => "/DIM-1/region",
                "END" => "/DIM1/region",
                _ => return "Unexpected region".to_owned(),
            };
            let region_name = format!("r.{x}.{y}.mca");

            if let Some(fp) = &self.file_path {
                let full_path = PathBuf::from(fp)
                    .join(PathBuf::from(dim_folder))
                    .join(PathBuf::from(&region_name));
                if !full_path.exists() {
                    return "Region does not exists".to_owned();
                }
                if fs::copy(full_path, webserver_location).is_err() {
                    return "Failed to copy region into webserver folder".to_owned();
                }
                if let Some(ws_p) = &CONFIG.read().await.webserver_prefix {
                    return format!("{}/region/{region_name}", ws_p);
                }
            }
        }
        "no file path specified".to_string()
    }

    pub(crate) async fn copy_structure(&self, name: &str) -> String {
        if self.file_path.is_none()
            || CONFIG.read().await.webserver_location.is_none()
            || CONFIG.read().await.webserver_prefix.is_none()
        {
            return "webserver not configured".to_owned();
        }
        if let Some(ws_l) = &CONFIG.read().await.webserver_location {
            let webserver_location = PathBuf::from(ws_l).join(PathBuf::from("structure"));
            if !webserver_location.exists() && fs::create_dir_all(&webserver_location).is_err() {
                return "Unable to create region folder".to_owned();
            }
            if let Some(fp) = &self.file_path {
                let structure = PathBuf::from(fp)
                    .join(PathBuf::from("structure"))
                    .join(PathBuf::from(name));
                if !structure.exists() {
                    return "Structure does not exists".to_owned();
                }
                if fs::copy(structure, webserver_location).is_err() {
                    return "Failed to copy structure into webserver folder".to_owned();
                }
            }
            if let Some(ws_p) = &CONFIG.read().await.webserver_prefix {
                return format!("{}/structure/{name}", ws_p);
            }
        }
        "no file path specified".to_string()
    }

    #[inline(always)]
    pub(crate) fn bytes_to_human(bytes: u64) -> String {
        match bytes {
            1073741824.. => format!("{:.2} GiB", bytes as f64 / 1073741824.0),
            1000000.. => format!("{:.2} MiB", bytes as f64 / 1000000.0),
            1000.. => format!("{:.2} KiB", bytes as f32 / 1000.0),
            _ => format!("{bytes} B"),
        }
    }

    pub(crate) fn list_structures(&self) -> String {
        if let Some(fp) = &self.file_path {
            let structures = match PathBuf::from(fp)
                .join(PathBuf::from("structure"))
                .read_dir()
            {
                Ok(v) => v,
                Err(_) => return "Unable to access structure folder".to_owned(),
            };
            let mut response = Vec::new();
            for file in structures.flatten() {
                if let Ok(m) = file.metadata() {
                    response.push(format!(
                        "{} ({})",
                        file.file_name().to_string_lossy(),
                        Self::bytes_to_human(m.len())
                    ));
                }
            }
            response.sort();
            return response.join("\n");
        }
        "no configured file path".to_string()
    }

    pub(crate) async fn backup(&self, sys: &Sys, name: String, backup_location: String) -> String {
        if self.file_path.is_none() {
            return "Unable to reach file path".to_owned();
        }
        if sys.sys_health_check() {
            return "Backup aborted due to system constraints".to_owned();
        }
        let cwd = PathBuf::from(self.file_path.clone().unwrap());
        if !cwd.as_path().exists() {
            match create_dir_all(&cwd).await {
                Ok(_) => {}
                Err(_) => return "Backup location does not exists".to_owned(),
            };
        }
        tokio::spawn(async move {
            let world_name = &cwd.iter().next_back().unwrap_or_default().to_string_lossy();
            let now: DateTime<Local> = Local::now();
            let backup_name = &format!(
                "{name}_{:0>4}-{:0>2}-{:0>2}_{:0>2}_{:0>2}_{:0>2}.tar.gz",
                now.year(),
                now.month(),
                now.day(),
                now.hour(),
                now.minute(),
                now.second()
            );
            let _ = Command::new("cp")
                .args([
                    "-ur",
                    &cwd.to_string_lossy(),
                    &format!("{}/", backup_location),
                ])
                .kill_on_drop(true)
                .status()
                .await;
            let _ = Command::new("tar")
                .current_dir(backup_location)
                .args(["-czf", backup_name, world_name])
                .kill_on_drop(true)
                .status()
                .await;
        });
        "starting new backup".to_string()
    }
}

pub(crate) async fn delete_backups_older_than(name: &str, time: u64, backup_location: &str) {
    let dir = PathBuf::from(backup_location);
    if !dir.exists() {
        return;
    }
    let backups = match dir.read_dir() {
        Ok(v) => v,
        Err(_) => return,
    };
    for backup in backups.flatten() {
        let fname = backup.file_name().to_string_lossy().to_string();
        if name != "_" && !(fname.len() > name.len() && fname.starts_with(name)) {
            continue;
        }
        if let Ok(m) = backup.metadata() {
            let creation = match m.created() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let elapsed = match SystemTime::now().duration_since(creation) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if elapsed.as_secs() > time {
                let _ =
                    remove_file(PathBuf::from(&CONFIG.read().await.backup_location).join(fname))
                        .await;
            }
        }
    }
}

pub(crate) async fn list_backups(backup_locations: &Vec<Session>) -> String {
    let mut response = Vec::new();
    let mut used_locations = Vec::with_capacity(backup_locations.len());
    for location in backup_locations {
        let dir = PathBuf::from(if let Some(v) = &location.game {
            match &v.backup_path {
                Some(v) => v.to_owned(),
                None => CONFIG.read().await.backup_location.to_string(),
            }
        } else {
            CONFIG.read().await.backup_location.to_owned()
        });
        // horrible, but temp fix for now to prevent reading the same path like 50 times
        if used_locations.contains(&dir) {
            continue;
        }
        used_locations.push(dir.clone());
        if !dir.exists() {
            match fs::create_dir_all(dir) {
                Ok(_) => {}
                Err(_) => continue,
            };
            continue;
        }
        let backups = match dir.read_dir() {
            Ok(v) => v,
            Err(_) => continue,
        };
        for backup in backups.flatten() {
            if !backup
                .file_name()
                .to_string_lossy()
                .to_string()
                .contains("tar.gz")
            {
                continue;
            }
            if let Ok(m) = backup.metadata() {
                response.push(format!(
                    "{} ({})",
                    backup.file_name().to_string_lossy(),
                    Game::bytes_to_human(m.len())
                ));
            }
        }
    }
    response.sort();
    response.join("\n")
}
