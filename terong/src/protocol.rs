use crate::event::{KeyCode, MouseButton};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u8 = 0;

/// Server to client message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Event(Event),
}

impl From<Event> for ServerMessage {
    fn from(x: Event) -> Self {
        Self::Event(x)
    }
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum Event {
    MouseMove { dx: i32, dy: i32 },
    MouseButtonDown { button: MouseButton },
    MouseButtonUp { button: MouseButton },
    MouseScroll {},

    KeyDown { key: KeyCode },
    KeyUp { key: KeyCode },
}

/// Client to server message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Hello(Hello),
}

impl From<Hello> for ClientMessage {
    fn from(x: Hello) -> Self {
        Self::Hello(x)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Hello {
    pub protocol_version: u8,
    pub client_name: String,
}
