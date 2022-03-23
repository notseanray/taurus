use crate::{exit, utils::check_exist};
use std::ops::Deref;
use serde_derive::Deserialize;
use serde_json::from_str;
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

// store configuration for each session, description is purely for telling what it is
#[derive(Deserialize, Clone)]
pub struct Session {
    pub name: String,
    pub description: Option<String>,
    pub host: String,
    pub game: Option<Game>,
    pub rcon: Option<Rcon>,
}

// options for a session running a server that contains a chat bridge
#[derive(Deserialize, Clone)]
pub struct Game {
    pub file_path: Option<String>,
    pub backup_interval: Option<usize>,
    pub backup_keep: Option<usize>,
    pub in_game_cmd: Option<bool>,
    pub lines: Option<usize>,
}

// The ip being None defaults to localhost
#[derive(Deserialize, Clone)]
pub struct Rcon {
    pub ip: Option<String>,
    pub port: u64,
    pub password: String,
}

impl Config {
    pub fn load_config<T>(path: T) -> Config 
            where 
                T: Deref<Target=str> + std::fmt::Display 
        {
        let path = path.to_string();
        let config_path = &(path.to_owned() + "/config.json");
        if !check_exist(&config_path.as_str()) {
            Config::default(path.to_owned());
            Config::default_root_cfg(path.to_owned());
        }

        let data = match fs::read_to_string(path.to_owned() + "/config.json") {
            Ok(t) => t,
            Err(e) => {
                eprintln!("*error: \x1b[31mno config file found at {}!\x1b[0m", path);
                eprintln!("*error: \x1b[31m{}\x1b[0m", e);
                eprintln!("*info: generating default config");
                Config::default_root_cfg(path.to_owned());
                if !check_exist(&config_path.as_str()) {
                    eprintln!(
                        "*fatal: \x1b[31mcould not read just generated config, exiting\x1b[0m"
                    );
                    exit!();
                }
                fs::read_to_string(path.to_owned() + "/config.json").unwrap()
            }
        };

        let conf: Config = match from_str(&data) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("*error: \x1b[31m{}\x1b[0m", e);
                panic!("*fatal: \x1b[31minvalid config file! exiting\x1b[0m");
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
        File::create(path.to_owned() + "/config.json")
            .expect("*error: \x1b[31mfailed to create default config file\x1b[0m");

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

        fs::write(path.to_owned() + "/config.json", default)
            .expect("*error: \x1b[31mfailed to write defaults to config file\x1b[0m");
    }

    pub fn load_sessions(path: String) -> Vec<Session> {
        if !check_exist(&(path.to_owned() + "/servers/")) {
            Config::default(path.to_owned());
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
                    eprintln!("*error: \x1b[31m{}\x1b[0m", e);
                    panic!("*fatal: \x1b[31minvalid server config! exiting\x1b[0m");
                }
            };
        }
        sessions
    }

    // TODO
    // check if servers json will function correctly
    fn default(path: String) {
        let files = ["servers/servers.json", "scripts.json"];

        Config::default_root_cfg(path.to_owned());
        for i in files {
            if check_exist(&(path.to_owned() + "/" + i)) {
                continue;
            }

            println!("*info: creating file: {}", i);

            File::create(path.to_owned() + i)
                .expect("*error: \x1b[31mfailed to create default files\x1b[0m");
        }
    }
}
