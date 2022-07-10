use crate::{backup::Game, config::Rcon, ws::SESSIONS};
use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::{
    fs::{self, File},
    path::PathBuf,
};
use tokio::process::Command;

#[derive(Serialize)]
pub(crate) struct Bridge {
    pub name: String,
    pub line: usize,
    pub enabled: Option<bool>,
    pub state: bool,
}

// poll the log file and check for new messages, match them against a certain pattern to dermine if
// we need to send anything to the clients
#[inline(always)]
pub(crate) async fn update_messages(server: &mut Bridge, pattern: &Regex) -> Option<String> {
    if server.enabled == None {
        return None;
    }
    let file_path: String = format!("/tmp/{}-taurus", server.name);
    if !PathBuf::from(&file_path).exists() {
        gen_pipe(&server.name, false).await;
        return None;
    }
    let reader = BufReader::new(match File::open(file_path) {
        Ok(v) => v,
        Err(_) => return None,
    });
    let mut message = String::new();
    for (i, line) in reader.lines().enumerate() {
        // assign the real number of lines, if the file is empty lines returns 0 by default
        // if there is 1 line, there is still 0 lines due to it being 0 indexed
        let real = i + 1;
        if real <= server.line {
            continue;
        }
        server.line = real;
        let line = match line {
            Ok(v) => v,
            Err(_) => continue,
        };
        if line.len() < 3 {
            continue;
        }
        let mut message_out = String::with_capacity(line.len());
        let message_chars = line.chars().collect::<Vec<char>>();
        message_chars.iter().for_each(|c| message_out.push(*c));
        if message_chars.len() < 34 && !server.state {
            if let Some(true) = server.enabled {
                if &message_out[10..33] == " [Server thread/INFO]: " {
                    server.state = true;
                } else {
                    return None;
                }
            }
        }
        if pattern.is_match(&line) {
            // prevent potential panics from attempting to index weird unicode
            message.push_str(&format!("[{}] {}\n", server.name, &message_out[33..]));
            continue;
        }
        if message_out.len() > 52 && &message_out[10..33] == " [Server thread/INFO]: " {
            let list_message: Vec<&str> = (&message_out[33..]).split_ascii_whitespace().collect();
            if let Some(true) = server.enabled {
                if &message_out[33..52] == "Stopping the server" {
                    server.state = false;
                    continue;
                }
            }
            if list_message[2] == "has" && list_message.len() > 6 && !list_message[0].contains('<')
            {
                message.push_str(&message_out[33..]);
            }
        }
    }
    // if the log file is above 8k we can reset it to prevent parsing time from building up
    if server.line > 8000 {
        // reset pipe file and notify
        gen_pipe(&server.name, true).await;
        server.line = 0;
        return match message.len() {
            3.. => Some(message),
            _ => None,
        };
    }
    match message.len() {
        3.. => Some(message),
        _ => None,
    }
}

// set the initial hashmap value of lines so only new lines are sent
pub fn set_lines(server_name: &str) -> usize {
    let server_name = server_name.to_string();
    let file = match File::open(&format!("/tmp/{server_name}-taurus")) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);

    reader.lines().count()
}

// remove formatting when sending messages to discord
#[inline(always)]
pub fn replace_formatting(msg: &str) -> String {
    // regex to replace any '§' and following character, from MC color codes
    let replacements = Regex::new("§.").unwrap();
    let msg = replacements.replace_all(msg, "").to_owned().to_string();
    // ideally this would be redone using more regex, if possible, but this works alright for now
    msg.trim_end()
        .replace('\r', "\\r")
        .replace('\"', "\\\"")
        .replace('_', "\\_")
}

// generate the tmux pipe to the tmux session and attempt to remove it if needed
#[inline(always)]
pub async fn gen_pipe(server_name: &str, rm: bool) {
    let pipe = format!("/tmp/{server_name}-taurus");
    if rm {
        // we don't care if this fails
        let _ = fs::remove_file(&pipe);
    }

    let _ = Command::new("tmux")
        .args(["pipe-pane", "-t", server_name, &format!("cat > {pipe}")])
        .spawn();
}

// store configuration for each session, description is purely for telling what it is
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct Session {
    pub name: String,
    pub description: Option<String>,
    pub host: String,
    pub game: Option<Game>,
    pub rcon: Option<Rcon>,
}

macro_rules! send {
    ($bridges:expr, $message:expr, $type:expr) => {
        for client in &*SESSIONS {
            if let Some(v) = &client.game {
                for bridge in $bridges {
                    if bridge.name == client.name && v.chat_bridge == Some(true) && bridge.state {
                        client
                            .send_chat(client.rcon.as_ref(), $message, $type)
                            .await;
                    }
                }
            }
        }
    };
}

impl Session {
    // send messages to all servers with a 'game' session
    pub(crate) async fn send_chat(&self, rcon: Option<&Rcon>, message: &str, url: bool) {
        let lines: Vec<&str> = message.lines().collect();
        for line in lines {
            let line = line.replace("MSG ", "");
            let pos = line.find(']').unwrap_or(0);
            let msg = match Self::clear_formatting(&line) {
                Some(v) => v,
                None => continue,
            };
            if pos != 0 && (line[1..pos] == self.name || self.game.is_none()) {
                continue;
            }
            let message = if url {
                let link = msg.split_whitespace().collect::<Vec<&str>>();
                let text = if link.len() == 1 {
                    "attachment"
                } else {
                    &msg[link[0].len()..]
                };
                format!("tellraw @a {{ \"text\": \"{text}\", \"clickEvent\":{{ \"action\": \"open_url\", \"value\": \"{}\"}} }}", link[0])
            } else {
                format!("tellraw @a {{ \"text\": \"{msg}\" }}")
            };
            if let Some(v) = rcon {
                let _ = v.rcon_send(&message).await;
                continue;
            }
            Self::send_command(&self.name, &message);
        }
    }

    pub(crate) async fn send_chat_to_clients(bridges: &Vec<Bridge>, message: &str) {
        send!(bridges, message, false);
    }

    pub(crate) async fn send_url_to_clients(bridges: &Vec<Bridge>, message: &str) {
        send!(bridges, message, true);
    }

    // remove formatting when sending messages to the tmux session
    #[inline(always)]
    fn clear_formatting(msg: &str) -> Option<String> {
        let msg = msg
            .replace('\\', "")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('{', "{{")
            .replace('}', "}}")
            .replace('\"', "\\\"");
        match msg.len() {
            1.. => Some(msg),
            _ => None,
        }
    }

    // send command to tmux session
    #[inline(always)]
    pub fn send_command(name: &str, message: &str) {
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", name, message, "Enter"])
            .kill_on_drop(true)
            .spawn();
    }
}
