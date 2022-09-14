mod input_event;

use crate::{impl_from, transport::Certificate};
use anyhow::Error;
use bytes::{Buf, BufMut, BytesMut};
use futures::Future;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{convert::TryInto, fmt::Debug, marker::PhantomData};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::debug;

pub use self::input_event::*;

/// Sum type of server to client messages.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    /// Handshake response message.
    HelloReply(HelloReply),
    Event(InputEvent),
}

impl_from!(ServerMessage, {
     Self::HelloReply => HelloReply,
     Self::Event => InputEvent,
});

/// Sum type of client to server messages.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    /// Handshake initiation message.
    Hello(HelloMessage),
}

impl_from!(ClientMessage, {
    Self::Hello => HelloMessage,
});

/// Client hello message.
///
/// After sending this message client will wait for [HelloReply] response.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HelloMessage {
    /// Client app version.
    pub client_version: String,
    /// Client TLS certificate.
    ///
    /// The server will inspect this value before upgrading connection to TLS.
    pub client_tls_cert: Certificate,
}

/// Server response for [HelloMessage] from client.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum HelloReply {
    Ok(HelloReplyMessage),
    Err(HelloReplyError),
}

impl_from!(HelloReply, {
    Self::Ok => HelloReplyMessage,
    Self::Err => HelloReplyError,
});

/// Successful response for [HelloMessage].
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct HelloReplyMessage {
    /// Server TLS certificate.
    ///
    /// The client will inspect this value before upgrading connection to TLS.
    pub server_tls_cert: Certificate,
}

/// Error response for [HelloMessage].
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum HelloReplyError {
    /// Server and client are in different version.
    ///
    /// We doesn't have protocol version. Instead we require the server and the
    /// client to have identical version. In other words, we always assume
    /// different protocol version on each revision.
    VersionMismatch,
}
