use futures::{FutureExt, StreamExt};
use crate::*;
use lupus::Session;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::ws::{Message, WebSocket};
use warp::Reply;
use std::env;

lazy_static::lazy_static! {
    static ref SESSIONS: Vec<Session> = {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let path: Vec<String> = env::args().collect();
            let trimmed_path = path[0][..path[0].len() - 6].to_string();
            Config::load_sessions(trimmed_path)
        })
    }; 
}

/*
#[derive(Debug, Clone)]
pub struct WsClient {
    pub client_id: String,
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
}
*/

pub async fn client_connection(ws: WebSocket, clients: Clients) {
    println!("establishing new client connection...");
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();
    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            println!("error sending websocket msg: {}", e);
        } }));
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
                println!("error receiving message for id {}): {}", uuid.clone(), e);
                break;
            }
        };
        client_msg(&uuid, msg, &clients).await;
    }
    clients.lock().await.remove(&uuid);
    println!("{} disconnected", uuid);
}

async fn client_msg(client_id: &str, msg: Message, clients: &Clients) {
    // attempt to convert the message, ignore if we recieve garbage
    let message = match msg.to_str() {
        Ok(v) => v,
        Err(_) => return,
    };

    // split the command into the first word if applicable
    let command = match message.find(" ") {
        Some(v) => &message[0..v],
        None => message 
    };

    let response = match command {
        "MSG" => {
            create_rcon_connections(SESSIONS.to_vec(), "say".to_owned() + message).await.unwrap();
            return;
        },
        _ => "",
    };

    if response.len() == 0 { return; }

    let locked = clients.lock().await;
    match locked.get(client_id) {
        Some(v) => {
            if let Some(sender) = &v.sender {
                let _ = sender.send(Ok(Message::text("te")));
            }
        }
        None => {},
    }
}

pub async fn ws_handler(ws: warp::ws::Ws, clients: Clients) -> Result<impl Reply> {
    Ok(ws.on_upgrade(move |socket| client_connection(socket, clients)))
}
