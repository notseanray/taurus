use serde_derive::Deserialize;
use std::fs::File;
use lupus::check_dir;
use std::fs;

#[derive(Deserialize)]
pub struct Session {
    pub name: String,
    pub description: String,
    pub host: String,
    pub game: Option<Game>,
    pub rcon: Option<Rcon>,
}

#[derive(Deserialize)]
pub struct Game {
    pub file_path: Option<String>,
    pub backup_keep: Option<u64>,
    pub in_game_cmd: bool,
}

#[derive(Deserialize)]
pub struct Rcon {
    ip: Option<String>,
    port: u64,
    password: String,
}

#[derive(Deserialize)]
pub struct Config {
    pub ws_ip: String,
    pub ws_port: u64,
    pub backup_location: String,
    pub scripts_file: Option<Vec<String>>,
    pub restart_script: Option<String>,
    pub recompile_directory: Option<String>,
}

pub fn load_config(path: String) -> Config {
    if !check_dir(path.to_owned() + "/config.toml") {
        template_files(path.to_owned());
        default_main_config(path.to_owned());
    }

    let data = match fs::read_to_string(path.to_owned() + "/config.toml") {
        Ok(t) => t,
        Err(e) => {
            eprintln!("*error: no config file found at {}!", path);
            eprintln!("*error: {}", e);
            eprintln!("*info: generating default config");
            default_main_config(path.to_owned());
            if !check_dir(path.to_owned() + "/config.toml") {
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

fn default_main_config(path: String) {
    File::create(path.to_owned() + "/config.toml")
        .expect("*error: failed to create default config file");

    let default = "# lupus main configuration file
# more information on each value at https://github.com/NotCreative21/lupus
# if a field is commented out it is optional and will either be prefilled
# or not used

# if no ip or port is used the default is 127.0.0.1:7500
ws_ip: '127.0.0.1'
ws_port: 7500
backup_location: ''
#scripts_file: ''
#restart_script: ''
#recompile_directory: ''
";

    fs::write(path.to_owned() + "/config.toml", default)
        .expect("*error: failed to write defaults to config file");

}

fn template_files(path: String) {
    let files = [
        "config.toml", 
        "servers.toml", 
        "scripts.toml"
    ];

    for i in files {
        if check_dir(path.to_owned() + "/" + i) { continue; }

        println!("*info: creating file: {}", i);

        File::create(path.to_owned() + i).expect("*error: failed to create default files");

        if i != "config.toml" { continue; }

        default_main_config(path.to_owned());
    }
}
