mod transport_server;

use crate::logging::init_logger;
use tokio::{sync::mpsc, try_join};
use tracing::info;

pub async fn run() {
    init_logger();

    info!("starting server app");

    let (event_tx, event_rx) = mpsc::channel(1);

    let input_source = crate::input_source::start(event_tx);

    let server = transport_server::start(event_rx);

    try_join!(input_source, server).unwrap();

    info!("server app stopped");
}
