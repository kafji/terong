mod input_listener;
mod protocol_server;

use crate::event::InputEvent;
use crate::event::MousePosition;
use crate::server::input_listener::Signal;
use crossbeam::channel::{self, TryRecvError};
use log::debug;
use std::{
    collections::VecDeque,
    convert::identity,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

/// Run the server application.
pub fn run(config_file: Option<PathBuf>) {
    debug!("starting server");

    let mut app = App::new();

    let (stop_signal_tx, stop_signal_rx) = channel::bounded(0);

    let (producer_signal_tx, producer_signal_rx) = channel::unbounded();

    let (input_event_tx, input_event_rx) = channel::unbounded();
    let (event_tx, event_rx) = channel::unbounded();

    thread::scope(|s| {
        // start input listener
        let listener = thread::Builder::new()
            .name("input-listener".to_owned())
            .spawn_scoped(s, || {
                input_listener::run(input_event_tx, producer_signal_rx, stop_signal_rx.clone());
            })
            .expect("failed to create thread for event producer");

        // start protocol server
        let server = thread::Builder::new()
            .name("protocol-server".to_owned())
            .spawn_scoped(s, || {
                protocol_server::run(event_rx.clone(), stop_signal_rx.clone());
            })
            .expect("failed to create thread for protocol server");

        let workers = [listener, server];

        loop {
            let finished = workers.iter().map(|x| x.is_finished()).any(identity);
            if finished {
                break;
            }

            match input_event_rx.try_recv() {
                Ok(InputEvent::MousePosition(pos)) => {
                    app.drop_expired_events();

                    // find 'bump'
                    let found_first_bump = {
                        let i = app
                            .mouse_pos_buf
                            .iter()
                            .enumerate()
                            .find(|(_, (pos, _))| if pos.x < 1 { true } else { false })
                            .map(|(i, _)| i);

                        if let Some(i) = i {
                            let mut found = false;
                            for j in i + 1..app.mouse_pos_buf.len() {
                                let (pos, _) = app.mouse_pos_buf[j];
                                if pos.x > 1 {
                                    found = true;
                                    break;
                                }
                            }
                            found
                        } else {
                            false
                        }
                    };

                    if dbg!(found_first_bump && pos.x < 1) {
                        if !app.capture_input {
                            app.capture_input = true;
                            producer_signal_tx
                                .send(Signal::SetShouldCapture(true))
                                .unwrap();

                            stop_signal_tx.send(()).unwrap();
                        }
                    }

                    app.mouse_pos_buf.push_back((pos, Instant::now()));
                }
                Err(TryRecvError::Empty) => (),
                _ => todo!(),
            }
        }

        debug!("stopping server");
        stop_signal_tx.send(()).ok();

        for w in workers {
            w.join().unwrap();
        }
    });

    debug!("server stopped");
}

/// Application environment.
#[derive(Debug)]
struct App {
    /// Denotes if the input event listener should capture user inputs.
    ///
    /// The input event listener should still listen and propagate user inputs regardless of this value.
    capture_input: bool,
    /// Buffer of mouse positions.
    ///
    /// Must be guaranteed to be sorted ascendingly by time.
    mouse_pos_buf: VecDeque<(MousePosition, Instant)>,
}

impl App {
    pub fn new() -> Self {
        Self {
            capture_input: false,
            mouse_pos_buf: VecDeque::new(),
        }
    }

    /// Drop expired events from event buffer.
    pub fn drop_expired_events(&mut self) {
        let now = Instant::now();
        while let Some((_, t)) = self.mouse_pos_buf.front() {
            let delta = now - *t;
            if delta > Duration::from_millis(200) {
                self.mouse_pos_buf.pop_front();
            } else {
                break;
            }
        }
    }
}
