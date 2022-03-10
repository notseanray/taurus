use crate::config::Session;
use crate::{utils::check_exist, utils::reap};
use regex::Regex;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use tokio::process::Command;

// update messages from the log file, this takes in the log file, checks if the lines can be
// ignored, then checks if the new lines are in game commands, if they are then use handle command
// to check them and if not send them to discord
//
// unfortunately this is not very efficient but honestly I don't really care, this runs on separate
// threads from the mc server and if the log file gets above 2k lines it gets repiped with tmux to
// prevent the function from taing too long
pub async fn update_messages<T>(server_name: T, lines: usize) -> (Option<String>, usize)
where
    T: ToString,
{
    let server_name = server_name.to_string();
    let file_path: String = format!("/tmp/{server_name}-taurus");
    if !check_exist(&file_path.to_owned()) {
        return (None, 0);
    }

    // open the log file in bufreader
    let file = File::open(&file_path).unwrap();
    let reader = BufReader::new(file);
    let mut message = "".to_string();

    let mut cur_line: usize = lines;

    // Read the file line by line using the lines() iterator from std::io::BufRead.
    for (i, line) in reader.lines().enumerate() {
        // skip lines that are irrelevant
        if i > cur_line {
            // if they are new, update the counter
            cur_line = i;

            let line = line.unwrap();

            let raw = line.replace(|c: char| !c.is_ascii(), "");

            // if the line is too short then skip it
            if raw.len() < 35 || &raw[10..31] != " [Server thread/INFO]" {
                continue;
            }

            let newline = &raw[33..];

            if !(newline.contains("<") && newline.contains(">"))
                && !(newline.contains("joined") || newline.contains("left"))
            {
                continue;
            }

            if newline.len() < 1 {
                continue;
            }
            let nmessage = format!("[{server_name}] {newline}\n");

            message.push_str(&nmessage);
        }
    }

    // if the lines are under 2k, we don't need to replace the file since it doesn't take much time
    // to process in the first place
    if cur_line < 2000 {
        return (Some(message), cur_line);
    }

    // if it is above 2k however, we can reset the pipe and notify the to the console
    gen_pipe(&server_name, true).await;
    println!("*info: pipe file reset -> {server_name}");

    // return new line count to update the one in the main file
    (None, 0)
}

// checks the number of lines in the log file to set them initially, this prevents old messages
// from being spat out if the bot restarts (and makes it a lot less annoying)
pub fn set_lines<T>(server_name: T) -> usize
where
    T: ToString,
{
    let server_name = server_name.to_string();
    let file = File::open(&format!("/tmp/{server_name}-taurus")).unwrap();
    let reader = BufReader::new(file);

    // count the amount of lines in the log file
    reader.lines().count()
}

#[inline(always)]
pub fn replace_formatting<T>(msg: T) -> String
where
    T: ToString,
{
    let msg = msg.to_string();
    // TODO MORE REGEX
    // regex to replace any '§' followed by digits with a blank space
    let replacements = Regex::new(r"§.*\d").unwrap();
    replacements.replace_all(&msg, "").to_owned().to_string();
    msg.replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\"", "\\\"")
        .replace("_", "\\_")
}

#[inline(always)]
fn clear_formatting<T>(msg: T) -> Option<String>
where
    T: ToString,
{
    let msg = msg
        .to_string()
        .replace("\\", "")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("{", "{{")
        .replace("}", "}}")
        .replace("\"", "\\\"");
    if msg.len() < 1 {
        return None;
    }
    Some(msg)
}

pub fn send_chat<T>(servers: &Vec<Session>, message: T)
where
    T: ToString,
{
    let message = &message.to_string();
    let lines: Vec<&str> = message.split("\n").collect();
    for line in lines {
        let line = &line.replace("MSG ", "");
        let pos = match line.find("]") {
            Some(v) => v,
            None => 0,
        };
        // TODO replace formatting
        for server in servers.to_vec() {
            let name = server.name;
            if pos != 0 && line[1..pos] == name {
                continue;
            }
            let msg = match clear_formatting(line) {
                Some(v) => v,
                None => continue,
            };
            send_command(&name, &format!(r#"tellraw @a {{ "text": "{msg}" }}"#));
        }
    }
}

// small function to send a command to the specific tmux session, this replaces new lines due to it
// causing a problem with commands
//
// this is one of the limitations of this system, but it's not that bad because if there are
// multiple lines you can send the command multiple times
#[inline(always)]
pub fn send_command(server_name: &str, message: &str) {
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &server_name, &message, "Enter"])
        .spawn();

    reap();
}

// generate the tmux pipe connecting to the specified server, this also takes in the option to
// delete the file if it exists before generating it
// that can be used at startup or when just resetting the file in general
#[inline]
pub async fn gen_pipe(server_name: &str, rm: bool) {
    println!("PIPE {server_name}");
    let pipe = format!("/tmp/{server_name}-taurus");
    if rm {
        let _ = fs::remove_file(&pipe);
    }

    // create the tmux command that will be entered to set the pipe
    Command::new("tmux")
        .args(["pipe-pane", "-t", &server_name, &format!("cat > {pipe}")])
        .spawn()
        .expect("*error: \x1b[31mfailed to generate pipe file\x1b[0m");

    // call reap to remove any zombie processes generated by it
    reap();
}
