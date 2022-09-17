use crate::{
    protocol::{
        ClientMessage, HelloMessage, HelloReply, HelloReplyError, InputEvent, ServerMessage,
        UpgradeTransportRequest, UpgradeTransportResponse,
    },
    transport::{Certificate, PrivateKey, SingleCertVerifier, Transport, Transporter},
};
use anyhow::{Context, Error};
use futures::{future, FutureExt};
use std::{
    fmt::Debug,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc,
    task::{self, JoinHandle},
};
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor, TlsStream};
use tracing::{debug, info};

pub fn start(proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) -> JoinHandle<()> {
    task::spawn(async move { run(proto_event_rx).await.unwrap() })
}

async fn run(mut event_rx: mpsc::UnboundedReceiver<InputEvent>) -> Result<(), Error> {
    let server_addr: SocketAddr = "0.0.0.0:3000".parse().context("invalid socket address")?;

    info!("listening at {}", server_addr);
    let listener = TcpListener::bind(server_addr).await?;

    let mut session_handler: Option<SessionHandler> = None;

    loop {
        let finished = session_handler
            .as_mut()
            .map(|x| x.finish().boxed())
            .unwrap_or_else(|| future::pending().boxed());

        select! { biased;
            _ = finished => {

            }
            _ = event_rx.recv() => {

            }

            conn = listener.accept() => {
                let (stream, peer_addr) = conn?;
            }
        }
        let (stream, peer_addr) = listener.accept().await?;
        match &mut session_handler {
            Some(session_handler) => {
                if session_handler.is_finished() {
                    session_handler.finish().await?;
                } else {
                    info!("already have active session");
                }
            }
            None => {
                let transporter = Transporter::PlainText(Transport::new(stream));
                session_handler = Some(start_session(transporter));
            }
        }
    }

    Ok(())
}

struct SessionHandler {
    event_tx: mpsc::UnboundedSender<InputEvent>,
    task: JoinHandle<()>,
}

impl SessionHandler {
    async fn send_event(&mut self, event: InputEvent) -> Result<(), Error> {
        self.event_tx.send(event);
        Ok(())
    }

    fn is_finished(&mut self) -> bool {
        self.task.is_finished()
    }

    async fn finish(&mut self) -> Result<(), Error> {
        (&mut self.task).await?;
        Ok(())
    }
}

fn start_session(transporter: ServerTransporter) -> SessionHandler {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let task = task::spawn(async move { run_session(transporter, event_rx).await.unwrap() });
    SessionHandler { event_tx, task }
}

async fn run_session(
    mut transporter: ServerTransporter,
    mut event_rx: mpsc::UnboundedReceiver<InputEvent>,
) -> Result<(), Error> {
    let mut state = State::Handshaking;

    loop {
        state = match state {
            State::Handshaking => {
                let transport = transporter.plain()?;

                // wait for hello message
                let msg = transport.recv_msg().await?;

                if let ClientMessage::Hello(HelloMessage { client_version }) = msg {
                    // check version
                    let server_version = env!("CARGO_PKG_VERSION").to_owned();
                    if server_version == client_version {
                        // request upgrade transport
                        let server_tls_cert = todo!();
                        let msg: HelloReply = UpgradeTransportRequest { server_tls_cert }.into();
                        transport.send_msg(msg).await?;

                        let msg = transport.recv_msg().await?;
                        if let ClientMessage::UpgradeTransportReply(UpgradeTransportResponse {
                            client_tls_cert,
                        }) = msg
                        {
                            State::UpgradingTransport { client_tls_cert }
                        } else {
                            todo!()
                        }
                    } else {
                        todo!()
                    }
                } else {
                    todo!()
                }
            }

            State::UpgradingTransport { client_tls_cert } => {
                let server_tls_key = todo!();

                transporter = transporter
                    .upgrade(|stream| upgrade_stream(stream, server_tls_key, client_tls_cert))
                    .await?;

                State::Idle
            }

            State::Idle => {
                let event = event_rx.recv().await;
                let event = match event {
                    Some(x) => x,
                    None => break,
                };

                State::ReceivedEvent { event }
            }

            State::ReceivedEvent { event } => {
                let transport = transporter.secure()?;
                transport.send_msg(event).await?;

                State::Idle
            }
        }
    }
    Result::<_, Error>::Ok(())
}

#[derive(Debug)]
enum State {
    Handshaking,
    UpgradingTransport { client_tls_cert: Certificate },
    Idle,
    ReceivedEvent { event: InputEvent },
}

type ServerTransporter = Transporter<TcpStream, TlsStream<TcpStream>, ClientMessage, ServerMessage>;

#[derive(Debug)]
struct Session {
    transporter: Transporter<TcpStream, TlsStream<TcpStream>, ClientMessage, ServerMessage>,
    event_rx: mpsc::UnboundedReceiver<InputEvent>,
}

pub async fn upgrade_stream<S>(
    stream: S,
    server_tls_key: PrivateKey,
    client_tls_cert: Certificate,
) -> Result<TlsStream<S>, Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tls: TlsAcceptor = {
        let client_cert_verifier = Arc::new(SingleCertVerifier::new(client_tls_cert));
        let cfg = ServerConfig::builder()
            .with_safe_defaults()
            .with_client_cert_verifier(client_cert_verifier)
            .with_single_cert(vec![], rustls::PrivateKey(server_tls_key.into()))?;
        Arc::new(cfg).into()
    };
    let stream = tls.accept(stream).await?;
    Ok(stream.into())
}
