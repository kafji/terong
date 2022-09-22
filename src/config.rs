//! Applications configuration.

use crate::{client::config::ClientConfig, server::config::ServerConfig};
use anyhow::Error;
use serde::Deserialize;
use std::env;
use tokio::fs;

/// Get value of hidden configuration to disable TLS.
pub fn no_tls() -> bool {
    env::var("DUANGLER_NO_TLS")
        .ok()
        .and_then(|x| x.parse::<u8>().ok())
        .map(|x| x == 1)
        .unwrap_or_default()
}

/// Data structure representing config file scheme.
#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    client: Option<ClientConfig>,
    server: Option<ServerConfig>,
}

impl Config {
    pub async fn read_config() -> Result<Self, Error> {
        let path = "./duangler.toml";
        let config = fs::read_to_string(path).await?;
        let config = toml::from_str(&config)?;
        Ok(config)
    }

    pub fn client(self) -> ClientConfig {
        self.client.expect("missing client config")
    }

    pub fn server(self) -> ServerConfig {
        self.server.expect("missing server config")
    }
}
