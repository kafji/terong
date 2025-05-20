pub mod protocol;

use self::protocol::{ClientMessage, ServerMessage};
use crate::typing::newtype;
use anyhow::Error;
use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    convert::TryInto,
    fmt::{self, Debug},
    marker::PhantomData,
    pin::Pin,
};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use tracing::debug;

/// Protocol message marker trait.
pub trait Message: Serialize + DeserializeOwned {}

impl Message for ServerMessage {}

impl Message for ClientMessage {}

const HEADER_LEN: usize = (u16::BITS / 8) as _; // 16 bit = 2 byte

/// Send protocol message.
///
/// This function is not cancel safe.
async fn send_msg(
    sink: &mut (impl AsyncWrite + Unpin),
    msg: impl Message + Debug,
) -> Result<(), Error> {
    debug!(?msg, "sending message");

    let msg_len: u16 = bincode::serialized_size(&msg)?.try_into()?;
    let len = HEADER_LEN + msg_len as usize;

    let mut buf = vec![0; len];
    buf[0..HEADER_LEN].copy_from_slice(&msg_len.to_be_bytes());

    bincode::serialize_into(&mut buf[HEADER_LEN..], &msg)?;

    sink.write_all(&buf).await?;

    sink.flush().await?;

    Ok(())
}

#[derive(Debug)]
struct MessageReader<'a, B> {
    src: &'a mut Pin<Box<dyn AsyncStream + Send>>,
    buf: &'a mut B,
}

impl<'a, B> MessageReader<'a, B> {
    fn new(src: &'a mut Pin<Box<dyn AsyncStream + Send>>, buf: &'a mut B) -> Self {
        Self { src, buf }
    }
}

impl<'a, B> MessageReader<'a, B>
where
    B: Buf + BufMut,
{
    /// Fill buffer until the specified size is reached.
    ///
    /// This function is cancel safe.
    async fn fill_buf(&mut self, size: usize) -> Result<(), Error> {
        while self.buf.remaining() < size {
            let size = self.src.read_buf(&mut self.buf).await?;
            if size == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
            }
        }
        Ok(())
    }

    /// Receive protocol message.
    ///
    /// This function is cancel safe.
    async fn recv_msg<M>(&mut self) -> Result<M, Error>
    where
        M: Message + Debug,
    {
        loop {
            self.fill_buf(HEADER_LEN).await?;

            // get message length
            let length = self.buf.get_u16();

            // ignore 0 bytes message
            if length == 0 {
                continue;
            }

            self.fill_buf(length as _).await?;

            // take message length bytes
            let bytes = self.buf.copy_to_bytes(length as _);

            let msg: M = bincode::deserialize(&bytes)?;
            debug!(?msg, "received message");

            break Ok(msg);
        }
    }
}

pub trait AsyncStream: AsyncRead + AsyncWrite + Debug {}

impl<T: AsyncRead + AsyncWrite + Debug> AsyncStream for T {}

#[derive(Debug)]
pub struct Transport<IN, OUT> {
    /// The IO stream.
    stream: Pin<Box<dyn AsyncStream + Send>>,
    read_buf: BytesMut,
    /// Incoming message data type.
    _in: PhantomData<IN>,
    /// Outgoing message data type.
    _out: PhantomData<OUT>,
}

impl<IN, OUT> Transport<IN, OUT> {
    /// Creates a new transport.
    pub fn new(stream: impl AsyncStream + Send + 'static) -> Self {
        Self {
            stream: Box::pin(stream),
            read_buf: Default::default(),
            _in: PhantomData,
            _out: PhantomData,
        }
    }
}

impl<IN, OUT> Transport<IN, OUT>
where
    OUT: Message + Debug,
{
    /// Sends a protocol message.
    ///
    /// This method is not cancel safe.
    pub async fn send_msg<'a>(&mut self, msg: OUT) -> Result<(), Error> {
        send_msg(&mut self.stream, msg).await
    }
}

impl<IN, OUT> Transport<IN, OUT>
where
    IN: Message + Debug,
{
    fn as_msg_reader(&mut self) -> MessageReader<BytesMut> {
        MessageReader::new(&mut self.stream, &mut self.read_buf)
    }

    /// Waits for a protocol message.
    ///
    /// This method is cancel safe.
    pub async fn recv_msg(&mut self) -> Result<IN, Error> {
        let mut reader = self.as_msg_reader();
        reader.recv_msg().await
    }
}

newtype! {
    /// TLS certificate.
    #[derive(Clone, Serialize, Deserialize)]
    pub Certificate = Vec<u8>;
}

impl fmt::Debug for Certificate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Certificate")
            .field(&hex::encode(&self.0))
            .finish()
    }
}

newtype! {
    /// TLS private key.
    #[derive(Clone, Debug)]
    pub PrivateKey = Vec<u8>;
}
