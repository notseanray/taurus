use crate::{
    bridge::{Session, Bridge},
    config::Config,
    info,
    utils::{Clients, Result, Sys, WsClient},
    warn,
};
use futures::{FutureExt, StreamExt};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::{process::Command, sync::mpsc};
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::{
    ws::{Message, WebSocket},
    Reply,
};

lazy_static::lazy_static! {
    static ref CONFIG_PATH: String = {
        let path: Vec<String> = env::args().collect();
        path[0][..path[0].len() - 6].to_string()
    };
    pub(crate) static ref ARGS: Vec<String> = env::args().collect();
    pub(crate) static ref PATH: String = ARGS[0].to_owned()[..ARGS[0].len() - 6].to_string();
    pub(crate) static ref SESSIONS: Vec<Session> = Config::load_sessions(PATH.to_owned());
    pub(crate) static ref CONFIG: Config = Config::load_config(PATH.to_owned());
    static ref RESTART_SCRIPT: Option<String> = Config::load_config(CONFIG_PATH.to_string()).restart_script;
}

pub(crate) async fn client_connection(ws: WebSocket, clients: Clients) {
    println!("*info: establishing new client connection...");
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();
    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            warn!(format!("error sending websocket msg: {e}"));
        }
    }));
    let uuid = Uuid::new_v4().to_simple().to_string();
    let new_client = WsClient {
        client_id: uuid.clone(),
        sender: Some(client_sender),
        authed: false,
    };
    clients.lock().await.insert(uuid.clone(), new_client);
    while let Some(result) = client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                warn!(format!("error receiving message for id {uuid}): {e}"));
                break;
            }
        };
        client_msg(&uuid, msg, clients.clone()).await;
    }
    clients.lock().await.remove(&uuid);
    info!(format!("*info: {} disconnected", uuid));
}

async fn client_msg(client_id: &str, msg: Message, clients: Clients) {
    let msg = match msg.to_str() {
        Ok(v) => v,
        Err(_) => return
    };
    let mut locked = clients.lock().await;
    if let Some(mut v) = locked.get_mut(client_id) {
        if !v.authed {
            v.authed = {
                if CONFIG.ws_password.len() == msg.len() {
                    let mut result = 0;
                    for (x, y) in CONFIG.ws_password.chars().zip(msg.chars()) {
                        result |= x as u32 ^ y as u32;
                    }
                    result == 0
                } else {
                    false
                }
            };
            return;
        }
    }
    // we move locking after the response once authed, looks messy but should be better, I hope
    if let Some(response) = handle_response(msg).await {
        if let Some(v) = locked.get(client_id) {
            if let Some(sender) = &v.sender {
                let _ = sender.send(Ok(Message::text(response)));
            }
        }
    }
}

pub(crate) async fn ws_handler(ws: warp::ws::Ws, clients: Clients) -> Result<impl Reply> {
    Ok(ws.on_upgrade(move |socket| client_connection(socket, clients)))
}

fn get_cmd(msg: &str) -> Option<(&str, &str)> {
    let response = match msg.find(' ') {
        Some(v) => v,
        None => return None,
    };
    Some((&msg[..response], &msg[response + 1..]))
}

async fn handle_response(message: &str) -> Option<String> {
    let command_index = message.find(' ');

    // split the command into the first word if applicable
    let command = match command_index {
        Some(v) => &message[0..v],
        None => message,
    };
    let response = match command {
        "MSG" => {
            let (_, in_game_message) = match get_cmd(message) {
                Some(v) => v,
                None => return None,
            };
            // TODO
            // replace with tmux json + cleanse input
            Session::send_chat_to_clients(&SESSIONS, in_game_message);
            return None;
        }
        "LIST" => {
            Session::send_chat_to_clients(&SESSIONS, "list");
            return None;
        }
        "CMD" => {
            let command_index = match command_index {
                Some(v) => v,
                None => return Some("invalid command".to_string()),
            };
            let (target, cmd) = match get_cmd(&message[command_index + 1..]) {
                Some(v) => v,
                None => return None,
            };
            Session::send_command(target, cmd);
            return None;
        }
        "RESTART" => {
            let script_path = match RESTART_SCRIPT.to_owned() {
                Some(v) => v,
                None => return Some("no restart script found".to_string()),
            };
            let restart = Command::new("sh")
                .args(["-c", &script_path])
                .kill_on_drop(true)
                .status()
                .await
                .expect("could not execute restart script");
            if restart.success() {
                return Some("restarting...".to_string());
            }
            return Some("failed to execute restart script".to_string());
        }
        "SHELL" => {
            let instructions: Vec<&str> =
                message[command_index.unwrap() + 1..].split(' ').collect();
            let command = instructions[0];
            let args = match instructions.len() {
                2.. => Some(&instructions[1..]),
                _ => None,
            };

            info!(format!("shell cmd {command}"));
            let args = args.unwrap_or(&[]);
            let _ = Command::new(command).args(args).kill_on_drop(true).spawn();
            return None;
        }
        "HEARTBEAT" => Some(format!("{}", Sys::new().sys_health_check())),
        "CHECK" => Some(format!("{}", Sys::new())),
        "PING" => {
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            //send_to_clients(clients, &format!("PONG {time}")).await;
            return Some(format!("PONG {time}"));
        }
        _ => None,
    };
    response
}
