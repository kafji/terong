use serde::{Deserialize, Serialize};
use std::{fmt::Debug, time::Duration};
use tokio::time::{Instant, sleep_until};

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct Ping {}

#[derive(Debug)]
pub(crate) struct HeartbeatTimers {
    timeout: Duration,
    recv_deadline: Instant,
    send_deadline: Instant,
}

impl HeartbeatTimers {
    pub(crate) fn new() -> Self {
        let timeout = Duration::from_secs(20);
        let recv_deadline = Instant::now().checked_add(timeout).unwrap();
        let send_deadline = Instant::now().checked_add(timeout / 2).unwrap();
        Self {
            timeout: timeout,
            recv_deadline,
            send_deadline,
        }
    }

    pub(crate) fn recv_deadline(&self) -> impl Future<Output = ()> {
        sleep_until(self.recv_deadline)
    }

    pub(crate) fn reset_recv_deadline(&mut self) {
        self.recv_deadline = Instant::now().checked_add(self.timeout).unwrap();
    }

    pub(crate) fn send_deadline(&self) -> impl Future<Output = ()> {
        sleep_until(self.send_deadline)
    }

    pub(crate) fn reset_send_deadline(&mut self) {
        self.send_deadline = Instant::now().checked_add(self.timeout / 2).unwrap();
    }

    pub(crate) fn timeout(&self) -> Duration {
        self.timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn test_recv_deadline() {
        let timer = HeartbeatTimers::new();
        let start = Instant::now();
        timer.recv_deadline().await;
        assert_eq!(start.elapsed(), Duration::from_secs(20));
    }

    #[tokio::test(start_paused = true)]
    async fn test_send_deadline() {
        let timer = HeartbeatTimers::new();
        let start = Instant::now();
        timer.send_deadline().await;
        assert_eq!(start.elapsed(), Duration::from_secs(10));
    }
}
