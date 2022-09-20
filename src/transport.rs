use crate::protocol::{ClientMessage, ServerMessage};
use anyhow::{bail, Error};
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use futures::Future;
use macross::newtype;
use rustls::{
    client::{ServerCertVerified, ServerCertVerifier},
    server::{ClientCertVerified, ClientCertVerifier},
    DistinguishedNames, ServerName,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{convert::TryInto, fmt::Debug, marker::PhantomData, net::IpAddr, time::SystemTime};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::debug;

/// Protocol message marker trait.
pub trait Message: Serialize + DeserializeOwned {}

impl Message for ServerMessage {}

impl Message for ClientMessage {}

/// Send protocol message.
///
/// This function is not cancel safe.
async fn send_msg(
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
struct MessageReader<'a, S, B> {
    src: &'a mut S,
    buf: &'a mut B,
}

impl<'a, S, B> MessageReader<'a, S, B> {
    fn new(src: &'a mut S, buf: &'a mut B) -> Self {
        Self { src, buf }
    }
}

impl<'a, S, B> MessageReader<'a, S, B>
where
    S: AsyncRead + Unpin,
    B: Buf + BufMut,
{
    /// Fill buffer until the specified size is reached.
    ///
    /// This function is cancel safe.
    async fn fill_buf(&mut self, size: usize) -> Result<(), Error> {
        while self.buf.remaining() < size {
            let size = self.src.read_buf(&mut self.buf).await?;
            debug!("read {} bytes from source", size);
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
        self.fill_buf(2).await?;

        // get message length
        let length = self.buf.get_u16();

        self.fill_buf(length as _).await?;

        // take message length bytes
        let bytes = self.buf.copy_to_bytes(length as _);

        let msg: M = bincode::deserialize(&*bytes)?;
        debug!("received message {:?}", msg);

        Ok(msg)
    }
}

#[derive(Debug)]
pub struct Transport<S, IN, OUT> {
    /// The IO stream.
    stream: S,
    read_buf: BytesMut,
    /// Incoming message data type.
    _in: PhantomData<IN>,
    /// Outgoing message data type.
    _out: PhantomData<OUT>,
}

impl<S, IN, OUT> Transport<S, IN, OUT> {
    /// Creates a new transport.
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            read_buf: Default::default(),
            _in: PhantomData,
            _out: PhantomData,
        }
    }

    /// Maps stream while keeping other internal data intact.
    async fn try_map_stream<T, F, Fut>(self, map: F) -> Result<Transport<T, IN, OUT>, Error>
    where
        F: FnOnce(S) -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        let Self {
            stream,
            read_buf,
            _in,
            _out,
        } = self;
        let stream = map(stream).await?;
        let s = Transport {
            stream,
            read_buf,
            _in,
            _out,
        };
        Ok(s)
    }
}

impl<S, IN, OUT> Transport<S, IN, OUT>
where
    S: AsyncWrite + Unpin,
    OUT: Message + Debug,
{
    /// Sends a protocol message.
    ///
    /// This method is not cancel safe.
    pub async fn send_msg<'a>(&mut self, msg: OUT) -> Result<(), Error> {
        send_msg(&mut self.stream, &msg).await
    }
}

