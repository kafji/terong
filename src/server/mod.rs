mod protocol_server;

mod app {
    use crate::{
        input_listener::event::{LocalInputEvent, MousePosition},
        protocol::{self, InputEvent, KeyCode},
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

    #[derive(Default, Debug)]
    struct EventBuffer {
        buf: VecDeque<(LocalInputEvent, Instant)>,
    }

    impl EventBuffer {
        fn prev_mouse_pos(&self) -> Option<MousePosition> {
            self.buf
                .iter()
                .filter_map(|(x, _)| {
                    if let LocalInputEvent::MousePosition(pos) = x {
                        Some(*pos)
                    } else {
                        None
                    }
                })
                .next()
        }

        fn prev_pressed_key(&self) -> Option<KeyCode> {
            todo!()
        }

        fn push_input_event(&mut self, event: LocalInputEvent) {
            let now = Instant::now();

            // drop expired events
            let part = self.buf.partition_point(|(_, t)| {
                let d = now - *t;
                d <= Duration::from_millis(200)
            });
            self.buf.resize_with(part, || unreachable!());

            self.buf.push_front((event, now));

            self.buf.make_contiguous();
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

    /// Application environment.
    #[derive(Debug)]
    struct Inner {
        /// Denotes if the input event listener should capture user inputs.
        ///
        /// The input event listener should still listen and propagate user
        /// inputs regardless of this value.
        capture_input_tx: watch::Sender<bool>,

        evbuf: EventBuffer,

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
            let app = &mut self.inner;
            let evbuf = &mut app.evbuf;
            evbuf.push_input_event(event);

            let proto_event = match event {
                LocalInputEvent::MousePosition(pos) => {
                    let (dx, dy) = mouse_pos_to_mouse_rel(evbuf, &pos);
                    InputEvent::MouseMove { dx, dy }
                }
                LocalInputEvent::MouseButtonDown { button } => {
                    InputEvent::MouseButtonDown { button }
                }
                LocalInputEvent::MouseButtonUp { button } => InputEvent::MouseButtonUp { button },
                LocalInputEvent::MouseScroll {} => InputEvent::MouseScroll {},
                LocalInputEvent::KeyDown { key } => InputEvent::KeyDown { key },
                LocalInputEvent::KeyUp { key } => InputEvent::KeyUp { key },
            };

            app.proto_event_tx.send(proto_event)?;

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
use tracing::info;

/// Run the server application.
pub async fn run() {
    info!("starting server");

    let (input_event_tx, mut input_event_rx) = mpsc::unbounded_channel();
    let (mut app, proto_event_rx) = App::new();

    let listener = crate::input_listener::start(input_event_tx, app.get_capture_input_rx());

    let server = protocol_server::start(proto_event_rx);

    while let Some(event) = input_event_rx.recv().await {
        app.handle_input_event(event).await.unwrap();
    }

    drop(app);
    try_join!(listener, server).unwrap();

    info!("server stopped");
}
