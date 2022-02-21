use lupus::*;
use std::fs;
use std::fs::File;
use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub ws_ip: String,
    pub ws_port: u64,
    pub backup_location: String,
    pub scripts_file: Option<Vec<String>>,
    pub restart_script: Option<String>,
    pub recompile_directory: Option<String>,
}

impl Config {
    pub fn load_config(path: String) -> Config {
        let config_path = &(path.to_owned() + "/config.json");
        if !check_exist(config_path) {
            Config::default(path.to_owned());
            Config::default_root_cfg(path.to_owned());
        }

        let data = match fs::read_to_string(path.to_owned() + "/config.toml") {
            Ok(t) => t,
            Err(e) => {
                eprintln!("*error: no config file found at {}!", path);
                eprintln!("*error: {}", e);
                eprintln!("*info: generating default config");
                Config::default_root_cfg(path.to_owned());
                if !check_exist(config_path) {
                    eprintln!("*fatal: could not read just generated config, exiting");
                    std::process::exit(1);
                }
                fs::read_to_string(path.to_owned() + "/config.toml").unwrap()
            }
        };

        let conf = match toml::from_str(&data) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("*error: {}", e);
                eprintln!("*fatal: invalid config file! exiting");
                std::process::exit(1);
            }
        };

        conf
    }

    fn default_root_cfg(path: String) {
        File::create(path.to_owned() + "/config.toml")
            .expect("*error: failed to create default config file");

        let default = "# lupus main configuration file
# more information on each value at https://github.com/NotCreative21/lupus
# if a field is commented out it is optional and will either be prefilled
# or not used

# if no ip or port is used the default is 127.0.0.1:7500
ws_ip = '127.0.0.1'
ws_port = 7500
backup_location = ''
#scripts_file = ''
#restart_script = ''
#recompile_directory = ''
    ";

        fs::write(path.to_owned() + "/config.toml", default)
            .expect("*error: failed to write defaults to config file");
    }

    pub fn load_sessions(path: String) -> Vec<Session> {
        if !check_exist(&(path.to_owned() + "/servers/")) {
            Config::default(path.to_owned());
        }

        let mut sessions = Vec::new();

        for i in fs::read_dir(path.to_owned() + "/servers/")
            .expect("*error: failed to read server directory")
        {
            let data = match fs::read_to_string(i.unwrap().path()) {
                Ok(t) => t,
                Err(_) => continue,
            };

            match toml::from_str(&data) {
                Ok(t) => sessions.push(t),
                Err(e) => {
                    eprintln!("*error: {}", e);
                    eprintln!("*fatal: invalid server config! exiting");
                    std::process::exit(1);
                }
            };
        }
        sessions
    }

    // TODO
    // check if servers.toml will function correctly
    fn default(path: String) {
        let files = ["config.toml", "servers/servers.toml", "scripts.toml"];

        for i in files {
            if check_exist(&(path.to_owned() + "/" + i)) {
                continue;
            }

            println!("*info: creating file: {}", i);

            File::create(path.to_owned() + i).expect("*error: failed to create default files");

            if i != "config.toml" {
                continue;
            }

            Config::default_root_cfg(path.to_owned());
        }
    }
}
