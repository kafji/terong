mod transport_server;

pub mod config;

use crate::{
    config::Config,
    logging::init_tracing,
    server::{config::ServerConfig, transport_server::TransportServer},
    transport::{generate_tls_key_pair, protocol::Sha256},
};
use tokio::{sync::mpsc, try_join};
use tracing::info;

/// Run the server application.
pub async fn run() {
    init_tracing();

    let ServerConfig { port, addr } = Config::read_config()
        .await
        .expect("failed to read config")
        .server();

    let (tls_cert, tls_key) = generate_tls_key_pair(addr).expect("failed to generate tls key pair");

    info!("starting server app");

    println!(
        "Server TLS certificate hash is {}.",
        Sha256::from_bytes(tls_cert.as_ref())
    );

    let (event_tx, event_rx) = mpsc::channel(1);

    let input_source = crate::input_source::start(event_tx);

    let server = {
        let env = TransportServer {
            port,
            event_rx,
            tls_cert,
            tls_key,
        };
        transport_server::start(env)
    };

    try_join!(input_source, server).unwrap();

    info!("server app stopped");
}
