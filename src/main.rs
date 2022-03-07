mod args;
mod backup;
mod bridge;
mod config;
mod utils;
mod ws;
use args::parse_args;
use backup::backup;
use bridge::{gen_pipe, replace_formatting, set_lines, update_messages};
use config::{Config, Session};
use std::{
    collections::HashMap,
    convert::Infallible,
    env,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use utils::{send_to_clients, Clients};
use warp::Filter;
use ws::ws_handler;

lazy_static::lazy_static! {
    static ref ARGS: Vec<String> = env::args().collect();
    static ref PATH: String = ARGS[0].to_owned()[..ARGS[0].len() - 6].to_string();
    static ref SESSIONS: Vec<Session> = Config::load_sessions(PATH.to_owned());
}

#[tokio::main]
async fn main() {
    let startup = Instant::now();

    let config = Config::load_config(PATH.to_owned());

    if ARGS.len() > 1 {
        parse_args(ARGS.to_vec());
    }

    env::set_var("TAURUS_SESSIONS", SESSIONS.len().to_string());

    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let ws_route = warp::path("taurus")
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .and_then(ws_handler);
    let routes = ws_route.with(warp::cors().allow_any_origin());

    let mut ip = [0; 4];
    for (i, e) in config.ws_ip.to_owned().split(".").enumerate() {
        ip[i] = match e.parse::<u8>() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("*error: \x1b[31m{}\x1b[0m", e);
                panic!("*error: \x1b[31minvalid ip in config file! exiting\x1b[0m");
            }
        }
    }

    let mut line_map = HashMap::new();

    for i in &SESSIONS.to_owned() {
        if i.game.is_none() {
            println!(
                "*warn: \x1b[33mno game sessions detected in {}.json, continuing anyway\x1b[0m",
                i.name
            );
            continue;
        }
        // TODO
        // add docker support for piping
        match i.host.as_str() {
            "tmux" => gen_pipe(i.name.to_owned(), false).await,
            _ => {}
        };
        tokio::time::sleep(Duration::from_millis((SESSIONS.len() * 10) as u64)).await;
        line_map.insert(i.name.to_owned(), set_lines(i.name.to_owned()));
    }

    tokio::spawn(async move {
        loop {
            let mut response = Vec::new();
            for (key, value) in line_map.clone().iter() {
                let (msg, line_count) = update_messages(key.to_owned(), *value).await;
                let msg = match msg {
                    Some(v) => v,
                    None => continue,
                };
                if msg.len() < 8 {
                    break;
                }
                let key = key.clone().to_string();
                // This is very janky and probably should be redone
                let _ = line_map.to_owned().remove_entry(&key);
                line_map.insert(key, line_count);
                response.push(msg);
            }
            let msg = &response.join("\n");
            replace_formatting(msg.to_owned());
            send_to_clients(&clients.clone(), &format!("MSG {}", msg)).await;
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    });

    let mut clock: usize = 0;

    tokio::spawn(async move {
        loop {
            clock += 1;
            for i in &SESSIONS.to_owned() {
                if i.game.is_none() || i.game.to_owned().unwrap().backup_interval.is_none() {
                    continue;
                }

                let e = i.game.to_owned().unwrap();

                if e.backup_interval.is_some()
                    && clock % e.backup_interval.unwrap() == 0
                    && clock > e.backup_interval.unwrap()
                {
                    let keep_time = match e.backup_keep {
                        Some(t) => t,
                        None => usize::MAX,
                    };
                    if e.file_path.is_none() || e.backup_interval.is_none() {
                        continue;
                    }
                    let _ = backup(
                        None,
                        keep_time,
                        e.file_path.unwrap(),
                        config.backup_location.to_owned(),
                        e.backup_interval.to_owned().unwrap(),
                    );
                }
            }
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    });

    print!(
        "*info: \x1b[32mmanager loaded in: {:#?}, ",
        startup.elapsed()
    );

    println!(
        "starting websocket server on {}:{}\x1b[0m",
        config.ws_ip, config.ws_port
    );
    warp::serve(routes).run((ip, config.ws_port as u16)).await;
}
fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}
