use crate::protocol::{
    ClientMessage, HelloMessage, HelloReply, HelloReplyError, InputEvent, ServerMessage, Transport,
};
use anyhow::{Context, Error};
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

    let mut session: Option<ClientSession<_>> = None;

    loop {
        select! { biased;
            // handle event
            x = proto_event_rx.recv() => {
                if let Some(session) = session.as_mut() {
                    match x {
                        Some(event) => {
                            let msg: ServerMessage = event.into();
                            session.transport().send_msg(msg).await?;
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
                    session = ClientSession::new(stream).await?.into();
                } else {
                    info!("already have active session");
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct ClientSession<S> {
    transport: Transport<S, ClientMessage, ServerMessage>,
}

impl<S> ClientSession<S>
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

        let msg: ClientMessage = transport.recv_msg().await?;

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
                transport.send_msg(msg).await?;
            }
        }

        Ok(())
    }
}

impl<S> ClientSession<S> {
    fn transport(&mut self) -> &mut Transport<S, ClientMessage, ServerMessage> {
        &mut self.transport
    }
}
