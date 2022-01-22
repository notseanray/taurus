use warp::ws::Message;
use crate::Clients;

pub async fn send_chat(clients: &Clients, msg: String) {
    let locked = clients.lock().await;
    for (key, _) in locked.iter() {
        match locked.get(key) {
            Some(t) => {
                if let Some(t) = &t.sender {
                    let _ = t.send(Ok(Message::text(msg.to_owned())));
                }
            },
            None => continue,
        };
    }
}
