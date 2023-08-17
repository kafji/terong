mod transport_server;

pub mod config;

use crate::{
    config::{read_certs, read_private_key, Config},
    logging::init_tracing,
    server::{config::ServerConfig, transport_server::TransportServer},
};
use anyhow::Error;
use cfg_if::cfg_if;
use tokio::{sync::mpsc, try_join};
use tracing::info;

async fn start_server_app(cfg: ServerConfig) -> Result<(), Error> {
    info!(?cfg, "starting server app");

    let ServerConfig {
        port,
        tls_cert_path,
        tls_key_path,
        client_tls_cert_path,
        ..
    } = cfg;

    let (event_tx, event_rx) = mpsc::channel(1);

    let input_source = {
        cfg_if! {
            if #[cfg(target_os = "linux")] {
                crate::input_source::start(
                    cfg.linux.keyboard_device,
                    cfg.linux.mouse_device,
                    cfg.linux.touchpad_device,
                    event_tx
                )
            } else {
                crate::input_source::start(event_tx)
            }
        }
    };

    let server = {
        let tls_certs = read_certs(&tls_cert_path).await?;

        let tls_key = read_private_key(&tls_key_path).await?;

        let client_tls_certs = read_certs(&client_tls_cert_path).await?;

        let args = TransportServer {
            port,
            event_rx,
            tls_certs,
            tls_key,
            client_tls_certs,
        };
        transport_server::start(args)
    };

    try_join!(input_source, server).unwrap();

    info!("server app stopped");

    Ok(())
}

/// Run the server application.
pub async fn run() {
    init_tracing();

    let cfg = Config::get().await.server();

    start_server_app(cfg).await.unwrap();
}
