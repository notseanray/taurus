use crate::{utils::check_exist, utils::reap, config::{Game, Rcon}};
use regex::Regex;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::ops::Deref;
use tokio::process::Command;
use serde_derive::Deserialize;

pub struct Bridge {
    pub name: String,
    pub line: usize,
}

// poll the log file and check for new messages, match them against a certain pattern to dermine if
// we need to send anything to the clients
pub async fn update_messages<T>(
    server_name: T,
    lines: usize,
    pattern: &Regex,
) -> (Option<String>, usize)
where
    T: Deref<Target = str> + std::fmt::Display,
{
    let file_path: String = format!("/tmp/{server_name}-taurus");
    if !check_exist(&file_path) {
        gen_pipe(&server_name, false).await;
        return (None, 0);
    }
    let reader = BufReader::new(File::open(file_path).unwrap());
    let mut message = String::new();
    let mut cur_line: usize = lines;
    for (i, line) in reader.lines().enumerate() {
        // assign the real number of lines, if the file is empty lines returns 0 by default
        // if there is 1 line, there is still 0 lines due to it being 0 indexed
        let real = i + 1;
        if real > cur_line {
            let line = line.unwrap_or(String::from(""));
            cur_line = real;
            if !pattern.is_match(&line) {
                continue;
            }
            // prevent potential panics from attempting to index weird unicode
            let mut message_out = String::new();
            let message_chars = line.chars().collect::<Vec<char>>();
            message_chars.iter().for_each(|c| message_out.push(*c));
            message.push_str(&format!("[{server_name}] {}\n", &message_out[33..]));
        }
    }
    // if the log file is above 8k we can reset it to prevent parsing time from building up
    if cur_line > 8000 {
        // reset pipe file and notify
        gen_pipe(&server_name, true).await;
        println!("*info: pipe file reset -> {server_name}");
        return match message.len() {
            3.. => (Some(message), 0),
            _ => (None, 0),
        };
    }
    match message.len() {
        3.. => (Some(message), cur_line),
        _ => (None, cur_line),
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
    msg.replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\"", "\\\"")
        .replace("_", "\\_")
}


// generate the tmux pipe to the tmux session and attempt to remove it if needed
#[inline]
pub async fn gen_pipe(server_name: &str, rm: bool) {
    let pipe = format!("/tmp/{server_name}-taurus");
    if rm {
        // we don't care if this fails
        let _ = fs::remove_file(&pipe);
    }

    Command::new("tmux")
        .args(["pipe-pane", "-t", &server_name, &format!("cat > {pipe}")])
        .spawn()
        .expect("*error: \x1b[31mfailed to generate pipe file\x1b[0m");

    // clean up zombies
    reap();
}

// store configuration for each session, description is purely for telling what it is
#[derive(Deserialize, Clone)]
pub struct Session {
    pub name: String,
    pub description: Option<String>,
    pub host: String,
    pub game: Option<Game>,
    pub rcon: Option<Rcon>,
}

impl Session {
    // send messages to all servers with a 'game' session
    pub fn send_chat(&self, message: &str) {
        let lines: Vec<&str> = message.split("\n").collect();
        for line in lines {
            let line = &line.replace("MSG ", "");
            let pos = match line.find("]") {
                Some(v) => v,
                None => 0,
            };
            let msg = match Self::clear_formatting(line) {
                Some(v) => v,
                None => continue,
            };
            if pos != 0 && line[1..pos] == self.name || self.game.is_none() {
                continue;
            }
            Self::send_command(&self.name, &format!(r#"tellraw @a {{ "text": "{}" }}"#, msg));
        }
    }

    pub fn send_chat_to_clients(clients: &Vec<Self>, message: &str) {
        clients.iter().for_each(|x| x.send_chat(message));
    }

    // remove formatting when sending messages to the tmux session
    #[inline(always)]
    fn clear_formatting(msg: &str) -> Option<String> {
        let msg = msg
            .replace("\\", "")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("{", "{{")
            .replace("}", "}}")
            .replace("\"", "\\\"");
        match msg.len() {
            1.. => Some(msg),
            _ => None,
        }
    }

    // send command to tmux session
    #[inline(always)]
    pub fn send_command(name: &str, message: &str) {
        let _ = Command::new("tmux")
            .args(["send-keys", "-t", &name, &message, "Enter"])
            .spawn();

        // clean up zombies
        reap();
    }
}
