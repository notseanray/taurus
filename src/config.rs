use crate::ws::SESSIONS;
use crate::{bridge::Session, exit};
use log::error;
use rcon_rs::Client;
use serde_derive::{Deserialize, Serialize};
use serde_json::from_str;
use std::path::PathBuf;
use std::{fs, fs::File};
use tokio::process::Command;

// main config
#[derive(Deserialize)]
pub(crate) struct Config {
    pub ws_ip: String,
    pub ws_port: u64,
    pub ws_password: String,
    pub webserver_location: Option<String>,
    pub webserver_prefix: Option<String>,
    pub backup_location: String,
    pub scripts: Option<Vec<Script>>,
    pub restart_script: Option<String>,
    pub recompile_directory: Option<String>,
}

// for any optional scripts, if either interval or absolute is 0 then we will use the non zero
// file, otherwise the script never gets executed.
#[derive(Deserialize)]
pub(crate) struct Script {
    pub description: String,
    pub interval: Option<u64>,
    pub start_unix: Option<u64>,
    pub shell_cmd: Option<String>,
    pub session_name: Option<String>,
    pub rcon_cmd: Option<String>,
}

impl Script {
    pub(crate) async fn run(&self) {
        if let Some(rc) = &self.rcon_cmd {
            if let Some(sn) = &self.session_name {
                for session in &*SESSIONS {
                    if &session.name != sn {
                        continue;
                    }
                    if let Some(r) = &session.rcon {
                        let _ = r.rcon_send(rc).await;
                    }
                }
            }
        }
        if let Some(v) = &self.shell_cmd {
            let shell_command: Vec<&str> = v.split_whitespace().collect();
            let _ = Command::new(shell_command[0])
                .args(&shell_command[1..])
                .spawn();
        }
    }
}

// The ip being None defaults to localhost
#[derive(Deserialize, Clone, Serialize)]
pub(crate) struct Rcon {
    pub ip: Option<String>,
    pub port: u16,
    pub password: String,
}

impl Rcon {
    pub(crate) async fn rcon_send(&self, msg: &str) -> Result<(), std::io::Error> {
        let mut conn = self.connect().await?;
        if conn.auth(&self.password).is_ok() {
            let _ = conn.send(msg, None);
        };
        Ok(())
    }
    pub(crate) async fn rcon_send_with_response(
        &self,
        msg: &str,
    ) -> Result<Option<String>, std::io::Error> {
        let mut conn = self.connect().await?;
        if conn.auth(&self.password).is_ok() {
            return match conn.send(msg, None) {
                Ok(v) => Ok(Some(v)),
                Err(_) => Ok(None),
            };
        };
        Ok(None)
    }

    async fn connect(&self) -> Result<Client, std::io::Error> {
        Client::new(
            &self.ip.clone().unwrap_or_else(|| "localhost".to_string()),
            &self.port.to_string(),
        )
    }
}

impl Config {
    pub(crate) fn load_config<T>(path: T) -> Self
    where
        T: AsRef<str> + std::fmt::Display,
    {
        let path = path.to_string();
        let config_path = &(path.to_owned() + "/config.json");
        if !PathBuf::from(config_path).exists() {
            Config::default(&path);
            Config::default_root_cfg(path.to_owned());
        }

        let data = match fs::read_to_string(path.to_owned() + "/config.json") {
            Ok(t) => t,
            Err(e) => {
                error!("no config file found at {}!", path);
                eprintln!("*info: generating default config");
                Config::default_root_cfg(path.to_owned());
                if !PathBuf::from(config_path).exists() {
                    error!("could not read just generated config, exiting");
                    exit!();
                }
                fs::read_to_string(path + "/config.json").unwrap()
            }
        };

        let conf: Self = match from_str(&data) {
            Ok(t) => t,
            Err(e) => {
                error!("invalid config file! exiting");
                exit!();
            }
        };
        if conf.scripts.is_some() {
            for i in conf.scripts.as_ref().unwrap() {
                println!("*info: found script: {}", i.description);
            }
        }
        conf
    }

    fn default_root_cfg(path: String) {
        File::create(path.to_owned() + "/config.json").unwrap();

        let default = r#"{
    "ws_ip": "127.0.0.1",
    "ws_port": "7500",
    "backup_location": "",
    "scripts": [
        {
            "description": "very cool script",
            "interval": 0,
            "absolute": 0,
            "shell_cmd": "",
            "mc_cmd": ""
        }
    ],
    "restart_script": "",
    "recompile_directory": ""
}"#;

        fs::write(path + "/config.json", default).unwrap();
    }

    pub(crate) fn load_sessions(path: String) -> Vec<Session> {
        if !PathBuf::from(&(path.to_owned() + "/servers/")).exists() {
            Self::default(&path);
        }

        let mut sessions = Vec::new();

        for i in fs::read_dir(path + "servers/")
            .expect("*error: \x1b[31mfailed to read server directory\x1b[0m")
        {
            let i = i.unwrap();
            println!("*info: reading: {:#?}", i.path().display().to_string());

            let data = match fs::read_to_string(i.path()) {
                Ok(t) => t,
                Err(_) => continue,
            };

            match from_str(&data) {
                Ok(t) => sessions.push(t),
                Err(e) => {
                    error!("{:?}", e);
                    error!("invalid server config! exiting");
                    exit!();
                }
            };
        }
        sessions
    }

    fn default(path: &str) {
        let current_directory = PathBuf::from(path);
        let _ = fs::create_dir(current_directory.join("servers"));
        let _ = fs::File::create(current_directory.join("./server/servers.json"));
        let _ = fs::File::create(current_directory.join("scripts.json"));
    }
}
