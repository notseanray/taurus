use crate::{exit, Config, Session};
use std::fs::read_dir;

pub fn parse_args(args: Vec<String>) {
    if args.len() < 1 {
        return;
    }
    let parseable = args.iter().skip(1);
    for i in parseable {
        let path: String = args[0].to_owned()[..args[0].len() - 6].to_string();
        match i.as_str() {
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
                let _: Config = Config::load_config(path.clone());
                let _: Vec<Session> = Config::load_sessions(path.to_owned());
                println!("*info: \x1b[32mcheck successful, exiting\x1b[0m");
                exit!();
            }
            "backup" => {
                if args.len() == 2 || args.len() > 3 {
                    println!(
                        "invalid usage
example usage:
    lupus backup ls         | list backups
    lupus backup rm file    | remove certain backup
    lupus backup rm all     | remove all backups
    lupus backup config     | backup config"
                    );
                    exit!();
                }

                if &args[1] == "ls" {
                    let config = Config::load_config(path.clone());
                    let mut backups = "backups: ".to_string();
                    for i in read_dir(config.backup_location).unwrap() {
                        let i = i.unwrap();
                        backups.push_str(&format!("\t{}", i.file_name().to_string_lossy()));
                    }
                    println!("{backups}");
                }
                exit!();
            }
            _ => {
                eprintln!("*warn: \x1b[33minvalid argument -> {}\x1b[0m", i);
                println!("*info: \x1b[33mskipping argument \x1b[0m");
                continue;
            }
        };
    }
}
