//! Applications configuration.

use crate::client::config::ClientConfig;
use serde::Deserialize;
use std::env;
use tokio::fs;

/// Get value of hidden configuration to disable TLS.
pub fn no_tls() -> bool {
    env::var("TERONG_NO_TLS")
        .ok()
        .and_then(|x| x.parse::<u8>().ok())
        .map(|x| x == 1)
        .unwrap_or_default()
}

/// Data structure representing config file scheme.
#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    client: Option<ClientConfig>,
}

impl Config {
    pub async fn read_config() -> Self {
        let path = "./terong.toml";
        let config = fs::read_to_string(path)
            .await
            .expect("failed to read config file");
        let config = toml::from_str(&config).expect("failed to parse config");
        config
    }

    pub fn client(self) -> ClientConfig {
        self.client.expect("missing client config")
    }
}
