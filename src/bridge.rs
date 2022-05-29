use crate::{
    backup::Game,
    config::Rcon,
    utils::{check_exist, reap},
};
use regex::Regex;
use serde_derive::Deserialize;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use tokio::process::Command;

pub(crate) struct Bridge {
    pub name: String,
    pub line: usize,
    pub enabled: Option<bool>,
}

// poll the log file and check for new messages, match them against a certain pattern to dermine if
// we need to send anything to the clients
#[inline(always)]
pub(crate) async fn update_messages(server: &mut Bridge, pattern: &Regex) -> Option<String> {
    if server.enabled == None {
        return None;
    }
    let file_path: String = format!("/tmp/{}-taurus", server.name);
    if !check_exist(&file_path) {
        gen_pipe(&server.name, false).await;
        return None;
    }
    let reader = BufReader::new(File::open(file_path).unwrap());
    let mut message = String::new();
    let mut cur_line: usize = server.line;
    let mut list_cmd = false;
    let mut list_msg = false;
    for (i, line) in reader.lines().enumerate() {
        // assign the real number of lines, if the file is empty lines returns 0 by default
        // if there is 1 line, there is still 0 lines due to it being 0 indexed
        let real = i + 1;
        if cur_line <= real {
            continue;
        }
        let line = line.unwrap_or_else(|_| String::from(""));
        cur_line = real;
        let mut message_out = String::new();
        let message_chars = line.chars().collect::<Vec<char>>();
        message_chars.iter().for_each(|c| message_out.push(*c));
        if let Some(false) = server.enabled {
            if &message_out[33..62] == "Starting Minecraft server on " {
                server.enabled = Some(true);
            } else {
                return None;
            }
        }
        if pattern.is_match(&line) {
            // prevent potential panics from attempting to index weird unicode
            message.push_str(&format!("[{}] {}\n", server.name, &message_out[33..]));
            continue;
        }
        if list_cmd && message_out.len() > 43 && &message_out[10..33] == " [Server thread/INFO]: " {
            let list_message: Vec<&str> = (&message_out[33..]).split_ascii_whitespace().collect();
            if let Some(true) = server.enabled {
                if &message_out[33..52] == "Stopping the server" {
                    server.enabled = Some(false);
                    continue;
                }
            }
            if &message_out[33..43] == "There are " {
                let min: u32 = match list_message[2].parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let max: u32 = match list_message[7].parse() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                message.push_str(&format!("{}: [{min}/{max}]", server.name));
                // 1.13 +
                if list_message.len() > 10 {
                    message.push_str(&list_message[10..].join(" "));
                } else if min > 0 {
                    list_msg = true;
                }
            }
            if list_message[2] == "has" && list_message.len() > 6 && !list_message[0].contains('<')
            {
                message.push_str(&message_out[33..]);
            }
        }
        if list_msg {
            message.push_str(&message_out[33..]);
            list_msg = false;
        }
        list_cmd = line == "list";
    }
    // if the log file is above 8k we can reset it to prevent parsing time from building up
    if cur_line > 8000 {
        // reset pipe file and notify
        gen_pipe(&server.name, true).await;
        server.line = 0;
        return match message.len() {
            3.. => Some(message),
            _ => None,
        };
    }
    server.line = cur_line;
    match message.len() {
        3.. => Some(message),
        _ => None,
    }
}

// set the initial hashmap value of lines so only new lines are sent
pub fn set_lines(server_name: &str) -> usize {
    let server_name = server_name.to_string();
    let file = File::open(&format!("/tmp/{server_name}-taurus")).unwrap();
    let reader = BufReader::new(file);

    reader.lines().count()
}

// remove formatting when sending messages to discord
#[inline(always)]
pub fn replace_formatting(msg: &str) -> impl ToString {
    // regex to replace any '§' followed by digits with a blank space, from MC color codes
    let replacements = Regex::new(r"§.*\d").unwrap();
    replacements.replace_all(msg, "").to_owned().to_string();
    // ideally this would be redone using more regex, if possible, but this works alright for now
    msg.replace('\n', "\\n")
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
        .kill_on_drop(true)
        .spawn();

    // clean up zombies
    reap();
}

// store configuration for each session, description is purely for telling what it is
#[derive(Deserialize, Clone)]
pub(crate) struct Session {
    pub name: String,
    pub description: Option<String>,
    pub host: String,
    pub game: Option<Game>,
    pub rcon: Option<Rcon>,
}

impl Session {
    // send messages to all servers with a 'game' session
    pub(crate) fn send_chat(&self, message: &str) {
        let lines: Vec<&str> = message.lines().collect();
        for line in lines {
            let line = &line.replace("MSG ", "");
            let pos = line.find(']').unwrap_or(0);
            let msg = match Self::clear_formatting(line) {
                Some(v) => v,
                None => continue,
            };
            if pos != 0 && line[1..pos] == self.name || self.game.is_none() {
                continue;
            }
            Self::send_command(
                &self.name,
                &format!(r#"tellraw @a {{ "text": "{}" }}"#, msg),
            );
        }
    }

    pub fn send_chat_to_clients(clients: &[Self], message: &str) {
        clients
            .iter()
            .filter(|x| {
                if let Some(v) = &x.game {
                    v.chat_bridge == Some(true)
                } else {
                    false
                }
            })
            .for_each(|x| x.send_chat(message));
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

        // clean up zombies
        reap();
    }
}
