use super::event::LocalInputEvent;
use crate::{
    event_buffer::EventBuffer,
    event_logger::EventLogger,
    transport::protocol::{InputEvent, KeyCode},
};
use anyhow::Error;
use std::time::{Duration, Instant};
use tokio::{fs::File, sync::mpsc};
use tracing::debug;

pub struct InputController {
    /// Buffer for local input events.
    event_buf: EventBuffer<'static, Instant>,
    /// Input event sink.
    event_tx: mpsc::Sender<InputEvent>,
    /// If this is true input source should be consumed from its host and
    /// propagated to the input sink.
    relay: bool,
    /// Last time we detect inputs for toggling the relay flag.
    relay_toggled_at: Option<Instant>,
    event_logger: EventLogger<File, LocalInputEvent>,
}

impl InputController {
    pub async fn new(event_tx: mpsc::Sender<InputEvent>) -> Result<Self, anyhow::Error> {
        let event_buf = EventBuffer::new(|new, old| {
            // Evict events older than 300 milliseconds from the newest event.
            let d = *new - *old;
            d > Duration::from_millis(300)
        });
        let event_logger = {
            let log_file = File::create("./events.log").await?;
            EventLogger::new(log_file)
        };
        let this = Self {
            event_buf,
            event_tx,
            relay: false,
            relay_toggled_at: None,
            event_logger,
        };
        Ok(this)
    }

    /// Returns boolean that denote if the next successive inputs should be
    /// captured or not.
    pub async fn on_input_event(&mut self, event: LocalInputEvent) -> Result<bool, Error> {
        debug!(?event, "received local input event");

        self.event_logger.log(event).await?;

        self.event_buf.push_event(event, Instant::now());

        if self.relay {
            if let Some(event) = event.into_input_event() {
                debug!(?event, "relaying input event");
                self.event_tx.blocking_send(event)?;
            }
        }

        let (most_recent, second_most) = {
            let mut keys = self.event_buf.recent_pressed_keys(self.relay_toggled_at.as_ref());
            let first = keys.next();
            let second = keys.next();
            (first, second)
        };

        // if the right ctrl key are pressed twice consecutively
        if let (Some((KeyCode::RightCtrl, _)), Some((KeyCode::RightCtrl, _))) = (most_recent, second_most) {
            let new_relay = !self.relay;
            debug!(?new_relay, "relay toggled");
            self.event_buf.clear();
            self.relay = new_relay;
            self.relay_toggled_at = Some(Instant::now());
        }

        Ok(self.relay)
    }
}
