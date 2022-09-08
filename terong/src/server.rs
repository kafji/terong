mod input_listener;
mod protocol_server;

use crate::{input_event::InputEvent, protocol};
use std::{
    collections::VecDeque,
    convert::identity,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    select,
    sync::{mpsc, oneshot, watch},
    try_join,
};
use tracing::{debug, info};

use self::input_listener::event::{LocalInputEvent, MousePosition};

/// Run the server application.
pub async fn run(config_file: Option<PathBuf>) {
    info!("starting server");

    let (capture_input_flag_tx, capture_input_flag_rx) = watch::channel(false);
    let mut app = App::new(capture_input_flag_tx);

    // start input listener
    let (listener_event_sink, mut listener_event_source) = mpsc::unbounded_channel();
    let mut listener = tokio::spawn(input_listener::run(
        listener_event_sink,
        capture_input_flag_rx,
    ));

    // start protocol server
    let (server_event_sink, server_event_source) = tokio::sync::mpsc::unbounded_channel();
    let mut server = tokio::spawn(protocol_server::run(server_event_source));

    loop {
        select! {
            x = listener_event_source.recv() => {
                match x {
                    Some(event) => {
                        app.handle_event(event).await;
                        let pe = app.local_event_to_protocol_event(event);
                        server_event_sink.send(pe).unwrap();
                    }
                    None => {
                        break;
                    }
                }
            }
            _ = &mut listener => {
                break;
            }
            _ = &mut server => {
                break;
            }
        }
    }

    // stop workers
    drop(listener_event_source);
    drop(server_event_sink);
    drop(app);

    try_join!(listener, server).unwrap();

    info!("server stopped");
}

/// Application environment.
#[derive(Debug)]
struct Inner {
    /// Denotes if the input event listener should capture user inputs.
    ///
    /// The input event listener should still listen and propagate user inputs regardless of this value.
    capture_input_tx: watch::Sender<bool>,
    /// Buffer of mouse positions.
    ///
    /// Must be guaranteed to be sorted ascendingly by time.
    mouse_pos_buf: VecDeque<(MousePosition, Instant)>,
}

#[derive(Clone, Debug)]
pub struct App {
    inner: Arc<Mutex<Inner>>,
}

impl App {
    pub fn new(capture_input_tx: watch::Sender<bool>) -> Self {
        let inner = Inner {
            capture_input_tx,
            mouse_pos_buf: VecDeque::new(),
        };
        let inner = Arc::new(Mutex::new(inner));
        Self { inner }
    }

    /// Drop expired events from event buffer.
    pub fn drop_expired_events(&mut self) {
        let mut app = self.inner.lock().unwrap();
        let now = Instant::now();
        while let Some((_, t)) = app.mouse_pos_buf.front() {
            let delta = now - *t;
            if delta > Duration::from_millis(200) {
                app.mouse_pos_buf.pop_front();
            } else {
                break;
            }
        }
    }

    pub async fn handle_event(&mut self, event: LocalInputEvent) {
        self.drop_expired_events();

        let mut app = self.inner.lock().unwrap();

        match event {
            LocalInputEvent::MousePosition(pos) => {
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

                if found_first_bump && pos.x < 1 {
                    app.capture_input_tx.send_if_modified(|x| {
                        if *x == true {
                            return false;
                        }
                        *x = true;
                        true
                    });
                }

                app.mouse_pos_buf.push_back((pos, Instant::now()));
            }
            _ => (),
        }
    }

    fn local_event_to_protocol_event(&self, le: LocalInputEvent) -> protocol::InputEvent {
        match le {
            LocalInputEvent::MousePosition(pos) => {
                let app = self.inner.lock().unwrap();
                let (prev, _) = app.mouse_pos_buf.back().unwrap();
                let (dx, dy) = prev.delta_to(pos);
                InputEvent::MouseMove { dx, dy }
            }
            LocalInputEvent::MouseButtonDown { button } => InputEvent::MouseButtonDown { button },
            LocalInputEvent::MouseButtonUp { button } => InputEvent::MouseButtonUp { button },
            LocalInputEvent::MouseScroll {} => InputEvent::MouseScroll {},
            LocalInputEvent::KeyDown { key } => InputEvent::KeyDown { key },
            LocalInputEvent::KeyUp { key } => InputEvent::KeyUp { key },
        }
    }
}
