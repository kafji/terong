mod event_producer;
mod protocol_server;

use crossbeam::channel;
use log::debug;
use std::{convert::identity, path::PathBuf, thread};

/// Run the server application.
pub fn run(config_file: Option<PathBuf>) {
    debug!("starting server");

    let (stop_tx, stop_rx) = channel::bounded(0);

    let (event_tx, event_rx) = channel::unbounded();

    thread::scope(|s| {
        let producer = thread::Builder::new()
            .name("event-producer".to_owned())
            .spawn_scoped(s, || {
                event_producer::run(event_tx, stop_rx.clone());
            })
            .expect("failed to create thread for event producer");

        let server = thread::Builder::new()
            .name("protocol-server".to_owned())
            .spawn_scoped(s, || {
                protocol_server::run(event_rx, stop_rx.clone());
            })
            .expect("failed to create thread for protocol server");

        let workers = [producer, server];

        loop {
            let finished = workers.iter().map(|x| x.is_finished()).any(identity);
            if finished {
                break;
            }
            thread::yield_now();
        }

        debug!("stopping server");
        stop_tx.send(()).expect("failed to send stop signal");

        for w in workers {
            w.join().unwrap();
        }
    });

    debug!("server stopped");
}
