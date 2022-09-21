mod input_event;

use macross::impl_from;
use ring::digest;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Debug};

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
    /// Hash of server TLS certificate.
    ///
    /// The client will inspect this value before upgrading connection to TLS.
    pub server_tls_cert_hash: Sha256,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UpgradeTransportResponse {
    /// Hash of client TLS certificate.
    ///
    /// The server will inspect this value before upgrading connection to TLS.
    pub client_tls_cert_hash: Sha256,
}

/// Container for SHA-256.
#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct Sha256([u8; 32]);

impl Sha256 {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let out = digest::digest(&digest::SHA256, bytes);
        let mut hash = [0; digest::SHA256_OUTPUT_LEN];
        hash.copy_from_slice(out.as_ref());
        Self(hash)
    }
}

impl fmt::Display for Sha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}
