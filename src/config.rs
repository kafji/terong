//! Applications configuration.

use crate::{client::config::ClientConfig, server::config::ServerConfig};
use anyhow::{anyhow, Error};
use serde::Deserialize;
use std::{env, path::PathBuf};
use tokio::{fs::File, io::AsyncReadExt};
use tracing::debug;

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
        let mut paths = config_paths();

        let file = loop {
            let path = match paths.next() {
                Some(x) => x,
                None => break None,
            };

            match File::open(&path).await {
                Ok(x) => break Some(x),
                Err(err) => {
                    debug!(?path, ?err, "failed to open config file")
                }
            }
        };

        let mut file = file.ok_or_else(|| anyhow!("failed to find configuration file"))?;

        let mut buf = String::new();
        file.read_to_string(&mut buf).await?;

        let config = toml::from_str(&buf)?;

        Ok(config)
    }

    pub fn client(self) -> ClientConfig {
        self.client.expect("missing client config")
    }

    pub fn server(self) -> ServerConfig {
        self.server.expect("missing server config")
    }
}

fn config_paths() -> impl Iterator<Item = PathBuf> {
    [
        // in cwd
        Some("./duangler.toml".into()),
        // in os specific config dir
        {
            #[cfg(target_os = "linux")]
            {
                env::var("XDG_CONFIG_HOME").ok().map(PathBuf::from)
            }
            #[cfg(target_os = "windows")]
            {
                env::var("LocalAppData").ok().map(PathBuf::from)
            }
        }
        .map(|x| x.join("net.kafji.duangler").join("duangler.toml")),
    ]
    .into_iter()
    .flatten()
}
