use crate::{
    utils::reap,
    utils::check_exist,
};
use std::io::{
    BufRead, 
    BufReader
};
use std::{
    fs::File,
    process::Command
};
use regex::Regex;

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
    let file_path: String = format!("/tmp/{server_name}-lupus");
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

            // if the line is too short then skip it
            if &line.chars().count() < &35 {
                continue;
            }

            // check if the message starts with certain characters
            let line_sep: &str = &line[33..];
            if !line.starts_with("[") || (!line_sep.starts_with("<") && !line_sep.starts_with("§"))
            {
                continue;
            }

            let newline = &line[33..];

            if newline.len() < 1 {
                continue;
            }

            // if it's not an in game command, we can generate what the discord message will be
            //
            // firstly we put the server name then the new line message, this is where replace
            // formatting comes in to remove the special mc escape sequences
            let nmessage = format!("[{server_name}] {newline}\n");

            message.push_str(&nmessage);
        }
    }

    // if the lines are under 2k, we don't need to replace the file since it doesn't take much time
    // to process in the first place
    if lines < 2000 {
        return (Some(message), cur_line);
    }

    // if it is above 2k however, we can reset the pipe and notify the to the console
    gen_pipe(server_name.to_owned(), true).await;
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
    let file = File::open(&format!("/tmp/{server_name}-lupus")).unwrap();
    let reader = BufReader::new(file);

    // count the amount of lines in the log file
    reader.lines().count()
}

// This removes all the formmating codes coming from MC chat with regex
#[inline(always)]
pub fn replace_formatting(mut msg: String) -> String {
    // TODO MORE REGEX
    msg = msg.replace("_", "\\_");
    // regex to replace any '§' followed by digits with a blank space
    let mc_codes = Regex::new(r"§.*\d").unwrap();
    mc_codes.replace_all(&msg, "").to_owned().to_string()
}

// small function to send a command to the specific tmux session, this replaces new lines due to it
// causing a problem with commands
//
// this is one of the limitations of this system, but it's not that bad because if there are
// multiple lines you can send the command multiple times
#[inline(always)]
pub async fn send_command<T>(server_name: T, message: T)
where
    T: ToString,
{
    let (message, server_name) = (message.to_string(), server_name.to_string());
    // if there are any non ascii characters then we can return as there's likely problems with the
    // rest of the command
    message.chars().for_each(|c| {
        if !c.is_ascii() {
            return;
        }
    });

    Command::new("tmux")
        .args(["send-keys", "-t", &server_name, &message, "Enter"])
        .spawn()
        .expect("*error: failed to send to tmux session");

    reap();
}

// generate the tmux pipe connecting to the specified server, this also takes in the option to
// delete the file if it exists before generating it
// that can be used at startup or when just resetting the file in general
#[inline]
pub async fn gen_pipe<T>(server_name: T, rm: bool)
where
    T: ToString,
{
    let server_name = server_name.to_string();
    let pipe = format!("/tmp/{server_name}-taurus");
    if rm {
        // remove the old pipe file if it exists
        if check_exist(&pipe) {
            Command::new("rm")
                .arg(&pipe)
                .spawn()
                .expect("*error: \x1b[31mfailed to delete pipe file\x1b[0m");
        }
    }

    // create the tmux command that will be entered to set the pipe
    Command::new("tmux")
        .args(["pipe-pane", "-t", &server_name, &format!("cat > {pipe}")])
        .spawn()
        .expect("*error: \x1b[31mfailed to generate pipe file\x1b[0m");

    // call reap to remove any zombie processes generated by it
    reap();
}
