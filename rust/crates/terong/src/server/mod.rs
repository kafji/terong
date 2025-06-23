mod transport_server;

pub mod config;
pub mod input_source;

use crate::{
    config::{Config, read_certs, read_private_key},
    logging::init_tracing,
    server::{config::ServerConfig, transport_server::TransportServer},
};
use anyhow::{Context, Error};
use tokio::{sync::mpsc, try_join};
use tracing::info;

async fn start_app(cfg: ServerConfig, should_log: bool) -> Result<(), Error> {
    info!(?cfg, "starting server app");

    let ServerConfig {
        port,
        tls_cert_path,
        tls_key_path,
        tls_root_cert_path,
        ..
    } = cfg;

    let (event_tx, event_rx) = mpsc::channel(1);

    #[cfg(target_os = "linux")]
    let input_source = input_source::start(
        cfg.linux.keyboard_device,
        cfg.linux.mouse_device,
        cfg.linux.touchpad_device,
        event_tx,
    );

    #[cfg(target_os = "windows")]
    let input_source = input_source::start(event_tx, should_log);

    let server = {
        let tls_certs = read_certs(&tls_cert_path)
            .await
            .context("failed to read server tls cert")?;
        let tls_key = read_private_key(&tls_key_path)
            .await
            .context("failed to read server tls key")?;
        let root_certs = read_certs(&tls_root_cert_path)
            .await
            .context("failed to read tls root cert")?;
        let args = TransportServer {
            port,
            tls_certs,
            tls_key,
            tls_root_certs: root_certs,
        };
        transport_server::start(args, event_rx)
    };

    try_join!(input_source, server).unwrap();

    info!("server app stopped");

    Ok(())
}

/// Run the server application.
pub async fn run(should_log: bool) {
    init_tracing();

    let cfg = Config::get().await.server();

    start_app(cfg, should_log).await.unwrap();
}
