use crate::{
    protocol::{
        ClientMessage, HelloMessage, HelloReply, HelloReplyError, InputEvent, ServerMessage,
        UpgradeTransportRequest,
    },
    transport::{Certificate, PrivateKey, SingleCertVerifier, Transport, Transporter},
};
use anyhow::{Context, Error};
use rustls::{
    server::{ClientCertVerified, ClientCertVerifier},
    DistinguishedNames,
};
use std::{net::SocketAddr, sync::Arc, time::SystemTime};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    select,
    sync::mpsc,
    task::{self, JoinHandle},
};
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor};
use tracing::{debug, info};

pub fn start(mut proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) -> JoinHandle<()> {
    task::spawn(async move { run(&mut proto_event_rx).await.unwrap() })
}

async fn run(proto_event_rx: &mut mpsc::UnboundedReceiver<InputEvent>) -> Result<(), Error> {
    let server_addr: SocketAddr = "0.0.0.0:3000".parse().context("invalid socket address")?;

    info!("listening at {}", server_addr);
    let listener = TcpListener::bind(server_addr).await?;

    let mut transporter: Option<Transporter<_, _, _, _>> = None;

    let mut state = State::Handshaking;

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
                    session = Session::new(todo!(), todo!(), stream).await?.into();
                } else {
                    info!("already have active session");
                }
            }
        }
    }

    Ok(())
}

async fn run_session<S>(session: Session<S>) -> JoinHandle<()> {
    task::spawn(async {
        let Session { transport } = session;
        let mut state = State::Handshaking;
        loop {
            match state {
                State::Handshaking => todo!(),
                State::UpgradingTransport {
                    server_tls_cert,
                    client_tls_cert,
                } => todo!(),
                State::Idle => todo!(),
                State::ReceivedEvent { event } => todo!(),
            }
        }
    })
}

#[derive(Debug)]
enum State {
    Handshaking,
    UpgradingTransport {
        server_tls_cert: Certificate,
        client_tls_cert: Certificate,
    },
    Idle,
    ReceivedEvent {
        event: InputEvent,
    },
}

#[derive(Debug)]
struct Session<S> {
    transport: Transport<S, ClientMessage, ServerMessage>,
}

impl<S> Session<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn new(stream: S) -> Result<Session<S>, Error> {
        let transport = Transport::new(stream);
        let mut s = Self { transport };
        s.handshake().await?;
        Ok(s)
    }

    async fn handshake(&mut self) -> Result<(), Error> {
        let msg: ClientMessage = self.transport.recv_msg().await?;

        match msg {
            ClientMessage::Hello(HelloMessage {
                client_version,
                client_tls_cert,
            }) => {
                // We doesn't have protocol version, so instead we require identical version on
                // both server and client. In other words, we assume different protocol for each
                // version.
                let msg: ServerMessage = if client_version == env!("CARGO_PKG_VERSION") {
                    let reply: HelloReply = UpgradeTransportRequest {
                        server_tls_cert: self.server_tls_cert.clone(),
                    }
                    .into();
                    reply.into()
                } else {
                    let reply: HelloReply = HelloReplyError::VersionMismatch.into();
                    reply.into()
                };
                self.transport.send_msg(msg).await?;
            }
        }

        Ok(())
    }
}

impl<S> Session<'_, S> {
    fn transport(&mut self) -> &mut Transport<S, ClientMessage, ServerMessage> {
        &mut self.transport
    }
}
