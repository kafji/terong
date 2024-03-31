mod input_event;

use macross::impl_from;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub use self::input_event::*;

/// Client to server message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Ping(Ping),
}

impl_from!(ClientMessage, {
    Self::Ping => Ping,
});

/// Server to client message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    /// Propagated event from the server host machine.
    Event(InputEvent),
    Pong(Pong),
}

impl_from!(ServerMessage, {
     Self::Event => InputEvent,
     Self::Pong => Pong,
});

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Ping {
    pub counter: u16,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Pong {
    pub counter: u16,
}
