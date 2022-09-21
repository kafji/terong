use super::event::{LocalInputEvent, MouseMovement};
use crate::protocol::{self, InputEvent, KeyCode};
use anyhow::Error;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::debug;

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
    pub fn on_input_event(
        &mut self,
        (event, time): (LocalInputEvent, Instant),
    ) -> Result<bool, Error> {
        debug!(?event, "received local input event");

        self.event_buf.push_input_event(event);

        let (last_key, second_last_key) = {
            let mut keys = self
                .event_buf
                .recent_pressed_keys(self.relay_toggled_time.as_ref());
            let last = keys.next();
            let second_last = keys.next();
            (last, second_last)
        };

        if let (Some((KeyCode::RightCtrl, t)), Some((KeyCode::RightCtrl, _))) =
            (last_key, second_last_key)
        {
            let new_value = !self.relaying;

            debug!(?new_value, "relay toggled");

            self.relaying = new_value;
            self.relay_toggled_time = Some(*t);
            self.event_buf.clear();
        } else {
            if self.relaying {
                if let Some(event) = event.into_input_event() {
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
    fn recent_pressed_keys<'a, 'b>(
        &'a self,
        since: Option<&'b Instant>,
    ) -> impl Iterator<Item = (&KeyCode, &Instant)>
    where
        'b: 'a,
    {
        RecentKeyPresses::new(
            self.buf
                .iter()
                .filter_map(|(e, t)| KeyPressEvent::from_local_input_event(e, t))
                .rev(),
            since,
        )
    }

    fn clear(&mut self) {
        self.buf.clear()
    }
}

#[derive(Debug)]
enum KeyPressEvent<'a, Order> {
    Down(KeyPress<'a, Order>),
    Up(KeyPress<'a, Order>),
}

#[derive(Debug)]
struct KeyPress<'a, Order>(&'a KeyCode, &'a Order);

impl<'a, Order> KeyPressEvent<'a, Order> {
    fn from_local_input_event(event: &'a LocalInputEvent, time: &'a Order) -> Option<Self> {
        match event {
            LocalInputEvent::KeyDown { key } => KeyPressEvent::Down(KeyPress(key, time)).into(),
            LocalInputEvent::KeyUp { key } => KeyPressEvent::Up(KeyPress(key, time)).into(),
            _ => None,
        }
    }
}

struct RecentKeyPresses<'a, Order> {
    events: Box<dyn Iterator<Item = KeyPressEvent<'a, Order>> + 'a>,
    queue: VecDeque<KeyPressEvent<'a, Order>>,
}

impl<'a, Order> RecentKeyPresses<'a, Order>
where
    Order: Ord,
{
    fn new<'b>(
        events: impl Iterator<Item = KeyPressEvent<'a, Order>>,
        since: Option<&'b Order>,
    ) -> Self
    where
        'b: 'a,
    {
        let events: Box<dyn Iterator<Item = _>> = match since {
            Some(since) => {
                let xs = events.filter(|x| match x {
                    KeyPressEvent::Down(x) => x.1 > since,
                    KeyPressEvent::Up(x) => x.1 > since,
                });
                Box::new(xs)
            }
            _ => Box::new(events),
        };

        Self {
            events,
            queue: Default::default(),
        }
    }
}

impl<'a, Order> Iterator for RecentKeyPresses<'a, Order> {
    type Item = (&'a KeyCode, &'a Order);

    fn next(&mut self) -> Option<Self::Item> {
        // find key down from queue
        let key_down = loop {
            match self.queue.pop_front() {
                Some(KeyPressEvent::Down(x)) => break x.into(),
                // queue exhausted, key down not in queue
                None => break None,
                // found starting event other than key down
                _ => continue,
            }
        };

        // if key down not in the queue, find it in events
        let key_down = key_down.or_else(|| loop {
            match self.events.next() {
                Some(event) => match event {
                    KeyPressEvent::Down(x) => break Some(x),
                    // found starting event other than key down
                    _ => continue,
                },
                // iterator is exhausted
                None => break None,
            }
        });

        // if key down is not found then this iterator is exhausted
        let key_down = match key_down {
            Some(x) => x,
            None => return None,
        };

        // find key up from queue
        let key_up = {
            let mut q = Vec::new();
            let key = loop {
                match self.queue.pop_front() {
                    Some(KeyPressEvent::Up(x)) if x.0 == key_down.0 => break x.into(),
                    // queue exhausted, key down not in queue
                    None => break None,
                    // found other than key up with same key, collect it, and return it back in the same order to the queue
                    Some(x) => q.push(x),
                }
            };
            // return other key event to the queue
            for x in q.into_iter().rev() {
                self.queue.push_front(x);
            }
            key
        };

        let key_up = key_up.or_else(|| loop {
            match self.events.next() {
                Some(event) => match event {
                    KeyPressEvent::Up(x) if x.0 == key_down.0 => break Some(x),
                    other_key_down => {
                        self.queue.push_back(other_key_down);
                        continue;
                    }
                },
                None => break None,
            }
        });

        // if key down is not found then this iterator is exhausted
        let key_up = match key_up {
            Some(x) => x,
            None => return None,
        };

        Some((key_up.0, key_down.1))
    }
}
