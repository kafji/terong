mod input_sink;
mod transport_client;

pub mod config;

use crate::{
    client::{config::ClientConfig, transport_client::TransportClient},
    config::Config,
    logging::init_tracing,
    transport::{generate_tls_key_pair, protocol::Sha256},
};
use tokio::sync::mpsc;
use tracing::info;

/// Run the client application.
pub async fn run() {
    init_tracing();

    let ClientConfig { addr, server_addr } = Config::read_config()
        .await
        .expect("failed to read config")
        .client();

    let (tls_cert, tls_key) = generate_tls_key_pair(addr).expect("failed to generate tls key pair");

    info!("starting client app");

    println!(
        "Client TLS certificate hash is {}.",
        Sha256::from_bytes(tls_cert.as_ref())
    );

    // channel for input events from the transport client to the input sink
    let (event_tx, event_rx) = mpsc::channel(1);

    // transport client establishes connection with the server and propagate input
    // events through the channel
    let transport_client = {
        let env = TransportClient {
            server_addr,
            event_tx,
            tls_cert,
            tls_key,
        };
        transport_client::start(env)
    };

    // input sink receives input events and emulate the input events in its host
    // machine
    let input_sink = input_sink::start(event_rx);

    // The input event channel will be closed when one of the workers, transport
    // client or the input sink, is stopped,  In response to the channel closed
    // the other worker will stop as well and this join will resume.
    tokio::try_join!(transport_client, input_sink).unwrap();

    info!("client app stopped");
}
