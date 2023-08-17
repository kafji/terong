mod input_sink;
mod transport_client;

pub mod config;

use crate::{
    client::{config::ClientConfig, transport_client::TransportClient},
    config::{read_certs, read_private_key, Config},
    logging::init_tracing,
};
use anyhow::Error;
use tokio::sync::mpsc;
use tracing::info;

async fn start_client_app(cfg: ClientConfig) -> Result<(), Error> {
    info!(?cfg, "starting client app");

    let ClientConfig {
        tls_cert_path,
        tls_key_path,
        server_addr,
        server_tls_cert_path,
    } = cfg;

    // channel for input events from the transport client to the input sink
    let (event_tx, event_rx) = mpsc::channel(1);

    // transport client establishes connection with the server and propagate input
    // events through the channel
    let transport_client = {
        let tls_certs = read_certs(&tls_cert_path).await?;

        let tls_key = read_private_key(&tls_key_path).await?;

        let server_tls_certs = read_certs(&server_tls_cert_path).await?;

        let args = TransportClient {
            server_addr,
            tls_certs,
            tls_key,
            server_tls_certs,
            event_tx,
        };
        transport_client::start(args)
    };

    // input sink receives input events and emulate the input events in its host
    // machine
    let input_sink = input_sink::start(event_rx);

    // The input event channel will be closed when one of the workers, transport
    // client or the input sink, is stopped,  In response to the channel closed
    // the other worker will stop as well and this join will resume.
    tokio::try_join!(transport_client, input_sink)?;

    info!("client app stopped");

    Ok(())
}

/// Run the client application.
pub async fn run() {
    init_tracing();

    let cfg = Config::get().await.client();

    start_client_app(cfg).await.unwrap();
}
