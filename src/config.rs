use crate::{bridge::Session, exit, utils::check_exist, error};
use serde_derive::Deserialize;
use serde_json::from_str;
use std::path::PathBuf;
use std::{fs, fs::File};

// main config
#[derive(Deserialize)]
pub struct Config {
    pub ws_ip: String,
    pub ws_port: u64,
    pub backup_location: String,
    pub scripts: Option<Vec<Script>>,
    pub restart_script: Option<String>,
    pub recompile_directory: Option<String>,
}

// for any optional scripts, if either interval or absolute is 0 then we will use the non zero
// file, otherwise the script never gets executed.
#[derive(Deserialize)]
pub struct Script {
    pub description: String,
    pub interval: u64,
    pub absolute: u64,
    pub shell_cmd: String,
    pub mc_cmd: String,
}

// The ip being None defaults to localhost
#[derive(Deserialize, Clone)]
pub struct Rcon {
    pub ip: Option<String>,
    pub port: u64,
    pub password: String,
}

impl Config {
    pub fn load_config<T>(path: T) -> Self
    where
        T: AsRef<str> + std::fmt::Display,
    {
        let path = path.to_string();
        let config_path = &(path.to_owned() + "/config.json");
        if !check_exist(config_path) {
            Config::default(&path);
            Config::default_root_cfg(path.to_owned());
        }

        let data = match fs::read_to_string(path.to_owned() + "/config.json") {
            Ok(t) => t,
            Err(e) => {
                error!(format!("no config file found at {}!", path));
                error!(e);
                eprintln!("*info: generating default config");
                Config::default_root_cfg(path.to_owned());
                if !check_exist(config_path) {
                    error!("could not read just generated config, exiting");
                    exit!();
                }
                fs::read_to_string(path + "/config.json").unwrap()
            }
        };

        let conf: Self = match from_str(&data) {
            Ok(t) => t,
            Err(e) => {
                error!(e);
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

        fs::write(path.to_owned() + "/config.json", default).unwrap();
    }

    pub fn load_sessions(path: String) -> Vec<Session> {
        if !check_exist(&(path.to_owned() + "/servers/")) {
            Self::default(&path);
        }

        let mut sessions = Vec::new();

        for i in fs::read_dir(path.to_owned() + "servers/")
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
                    error!(e);
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
