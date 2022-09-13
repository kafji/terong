mod input_sink;
mod transport_client;

use tokio::sync::mpsc;
use tracing::info;

/// Run the client application.
pub async fn run() {
    info!("starting client");

    let (event_tx, event_rx) = mpsc::unbounded_channel();

    let client = transport_client::start(event_tx.clone());

    let consumer = input_sink::start(event_rx);

    tokio::try_join!(client, consumer).unwrap();

    info!("client stopped");
}
