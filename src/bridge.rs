use crate::config::Session;
use crate::{utils::check_exist, utils::reap};
use regex::Regex;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use tokio::process::Command;

// poll the log file and check for new messages, match them against a certain pattern to dermine if
// we need to send anything to the clients
pub async fn update_messages<T: ToString>(server_name: T, lines: usize) -> (Option<String>, usize) {
    let server_name = server_name.to_string();
    let file_path: String = format!("/tmp/{server_name}-taurus");
    if !check_exist(&file_path.to_owned()) {
        return (None, 0);
    }

    // unwrap: we already know the file exist from checking right above
    let reader = BufReader::new(File::open(&file_path).unwrap());
    let mut message = String::new();

    let mut cur_line: usize = lines;

    // Read the file line by line using the lines() iterator from std::io::BufRead.
    for (i, line) in reader.lines().enumerate() {
        if i < cur_line { continue; }
        cur_line = i;
        // filter out non ascii to prevent potential panics, alternatively I should just split the
        // lines into chars then use the index of that but we should not have to index string
        // contents that contains non ascii if it is going to be sent to the chat bridge clients
        let raw = line.unwrap().replace(|c: char| !c.is_ascii(), "");

        // we only care about the line if it's a server 'info' message
        if raw.len() < 35 || &raw[10..31] != " [Server thread/INFO]" { continue; }

        let newline = &raw[33..];

        // allow join and leave messages to pass through the filter
        if !(newline.contains("<") && newline.contains(">"))
            && !(newline.contains("joined") || newline.contains("left"))
        {
            continue;
        }

        if newline.len() > 1 {
            let nmessage = format!("[{server_name}] {newline}\n");
            message.push_str(&nmessage);
        }
    }
    // if the log file is above 4k we can reset it to prevent parsing time from building up
    if cur_line < 4000 {
        return (Some(message), cur_line);
    }

    // reset pipe file and notify
    gen_pipe(&server_name, true).await;
    println!("*info: pipe file reset -> {server_name}");
    (None, 0)
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
        _ => None
    }
}

// send messages to all servers with a 'game' session
pub fn send_chat(servers: &Vec<Session>, message: &str) {
    let lines: Vec<&str> = message.split("\n").collect();
    for line in lines {
        let line = &line.replace("MSG ", "");
        let pos = match line.find("]") {
            Some(v) => v,
            None => 0,
        };
        let msg = match clear_formatting(line) {
            Some(v) => v,
            None => continue,
        };
        for server in servers.to_vec() {
            let name = server.name;
            if pos != 0 && line[1..pos] == name || server.game.is_none() {
                continue;
            }
            send_command(
                &name,
                &format!(r#"tellraw @a {{ "text": "{}" }}"#, msg),
            );
        }
    }
}

// send command to tmux session
#[inline(always)]
pub fn send_command(server_name: &str, message: &str) {
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &server_name, &message, "Enter"])
        .spawn();

    // clean up zombies
    reap();
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
