use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Ping {
    pub counter: u16,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Pong {
    pub counter: u16,
}
