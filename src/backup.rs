use crate::utils::{check_exist, reap, Sys};
use chrono::{DateTime, Utc};
use serde_derive::Deserialize;
use std::{
    ffi::OsStr,
    fs::{self, read_dir},
    os::unix::prelude::OsStrExt,
    process::Command,
    time::SystemTime,
};
use walkdir::WalkDir;

// options for a session running a server that contains a chat bridge
#[derive(Deserialize, Clone)]
pub struct Game {
    file_path: Option<String>,
    backup_interval: Option<usize>,
    backup_keep: Option<usize>,
    in_game_cmd: Option<bool>,
    pub chat_bridge: Option<bool>,
}

impl Game {
    pub fn backup(&self, backup_location: &str) {
        if self.backup_interval.is_none() || self.file_path.is_none() {
            return;
        }
        if let Some(v) = &self.file_path {
            // use iterator for path
            let folders: Vec<&str> = v.split("/").collect();
            let name = folders[folders.len() - 1];
            let file_name = format!(
                "{name}-{}",
                &DateTime::<Utc>::from(SystemTime::now()).to_rfc3339()
            );
            let _ = Command::new("tar")
                .args([
                    "--create",
                    "--gzip",
                    &format!("--listed-incremental={backup_location}/{name}/{name}.sngz"),
                    &format!(
                        "--file={backup_location}/{name}/{}",
                        &file_name[..file_name.len() - 6]
                    ),
                    v,
                ])
                .spawn();
        }
    }

    pub fn list(backup_location: &str) -> String {
        let mut response = String::new();
        for folder in read_dir(backup_location) {
            for file in folder {
                let file = match file {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if OsStr::from_bytes(b"tar.gz") == file.file_name() {
                    response.push_str(&file.file_name().to_string_lossy().to_string());
                }
            }
        }
        response
    }

    pub fn check_for_tar() {}
}
