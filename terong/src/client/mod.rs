mod event_consumer;
mod protocol_client;

use log::debug;
use std::{convert::identity, sync::mpsc, thread};

/// Run the client application.
pub fn run() {
    debug!("starting client");

    let (event_tx, event_rx) = mpsc::channel();

    let (stop_client_tx, stop_client_rx) = mpsc::sync_channel(0);
    let client = thread::Builder::new()
        .name("protocol-client".to_owned())
        .spawn(|| {
            protocol_client::run(event_tx, stop_client_rx);
        })
        .expect("failed to create thread for protocol client");

    let consumer = thread::Builder::new()
        .name("event-consumer".to_owned())
        .spawn(|| {
            event_consumer::run(event_rx);
        })
        .expect("failed to create thread for event consumer");

    let mut workers = [Some(client), Some(consumer)];
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

    debug!("client stopped");
}
