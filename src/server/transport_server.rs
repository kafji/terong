use crate::protocol::{
    self, ClientMessage, HelloMessage, HelloReply, HelloReplyError, InputEvent, MessageInboxRef,
    ServerMessage,
};
use anyhow::{Context, Error};
use bytes::BytesMut;
use std::net::SocketAddr;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    select,
    sync::mpsc,
    task::{self, JoinHandle},
};
use tokio_native_tls::TlsAcceptor;
use tracing::{debug, info};

pub fn start(mut proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) -> JoinHandle<()> {
    task::spawn(async move { run(&mut proto_event_rx).await.unwrap() })
}

async fn run(proto_event_rx: &mut mpsc::UnboundedReceiver<InputEvent>) -> Result<(), Error> {
    let tls: TlsAcceptor = {
        use tokio_native_tls::native_tls::{Identity, Protocol, TlsAcceptor};
        let identity = Identity::from_pkcs8(&[], &[])?;
        TlsAcceptor::builder(identity)
            .min_protocol_version(Some(Protocol::Tlsv12))
            .build()?
            .into()
    };

    let server_addr: SocketAddr = "0.0.0.0:3000".parse().context("invalid socket address")?;

    info!("listening at {}", server_addr);
    let listener = TcpListener::bind(server_addr).await?;

    let mut session: Option<Session<_>> = None;

    loop {
        select! { biased;
            // handle event
            x = proto_event_rx.recv() => {
                if let Some(session) = session.as_mut() {
                    match x {
                        Some(event) => {
                            let msg: ServerMessage = event.into();
                            session.send_msg(&msg).await?;
                        }
                        None => break,
                    }
                }
            }

            // handle incoming connection
            x = listener.accept() => {
                let (stream, peer_addr) = x?;
                if let None = session.as_ref() {
                    debug!(?peer_addr, "upgrading connection to tls");
                    let stream = tls
                        .accept(stream)
                        .await
                        .context("failed to upgrade connection to tls")?;
                    info!(?peer_addr, "creating new session");
                    session = Session::new(stream, peer_addr).await?.into();
                } else {
                    info!("already have active session");
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Session<S> {
    stream: S,
    peer_addr: SocketAddr,
    read_buf: BytesMut,
}

impl<S> Session<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn new(stream: S, peer_addr: SocketAddr) -> Result<Session<S>, Error> {
        let mut s = Self {
            stream,
            peer_addr,
            read_buf: Default::default(),
        };

        s.handshake().await?;

        Ok(s)
    }

    async fn handshake(&mut self) -> Result<(), Error> {
        let peer_addr = self.peer_addr;
        debug!(?peer_addr, "protocol handshaking");

        let msg: ClientMessage = self.recv_msg().await?;

        match msg {
            ClientMessage::Hello(HelloMessage { version }) => {
                // We doesn't have protocol version, so instead we require identical version on
                // both server and client. In other words, we assume different protocol for each
                // version.
                let msg: ServerMessage = if version == env!("CARGO_PKG_VERSION") {
                    let reply = HelloReply::Ok;
                    reply.into()
                } else {
                    let reply: HelloReply = HelloReplyError::VersionMismatch.into();
                    reply.into()
                };
                self.send_msg(&msg).await?;
            }
        }

        Ok(())
    }
}

impl<S> Session<S>
where
    S: AsyncWrite + Unpin,
{
    async fn send_msg(&mut self, msg: &ServerMessage) -> Result<(), Error> {
        protocol::send_msg(&mut self.stream, msg).await
    }
}

impl<S> Session<S>
where
    S: AsyncRead + Unpin,
{
    fn as_inbox(&mut self) -> MessageInboxRef<S, BytesMut> {
        MessageInboxRef::new(&mut self.stream, &mut self.read_buf)
    }

    pub async fn recv_msg(&mut self) -> Result<ClientMessage, Error> {
        let mut inbox = self.as_inbox();
        inbox.recv_msg().await
    }
}
