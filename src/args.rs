use super::config::load_config;

pub fn parse_args(args: Vec<String>) {
    if args.len() < 1 {
        return;
    }
    let parseable = args.iter().skip(1);
    for i in parseable {
        match i.as_str() {
            "check" => {
                load_config(args[0].to_owned()[..args.len() - 6].to_string());
            }
            _ => {
                eprintln!("*warn: invalid argument -> {}", i);
                println!("*info: skipping argument");
                continue;
            }
        };
    }
}
