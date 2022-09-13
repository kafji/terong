mod transport_server;

mod app {
    use crate::{
        input_source::event::{LocalInputEvent, MousePosition},
        protocol::{self, InputEvent, KeyCode},
    };
    use anyhow::Error;
    use std::time::{Duration, Instant};
    use tokio::sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        watch,
    };
    use tracing::debug;

    #[derive(Default, Debug)]
    struct EventBuffer {
        buf: Vec<(LocalInputEvent, Instant)>,
    }

    impl EventBuffer {
        fn prev_mouse_pos(&self) -> Option<MousePosition> {
            self.buf.iter().find_map(|(x, _)| {
                if let LocalInputEvent::MousePosition(pos) = x {
                    Some(*pos)
                } else {
                    None
                }
            })
        }

        fn recent_pressed_keys(&self) -> Vec<KeyCode> {
            if self.buf.len() < 2 {
                return Vec::new();
            }
            // pairs of key up & key down
            let mut pressed = Vec::new();
            for (i, (x, _)) in self.buf[..=self.buf.len() - 2].iter().enumerate() {
                if let LocalInputEvent::KeyUp { key: up } = x {
                    for (y, _) in &self.buf[i + 1..] {
                        if let LocalInputEvent::KeyDown { key: down } = y {
                            if up == down {
                                pressed.push(*up);
                            }
                        }
                    }
                }
            }
            pressed
        }

        fn push_input_event(&mut self, event: LocalInputEvent) {
            let now = Instant::now();

            // drop expired events
            let part = self.buf.partition_point(|(_, t)| {
                let d = now - *t;
                d <= Duration::from_millis(500)
            });
            self.buf.truncate(part);

            self.buf.insert(0, (event, now));
        }
    }

    /// Converts mouse absolute position to mouse relative position.
    fn mouse_pos_to_mouse_rel(
        evbuf: &EventBuffer,
        pos: &MousePosition,
    ) -> (i32 /* dx */, i32 /* dy */) {
        match evbuf.prev_mouse_pos() {
            Some(prev) => prev.delta_to(pos),
            None => Default::default(),
        }
    }

    /// Converts local input event into protocol input event.
    fn local_event_to_proto_event(
        evbuf: &EventBuffer,
        local: LocalInputEvent,
    ) -> protocol::InputEvent {
        match local {
            LocalInputEvent::MousePosition(pos) => {
                let (dx, dy) = mouse_pos_to_mouse_rel(evbuf, &pos);
                InputEvent::MouseMove { dx, dy }
            }
            LocalInputEvent::MouseButtonDown { button } => InputEvent::MouseButtonDown { button },
            LocalInputEvent::MouseButtonUp { button } => InputEvent::MouseButtonUp { button },
            LocalInputEvent::MouseScroll {} => InputEvent::MouseScroll {},
            LocalInputEvent::KeyDown { key } => InputEvent::KeyDown { key },
            LocalInputEvent::KeyRepeat { key } => InputEvent::KeyRepeat { key },
            LocalInputEvent::KeyUp { key } => InputEvent::KeyUp { key },
        }
    }

    /// Application environment.
    #[derive(Debug)]
    struct Inner {
        /// Denotes if the input event listener should capture user inputs.
        ///
        /// The input event listener should still listen and propagate user
        /// inputs regardless of this value.
        capture_input_tx: watch::Sender<bool>,
        /// Event buffer.
        evbuf: EventBuffer,
        /// Protocol input event sink.
        proto_event_tx: UnboundedSender<protocol::InputEvent>,
    }

    impl Inner {
        /// Updates capture input flag.
        fn toggle_capture_input(&mut self) -> bool {
            let old = *self.capture_input_tx.borrow();
            let new = !old;
            debug!("setting capture input to {}", new);
            self.capture_input_tx.send_replace(new);
            new
        }
    }

    #[derive(Debug)]
    pub struct App {
        inner: Inner,
    }

    impl App {
        pub fn new() -> (Self, UnboundedReceiver<protocol::InputEvent>) {
            let (capture_input_tx, _) = watch::channel(false);

            let (proto_event_tx, proto_event_rx) = mpsc::unbounded_channel();
            let inner = Inner {
                capture_input_tx,
                evbuf: Default::default(),
                proto_event_tx,
            };
            let s = Self { inner };
            (s, proto_event_rx)
        }

        pub async fn handle_input_event(&mut self, event: LocalInputEvent) -> Result<(), Error> {
            let evbuf = &mut self.inner.evbuf;

            evbuf.push_input_event(event);

            let capture = {
                let keys = evbuf.recent_pressed_keys();
                debug!("recent pressed keys {:?}", keys);
                let mut keys = keys.iter();
                let first = keys.next();
                let second = keys.next();
                if let (Some(KeyCode::RightCtrl), Some(KeyCode::RightCtrl)) = (first, second) {
                    self.inner.toggle_capture_input()
                } else {
                    *self.inner.capture_input_tx.borrow()
                }
            };

            if capture {
                let proto_event = local_event_to_proto_event(&mut self.inner.evbuf, event);
                self.inner.proto_event_tx.send(proto_event)?;
            }

            Ok(())
        }

        pub fn get_capture_input_rx(&self) -> watch::Receiver<bool> {
            let app = &self.inner;
            app.capture_input_tx.subscribe()
        }
    }
}

use self::app::App;
use tokio::{sync::mpsc, try_join};
use tracing::{debug, info};

/// Run the server application.
pub async fn run() {
    info!("starting server");

    let (input_event_tx, mut input_event_rx) = mpsc::unbounded_channel();
    let (mut app, proto_event_rx) = App::new();

    let listener = crate::input_source::start(input_event_tx, app.get_capture_input_rx());

    let server = transport_server::start(proto_event_rx);

    while let Some(event) = input_event_rx.recv().await {
        debug!("received local event {:?}", event);
        app.handle_input_event(event).await.unwrap();
    }

    drop(app);
    try_join!(listener, server).unwrap();

    info!("server stopped");
}
