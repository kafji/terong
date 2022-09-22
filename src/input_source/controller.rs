use super::event::LocalInputEvent;
use crate::transport::protocol::{InputEvent, KeyCode};
use anyhow::Error;
use std::{
    collections::VecDeque,
    ops::Sub,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::debug;

pub struct InputController {
    /// Buffer of local input events.
    event_buf: EventBuffer<Instant>,
    /// Input event sink.
    event_tx: mpsc::Sender<InputEvent>,
    /// If this is true input source should be consumed from its host and
    /// propagated to the input sink.
    relay: bool,
    /// Last time we detect inputs for toggling the relay flag.
    relay_toggled_at: Option<Instant>,
}

impl InputController {
    pub fn new(event_tx: mpsc::Sender<InputEvent>) -> Self {
        Self {
            event_buf: Default::default(),
            event_tx,
            relay: false,
            relay_toggled_at: None,
        }
    }

    /// Returns boolean that denote if the next successive inputs should be
    /// captured or not.
    pub fn on_input_event(&mut self, event: LocalInputEvent) -> Result<bool, Error> {
        debug!(?event, "received local input event");

        self.event_buf.push_input_event(event, Instant::now());

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

        if let (Some((KeyCode::RightCtrl, _)), Some((KeyCode::RightCtrl, _))) =
            (most_recent, second_most)
        {
            let new_value = !self.relay;

            debug!(?new_value, "relay toggled");

            self.event_buf.clear();
            self.relay = new_value;
            self.relay_toggled_at = Some(Instant::now());
        }

        Ok(self.relay)
    }
}

#[derive(Debug)]
struct EventBuffer<T> {
    buf: Vec<(LocalInputEvent, T)>,
}

impl<T> Default for EventBuffer<T> {
    fn default() -> Self {
        Self {
            buf: Default::default(),
        }
    }
}

impl<OrderKey> EventBuffer<OrderKey>
where
    OrderKey: Sub<Output = Duration> + Copy,
{
    /// Add event to buffer and drop outdated events.
    ///
    /// Outdated events are events older than 300 milliseconds from the newest
    /// event.
    fn push_input_event(&mut self, event: LocalInputEvent, time: OrderKey) {
        // drop outdated events
        let part = self.buf.partition_point(|(_, t)| {
            let d = time - *t;
            d <= Duration::from_millis(300)
        });
        self.buf.truncate(part);

        self.buf.insert(0, (event, time));
    }
}

impl<T> EventBuffer<T> {
    fn clear(&mut self) {
        self.buf.clear()
    }
}

impl<OrderKey> EventBuffer<OrderKey>
where
    OrderKey: Ord,
{
    /// Query recent pressed keys.
    ///
    /// Recent pressed keys are keys where its key up and key down events exist
    /// in the buffer.
    fn recent_pressed_keys<'a, 'b>(
        &'a self,
        since: Option<&'b OrderKey>,
    ) -> impl Iterator<Item = (&KeyCode, &OrderKey)>
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
}

#[derive(Debug)]
enum KeyPressEvent<'a, T> {
    Down(KeyPress<'a, T>),
    Up(KeyPress<'a, T>),
}

#[derive(Debug)]
struct KeyPress<'a, OrderKey> {
    key: &'a KeyCode,
    order_key: &'a OrderKey,
}

impl<'a, OrderKey> KeyPressEvent<'a, OrderKey> {
    fn from_local_input_event(event: &'a LocalInputEvent, order_key: &'a OrderKey) -> Option<Self> {
        match event {
            LocalInputEvent::KeyDown { key } => {
                KeyPressEvent::Down(KeyPress { key, order_key }).into()
            }
            LocalInputEvent::KeyUp { key } => KeyPressEvent::Up(KeyPress { key, order_key }).into(),
            _ => None,
        }
    }
}

struct RecentKeyPresses<'a, T> {
    events: Box<dyn Iterator<Item = KeyPressEvent<'a, T>> + 'a>,
    queue: VecDeque<KeyPressEvent<'a, T>>,
}

impl<'a, OrderKey> RecentKeyPresses<'a, OrderKey>
where
    OrderKey: Ord,
{
    fn new<'b>(
        events: impl Iterator<Item = KeyPressEvent<'a, OrderKey>> + 'a,
        since: Option<&'b OrderKey>,
    ) -> Self
    where
        'b: 'a,
    {
        let events: Box<dyn Iterator<Item = _>> = match since {
            Some(since) => {
                let xs = events.filter(|x| match x {
                    KeyPressEvent::Down(x) => x.order_key > since,
                    KeyPressEvent::Up(x) => x.order_key > since,
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

impl<'a, T> RecentKeyPresses<'a, T> {
    fn find_key_down(&mut self) -> Option<KeyPress<'a, T>> {
        // find key down from queue
        let key_down = loop {
            match self.queue.pop_back() {
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

        key_down
    }

    fn find_key_up(&mut self, key: &KeyCode) -> Option<KeyPress<'a, T>> {
        // find key up from queue
        let key_up = {
            let mut q = Vec::new();
            let key = loop {
                match self.queue.pop_back() {
                    Some(KeyPressEvent::Up(x)) if x.key == key => break x.into(),
                    // queue exhausted, key down not in queue
                    None => break None,
                    // found other than key up with same key, collect it, and return it back in the
                    // same order to the queue
                    Some(x) => q.push(x),
                }
            };
            // return other key event to the queue
            for x in q.into_iter().rev() {
                self.queue.push_back(x);
            }
            key
        };

        // if key up not in the queue, find it in events
        let key_up = key_up.or_else(|| loop {
            match self.events.next() {
                Some(event) => match event {
                    KeyPressEvent::Up(x) if x.key == key => break Some(x),
                    other_key_down => {
                        self.queue.push_front(other_key_down);
                        continue;
                    }
                },
                None => break None,
            }
        });

        key_up
    }
}

impl<'a, T> Iterator for RecentKeyPresses<'a, T> {
    type Item = (&'a KeyCode, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        // if key down is not found then this iterator is exhausted
        let key_down = match self.find_key_down() {
            Some(x) => x,
            None => return None,
        };

        // if key down is not found then this iterator is exhausted
        let key_up = match self.find_key_up(key_down.key) {
            Some(x) => x,
            None => return None,
        };

        Some((key_up.key, key_up.order_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
