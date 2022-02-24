use std::fs::{
    read_dir, 
    remove_file
};
use crate::{
    exit, 
    Config, 
    Session
};

pub fn parse_args(args: Vec<String>) {
    if args.len() < 1 { return; }
    let parseable = args.iter().skip(1);
    let path: String = args[0].to_owned()[..args[0].len() - 6].to_string();
    for (e, i) in parseable.enumerate() {
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
    lupus backup rm all     | remove all backups"
                    );
                    exit!();
                }

                let config = Config::load_config(path.clone());
                match args[e + 1].as_str() {
                    "ls" => {
                        let mut backups = "backups: ".to_string();
                        for i in read_dir(config.backup_location).unwrap() {
                            let i = i.unwrap();
                            backups.push_str(&format!("\t{}", i.file_name().to_string_lossy()));
                        }
                        println!("{backups}");
                    },
                    "rm" => {
                        if args.len() < 3 {
                            panic!("\x1b[31m*error:\x1b[0m invalid args! please specify file to operate on");
                        }

                        if &args[e + 2] == "all" {
                            let mut files = 0;
                            for i in read_dir(config.backup_location).unwrap() {
                                let i = match i {
                                    Ok(v) => v,
                                    Err(e) => panic!("\x1b[31m*error:\x1b[0m failed to read file due to: {e}") 
                                };
                                match remove_file(i.path()) {
                                    Ok(_) => {
                                        println!("*info: successfully removed {:#?}", i.file_name());
                                        files += 1;
                                    },
                                    Err(e) => panic!("\x1b[31m*error:\x1b[0m failed to remove file due to: {e}")
                                };
                            }
                            println!("*info: removed {files} files, exiting now");
                            exit!();
                        }

                        match remove_file(config.backup_location + &args[e + 2]) {
                            Ok(_) => {},
                            Err(e) => panic!("\x1b[31m*error:\x1b[0m failed to remove file due to: {e}")
                        };
                    },
                    _ => {}
                };
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
