use serde::{Deserialize, Serialize};
use std::{fmt::Debug, time::Duration};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Ping {
    pub counter: u16,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Pong {
    pub counter: u16,
}

pub const PING_INTERVAL_DURATION: Duration = Duration::from_secs(1);
