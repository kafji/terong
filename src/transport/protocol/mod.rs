mod input_event;

use macross::impl_from;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

pub use self::input_event::*;

/// Client to server message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    /// Handshake initiation message.
    Hello(HelloMessage),
}

impl_from!(ClientMessage, {
    Self::Hello => HelloMessage,
});

/// Server to client message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    /// Handshake response message.
    HelloReply(HelloReply),
    /// Propagated event from the server host machine.
    Event(InputEvent),
}

impl_from!(ServerMessage, {
     Self::HelloReply => HelloReply,
     Self::Event => InputEvent,
});

/// Client's hello message.
///
/// After sending this message client will wait for [HelloReply] response.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HelloMessage {
    /// Client app version.
    pub client_version: String,
}

/// Client's [HelloMessage] reply.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum HelloReply {
    /// The hello message processed sucessfully, request to upgrade transport to
    /// TLS.
    Ok,
    /// Unrecoverable failure while processing the hello message.
    Err(HelloReplyError),
}

impl_from!(HelloReply, {
    Self::Err => HelloReplyError,
});

/// Error for [HelloMessage].
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum HelloReplyError {
    /// Server and client are in different version.
    ///
    /// We doesn't have protocol version. Instead we require the server and the
    /// client to have an identical version. In other words, we always assume
    /// different protocol version on each revision.
    VersionMismatch,
}
