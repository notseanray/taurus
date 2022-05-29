mod args;
mod backup;
mod bridge;
mod config;
mod utils;
mod ws;
use crate::{
    args::parse_args,
    bridge::{Bridge, Session},
    utils::Sys,
};
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
use ws::{ws_handler, ARGS, SESSIONS, CONFIG};

pub async fn run() {
    let startup = Instant::now();

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
    for (i, e) in CONFIG.ws_ip.to_owned().split('.').enumerate() {
        if let Ok(v) = e.parse::<u8>() {
            ip[i] = v;
        } else {
            error!("invalid ip in config file! exiting");
            exit!();
        }
    }

    let mut line_map: Vec<Bridge> = Vec::new();

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
        let enabled = match &session.game {
            Some(v) => v.chat_bridge,
            None => None,
        };
        // Wait for tmux to generate the pipe
        tokio::time::sleep(Duration::from_millis(10)).await;
        line_map.push(Bridge {
            name: name.to_string(),
            line: set_lines(name),
            enabled,
        });
    }

    if SESSIONS.len() > 0 {
        tokio::spawn(async move {
            let parse_pattern = Regex::new(r"^\[\d{2}:\d{2}:\d{2}\] \[Server thread/INFO\]: (<.*|[\w ]+ (joined|left) the game)$").unwrap();
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                let mut response: Vec<String> = Vec::new();
                for session in line_map.iter_mut() {
                    let msg = update_messages(session, &parse_pattern).await;
                    if let Some(v) = msg {
                        response.push(v);
                    }
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
                }
            }
        });
    }

    info!(format!("manager loaded in: {:#?}, ", startup.elapsed()));

    info!(format!(
        "starting websocket server on {}:{}",
        CONFIG.ws_ip, CONFIG.ws_port
    ));
    warp::serve(routes).run((ip, CONFIG.ws_port as u16)).await;
}
fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}
