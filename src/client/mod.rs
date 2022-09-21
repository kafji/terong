mod config;
mod input_sink;
mod transport_client;

use crate::{client::config::ClientConfig, logging::init_logger};
use tokio::sync::mpsc;
use tracing::info;

/// Run the client application.
pub async fn run() {
    init_logger();

    let config @ ClientConfig { server_addr } = ClientConfig::read_config()
        .await
        .expect("failed to read config");

    info!(?config, "starting client app");

    let (event_tx, event_rx) = mpsc::channel(1);

    let client = transport_client::start(server_addr, event_tx.clone());

    let consumer = input_sink::start(event_rx);

    tokio::try_join!(client, consumer).unwrap();

    info!("client app stopped");
}
