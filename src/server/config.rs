use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Deserialize, Debug)]
pub struct ServerConfig {
    pub port: u16,
    pub addr: IpAddr,
}
