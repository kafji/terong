use crate::{input_event::KeyCode, server::input_source::event::LocalInputEvent};
use std::collections::VecDeque;

pub struct EventBuffer<'evictor, T> {
    /// Ordered from oldest to newest.
    buf: Vec<(LocalInputEvent, T)>,
    evictor: Box<dyn Fn(&T, &T) -> bool + Send + Sync + 'evictor>,
}

impl<'evictor, OrderKey> EventBuffer<'evictor, OrderKey> {
    /// Creates new `EventBuffer`.
    ///
    /// `evictor` takes new `OrderKey`, old `OrderKey`, and returns `true` if event can be evicted and `false` otherwise.
    pub fn new<E>(evictor: E) -> Self
    where
        for<'new, 'old> E: Fn(&'new OrderKey, &'old OrderKey) -> bool + Send + Sync + 'evictor,
    {
        Self {
            buf: Default::default(),
            evictor: Box::new(evictor),
        }
    }

    pub fn clear(&mut self) {
        self.buf.clear()
    }
}

impl<OrderKey> EventBuffer<'_, OrderKey> {
    /// Add event to buffer and drop outdated events.
    pub fn push_event(&mut self, event: LocalInputEvent, order_key: OrderKey) {
        // drop outdated events
        let part = self.buf.partition_point(|(_, old)| (self.evictor)(&order_key, old));
        self.buf.drain(0..part);

        self.buf.push((event, order_key));
    }
}

impl<OrderKey> EventBuffer<'_, OrderKey>
where
    OrderKey: Ord,
{
    /// Query recent pressed keys.
    ///
    /// Recent pressed keys are keys where its key up and key down events exist
    /// in the buffer.
    pub fn recent_pressed_keys<'a, 'b>(
        &'a self,
        since: Option<&'b OrderKey>,
    ) -> impl Iterator<Item = (&'a KeyCode, &'a OrderKey)>
    where
        'b: 'a,
    {
        RecentKeyPresses::new(
            self.buf
                .iter()
                .filter_map(|(e, t)| KeyPressEvent::from_local_input_event(e, t)),
            since,
        )
    }
}

#[derive(Copy, Clone, Debug)]
enum KeyPressEvent<'a, T> {
    Down(KeyPress<'a, T>),
    Up(KeyPress<'a, T>),
}

#[derive(Copy, Clone, Debug)]
struct KeyPress<'a, OrderKey> {
    key_code: &'a KeyCode,
    order_key: &'a OrderKey,
}

impl<'a, OrderKey> KeyPressEvent<'a, OrderKey> {
    pub fn from_local_input_event(event: &'a LocalInputEvent, order_key: &'a OrderKey) -> Option<Self> {
        match event {
            LocalInputEvent::KeyDown { key: key_code } => KeyPressEvent::Down(KeyPress { key_code, order_key }).into(),
            LocalInputEvent::KeyUp { key: key_code } => KeyPressEvent::Up(KeyPress { key_code, order_key }).into(),
            _ => None,
        }
    }
}

pub struct RecentKeyPresses<'a, T> {
    events: Box<dyn Iterator<Item = KeyPressEvent<'a, T>> + 'a>,
    /// Stores seen but unmatched events.
    queue: VecDeque<KeyPressEvent<'a, T>>,
}

impl<'a, OrderKey> RecentKeyPresses<'a, OrderKey>
where
    OrderKey: Ord,
{
    fn new<'b>(events: impl Iterator<Item = KeyPressEvent<'a, OrderKey>> + 'a, since: Option<&'b OrderKey>) -> Self
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
        let key_down = key_down.or_else(|| {
            loop {
                match self.events.next() {
                    Some(event) => match event {
                        KeyPressEvent::Down(x) => break Some(x),
                        // found starting event other than key down
                        _ => continue,
                    },
                    // iterator is exhausted
                    None => break None,
                }
            }
        });

        key_down
    }

    fn find_key_up(&mut self, key_code: &KeyCode) -> Option<KeyPress<'a, T>> {
        // find key up from queue
        let key_up = {
            let mut q = Vec::new();
            let key = loop {
                match self.queue.pop_back() {
                    Some(KeyPressEvent::Up(x)) if x.key_code == key_code => break x.into(),
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
        let key_up = key_up.or_else(|| {
            loop {
                match self.events.next() {
                    Some(event) => match event {
                        KeyPressEvent::Up(x) if x.key_code == key_code => break Some(x),
                        other_key_down => {
                            self.queue.push_front(other_key_down);
                            continue;
                        }
                    },
                    None => break None,
                }
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

        // if key up for the matching key down is not found then this iterator is exhausted
        let key_up = match self.find_key_up(key_down.key_code) {
            Some(x) => x,
            None => return None,
        };

        Some((key_up.key_code, key_up.order_key))
    }
}
