mod event_producer;
mod protocol_server;

use log::debug;
use std::{convert::identity, sync::mpsc, thread};

/// Run the server application.
pub fn run() {
    debug!("starting server");

    let (event_tx, event_rx) = mpsc::channel();

    let (stop_producer_tx, stop_producer_rx) = mpsc::sync_channel(0);
    let producer = thread::Builder::new()
        .name("event-producer".to_owned())
        .spawn(|| {
            event_producer::run(event_tx, stop_producer_rx);
        })
        .expect("failed to create thread for event producer");

    let (stop_server_tx, stop_server_rx) = mpsc::sync_channel(0);
    let server = thread::Builder::new()
        .name("protocol-server".to_owned())
        .spawn(|| {
            protocol_server::run(event_rx, stop_server_rx);
        })
        .expect("failed to create thread for protocol server");

    let mut workers = [Some(producer), Some(server)];
    loop {
        if workers.iter().map(|x| x.is_none()).all(identity) {
            break;
        }
        for w in workers.iter_mut() {
            let finished = w.as_ref().map(|x| x.is_finished()).unwrap_or_default();
            if finished {
                let worker = w.take().unwrap();
                worker.join().unwrap();
            }
        }
    }

    debug!("server stopped");
}
