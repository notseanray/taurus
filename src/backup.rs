use crate::{utils::Sys, ws::CONFIG};
use chrono::{DateTime, Datelike, Local, Timelike};
use serde_derive::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};
use tokio::{
    fs::{create_dir_all, remove_file},
    process::Command,
};

// options for a session running a server that contains a chat bridge
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Game {
    file_path: Option<String>,
    pub backup_interval: Option<u64>,
    pub backup_keep: Option<u64>,
    in_game_cmd: Option<bool>,
    pub chat_bridge: Option<bool>,
}

impl Game {
    pub(crate) fn copy_region(&self, dim: &str, x: i32, y: i32) -> String {
        if self.file_path.is_none()
            || CONFIG.webserver_location.is_none()
            || CONFIG.webserver_prefix.is_none()
        {
            return "webserver not configured".to_owned();
        }
        if let Some(ws_l) = &CONFIG.webserver_location {
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
                if let Some(ws_p) = &CONFIG.webserver_prefix {
                    return format!("{}/region/{region_name}", ws_p);
                }
            }
        }
        "no file path specified".to_string()
    }

    pub(crate) fn copy_structure(&self, name: &str) -> String {
        if self.file_path.is_none()
            || CONFIG.webserver_location.is_none()
            || CONFIG.webserver_prefix.is_none()
        {
            return "webserver not configured".to_owned();
        }
        if let Some(ws_l) = &CONFIG.webserver_location {
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
            if let Some(ws_p) = &CONFIG.webserver_prefix {
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
        if !sys.sys_health_check() {
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

pub(crate) async fn delete_backups_older_than(name: &str, time: u64) {
    let dir = PathBuf::from(&CONFIG.backup_location);
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
            if elapsed > Duration::from_secs(time) {
                let _ = remove_file(PathBuf::from(&CONFIG.backup_location).join(fname)).await;
            }
        }
    }
}

pub(crate) fn list_backups() -> String {
    let dir = PathBuf::from(&CONFIG.backup_location);
    if !dir.exists() {
        match fs::create_dir_all(&CONFIG.backup_location) {
            Ok(_) => {}
            Err(_) => return "Unable to create backup directory".to_owned(),
        };
        return "No backups stored at this time".to_owned();
    }
    let backups = match dir.read_dir() {
        Ok(v) => v,
        Err(_) => return "Unable to access backup directory".to_string(),
    };
    let mut response = Vec::new();
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
    response.sort();
    response.join("\n")
}
