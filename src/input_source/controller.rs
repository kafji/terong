use super::event::{LocalInputEvent, MousePosition};
use crate::protocol::{self, InputEvent, KeyCode};
use anyhow::Error;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::debug;

pub struct InputController {
    /// Buffer of local input events.
    event_buf: EventBuffer,
    /// Input event sink.
    event_tx: mpsc::Sender<InputEvent>,
    /// Capture input flag.
    ///
    /// If this is true, input source should be captured from its host.
    capturing: bool,
    /// Last time we detect inputs for toggling capture input flag.
    capturing_toggled_time: Option<Instant>,
}

impl InputController {
    pub fn new(event_tx: mpsc::Sender<InputEvent>) -> Self {
        Self {
            event_buf: Default::default(),
            event_tx,
            capturing: false,
            capturing_toggled_time: None,
        }
    }

    /// Returns boolean that denote if the next successive inputs should be
    /// captured or not.
    pub fn on_input_event(&mut self, event: LocalInputEvent) -> Result<bool, Error> {
        debug!(?event, "received local input event");

        let proto_event = local_event_to_proto_event(&self.event_buf, event);

        self.event_buf.push_input_event(event);

        let recent_keys = self
            .event_buf
            .recent_pressed_keys(self.capturing_toggled_time);
        let mut keys = recent_keys.into_iter();
        let first_key = keys.next();
        let second_key = keys.next();

        if let (Some((KeyCode::RightCtrl, t)), Some((KeyCode::RightCtrl, _))) =
            (first_key, second_key)
        {
            let new_value = !self.capturing;
            debug!(?new_value, "toggle should capture input");
            self.capturing = new_value;
            self.capturing_toggled_time = Some(*t);
        } else {
            if self.capturing {
                self.event_tx.blocking_send(proto_event)?;
            }
        }

        Ok(self.capturing)
    }
}

#[derive(Default, Debug)]
struct EventBuffer {
    buf: Vec<(LocalInputEvent, Instant)>,
}

impl EventBuffer {
    /// Query mouse last absolute position.
    fn prev_mouse_pos(&self) -> Option<MousePosition> {
        self.buf.iter().find_map(|(x, _)| {
            if let LocalInputEvent::MousePosition(pos) = x {
                Some(*pos)
            } else {
                None
            }
        })
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

        // pairs of key up & key down
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

        pressed
    }

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
}

/// Converts mouse absolute position to mouse relative position.
fn mouse_pos_to_mouse_rel(
    event_buf: &EventBuffer,
    pos: &MousePosition,
) -> (i32 /* dx */, i32 /* dy */) {
    match event_buf.prev_mouse_pos() {
        Some(prev) => prev.delta_to(pos),
        None => Default::default(),
    }
}

/// Converts local input event into protocol input event.
fn local_event_to_proto_event(
    event_buf: &EventBuffer,
    local: LocalInputEvent,
) -> protocol::InputEvent {
    match local {
        LocalInputEvent::MousePosition(pos) => {
            let (dx, dy) = mouse_pos_to_mouse_rel(event_buf, &pos);
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
