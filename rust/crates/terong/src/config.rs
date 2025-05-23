//! Applications configuration.

use crate::{
    client::config::ClientConfig,
    server::config::ServerConfig,
    transport::{Certificate, PrivateKey},
};
use anyhow::Error;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::{
    fs::{self, File},
    io::AsyncReadExt,
};
use tracing::{debug, info};

/// Data structure representing config file scheme.
#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    client: Option<ClientConfig>,
    server: Option<ServerConfig>,
}

impl Config {
    pub async fn get() -> Self {
        let mut paths = config_paths();

        let file = loop {
            let path = match paths.next() {
                Some(x) => x,
                None => break None,
            };

            match File::open(&path).await {
                Ok(x) => {
                    info!(?path, "found config file");
                    break Some(x);
                }
                Err(err) => {
                    debug!(?path, ?err, "failed to open config file");
                }
            }
        };

        let mut file = file.expect("failed to find config file");

        let config = Self::from_file(&mut file)
            .await
            .expect("failed to read config from file");

        config
    }

    async fn from_file(file: &mut File) -> Result<Self, Error> {
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
    // there used to be 2 elements in here:
    //   1. in cwd, `./terong.toml`
    //   2. in os specific config dir, i.e.
    //     linux: `XDG_CONFIG_HOME/{namespace}/terong.toml`,
    //     windows: `LOCALAPPDATA/{namespace}/terong.toml`
    ["./terong.toml".into()].into_iter()
}

pub async fn read_certs(path: &Path) -> Result<Vec<Certificate>, Error> {
    let buf = fs::read(path).await?;
    Ok(vec![buf.into()])
}

pub async fn read_private_key(path: &Path) -> Result<PrivateKey, Error> {
    let buf = fs::read(path).await?;
    Ok(buf.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_example() {
        let mut file = File::open("../../example.terong.toml").await.unwrap();
        Config::from_file(&mut file).await.unwrap();
    }
}
