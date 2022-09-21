use anyhow::Error;
use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Clone, Deserialize, Debug)]
pub struct ClientConfig {
    pub server_addr: SocketAddr,
}

impl ClientConfig {
    pub async fn read_config() -> Result<Self, Error> {
        todo!()
    }
}
