use crate::{Config, Session};

pub fn parse_args(args: Vec<String>) {
    if args.len() < 1 {
        return;
    }
    let parseable = args.iter().skip(1);
    for i in parseable {
        match i.as_str() {
            "check" => {
                let path: String = args[0].to_owned()[..args[0].len() - 6].to_string();
                let _: Config = Config::load_config(path.clone());
                let _: Vec<Session> = Config::load_sessions(path.to_owned());
                println!("*info: \x1b[32mcheck successful, exiting\x1b[0m");
                std::process::exit(0);
            },
            "backup" => {
                if args.len() == 1 || args.len() > 3 {
                    println!("invalid usage
example usage:
    lupus backup ls         | list backups
    lupus backup rm file    | remove certain backup
    lupus backup rm all     | remove all backups
    lupus backup config     | backup config");
                }
                
                std::process::exit(0);
            }
            _ => {
                eprintln!("*warn: \x1b[33minvalid argument -> {}\x1b[0m", i);
                println!("*info: \x1b[33mskipping argument \x1b[0m");
                continue;
            }
        };
    }
}
