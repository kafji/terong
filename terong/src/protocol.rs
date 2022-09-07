use crate::event::InputEvent;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u8 = 0;

/// Server to client message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    InputEvent(InputEvent),
}

impl From<InputEvent> for ServerMessage {
    fn from(x: InputEvent) -> Self {
        Self::InputEvent(x)
    }
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
