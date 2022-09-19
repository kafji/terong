use crate::{
    config::no_tls,
    protocol::{
        ClientMessage, HelloMessage, HelloReply, InputEvent, ServerMessage,
        UpgradeTransportRequest, UpgradeTransportResponse,
    },
    transport::{Certificate, PrivateKey, SingleCertVerifier, Transport, Transporter},
};
use anyhow::{Context, Error};
use futures::{future, FutureExt};
use std::{fmt::Debug, net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc::{self, error::SendError},
    task::{self, JoinError, JoinHandle},
};
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor, TlsStream};
use tracing::{info, warn};

pub fn start(proto_event_rx: mpsc::Receiver<InputEvent>) -> JoinHandle<()> {
    task::spawn(async move { run(proto_event_rx).await.unwrap() })
}

async fn run(mut event_rx: mpsc::Receiver<InputEvent>) -> Result<(), Error> {
    let server_addr: SocketAddr = "0.0.0.0:3000".parse().context("invalid socket address")?;

    info!("listening at {}", server_addr);
    let listener = TcpListener::bind(server_addr).await?;

    let mut session_handler: Option<SessionHandler> = None;

    loop {
        let finished = session_handler
            .as_mut()
            .map(|x| x.finished().boxed())
            .unwrap_or_else(|| future::pending().boxed());

        select! { biased;

            // check if session is finished if it's exist
            x = finished => {
                x?;
                session_handler.take();
            }

            // propagate to session if it's exist
            event = event_rx.recv() => {
                match (event, &mut session_handler) {
                    (Some(event), Some(session)) => { session.send_event(event).await.ok(); },
                    (None, _) => break,
                    _ => (),
                }
            }

            // handle incoming connection, create a new session if it's not exist
            conn = listener.accept() => {
                let (stream, _) = conn?;
                if session_handler.is_none() {
                    let transporter = Transporter::Plain(Transport::new(stream));
                    let handler = create_session(transporter);
                    session_handler = Some(handler);
                }
            }
        }
    }

    Ok(())
}

/// Handler to a session.
#[derive(Debug)]
struct SessionHandler {
    event_tx: mpsc::Sender<InputEvent>,
    task: JoinHandle<()>,
}

impl SessionHandler {
    /// Send input event to this session.
    async fn send_event(&mut self, event: InputEvent) -> Result<(), SendError<InputEvent>> {
        self.event_tx.send(event).await?;
        Ok(())
    }

    /// This method is cancel safe.
    async fn finished(&mut self) -> Result<(), JoinError> {
        (&mut self.task).await?;
        Ok(())
    }
}

/// Creates a new session.
fn create_session(transporter: ServerTransporter) -> SessionHandler {
    let (event_tx, event_rx) = mpsc::channel(1);
    let task = task::spawn(async move { run_session(transporter, event_rx).await.unwrap() });
    SessionHandler { event_tx, task }
}

/// The session loop.
async fn run_session(
    mut transporter: ServerTransporter,
    mut event_rx: mpsc::Receiver<InputEvent>,
) -> Result<(), Error> {
    let mut state = State::Handshaking;

    let cert = {
        let mut params = rcgen::CertificateParams::default();
        params
            .subject_alt_names
            .push(rcgen::SanType::IpAddress("192.168.123.31".parse().unwrap()));
        let cert = rcgen::Certificate::from_params(params).unwrap();
        cert
    };

    loop {
        state = match state {
            State::Handshaking => {
                let transport = transporter.plain()?;

                // wait for hello message
                let msg = transport.recv_msg().await?;
                let client_version =
                    if let ClientMessage::Hello(HelloMessage { client_version }) = msg {
                        client_version
                    } else {
                        todo!()
                    };

                // check version
                let server_version = env!("CARGO_PKG_VERSION").to_owned();
                if client_version != server_version {
                    todo!()
                }

                // request upgrade transport
                let server_tls_cert = cert.serialize_der().unwrap().into();
                let msg: HelloReply = UpgradeTransportRequest { server_tls_cert }.into();
                transport.send_msg(msg.into()).await?;

                // wait for upgrade transport reply
                let msg = transport.recv_msg().await?;
                let client_tls_cert =
                    if let ClientMessage::UpgradeTransportReply(UpgradeTransportResponse {
                        client_tls_cert,
                    }) = msg
                    {
                        client_tls_cert
                    } else {
                        todo!()
                    };

                // upgrade to tls
                let no_tls = no_tls();
                if no_tls {
                    warn!("tls disabled");
                } else {
                    let server_tls_key = cert.serialize_private_key_der().into();
                    transporter = transporter
                        .upgrade(|stream| upgrade_stream(stream, server_tls_key, client_tls_cert))
                        .await?;
                }

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
                let transport = transporter.any();

                let msg = event.into();
                transport.send_msg(msg).await?;

                State::Idle
            }
        }
    }

    Result::<_, Error>::Ok(())
}

#[derive(Debug)]
enum State {
    Handshaking,
    Idle,
    ReceivedEvent { event: InputEvent },
}

type ServerTransporter = Transporter<TcpStream, TlsStream<TcpStream>, ClientMessage, ServerMessage>;

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
        let key = rustls::PrivateKey(server_tls_key.into());
        let cfg = ServerConfig::builder()
            .with_safe_defaults()
            .with_client_cert_verifier(client_cert_verifier)
            .with_single_cert(vec![], key)
            .context("failed to create server config tls")?;
        Arc::new(cfg).into()
    };

    let stream = tls.accept(stream).await.context("tls accept failed")?;

    Ok(stream.into())
}
