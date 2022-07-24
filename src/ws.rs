use crate::{
    backup::list_backups,
    bridge::{Bridge, Session},
    config::Config,
    utils::{Clients, Result, Sys, SysDisplay, WsClient},
};
use futures::{FutureExt, StreamExt};
use log::{info, warn};
use serde_json::json;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::{fs::remove_file, sync::Mutex};
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
    pub(crate) static ref SESSIONS: Arc<RwLock<Vec<Session>>> = Arc::new(RwLock::new(Config::load_sessions(PATH.to_owned())));
    pub(crate) static ref CONFIG: Arc<RwLock<Config>> = Arc::new(RwLock::new(Config::load_config(PATH.to_owned())));
    pub(crate) static ref BRIDGES: Arc<Mutex<Vec<Bridge>>> = Arc::new(Mutex::new(Vec::new()));
    static ref RESTART_SCRIPT: Option<String> = None;
    // Config::load_config(CONFIG_PATH.to_string()).restart_script;
}

pub(crate) async fn client_connection(ws: WebSocket, clients: Clients) {
    println!("*info: establishing new client connection...");
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();
    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            warn!("error sending websocket msg: {e}");
        }
    }));
    let uuid = Uuid::new_v4().to_simple().to_string();
    let new_client = WsClient {
        sender: Some(client_sender),
        authed: false,
    };
    clients.lock().await.insert(uuid.clone(), new_client);
    while let Some(result) = client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                warn!("error receiving message for id {uuid}): {e}");
                break;
            }
        };
        client_msg(&uuid, msg, &clients).await;
    }
    clients.lock().await.remove(&uuid);
    info!("{} disconnected", uuid);
}

