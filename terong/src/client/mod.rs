mod event_consumer;
mod protocol_client;

use std::{sync::mpsc, thread};

/// Run the client application.
pub fn run() {
    let (tx, rx) = mpsc::channel();

    let client = thread::Builder::new()
        .name("protocol-client".to_owned())
        .spawn(|| {
            protocol_client::run(tx);
        })
        .expect("failed to create thread for protocol client");

    client.join().unwrap();
}
