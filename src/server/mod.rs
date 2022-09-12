mod protocol_server;

mod app {
    use crate::{
        input_listener::event::{LocalInputEvent, MousePosition},
        protocol::{self, InputEvent},
    };
    use anyhow::Error;
    use std::{
        collections::VecDeque,
        time::{Duration, Instant},
    };
    use tokio::sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        watch,
    };

    /// Application environment.
    #[derive(Debug)]
    struct Inner {
        /// 0 for local (server)
        active: u8,
        /// Denotes if the input event listener should capture user inputs.
        ///
        /// The input event listener should still listen and propagate user
        /// inputs regardless of this value.
        capture_input_tx: watch::Sender<bool>,
        /// Buffer of mouse positions.
        ///
        /// Must be guaranteed to be sorted ascendingly by time.
        mouse_pos_buf: VecDeque<(MousePosition, Instant)>,

        input_event_rx: UnboundedReceiver<LocalInputEvent>,

        proto_event_tx: UnboundedSender<protocol::InputEvent>,
    }

    impl Inner {
        /// Updates capture input flag.
        fn set_capture_input(&self, new: bool) {
            self.capture_input_tx.send_if_modified(|old| {
                if *old != new {
                    *old = new;
                    return true;
                }
                false
            });
        }

        fn drop_expired_mouse_position(&mut self) {
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

    #[derive(Debug)]
    pub struct App {
        inner: Inner,
    }

    impl App {
        pub fn new() -> (
            Self,
            UnboundedSender<LocalInputEvent>,
            UnboundedReceiver<protocol::InputEvent>,
        ) {
            let (capture_input_tx, _) = watch::channel(false);
            let (input_event_tx, input_event_rx) = mpsc::unbounded_channel();
            let (proto_event_tx, proto_event_rx) = mpsc::unbounded_channel();
            let inner = Inner {
                active: 0,
                capture_input_tx,
                mouse_pos_buf: VecDeque::new(),
                input_event_rx,
                proto_event_tx,
            };
            let s = Self { inner };
            (s, input_event_tx, proto_event_rx)
        }

        pub async fn handle_input_event(&mut self) -> Result<(), Error> {
            let mut app = &mut self.inner;
            let event = app.input_event_rx.recv().await.unwrap();

            app.drop_expired_mouse_position();

            let proto_event = match event {
                LocalInputEvent::MousePosition(pos) => {
                    let found_first_bump = {
                        // bump-in
                        let i = app
                            .mouse_pos_buf
                            .iter()
                            .enumerate()
                            .find(|(_, (pos, _))| if pos.x < 1 { true } else { false })
                            .map(|(i, _)| i);

                        // bump-out
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
                        app.set_capture_input(true);
                        app.active = 1;
                    }

                    let pe = {
                        if let Some((prev, _)) = app.mouse_pos_buf.back() {
                            let (dx, dy) = prev.delta_to(&pos);
                            InputEvent::MouseMove { dx, dy }
                        } else {
                            InputEvent::MouseMove { dx: 0, dy: 0 }
                        }
                    };

                    app.mouse_pos_buf.push_back((pos, Instant::now()));

                    pe
                }
                LocalInputEvent::MouseButtonDown { button } => {
                    InputEvent::MouseButtonDown { button }
                }
                LocalInputEvent::MouseButtonUp { button } => InputEvent::MouseButtonUp { button },
                LocalInputEvent::MouseScroll {} => InputEvent::MouseScroll {},
                LocalInputEvent::KeyDown { key } => InputEvent::KeyDown { key },
                LocalInputEvent::KeyUp { key } => InputEvent::KeyUp { key },
            };

            app.proto_event_tx.send(proto_event).unwrap();

            Ok(())
        }

        pub fn get_capture_input_rx(&self) -> watch::Receiver<bool> {
            let app = &self.inner;
            app.capture_input_tx.subscribe()
        }
    }
}

use self::app::App;
use tracing::info;

/// Run the server application.
pub async fn run() {
    info!("starting server");
    let (mut app, input_event_tx, proto_event_rx) = App::new();
    let _listener = crate::input_listener::start(input_event_tx, app.get_capture_input_rx());
    let _server = protocol_server::start(proto_event_rx);
    loop {
        app.handle_input_event().await.unwrap();
    }
}
