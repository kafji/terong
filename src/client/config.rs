use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Deserialize, Debug)]
pub struct ClientConfig {
    pub addr: IpAddr,
    pub server_addr: SocketAddr,
}
