extern crate libc;
use libc::{c_int, pid_t};
use serde_derive::Deserialize;
use std::{
    path::PathBuf,
    process::Command,
};
use warp::ws::Message;
use sysinfo::{DiskExt, System, SystemExt};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use warp::Rejection;
use tokio::sync::mpsc;

pub type Clients = Arc<Mutex<HashMap<String, WsClient>>>;
pub type Result<T> = std::result::Result<T, Rejection>;

#[derive(Debug, Clone)]
pub struct WsClient {
    pub client_id: String,
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
}

extern "C" {
    pub fn waitpid(pid: pid_t, stat_loc: *mut c_int, options: c_int) -> pid_t;
}

#[derive(Deserialize, Debug, Clone)]
pub struct Session {
    pub name: String,
    pub description: String,
    pub host: String,
    pub game: Option<Game>,
    pub rcon: Option<Rcon>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Game {
    pub file_path: Option<String>,
    pub backup_interval: Option<usize>,
    pub backup_keep: Option<usize>,
    pub in_game_cmd: Option<bool>,
    pub lines: Option<usize>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Rcon {
    pub ip: Option<String>,
    pub port: u64,
    pub password: String,
}

// std::Command can leave behind zombie processes that buid up over time, this small function uses
#[inline]
pub fn reap() {
    unsafe {
        waitpid(-1, std::ptr::null_mut(), 0x00000001);
    }
}

// function to check if the file or folder exist, if it does not exists emit a warning depending if
// the warning should be silenced or not
#[inline]
pub fn check_exist(dir: &str) -> bool {
    let current_path = PathBuf::from(dir);
    return current_path.exists();
}

pub async fn send_to_clients(clients: &Clients, msg: String) {
    let locked = clients.lock().await;
    for (key, _) in locked.iter() {
        match locked.get(key) {
            Some(t) => {
                if let Some(t) = &t.sender {
                    let _ = t.send(
                        Ok(Message::text(
                            "CHAT_OUT ".to_owned() + 
                            &msg.to_owned()
                            )
                            )
                        );
                }
            }
            None => continue,
        };
    }
}

// small function to send a command to the specific tmux session, this replaces new lines due to it
// causing a problem with commands
//
// this is one of the limitations of this system, but it's not that bad because if there are
// multiple lines you can send the command multiple times
#[inline(always)]
pub async fn send_command(server_name: String, message: String) {
    // if there are any non ascii characters then we can return as there's likely problems with the
    // rest of the command
    message.chars().for_each(|c| if !c.is_ascii() { return; });

    Command::new("tmux")
        .args(["send-keys", "-t", &server_name, &message, "Enter"])
        .spawn()
        .expect("*error: failed to send to tmux session");

    reap();
}
// TODO
// fix disk usage
//

pub async fn sys_check(dis: bool, chat_id: u64) {
    let (mut sys, mut warn) = (System::new_all(), false);
    sys.refresh_all();
    let mut response = String::new();

    // future, if first element < 100, it is the index of the disk that has problems
    let (u, t, i) = check_disk(&sys);

    // rustc is phasing out floats in match statements for obvious reasons, since we only need to check
    // for one value we can multiply this to get around it
    let x = (i * 100.0) as u16;
    if x != 10 {
        let drive = format!("drive low on space!:\nindex: {}\n", i);
        warn = true;
        response.push_str(&drive);
    }
    let drive = format!(
        "drive usage: {:.1} Mb /{:.1} Mb ({:.1}%)\n",
        u / 1048576.0,
        t / 1048576.0,
        (u / t) * 100.0
    );
    response.push_str(&drive);

    let ldavg = &sys.load_average().five;

    if ldavg > &0.0 {
        if ldavg > &(sys.physical_core_count().unwrap() as f64) {
            warn = true;
            response.push_str(&"high load average detected!\n");
        }
        // core count is only accurate if you have hyperthreading
        let avg = format!(
            "load average (5 minutes): {} -> {} logical cores\n",
            ldavg,
            sys.physical_core_count().unwrap() * 2
        );

        response.push_str(&avg);
    }

    if (sys.used_memory() as f64 / sys.total_memory() as f64) > 0.9 {
        response.push_str(&"high ram usage detected!\n");
        warn = true;
    }
    let ramu = format!(
        "ram usage: {} Mb / {} Mb ({:.2}%)\n",
        sys.used_memory() / 1045,
        sys.total_memory() / 1045,
        (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0
    );
    response.push_str(&ramu);

    let uptime = format!("server uptime: {} hrs\n", (sys.uptime() / 3600));

    response.push_str(&uptime);
}

// check the disk space avaible on the server, overfilling a drive is never a good thing and having
// this automatically be checked every few minutes is quite nice
//
// the function returns the index of the drive if there is one in trouble, this can help quickly sort
// things out through df -h if needed
pub fn check_disk(sys: &System) -> (f64, f64, f64) {
    let (mut used_biggest, mut used_total) = (0.0, 0.0);
    let (mut warn_i, mut cur_i) = (0, 0);
    let mut warn: bool = false;
    for disk in sys.disks() {
        // check if the disk space is over 10 gig total, if it is smaller it could be a ramfs or
        // temp partition that we can ignore
        if disk.total_space() < 10737418240 {
            continue;
        }

        let total_space = disk.total_space() as f64;

        if total_space > used_total {
            used_total = total_space;
            used_biggest = disk.available_space() as f64;
        }

        if ((used_total - used_biggest) / disk.total_space() as f64) > 0.9 {
            warn = true;
            warn_i = cur_i;
            println!("*warn: drive space low on drive index: {}", warn_i);
        }

        cur_i += 1;
    }
    if warn {
        return (used_total - used_biggest, used_total, warn_i as f64);
    } else {
        return (used_total - used_biggest, used_total, 0.1);
    }
}
