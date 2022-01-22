    // simple log
    //
    // tcp server
    // backup loop
    // read chat
    // docker & tmux support - map to store settings
    // serde
    //
    // script system, different timing per a script from scripts.cfg

use std::{
    collections::HashMap, 
    convert::Infallible, 
    sync::Arc,
    env,
};
use tokio::sync::Mutex;
use warp::{Filter, Rejection};

mod ws;
mod config;
mod args;
mod bridge;
use ws::WsClient;
use config::*;
use lupus::*;
use bridge::send_chat;
use std::time::{Duration, Instant};

type Clients = Arc<Mutex<HashMap<String, WsClient>>>;
type Result<T> = std::result::Result<T, Rejection>;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {

    let startup = Instant::now();

    let args: Vec<String> = env::args().collect();

    let path = args[0].to_owned()[..args[0].len() - 6].to_string(); 

    let config = load_config(path.to_owned());

    let sessions: Vec<Session> = load_sessions(path.to_owned());

    //env::set_var("LUPUS_SESSIONS", sessions.clone());

    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let ws_route = warp::path("lupus")
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .and_then(ws::ws_handler);
    let routes = ws_route.with(
        warp::cors()
        .allow_any_origin()
    );

    let mut ip = [0; 4];
    for (i, e) in config.ws_ip.to_owned().split(".").enumerate() {
        ip[i] = match e.parse::<u8>() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("*error: {}", e);
                eprintln!("*error: invalid ip in config file! exiting");
                std::process::exit(1);
            }
        }
    }

    let mut line_map = HashMap::new();

    for i in sessions {
        if i.game.is_none() { return; }
        gen_pipe(i.name.to_owned(), false).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        line_map.insert(i.name.to_owned(), set_lines(i.name.to_owned()));
    }

    tokio::spawn(async move {
        let mut response = Vec::new();
        for (key, mut value) in line_map.iter_mut() {
            let (msg, mut line_count) = update_messages(key.to_owned(), *value).await;
            value = &mut line_count;
            response.push(msg);
        }
        send_chat(&clients, response.join("\n")).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
    });

    print!("manager loaded in: {:#?}, ", startup.elapsed());
    
    println!("starting websocket server on {}:{}", config.ws_ip, config.ws_port);
    warp::serve(routes).run((ip, config.ws_port as u16)).await;
}
fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}