impl<S, IN, OUT> Transport<S, IN, OUT>
where
    S: AsyncRead + Unpin,
    IN: Message + Debug,
{
    fn as_msg_reader(&mut self) -> MessageReader<S, BytesMut> {
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

#[async_trait]
pub trait Messenger {
    type In: Message;
    type Out: Message;

    async fn recv_msg(&mut self) -> Result<Self::In, Error>;
    async fn send_msg<'a>(&mut self, msg: Self::Out) -> Result<(), Error>;
}

#[async_trait]
impl<T, IN, OUT> Messenger for Transport<T, IN, OUT>
where
    T: AsyncRead + AsyncWrite + Unpin + Send,
    IN: Message + Debug + Send,
    OUT: Message + Debug + Send + Sync,
{
    type In = IN;
    type Out = OUT;

    async fn recv_msg(&mut self) -> Result<Self::In, Error> {
        let x = Transport::recv_msg(self).await?;
        Ok(x)
    }

    async fn send_msg<'a>(&mut self, msg: Self::Out) -> Result<(), Error> {
        Transport::send_msg(self, msg).await?;
        Ok(())
    }
}

/// Facilitates acquiring and upgrading [Transport].
#[derive(Debug)]
pub enum Transporter<PS /* plain stream */, SS /* secure stream */, IN, OUT> {
    Plain(Transport<PS, IN, OUT>),
    Secure(Transport<SS, IN, OUT>),
}

impl<PS, SS, IN, OUT> Transporter<PS, SS, IN, OUT>
where
    PS: AsyncRead + AsyncWrite + Debug + Send + Unpin,
    SS: AsyncRead + AsyncWrite + Debug + Send + Unpin,
    IN: Message + Debug + Send,
    OUT: Message + Debug + Send + Sync,
{
    /// Mutably borrow plain transport.
    pub fn plain(&mut self) -> Result<&mut Transport<PS, IN, OUT>, Error> {
        if let Self::Plain(t) = self {
            Ok(t)
        } else {
            bail!("expecting plain text transport, but was {:?}", self)
        }
    }

    /// Upgrades plain text transport to secure transport.
    pub async fn upgrade<F, Fut>(self, upgrader: F) -> Result<Self, Error>
    where
        F: FnOnce(PS) -> Fut,
        Fut: Future<Output = Result<SS, Error>>,
    {
        match self {
            Self::Plain(t) => {
                let t = t.try_map_stream(upgrader).await?;
                Ok(Self::Secure(t))
            }
            _ => bail!("expecting plain text transport, but was {:?}", self),
        }
    }

    /// Mutably borrow secure transport.
    pub fn secure(&mut self) -> Result<&mut Transport<SS, IN, OUT>, Error> {
        if let Self::Secure(t) = self {
            Ok(t)
        } else {
            bail!("expecting secure transport, but was {:?}", self)
        }
    }

    /// Mutably borrow current transport.
    pub fn any(&mut self) -> &mut (dyn Messenger<In = IN, Out = OUT> + Send) {
        match self {
            Transporter::Plain(x) => x,
            Transporter::Secure(x) => x,
        }
    }
}

newtype! {
    /// TLS certificate.
    #[derive(Clone, Serialize, Deserialize, Debug)]
    pub Certificate = Vec<u8>;
}

newtype! {
    /// TLS private key.
    #[derive(Clone, Debug)]
    pub PrivateKey = Vec<u8>;
}

/// Certifier for a single known certificate.
#[derive(Clone, Debug)]
pub struct SingleCertVerifier {
    cert: Certificate,
}

impl SingleCertVerifier {
    pub fn new(cert: Certificate) -> Self {
        Self { cert }
    }
}

impl ServerCertVerifier for SingleCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: SystemTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if &end_entity.0 == self.cert.as_ref() {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("invalid server certificate".into()))
        }
    }
}

impl ClientCertVerifier for SingleCertVerifier {
    fn client_auth_root_subjects(&self) -> Option<DistinguishedNames> {
        Some(vec![])
    }

    fn verify_client_cert(
        &self,
        end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _now: SystemTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        if &end_entity.0 == self.cert.as_ref() {
            Ok(ClientCertVerified::assertion())
        } else {
            Err(rustls::Error::General("invalid client certificate".into()))
        }
    }
}

pub fn generate_tls_key_pair(host: IpAddr) -> Result<(Certificate, PrivateKey), Error> {
    let mut params = rcgen::CertificateParams::default();
    params
        .subject_alt_names
        .push(rcgen::SanType::IpAddress(host));
    let cert = rcgen::Certificate::from_params(params).unwrap();
    let private_key = cert.serialize_private_key_der().into();
    let cert = cert.serialize_der()?.into();
    Ok((cert, private_key))
}
