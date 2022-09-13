use crate::protocol::{
    self, ClientMessage, HelloMessage, HelloReply, InputEvent, MessageInbox, MessageReader,
    ServerMessage, Transport,
};
use anyhow::{bail, Context, Error};
use bytes::BytesMut;
use std::{fmt::Debug, net::SocketAddr};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::mpsc,
    task::{self, JoinHandle},
};
use tokio_native_tls::{native_tls::Certificate, TlsConnector};
use tracing::info;

pub fn start(mut event_tx: mpsc::UnboundedSender<InputEvent>) -> JoinHandle<()> {
    task::spawn(async move { run_client(&mut event_tx).await.unwrap() })
}

async fn run_client(event_tx: &mut mpsc::UnboundedSender<InputEvent>) -> Result<(), Error> {
    let tls: TlsConnector = {
        use tokio_native_tls::native_tls::{Identity, Protocol, TlsConnector};
        let identity = Identity::from_pkcs8(&[], &[])?;
        let certificate = Certificate::from_pem(&[])?;
        TlsConnector::builder()
            .identity(identity)
            .min_protocol_version(Some(Protocol::Tlsv12))
            .add_root_certificate(certificate)
            .disable_built_in_roots(true)
            .build()?
            .into()
    };

    todo!();

    let addr: SocketAddr = "192.168.123.31:3000"
        .parse()
        .context("server address was invalid")?;

    // open connection with the server
    info!("connecting to {}", addr);
    let mut stream = TcpStream::connect(addr)
        .await
        .context("failed to connect to the server")?;
    info!("connected to {}", addr);

    let (mut source, mut sink) = stream.split();
    let mut inbox = MessageInbox::new(&mut source);

    // send handshake message
    let hello_msg = HelloMessage { version: "".into() };
    {
        let msg: ClientMessage = hello_msg.into();
        protocol::send_msg(&mut sink, &msg).await
    }
    .context("failed to send hello message")?;

    // read handshake reply
    let msg = inbox
        .recv_msg()
        .await
        .context("failed to read hello reply")?;
    if let ServerMessage::HelloReply(reply) = msg {
        if let HelloReply::Err(err) = reply {
            bail!("handshake failure, {:?}", err)
        }
    } else {
        bail!("expecting hello reply, but was {:?}", msg);
    }

    // handshake successful

    loop {
        // read event message
        let msg = inbox
            .recv_msg()
            .await
            .context("failed to read event message")?;
        if let ServerMessage::Event(event) = msg {
            if let Err(_) = event_tx.send(event) {
                break;
            }
        } else {
            bail!("expecting event message, but was {:?}", msg);
        }
    }

    Ok(())
}

#[derive(Debug)]
struct ConnectionSession<S> {
    transport: Transport<S, ServerMessage, ClientMessage>,
}

impl<S> ConnectionSession<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn new(stream: S) -> Result<Self, Error> {
        let transport = Transport::new(stream);
        let mut s = Self { transport };
        s.handshake().await?;
        Ok(s)
    }

    async fn handshake(&mut self) -> Result<(), Error> {
        let transport = self.transport();

        let msg = HelloMessage {
            version: env!("CARGO_PKG_VERSION").into(),
        };
        transport.send_msg(msg).await?;

        let msg = transport.recv_msg().await?;
        if let ServerMessage::HelloReply(reply) = msg {
            if let HelloReply::Err(err) = reply {
                bail!("handshake failure, {:?}", err)
            }
        }

        Ok(())
    }
}

impl<S> ConnectionSession<S> {
    fn transport(&mut self) -> &mut Transport<S, ServerMessage, ClientMessage> {
        &mut self.transport
    }
}
