use anyhow::Error;
use serde::Deserialize;
use std::net::SocketAddr;
use tokio::fs;

#[derive(Clone, Deserialize, Debug)]
pub struct ClientConfig {
    pub server_addr: SocketAddr,
}

#[derive(Deserialize, Debug)]
struct Config {
    client: ClientConfig,
}

impl ClientConfig {
    pub async fn read_config() -> Result<Self, Error> {
        let path = "terong.toml";
        let config = fs::read_to_string(path).await?;
        let config: Config = toml::from_str(&config)?;
        Ok(config.client)
    }
}
