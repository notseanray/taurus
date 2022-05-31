use crate::{utils::Sys, ws::CONFIG};
use chrono::{DateTime, Local, Datelike, Timelike};
use serde_derive::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    fs,
    time::{SystemTime, Instant, Duration},
};
use tokio::{process::Command, fs::remove_file};

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
        if self.file_path.is_none() || CONFIG.webserver_location.is_none() || CONFIG.webserver_prefix.is_none() {
            return "webserver not configured".to_owned();
        }
        let webserver_location = PathBuf::from(CONFIG.webserver_location.unwrap())
            .join(PathBuf::from("region"));
        if !webserver_location.exists() {
            if let Err(_) = fs::create_dir_all(webserver_location) {
                return "Unable to create region folder".to_owned();
            };
        }
        let dim_folder = match dim {
            "OW" => "/region",
            "NETHER" => "/DIM-1/region",
            "END" => "/DIM1/region",
        };
        let region_name = format!("r.{x}.{y}.mca");
        let full_path = PathBuf::from(self.file_path.unwrap())
            .join(PathBuf::from(dim_folder))
            .join(PathBuf::from(region_name));
        if !full_path.exists() {
            return "Region does not exists".to_owned();
        }
        if let Err(_) = fs::copy(full_path, webserver_location) {
            return "Failed to copy region into webserver folder".to_owned(); 
        }
        format!("{}/region/{region_name}", CONFIG.webserver_prefix.unwrap())
    }

    pub(crate) fn copy_structure(&self, name: &str) -> String {
        if self.file_path.is_none() || CONFIG.webserver_location.is_none() || CONFIG.webserver_prefix.is_none() {
            return "webserver not configured".to_owned();
        }
        let webserver_location = PathBuf::from(CONFIG.webserver_location.unwrap())
            .join(PathBuf::from("structure"));
        if !webserver_location.exists() {
            if let Err(_) = fs::create_dir_all(webserver_location) {
                return "Unable to create region folder".to_owned();
            };
        }
        let structure = PathBuf::from(self.file_path.unwrap())
            .join(PathBuf::from("structure"))
            .join(PathBuf::from(name));
        if !structure.exists() {
            return "Structure does not exists".to_owned();
        }
        if let Err(_) = fs::copy(structure, webserver_location) {
            return "Failed to copy structure into webserver folder".to_owned(); 
        }
        format!("{}/structure/{name}", CONFIG.webserver_prefix.unwrap())
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
        let structures = match PathBuf::from(self.file_path.unwrap())
            .join(PathBuf::from("structure")).read_dir() {
                Ok(v) => v,
                Err(_) => return "Unable to access structure folder".to_owned(),
            };
        let mut response = Vec::new();
        for file in structures {
            if let Ok(v) = file {
                if let Ok(m) = v.metadata() {
                    response.push(format!("{} ({})", 
                      v.file_name()
                      .to_string_lossy()
                      .to_string(), Self::bytes_to_human(m.len())));
                }
            }
        } 
        response.sort();
        response.join("\n")
    }

    pub(crate) async fn backup(&self, sys: &Sys, name: &str, backup_location: &str) -> String {
        if self.file_path.is_none() {
            return "Unable to reach file path".to_owned();
        }
        if !sys.sys_health_check() {
            return "Backup aborted due to system constraints".to_owned();
        }
        let cwd = PathBuf::from(self.file_path.clone().unwrap());
        let mut cwd = cwd.iter();
        cwd.next_back();
        let now: DateTime<Local> = Local::now();
        let backup_name = &format!("{name}_{:0>4}-{:0>2}-{:0>2}_{:0>2}_{:0>2}_{:0>2}.tar.gz", 
                                   now.year(), 
                                   now.month(), 
                                   now.day(),
                                   now.hour(),
                                   now.minute(),
                                   now.second());
        let start = Instant::now();
        let _ = Command::new("tar")
            .current_dir(cwd.as_path())
            .args(["-czf", backup_name])
            .kill_on_drop(true)
            .status()
            .await;
        format!("finished in {:.2} seconds", start.elapsed().as_millis() as f32 / 1000.0)
    }
}

pub(crate) fn delete_backups_older_than(name: &str, time: u64) {
    let dir = PathBuf::from(CONFIG.backup_location);
    if !dir.exists() {
        return;
    }
    let backups = match dir.read_dir() {
        Ok(v) => v,
        Err(_) => return,
    };
    for backup in backups {
        if let Ok(v) = backup {
            let fname = v.file_name().to_string_lossy().to_string();
            if name != "_" && (fname.len() > name.len() && &fname[..name.len()] == name) {
                continue;
            }
            if let Ok(m) = v.metadata() {
                let creation = match m.created() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let elapsed = match SystemTime::now().duration_since(creation) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if elapsed > Duration::from_secs(time) {
                    remove_file(dir);
                }
            }
        }
    } 
}

pub(crate) fn list_backups() -> String {
    let dir = PathBuf::from(CONFIG.backup_location);
    if !dir.exists() {
        match fs::create_dir_all(CONFIG.backup_location) {
            Ok(_) => {},
            Err(_) => return "Unable to create backup directory".to_owned(),
        };
        return "No backups stored at this time".to_owned();
    }
    let backups = match dir.read_dir() {
        Ok(v) => v,
        Err(_) => return "Unable to access backup directory".to_string(),
    };
    let mut response = Vec::new();
    for backup in backups {
        if let Ok(v) = backup {
            if !v.file_name().to_string_lossy().to_string().contains("tar.gz") {
                continue;
            }
            if let Ok(m) = v.metadata() {
                response.push(format!("{} ({})", 
                  v.file_name()
                  .to_string_lossy()
                  .to_string(), Game::bytes_to_human(m.len())));
            }
        }
    } 
    response.sort();
    response.join("\n")
}
