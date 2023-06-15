mod args;
mod backup;
mod bridge;
mod config;
mod utils;
mod ws;
use crate::{
    args::parse_args,
    backup::delete_backups_older_than,
    bridge::{Bridge, Session},
    utils::Sys,
    ws::PATH,
};
use bridge::{gen_pipe, replace_formatting, set_lines, update_messages};
use config::Config;
use log::{error, info};
use notify::{watcher, RecursiveMode, Watcher};
use regex::Regex;
use std::{
    collections::HashMap,
    convert::Infallible,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use utils::Clients;
use warp::Filter;
use ws::{ws_handler, ARGS, BRIDGES, CONFIG, SESSIONS};

pub async fn run() {
    let startup = Instant::now();

    if ARGS.len() > 1 {
        parse_args(ARGS.to_vec());
    }

    let clients = Arc::new(Mutex::new(HashMap::new()));
    let ws_route = warp::path("taurus")
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .and_then(ws_handler);
    let routes = ws_route.with(warp::cors().allow_any_origin());

    let mut ip = [0; 4];
    for (i, e) in CONFIG.read().await.ws_ip.to_owned().split('.').enumerate() {
        if let Ok(v) = e.parse::<u8>() {
            ip[i] = v;
        } else {
            error!("invalid ip in config file! exiting");
            exit!();
        }
    }

    for session in &*SESSIONS.read().await {
        let name = &session.name;
        if session.game.is_none() {
            println!(
                "*warn: \x1b[33mno game sessions detected in {name}.json, continuing anyway\x1b[0m"
            );
            continue;
        }
        // TODO
        // add docker support for piping
        match session.host.as_str().trim() {
            "tmux" => gen_pipe(&session.name, false).await,
            _ => {}
        };
        let enabled = match &session.game {
            Some(v) => v.chat_bridge,
            None => None,
        };
        // Wait for tmux to generate the pipe
        tokio::time::sleep(Duration::from_millis(5)).await;
        let mut locked = BRIDGES.lock().await;
        locked.push(Bridge {
            name: name.to_string(),
            line: set_lines(name),
            enabled,
            state: enabled.unwrap_or_default(),
        });
    }

    if SESSIONS.read().await.len() > 0 {
        tokio::spawn(async move {
            let parse_pattern = Regex::new(r"^\[\d{2}:\d{2}:\d{2}\] \[Server thread/INFO\]: (<.*|[\w §]+ (joined|left) the game)$").unwrap();
            let bridges = BRIDGES.clone();
            loop {
                tokio::time::sleep(Duration::from_millis(333)).await;
                let mut response: Vec<String> = Vec::new();
                let mut locked = bridges.lock().await;
                for session in locked.iter_mut() {
                    let msg = update_messages(session, &parse_pattern).await;
                    if let Some(v) = msg {
                        response.push(v);
                    }
                }
                let collected = &response.join("\n");
                let msg = format!("MSG {}", &collected);
                let msg = replace_formatting(&msg);
                if msg.trim().len() > 4 {
                    Session::send_chat_to_clients(&locked, &msg).await;
                    let ws_clients = clients.lock().await;
                    for client in (*ws_clients).values() {
                        client.send(&*msg).await;
                    }
                }
            }
        });

        tokio::spawn(async move {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut watcher = watcher(tx, Duration::from_secs(5)).unwrap();
            watcher.watch(&*PATH, RecursiveMode::Recursive).unwrap();
            loop {
                // Send cannot be sent unless the event is dropped, so we must wait until an event
                // happens then reload the config and continue
                while let Ok(notify::DebouncedEvent::Write(_)) = rx.recv() {}
                *SESSIONS.write().await = Config::load_sessions(PATH.to_owned());
                *CONFIG.write().await = Config::load_config(PATH.to_owned());
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
                clock = clock.wrapping_add(1);
                for i in &*SESSIONS.read().await {
                    let game = match &i.game {
                        Some(v) => v,
                        None => continue,
                    };
                    game.perform_scheduled_backups(i.name.as_str(), clock, &sys)
                        .await;
                }
                // todo if disk is low then reduce keep time
            }
        });
    }

    info!("manager loaded in: {} ms, ", startup.elapsed().as_millis());

    let port = CONFIG.read().await.ws_port;

    info!(
        "starting websocket server on {}:{port}",
        CONFIG.read().await.ws_ip
    );
    warp::serve(routes).run((ip, port as u16)).await;
}
fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}
