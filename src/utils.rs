use serde::{Serialize, Serializer};
use serde_derive::Serialize;
use std::fmt;
use std::{collections::HashMap, sync::Arc};
use sysinfo::{DiskExt, System, SystemExt};
use tokio::sync::{mpsc, Mutex};
use warp::{ws::Message, Rejection};

pub(crate) type Clients = Arc<Mutex<HashMap<String, WsClient>>>;
pub(crate) type Result<T> = std::result::Result<T, Rejection>;

// colored println macros, are they useful? No, do I care? also no

#[macro_export]
macro_rules! exit {
    () => {
        std::process::exit(0);
    };
}

pub(crate) struct WsClient {
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
    pub authed: bool,
}

impl WsClient {
    pub(crate) async fn send<'a, T: Into<String>>(&self, msg: T) {
        if let Some(v) = &self.sender {
            let _ = v.send(Ok(Message::text(msg)));
        }
    }
}

pub(crate) struct Sys {
    disk: Option<u8>,
    disk_info: Vec<(u64, u64, f32)>,
    cpu_avg: (f32, f32),
    ram: (u64, u64),
    uptime: u64,
    sys: System,
}

#[derive(Serialize)]
pub(crate) struct SysDisplay {
    disk: Option<u8>,
    disk_info: Vec<(u64, u64, f32)>,
    cpu_avg: (f32, f32),
    ram: (u64, u64),
    uptime: u64,
}

#[allow(clippy::from_over_into)]
impl Into<SysDisplay> for Sys {
    fn into(self) -> SysDisplay {
        SysDisplay {
            disk: self.disk,
            disk_info: self.disk_info,
            cpu_avg: self.cpu_avg,
            ram: self.ram,
            uptime: self.uptime,
        }
    }
}

impl Sys {
    pub(crate) fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        Self {
            disk: Self::check_disk(&sys),
            disk_info: Self::disk_info(&sys),
            cpu_avg: Self::cpu_average(&sys),
            ram: Self::get_ram(&sys),
            uptime: Self::uptime(&sys),
            sys,
        }
    }

    pub(crate) fn refresh(&mut self) {
        let sys = &mut self.sys;
        sys.refresh_all();
        self.disk = Self::check_disk(sys);
        self.disk_info = Self::disk_info(sys);
        self.cpu_avg = Self::cpu_average(sys);
        self.uptime = Self::uptime(sys);
    }

    pub(crate) fn sys_health_check(&self) -> bool {
        let ram = self.ram;
        if ram.0 as f64 / ram.1 as f64 > 0.85
            || self.cpu_avg.1 > 0.8
            || Self::check_disk(&self.sys).is_some()
        {
            return true;
        }
        false
    }

    fn cpu_average(sys: &System) -> (f32, f32) {
        let ldavg = &sys.load_average().five;
        if *ldavg < 0.0 {
            return (0.0, 0.0);
        }
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
            if disk.available_space() as f32 / disk.total_space() as f32 > 0.85 {
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
}

impl fmt::Display for Sys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut response = String::new();
        if self.disk.is_some() {
            response.push_str(&format!(
                "\\*warn: disk space low on drive index: {}\n",
                self.disk.unwrap()
            ));
        }
        response.push_str("disks: \n");
        for disk in &self.disk_info {
            response.push_str(&format!(
                "{} MiB / {} MiB {}%",
                Sys::make_mb(disk.0),
                Sys::make_mb(disk.1),
                disk.2
            ));
        }

        write!(
            f,
            "{response}\nload average: {} \n\
        cpu average: {}% system uptime: {} hrs",
            self.cpu_avg.0,
            self.cpu_avg.1,
            self.uptime / 3600
        )
    }
}
