mod transport_server;

use tokio::{sync::mpsc, try_join};
use tracing::info;

/// Run the server application.
pub async fn run() {
    info!("starting server app");

    let (event_tx, event_rx) = mpsc::channel(1);

    let input_source = crate::input_source::start(event_tx);

    let server = transport_server::start(event_rx);

    try_join!(input_source, server).unwrap();

    info!("server app stopped");
}
