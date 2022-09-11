mod input_event_consumer;
mod protocol_client;

use std::{convert::identity, path::PathBuf, thread};
use tracing::debug;

/// Run the client application.
pub fn run() {
    debug!("starting client");

    // let (stop_tx, stop_rx) = channel::bounded(0);

    // let (input_event_tx, input_event_rx) = channel::unbounded();
    // let (event_tx, event_rx) = channel::unbounded();

    // thread::scope(|s| {
    //     let client = thread::Builder::new()
    //         .name("protocol-client".to_owned())
    //         .spawn_scoped(s, || {
    //             protocol_client::run(event_tx, stop_rx.clone());
    //         })
    //         .expect("failed to create thread for protocol client");

    //     let consumer = thread::Builder::new()
    //         .name("event-consumer".to_owned())
    //         .spawn_scoped(s, || {
    //             input_event_consumer::run(input_event_rx, stop_rx.clone());
    //         })
    //         .expect("failed to create thread for event consumer");

    //     let workers = [client, consumer];

    //     loop {
    //         let finished = workers.iter().map(|x| x.is_finished()).any(identity);
    //         if finished {
    //             break;
    //         }
    //     }

    //     debug!("stopping server");
    //     stop_tx.send(()).expect("failed to send stop signal");

    //     for w in workers {
    //         w.join().unwrap();
    //     }
    // });

    debug!("client stopped");
}
