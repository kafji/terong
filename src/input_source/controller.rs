use super::event::{LocalInputEvent, MousePosition};
use crate::protocol::{self, InputEvent, KeyCode};
use anyhow::Error;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::debug;

pub struct InputController {
    event_buf: EventBuffer,
    event_tx: mpsc::Sender<InputEvent>,
    capturing: bool,
}

impl InputController {
    pub fn new(event_tx: mpsc::Sender<InputEvent>) -> Self {
        Self {
            event_buf: Default::default(),
            event_tx,
            capturing: false,
        }
    }

    /// Returns boolean that denote if the next successive inputs should be
    /// captured or not.
    pub fn on_input_event(&mut self, event: LocalInputEvent) -> Result<bool, Error> {
        debug!(?event, "received local input event");

        let recent_keys = self.event_buf.recent_pressed_keys();
        let mut keys = recent_keys.iter();
        let first_key = keys.next();
        let second_key = keys.next();

        if let (Some(KeyCode::RightCtrl), Some(KeyCode::RightCtrl)) = (first_key, second_key) {
            self.capturing = !self.capturing;
        } else {
            if self.capturing {
                let event = local_event_to_proto_event(&self.event_buf, event);
                self.event_tx.blocking_send(event)?;
            }
            self.event_buf.push_input_event(event);
        }

        Ok(self.capturing)
    }
}

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
