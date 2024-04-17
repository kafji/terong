use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Clone, Deserialize, Debug)]
pub struct ClientConfig {
    pub tls_cert_path: PathBuf,
    pub tls_key_path: PathBuf,

    pub server_addr: SocketAddr,

    pub server_tls_cert_path: PathBuf,
}
