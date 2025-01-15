mod parser;

use serde::Serialize;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Read,
};

#[derive(PartialEq, Clone, Debug)]
pub struct ProcStat {
    pub user: u32,
    pub nice: u32,
    pub system: u32,
    pub idle: u32,
    pub iowait: u32,
    pub irq: u32,
    pub softirq: u32,
    pub btime: u32,
}

#[derive(PartialEq, Clone, Debug)]
pub struct ProcPidStat {
    pub pid: u32,
    pub comm: String,
    pub state: char,
    pub utime: u32,
    pub stime: u32,
}

pub fn pids_by_comm(comm: &str) -> Vec<u32> {
    let pids = fs::read_dir("/proc/")
        .unwrap()
        .filter_map(|x| x.ok())
        .filter(|x| x.file_type().ok().map(|x| x.is_dir()).unwrap_or_default())
        .filter_map(|x| x.file_name().to_str().and_then(|x| x.parse::<u32>().ok()));

    let mut out = Vec::new();

    let mut buf = String::new();
    for pid in pids {
        let p = format!("/proc/{}/comm", pid);
        let mut f = match File::open(p) {
            Ok(f) => f,
            Err(_) => continue,
        };

        buf.clear();
        f.read_to_string(&mut buf).unwrap();

        if buf.trim() == comm {
            out.push(pid);
        }
    }

    out
}

pub fn fetch_proc_stat() -> ProcStat {
    let buf = fs::read_to_string("/proc/stat").unwrap();
    parser::parse_proc_stat(&buf)
}

pub fn fetch_proc_pid_stat(pidlike: &str) -> ProcPidStat {
    let path = format!("/proc/{}/stat", pidlike);
    let buf = fs::read_to_string(path).unwrap();
    parser::parse_proc_pid_stat(&buf)
}

#[derive(Serialize, PartialEq, Clone, Debug)]
pub struct Metrics {
    pub max: u32,
    pub overall: u32,
    #[serde(rename = "self")]
    pub self_: u32,
    pub processes: HashMap<String, u32>,
}
