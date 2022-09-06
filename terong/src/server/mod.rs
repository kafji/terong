mod event_producer;
mod protocol_server;

use std::{sync::mpsc, thread};

/// Run the server application.
pub fn run() {
    println!("starting server");

    let (tx, rx) = mpsc::channel();

    let producer = thread::Builder::new()
        .name("event-producer".to_owned())
        .spawn(|| {
            event_producer::run(tx);
        })
        .expect("failed to create thread for event producer");

    let server = thread::Builder::new()
        .name("protocol-server".to_owned())
        .spawn(|| {
            protocol_server::run(rx);
        })
        .expect("failed to create thread for protocol server");

    producer.join().unwrap();
    server.join().unwrap();

    println!("server stopped");
}
