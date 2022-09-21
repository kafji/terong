mod input_sink;
mod transport_client;

use crate::logging::init_logger;
use tokio::sync::mpsc;
use tracing::info;

/// Run the client application.
pub async fn run() {
    init_logger();

    info!("starting client app");

    let (event_tx, event_rx) = mpsc::channel(1);

    let client = transport_client::start(event_tx.clone());

    let consumer = input_sink::start(event_rx);

    tokio::try_join!(client, consumer).unwrap();

    info!("client app stopped");
}