async fn client_msg(client_id: &str, msg: Message, clients: &Clients) {
    let msg = match msg.to_str() {
        Ok(v) => v,
        Err(_) => return,
    };
    let mut locked = clients.lock().await;
    if let Some(mut v) = locked.get_mut(client_id) {
        if !v.authed {
            v.authed = {
                if CONFIG.read().await.ws_password.len() == msg.len() {
                    let mut result = 0;
                    for (x, y) in CONFIG.read().await.ws_password.chars().zip(msg.chars()) {
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
            let bridges = BRIDGES.lock().await;
            // TODO
            // replace with tmux json + cleanse input
            Session::send_chat_to_clients(&bridges, in_game_message).await;
            None
        }
        "URL" => {
            let (_, in_game_message) = match get_cmd(message) {
                Some(v) => v,
                None => return None,
            };
            let bridges = BRIDGES.lock().await;
            // TODO
            // replace with tmux json + cleanse input
            Session::send_url_to_clients(&bridges, in_game_message).await;
            None
        }
        "LIST" => {
            let mut lists = Vec::new();
            for session in &*SESSIONS.read().await {
                if let Some(v) = &session.rcon {
                    lists.push(match v.rcon_send_with_response("list").await {
                        Ok(Some(v)) => format!("{} {v}", session.name),
                        _ => continue,
                    });
                }
            }
            Some(format!("LIST {}", lists.join("\n")))
        }
        "BACKUP" => {
            let (_, target) = match get_cmd(message) {
                Some(v) => v,
                None => return None,
            };
            let mut set = false;
            let mut response = String::new();
            for session in &*SESSIONS.read().await.clone() {
                if session.name != target {
                    continue;
                }
                if let Some(v) = &session.game {
                    set = true;
                    let mut sys = Sys::new();
                    sys.refresh();
                    response = v
                        .backup(
                            &sys,
                            target.to_string(),
                            CONFIG.read().await.backup_location.clone().to_string(),
                        )
                        .await;
                }
            }
            if set {
                Some(format!("BACKUP {response}"))
            } else {
                Some("BACKUP Invalid Session Target".to_owned())
            }
        }
        "CP_REGION" => {
            let (_, args) = match get_cmd(message) {
                Some(v) => v,
                None => return None,
            };
            let args: Vec<&str> = args.split_whitespace().collect();
            if args.len() != 4 {
                return Some("CP_REGION Invalid Arguments".into());
            }
            let (x, z): (i32, i32) = match (args[2].parse(), args[3].parse()) {
                (Ok(v), Ok(e)) => (v, e),
                _ => return Some("CP_REGION Invalid Region Identifier".into()),
            };
            let dim_arg = args[1].to_uppercase();
            let dim = match dim_arg.as_str() {
                "OW" | "NETHER" | "END" => &dim_arg,
                _ => return Some("CP_REGION Invalid Dimension Provided".into()),
            };
            let mut response = String::new();
            for session in &*SESSIONS.read().await {
                if session.name != args[0] {
                    continue;
                }
                if let Some(v) = &session.game {
                    response = v.copy_region(dim, x, z).await;
                }
            }
            Some(format!("CP_REGION {response}"))
        }
        "LIST_BRIDGES" => {
            let locked = BRIDGES.lock().await;
            let mut response = Vec::with_capacity(locked.len());
            for bridge in &*locked {
                let state = match bridge.enabled {
                    Some(true) => "true",
                    Some(false) => "false",
                    _ => "disabled",
                };
                response.push(format!("Name: {} State: {state}", bridge.name));
            }
            Some(format!("LIST_BRIDGES {}", response.join("\n")))
        }
        "RM_BACKUP" => {
            let (_, args) = match get_cmd(message) {
                Some(v) => v,
                None => return None,
            };
            let args: Vec<&str> = args.split_whitespace().collect();
            if args.len() != 1 {
                return Some("RM_BACKUP Invalid Arguments".to_owned());
            }
            Some(
                match remove_file(
                    PathBuf::from(&*CONFIG.read().await.backup_location).join(args[0]),
                )
                .await
                {
                    Ok(_) => "RM_BACKUP removed backup successfully".to_owned(),
                    Err(_) => "RM_BACKUP unable to remove backup".to_owned(),
                },
            )
        }
        "TOGGLE_BRIDGE" => {
            let (_, args) = match get_cmd(message) {
                Some(v) => v,
                None => return None,
            };
            let args: Vec<&str> = args.split_whitespace().collect();
            if args.len() != 1 {
                return Some("TOGGLE_BRIDGE Invalid Arguments".to_owned());
            }
            let mut locked = BRIDGES.lock().await;
            let mut changed = false;
            for bridge in locked.iter_mut() {
                if bridge.name == args[0] {
                    if let Some(v) = bridge.enabled {
                        bridge.enabled = Some(!v);
                        changed = true;
                    }
                }
            }
            Some(
                (if changed {
                    "TOGGLE_BRIDGE Toggled state"
                } else {
                    "TOGGLE_BRIDGE Session not found"
                })
                .to_owned(),
            )
        }
        "CMD" => {
            let command_index = match command_index {
                Some(v) => v,
                None => return Some("CMD invalid command".to_string()),
            };
            let (target, cmd) = match get_cmd(&message[command_index + 1..]) {
                Some(v) => v,
                None => return None,
            };
            Session::send_command(target, cmd);
            None
        }
        "RCON" => {
            let command_index = match command_index {
                Some(v) => v,
                None => return Some("RCON invalid command".to_string()),
            };
            let (target, cmd) = match get_cmd(&message[command_index + 1..]) {
                Some(v) => v,
                None => return None,
            };
            let mut response = String::new();
            for session in &*SESSIONS.read().await {
                if session.name != target {
                    continue;
                }
                if let Some(v) = &session.rcon {
                    response = match v.rcon_send_with_response(cmd).await {
                        Ok(Some(x)) => x,
                        _ => continue,
                    };
                }
            }
            Some(format!("RCON {response}"))
        }
        "CP_STRUCTURE" => {
            let (_, args) = match get_cmd(message) {
                Some(v) => v,
                None => return None,
            };
            let args: Vec<&str> = args.split_whitespace().collect();
            if args.len() != 2 {
                return Some("CP_STRUCTURE Invalid Arguments".into());
            }
            let mut response = String::new();
            for session in &*SESSIONS.read().await {
                if session.name != args[0] {
                    continue;
                }
                if let Some(v) = &session.game {
                    response = v.copy_structure(args[1]).await;
                }
            }
            Some(format!("CP_STRUCTURE {response}"))
        }
        "LIST_STRUCTURES" => {
            let (_, args) = match get_cmd(message) {
                Some(v) => v,
                None => return None,
            };
            let args: Vec<&str> = args.split_whitespace().collect();
            if args.len() != 1 {
                return Some("LIST_STRUCTURES Invalid Arguments".into());
            }
            let mut response = String::new();
            for session in &*SESSIONS.read().await {
                if session.name != args[0] {
                    continue;
                }
                if let Some(v) = &session.game {
                    response = v.list_structures();
                }
            }
            Some(format!("LIST_STRUCTURES {response}"))
        }
        "LIST_BACKUPS" => Some(format!(
            "LIST_BACKUPS {}",
            list_backups(&*SESSIONS.read().await).await
        )),
        "RESTART" => {
            let script_path = match RESTART_SCRIPT.to_owned() {
                Some(v) => v,
                None => return Some("RESTART no restart script found".to_string()),
            };
            let restart = Command::new("sh")
                .args(["-c", &script_path])
                .kill_on_drop(true)
                .status()
                .await
                .expect("could not execute restart script");
            if restart.success() {
                return Some("RESTART restarting...".to_string());
            }
            Some("RESTART failed to execute restart script".to_string())
        }
        "LIST_SESSIONS" => Some(format!(
            "LIST_SESSIONS {}",
            json!(*SESSIONS.read().await.clone())
        )),
        "SHELL" => {
            let instructions: Vec<&str> =
                message[command_index.unwrap() + 1..].split(' ').collect();
            let command = instructions[0];
            let args = match instructions.len() {
                2.. => Some(&instructions[1..]),
                _ => None,
            };

            info!("shell cmd {command}");
            let args = args.unwrap_or(&[]);
            let _ = Command::new(command).args(args).kill_on_drop(true).spawn();
            None
        }
        "HEARTBEAT" => Some(format!("HEARTBEAT {}", Sys::new().sys_health_check())),
        "CHECK" => {
            let sys: SysDisplay = Sys::new().into();
            Some(format!("CHECK {}", json!(sys)))
        }
        "PING" => {
            let time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            Some(format!("PONG {time}"))
        }
        _ => None,
    };
    response
}
