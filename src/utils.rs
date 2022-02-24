use sysinfo::{
    DiskExt, 
    System, 
    SystemExt
};
use std::{
    collections::HashMap, 
    sync::Arc,
    path::PathBuf
};
use libc::{
    c_int, 
    pid_t
};
use tokio::sync::{
    Mutex, 
    mpsc
};
use warp::{
    ws::Message,
    Rejection
};

pub type Clients = Arc<Mutex<HashMap<String, WsClient>>>;
pub type Result<T> = std::result::Result<T, Rejection>;

#[derive(Debug, Clone)]
pub struct WsClient {
    pub client_id: String,
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
}

#[macro_export]
macro_rules! exit {
    () => {
        std::process::exit(0);
    };
}

extern "C" {
    pub fn waitpid(pid: pid_t, stat_loc: *mut c_int, options: c_int) -> pid_t;
}

// std::Command can leave behind zombie processes that buid up over time
#[inline]
pub fn reap() {
    unsafe {
        waitpid(-1, std::ptr::null_mut(), 0x00000001);
    }
}

// function to check if the file or folder exist
#[inline]
pub fn check_exist<T>(dir: T) -> bool
where
    T: ToString,
{
    let current_path = PathBuf::from(dir.to_string());
    return current_path.exists();
}

pub async fn send_to_clients<T>(clients: &Clients, msg: T)
where
    T: ToString,
{
    let locked = clients.lock().await;
    for (key, _) in locked.iter() {
        match locked.get(key) {
            Some(t) => {
                if let Some(t) = &t.sender {
                    let _ = t.send(Ok(Message::text(msg.to_string().clone())));
                }
            }
            None => continue,
        };
    }
}

pub fn sys_check() -> String {
    let mut response = String::new();
    let mut sys = System::new_all();
    sys.refresh_all();

    let disk = check_disk(&sys);

    if disk.is_some() {
        response.push_str(&format!(
            "\\*warn: disk space low on drive index: {}",
            disk.unwrap()
        ));
    }

    response.push_str("disks: ");
    for i in disk_info(&sys) {
        response.push_str(&format!(
            "{} MiB / {} MiB {}%",
            make_mb(i.0),
            make_mb(i.1),
            i.2
        ));
    }

    let (avg, per) = cpu_average(&sys);

    response.push_str(&format!("load average: {avg} cpu average: {per}%"));

    response.push_str(&format!("system uptime: {} hrs", uptime(&sys) / 3600));

    response
}

pub fn sys_health_check() -> bool {
    let mut sys = System::new_all();
    sys.refresh_all();
    let (used, total) = get_ram(&sys);
    if used as f64 / total as f64 > 0.85 {
        return true;
    }

    let (_, per) = cpu_average(&sys);
    if per > 0.7 { return true; }

    false
}

fn cpu_average(sys: &System) -> (f32, f32) {
    let ldavg = &sys.load_average().five;
    if *ldavg < 0.0 { return (0.0, 0.0); }
    let corec = sys.physical_core_count().unwrap();
    (*ldavg as f32, *ldavg as f32 / corec as f32)
}

fn uptime(sys: &System) -> u64 {
    sys.uptime()
}

fn get_ram(sys: &System) -> (u64, u64) {
    (sys.used_memory(), sys.total_memory())
}

fn make_mb(num: u64) -> u64 {
    (num as f32 / 1073.7) as u64
}

fn check_disk(sys: &System) -> Option<u8> {
    for (i, disk) in sys.disks().iter().enumerate() {
        if disk.total_space() < 10737418240 {
            continue;
        }
        if disk.available_space() as f32 / disk.total_space() as f32 > 0.1 {
            return Some(i as u8);
        }
    }
    None
}

fn disk_info(sys: &System) -> Vec<(u64, u64, f32)> {
    let mut response: Vec<(u64, u64, f32)> = Vec::new();
    for disk in sys.disks() {
        let total = disk.total_space();
        if total < 10737418240 || disk.is_removable() {
            continue;
        }
        let used = total - disk.available_space();
        response.push((used, total, (used as f64 / total as f64) as f32));
    }
    response
}

