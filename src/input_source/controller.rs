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

pub struct InputController<Order> {
    /// Buffer of local input events.
    event_buf: EventBuffer<Order>,
    /// Input event sink.
    event_tx: mpsc::Sender<InputEvent>,
    /// If this is true input source should be consumed from its host and propagated to the input sink.
    relay: bool,
    /// Last time we detect inputs for toggling the relay flag.
    relay_toggled_at: Option<Order>,
}

impl<T> InputController<T> {
    pub fn new(event_tx: mpsc::Sender<InputEvent>) -> Self {
        Self {
            event_buf: Default::default(),
            event_tx,
            relay: false,
            relay_toggled_at: None,
        }
    }
}

impl InputController<Instant> {
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
struct EventBuffer<Order> {
    buf: Vec<(LocalInputEvent, Order)>,
}

impl<Order> Default for EventBuffer<Order> {
    fn default() -> Self {
        Self {
            buf: Default::default(),
        }
    }
}

impl<Order> EventBuffer<Order>
where
    Order: Sub<Output = Duration> + Copy,
{
    /// Add event to buffer and drop outdated events.
    ///
    /// Outdated events are events older than 300 milliseconds from the newest event.
    fn push_input_event(&mut self, event: LocalInputEvent, time: Order) {
        // drop outdated events
        let part = self.buf.partition_point(|(_, t)| {
            let d = time - *t;
            d <= Duration::from_millis(300)
        });
        self.buf.truncate(part);

        self.buf.insert(0, (event, time));
    }
}

impl<Order> EventBuffer<Order> {
    fn clear(&mut self) {
        self.buf.clear()
    }
}

impl<Order> EventBuffer<Order>
where
    Order: Ord,
{
    /// Query recent pressed keys.
    ///
    /// Recent pressed keys are keys where its key up and key down events exist in the buffer.
    fn recent_pressed_keys<'a, 'b>(
        &'a self,
        since: Option<&'b Order>,
    ) -> impl Iterator<Item = (&KeyCode, &Order)>
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
enum KeyPressEvent<'a, Order> {
    Down(KeyPress<'a, Order>),
    Up(KeyPress<'a, Order>),
}

#[derive(Debug)]
struct KeyPress<'a, Order> {
    key: &'a KeyCode,
    order: &'a Order,
}

impl<'a, Order> KeyPressEvent<'a, Order> {
    fn from_local_input_event(event: &'a LocalInputEvent, order: &'a Order) -> Option<Self> {
        match event {
            LocalInputEvent::KeyDown { key } => KeyPressEvent::Down(KeyPress { key, order }).into(),
            LocalInputEvent::KeyUp { key } => KeyPressEvent::Up(KeyPress { key, order }).into(),
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
        events: impl Iterator<Item = KeyPressEvent<'a, Order>> + 'a,
        since: Option<&'b Order>,
    ) -> Self
    where
        'b: 'a,
    {
        let events: Box<dyn Iterator<Item = _>> = match since {
            Some(since) => {
                let xs = events.filter(|x| match x {
                    KeyPressEvent::Down(x) => x.order > since,
                    KeyPressEvent::Up(x) => x.order > since,
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
                    // found other than key up with same key, collect it, and return it back in the same order to the queue
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

        Some((key_up.key, key_up.order))
    }
}
