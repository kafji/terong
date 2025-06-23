use super::event::LocalInputEvent;
use crate::{
    EVENT_LOG_FILE_PATH,
    event_buffer::EventBuffer,
    event_logger::EventLogger,
    transport::protocol::{InputEvent, KeyCode},
};
use anyhow::Error;
use std::{
    fs::File,
    thread,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::{Level, debug, error, span, warn};

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
    logger_tx: Option<std::sync::mpsc::Sender<LocalInputEvent>>,
}

impl InputController {
    pub fn new(
        event_tx: mpsc::Sender<InputEvent>,
        should_log: bool,
    ) -> Result<Self, anyhow::Error> {
        let event_buf = EventBuffer::new(|new, old| {
            // Evict events older than 300 milliseconds from the newest event.
            let d = *new - *old;
            d > Duration::from_millis(300)
        });

        let logger_tx = if should_log {
            let (logger_tx, logger_rx) = std::sync::mpsc::channel();
            thread::spawn(move || {
                let span = span!(Level::INFO, "event logger");
                let _enter = span.enter();
                let file = match File::create(EVENT_LOG_FILE_PATH) {
                    Ok(file) => file,
                    Err(err) => {
                        error!(error = %err, "failed to create log file");
                        return;
                    }
                };
                let mut logger = EventLogger::new(file);
                while let Ok(event) = logger_rx.recv() {
                    if let Err(err) = logger.log(event) {
                        warn!(error = %err, "failed to log event");
                    }
                }
                debug!("channel closed");
            });
            Some(logger_tx)
        } else {
            None
        };

        let this = Self {
            event_buf,
            event_tx,
            relay: false,
            relay_toggled_at: None,
            logger_tx,
        };
        Ok(this)
    }

    /// Returns boolean that denote if the next successive inputs should be
    /// captured or not.
    pub fn on_input_event(&mut self, event: LocalInputEvent) -> Result<bool, Error> {
        debug!(?event, "received local input event");

        if let Some(logger_tx) = &self.logger_tx {
            if let Err(err) = logger_tx.send(event) {
                warn!(error = %err, "failed to log event, channel closed");
            }
        }

        self.event_buf.push_event(event, Instant::now());

        if self.relay {
            if let Some(event) = event.into_input_event() {
                debug!(?event, "relaying input event");
                self.event_tx.blocking_send(event)?;
            }
        }

        let (most_recent, second_most) = {
            let mut keys = self
                .event_buf
                .recent_pressed_keys(self.relay_toggled_at.as_ref());
            let first = keys.next();
            let second = keys.next();
            (first, second)
        };

        // if the right ctrl key are pressed twice consecutively
        if let (Some((KeyCode::RightCtrl, _)), Some((KeyCode::RightCtrl, _))) =
            (most_recent, second_most)
        {
            let new_relay = !self.relay;
            debug!(?new_relay, "relay toggled");
            self.event_buf.clear();
            self.relay = new_relay;
            self.relay_toggled_at = Some(Instant::now());
        }

        Ok(self.relay)
    }
}
