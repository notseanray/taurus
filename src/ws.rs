use crate::*;
use std::time::{SystemTime, UNIX_EPOCH};
use futures::{FutureExt, StreamExt};
use std::env;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::ws::{Message, WebSocket};
use warp::Reply;

lazy_static::lazy_static! {
    static ref SESSIONS: Vec<Session> = {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let path: Vec<String> = env::args().collect();
            let trimmed_path = path[0][..path[0].len() - 6].to_string();
            Config::load_sessions(trimmed_path)
        })
    };
}

pub async fn client_connection(ws: WebSocket, clients: Clients) {
    println!("*info: establishing new client connection...");
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();
    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            println!("*warn: \x1b[33merror sending websocket msg: {}\x1b[0m", e);
        }
    }));
    let uuid = Uuid::new_v4().to_simple().to_string();
    let new_client = WsClient {
        client_id: uuid.clone(),
        sender: Some(client_sender),
    };
    clients.lock().await.insert(uuid.clone(), new_client);
    while let Some(result) = client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                println!("*warn: \x1b[33merror receiving message for id {}): {}\x1b[0m", uuid.clone(), e);
                break;
            }
        };
        client_msg(&uuid, msg, &clients).await;
    }
    clients.lock().await.remove(&uuid);
    println!("*info: {} disconnected", uuid);
}

async fn client_msg(client_id: &str, msg: Message, clients: &Clients) {
    let response = handle_response(msg).await;
    if response.is_none() { return; }

    let locked = clients.lock().await;
    match locked.get(client_id) {
        Some(v) => {
            if let Some(sender) = &v.sender {
                let _ = sender.send(Ok(Message::text(response.unwrap())));
            }
        }
        None => {}
    }
}

pub async fn ws_handler(ws: warp::ws::Ws, clients: Clients) -> Result<impl Reply> {
    Ok(ws.on_upgrade(move |socket| client_connection(socket, clients)))
}

fn get_cmd(msg: &str) -> Option<(&str, &str)> {
    let response = match msg.find(" ") {
        Some(v) => v,
        None => return None
    };
    Some((&msg[..response], &msg[response..]))
}

async fn handle_response(msg: Message) -> Option<String> {
    let message = match msg.to_str() {
        Ok(v) => v,
        Err(_) => return None,
    };

    let command_index = message.find(" ");

    // split the command into the first word if applicable
    let command = match command_index {
        Some(v) => &message[0..v],
        None => message,
    };
    let response = match command {
        "MSG" => {
            create_rcon_connections(SESSIONS.to_vec(), "say".to_owned() + message)
                .await
                .unwrap();
            return None;
        },
        "CMD" => {
            if command_index.is_none() { return None; }
            let (target, cmd) = match get_cmd(&message[command_index.unwrap()..]) {
                Some(v) => v,
                None => return None
            };
            send_command(target, cmd).await;
            return None;
        },
        "CHECK" => Some(sys_check()),
        "HEARTBEAT" => Some(sys_health_check().to_string()),
        "PING" => {
            let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
            //send_to_clients(clients, &format!("PONG {time}")).await;
            return Some(format!("PONG {time}")); 
        }
        _ => None
    };
    response
}
