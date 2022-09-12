use anyhow::Error;
use bytes::{Buf, BytesMut};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{convert::TryInto, fmt::Debug};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::debug;

pub use crate::input_event::*;

// server messages

/// Server to client message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Event(InputEvent),
    HelloReply(ServerHello),
}

impl From<InputEvent> for ServerMessage {
    fn from(x: InputEvent) -> Self {
        Self::Event(x)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerHello {
    Ok,
    Err(String),
}

// client messages

/// Client to server message.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Hello(ClientHello),
}

impl From<ClientHello> for ClientMessage {
    fn from(x: ClientHello) -> Self {
        Self::Hello(x)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ClientHello {
    /// Client app version.
    pub version: String,
}

// protocol wire format and message read/write

pub trait Message: Serialize + DeserializeOwned {}

impl Message for ServerMessage {}

impl Message for ClientMessage {}

pub async fn send_msg(
    sink: &mut (impl AsyncWrite + Unpin),
    msg: &(impl Message + Debug),
) -> Result<(), Error> {
    debug!("sending message {:?}", msg);
    let msg_len: u16 = bincode::serialized_size(&msg)?.try_into()?;
    let len = 2 + msg_len as usize;
    let mut buf = vec![0; len];
    buf[0..2].copy_from_slice(&msg_len.to_be_bytes());
    bincode::serialize_into(&mut buf[2..], &msg)?;
    sink.write_all(&buf).await?;
    Ok(())
}

#[derive(Debug)]
pub struct MessageInbox<'a, R> {
    buf: BytesMut,
    src: &'a mut R,
}

impl<'a, R> MessageInbox<'a, R>
where
    R: AsyncRead + Unpin,
{
    pub fn new(src: &'a mut R) -> Self {
        Self {
            buf: Default::default(),
            src,
        }
    }

    async fn fill_buf(&mut self, size: usize) -> Result<(), Error> {
        while self.buf.len() < size {
            let size = self.src.read_buf(&mut self.buf).await?;
            debug!("read {} bytes from source", size);
            if size == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
            }
        }
        Ok(())
    }

    pub async fn recv_msg<M>(&mut self) -> Result<M, Error>
    where
        M: Message + Debug,
    {
        self.fill_buf(2).await?;
        let length = self.buf.get_u16();
        self.fill_buf(length as _).await?;
        let msg = self.buf.copy_to_bytes(length as _);
        let msg: M = bincode::deserialize(&*msg)?;
        debug!("received message {:?}", msg);
        Ok(msg)
    }
}
