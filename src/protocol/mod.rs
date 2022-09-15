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

/// Client to server message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    /// Handshake initiation message.
    Hello(HelloMessage),
    /// Reply to server's upgrade transport request.
    UpgradeTransportReply(UpgradeTransportResponse),
}

impl_from!(ClientMessage, {
    Self::Hello => HelloMessage,
    Self::UpgradeTransportReply => UpgradeTransportResponse,
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
    Ok(UpgradeTransportRequest),
    /// Unrecoverable failure while processing the hello message.
    Err(HelloReplyError),
}

impl_from!(HelloReply, {
    Self::Ok => UpgradeTransportRequest,
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

/// Successful response for [HelloMessage].
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UpgradeTransportRequest {
    /// Server TLS certificate.
    ///
    /// The client will inspect this value before upgrading connection to TLS.
    pub server_tls_cert: Certificate,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UpgradeTransportResponse {
    /// Client TLS certificate.
    ///
    /// The server will inspect this value before upgrading connection to TLS.
    pub client_tls_cert: Certificate,
}
