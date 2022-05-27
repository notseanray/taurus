mod args;
mod backup;
mod bridge;
mod config;
mod utils;
mod ws;
mod newbackup;
use crate::{
    bridge::{Bridge, Session},
    utils::Sys
};
use args::parse_args;
use bridge::{gen_pipe, replace_formatting, set_lines, update_messages};
use config::Config;
use regex::Regex;
use std::{
    collections::HashMap,
    convert::Infallible,
    env,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use utils::Clients;
use warp::Filter;
use ws::{ws_handler, ARGS, PATH, SESSIONS};

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
    for (i, e) in config.ws_ip.to_owned().split('.').enumerate() {
        ip[i] = match e.parse::<u8>() {
            Ok(t) => t,
            Err(e) => {
                error!(e);
                error!("invalid ip in config file! exiting");
                exit!();
            }
        }
    }

    let mut line_map = Vec::new();

    for session in &SESSIONS.to_vec() {
        let name = &session.name;
        if session.game.is_none() {
            println!(
                "*warn: \x1b[33mno game sessions detected in {name}.json, continuing anyway\x1b[0m"
            );
            continue;
        }
        // TODO
        // add docker support for piping
        match session.host.as_str() {
            "tmux" => gen_pipe(&session.name, false).await,
            _ => {}
        };
        // Wait for tmux to generate the pipe
        tokio::time::sleep(Duration::from_millis(10)).await;
        //line_map.insert(name.to_string(), set_lines(name));
        line_map.push(Bridge {
            name: name.to_string(),
            line: set_lines(name),
        });
    }

    if SESSIONS.len() > 0 {
        tokio::spawn(async move {
            let parse_pattern = Regex::new(r"^\[\d{2}:\d{2}:\d{2}\] \[Server thread/INFO\]: (<.*|[\w ]+ (joined|left) the game)$").unwrap();
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let mut response = Vec::new();
                for session in line_map.iter_mut() {
                    let (msg, line_count) =
                        update_messages(session.name.to_owned(), session.line, &parse_pattern)
                            .await;
                    session.line = line_count;
                    let msg = match msg {
                        Some(v) => v,
                        None => continue,
                    };
                    response.push(msg);
                }
                let collected = &response.join("\n");
                if collected.is_empty() {
                    continue;
                }
                let msg = format!("MSG {}", &collected);
                replace_formatting(&msg);
                Session::send_chat_to_clients(&SESSIONS, &msg);
                let lock = clients.clone();
                for (_, client) in lock.lock().await.iter() {
                    client.send(&msg[..msg.len() - 1]).await;
                }
            }
        });
        let mut clock: u64 = 0;
        let mut sys = Sys::new();
        sys.refresh();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                // though this will probably literally never be needed, we can loop forever
                // max backup interval is u64::MAX
                clock.wrapping_add(1);
                for i in &SESSIONS.to_owned() {
                    let e = i.game.to_owned().unwrap();

                    /*
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
                            &sys,
                        );
                    }*/
                }
            }
        });
    }

    info!(format!("manager loaded in: {:#?}, ", startup.elapsed()));

    info!(format!(
        "starting websocket server on {}:{}",
        config.ws_ip, config.ws_port
    ));
    warp::serve(routes).run((ip, config.ws_port as u16)).await;
}
fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}
