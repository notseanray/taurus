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
use ws::WsClient;
use config::load_config;

type Clients = Arc<Mutex<HashMap<String, WsClient>>>;
type Result<T> = std::result::Result<T, Rejection>;

#[tokio::main]
async fn main() {

    let mut args: Vec<String> = env::args().collect();

    let config = load_config(args[0].to_owned()[..args[0].len() - 6].to_string());

    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let ws_route = warp::path("lupus")
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .and_then(ws::ws_handler);
    let routes = ws_route.with(
        warp::cors()
        .allow_any_origin()
    );

    let mut ip: [u8; 4] = [0; 4];
    for (i, e) in config.ws_ip.to_owned().split(".").enumerate() {
        println!("{}", e);
        if e == "." { continue; }
        ip[i] = match e.parse::<u8>() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("*error: {}", e);
                eprintln!("*error: invalid ip in config file! exiting");
                std::process::exit(1);
            }
        }
    }
    println!("starting websocket server");
    warp::serve(routes).run((ip, 8000)).await;
}
fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}
