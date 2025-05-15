use sage::{fetch_proc_pid_stat, fetch_proc_stat, pids_by_comm, Metrics, ProcPidStat, ProcStat};
use std::{collections::HashMap, thread, time::Duration};

fn main() {
    let mut monitor = Monitor::new(&["firefox", "terong-client", "thunderbird"]);
    loop {
        thread::sleep(Duration::from_secs(1));

        let metrics = monitor.get_metrics();
        println!("{}", serde_json::to_string(&metrics).unwrap());
    }
}

#[derive(Debug)]
struct Monitor<'a> {
    stat: ProcStat,
    self_stat: ProcPidStat,
    pstats: HashMap<&'a str, Vec<ProcPidStat>>,
}

impl<'a> Monitor<'a> {
    fn new(pnames: &[&'a str]) -> Self {
        let stat = fetch_proc_stat();
        let self_stat = fetch_proc_pid_stat("self");
        let pstats = {
            let values = pnames
                .iter()
                .copied()
                .map(|name| pids_by_comm(name))
                .map(|pids| {
                    pids.into_iter()
                        .map(|pid| fetch_proc_pid_stat(&pid.to_string()))
                        .collect()
                });
            pnames
                .iter()
                .copied()
                .zip(values)
                .collect::<HashMap<_, _>>()
        };
        Self {
            stat,
            self_stat,
            pstats,
        }
    }

    fn get_metrics(&mut self) -> Metrics {
        let pnames = self.pstats.keys().copied().collect::<Vec<_>>();
        let new = Self::new(&pnames);

        let overall = (new.stat.user - self.stat.user) + (new.stat.system - self.stat.system);
        let max = overall + (new.stat.idle - self.stat.idle);

        let self_ = (new.self_stat.utime + new.self_stat.stime)
            - (self.self_stat.utime + self.self_stat.stime);

        let mut metrics = Metrics {
            overall,
            max,
            self_,
            processes: HashMap::new(),
        };

        for (&name, stats) in &new.pstats {
            metrics.processes.insert(name.to_owned(), 0);

            for stat in stats {
                if let Some(prev) = self.pstats[name].iter().find(|x| x.pid == stat.pid) {
                    let usage = (stat.utime + stat.stime) - (prev.utime + prev.stime);
                    if let Some(x) = metrics.processes.get_mut(name) {
                        *x += usage;
                    }
                }
            }
        }

        *self = new;

        metrics
    }
}
