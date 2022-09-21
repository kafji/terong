use super::event::{LocalInputEvent, MouseMovement};
use crate::protocol::{self, InputEvent, KeyCode};
use anyhow::Error;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, info};

pub struct InputController {
    /// Buffer of local input events.
    event_buf: EventBuffer,
    /// Input event sink.
    event_tx: mpsc::Sender<InputEvent>,
    /// If this is true, input source should be captured from its host.
    relaying: bool,
    /// Last time we detect inputs for toggling capture input flag.
    relay_toggled_time: Option<Instant>,
}

impl InputController {
    pub fn new(event_tx: mpsc::Sender<InputEvent>) -> Self {
        Self {
            event_buf: Default::default(),
            event_tx,
            relaying: false,
            relay_toggled_time: None,
        }
    }

    /// Returns boolean that denote if the next successive inputs should be
    /// captured or not.
    pub fn on_input_event(&mut self, event: LocalInputEvent) -> Result<bool, Error> {
        debug!(?event, "received local input event");

        self.event_buf.push_input_event(event);

        let (last_key, second_last_key) = {
            let mut keys = self
                .event_buf
                .recent_pressed_keys(self.relay_toggled_time)
                .into_iter();
            let last = keys.next();
            let second_last = keys.next();
            (last, second_last)
        };

        if let (Some((KeyCode::RightCtrl, t)), Some((KeyCode::RightCtrl, _))) =
            (last_key, second_last_key)
        {
            let new_value = !self.relaying;

            info!(?new_value, "relay toggled");

            self.relaying = new_value;
            self.relay_toggled_time = Some(*t);
        } else {
            if self.relaying {
                if let Some(event) = local_event_to_proto_event(event) {
                    self.event_tx.blocking_send(event)?;
                }
            }
        }

        Ok(self.relaying)
    }
}

#[derive(Default, Debug)]
struct EventBuffer {
    buf: Vec<(LocalInputEvent, Instant)>,
}

impl EventBuffer {
    /// Add event to buffer and drop expired events.
    ///
    /// Expired events are events older than 300 milliseconds.
    fn push_input_event(&mut self, event: LocalInputEvent) {
        let now = Instant::now();

        // drop expired events
        let part = self.buf.partition_point(|(_, t)| {
            let d = now - *t;
            d <= Duration::from_millis(300)
        });
        self.buf.truncate(part);

        self.buf.insert(0, (event, now));
    }

    /// Query recent pressed keys.
    ///
    /// Recent pressed keys are keys where its key up and key down events exist in the buffer.
    fn recent_pressed_keys(&self, since: Option<Instant>) -> Vec<(&KeyCode, &Instant)> {
        let buf: Box<dyn Iterator<Item = _>> = match since.as_ref() {
            Some(since) => Box::new(self.buf.iter().filter(|(_, x)| x > since)),
            None => Box::new(self.buf.iter()),
        };

        let buf = buf.collect::<Vec<_>>();

        if buf.len() < 2 {
            return Vec::new();
        }

        let mut pressed = Vec::new();

        for (i, (x, t)) in buf[..=buf.len() - 2].iter().enumerate() {
            if let LocalInputEvent::KeyUp { key: up } = x {
                for (y, _) in &buf[i + 1..] {
                    if let LocalInputEvent::KeyDown { key: down } = y {
                        if up == down {
                            pressed.push((up, t));
                        }
                    }
                }
            }
        }

        pressed.reverse();

        pressed
    }
}

/// Converts local input event into protocol input event.
fn local_event_to_proto_event(local: LocalInputEvent) -> Option<protocol::InputEvent> {
    match local {
        LocalInputEvent::MouseMove(MouseMovement { dx, dy }) => {
            InputEvent::MouseMove { dx, dy }.into()
        }
        LocalInputEvent::MouseButtonDown { button } => {
            InputEvent::MouseButtonDown { button }.into()
        }
        LocalInputEvent::MouseButtonUp { button } => InputEvent::MouseButtonUp { button }.into(),
        LocalInputEvent::MouseScroll { direction } => InputEvent::MouseScroll { direction }.into(),
        LocalInputEvent::KeyDown { key } => InputEvent::KeyDown { key }.into(),
        LocalInputEvent::KeyRepeat { key } => InputEvent::KeyRepeat { key }.into(),
        LocalInputEvent::KeyUp { key } => InputEvent::KeyUp { key }.into(),
        _ => None,
    }
}
