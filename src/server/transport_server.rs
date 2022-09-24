use crate::{
    log_error,
    transport::{
        protocol::{
            ClientMessage, HelloMessage, HelloReply, HelloReplyError, InputEvent, ServerMessage,
            Sha256, UpgradeTransportRequest, UpgradeTransportResponse,
        },
        Certificate, PrivateKey, SingleCertVerifier, Transport, Transporter,
    },
};
use anyhow::{bail, Context, Error};
use futures::{future, FutureExt};
use std::{
    fmt::Debug,
    io::Write,
    net::{IpAddr, SocketAddr, SocketAddrV4},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc::{self, error::SendError},
    task::{self, JoinError, JoinHandle},
};
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor, TlsStream};
use tracing::{debug, error, info};

type ServerTransporter = Transporter<TcpStream, TlsStream<TcpStream>, ClientMessage, ServerMessage>;

#[derive(Debug)]
pub struct TransportServer {
    pub port: u16,
    pub event_rx: mpsc::Receiver<InputEvent>,
    pub tls_cert: Certificate,
    pub tls_key: PrivateKey,
}

pub fn start(args: TransportServer) -> JoinHandle<()> {
    task::spawn(async move { run(args).await })
}

async fn run(env: TransportServer) {
    let TransportServer {
        port,
        mut event_rx,
        tls_cert,
        tls_key,
    } = env;

    let tls_cert = Arc::new(tls_cert);
    let tls_key = Arc::new(tls_key);

    let server_addr = SocketAddrV4::new([0, 0, 0, 0].into(), port);

    info!("listening at {}", server_addr);
    let listener = TcpListener::bind(server_addr)
        .await
        .expect("failed to bind server");

    let mut session_handler: Option<SessionHandler> = None;

    loop {
        let finished = session_handler
            .as_mut()
            .map(|x| x.finished().boxed())
            .unwrap_or_else(|| future::pending().boxed());

        select! { biased;
            // check if session is finished if it's exist
            Ok(()) = finished => {
                session_handler.take();
            }

            // propagate to session if it's exist
            event = event_rx.recv() => {
                match (event, &mut session_handler) {
                    // propagate event to session
                    (Some(event), Some(session)) if session.is_connected() => { session.send_event(event).await.ok(); },
                    // stop server if channel is closed
                    (None, _) => break,
                    // drop event if we didn't have connected session
                    _ => (),
                }
            }

            Ok((stream, peer_addr)) = listener.accept() => {
                handle_incoming_connection(
                    tls_cert.clone(),
                    tls_key.clone(),
                    &mut session_handler,
                    stream, peer_addr
                ).await
            },
        }
    }
}

// Handle incoming connection, create a new session if it's not exist, otherwise
// drop the connection.
async fn handle_incoming_connection(
    tls_cert: Arc<Certificate>,
    tls_key: Arc<PrivateKey>,
    session_handler: &mut Option<SessionHandler>,
    stream: TcpStream,
    peer_addr: SocketAddr,
) {
    info!(?peer_addr, "received incoming connection");
    if session_handler.is_none() {
        let transporter = Transporter::Plain(Transport::new(stream));
        let handler = spawn_session(tls_cert, tls_key, peer_addr, transporter);
        *session_handler = Some(handler);
    } else {
        info!(?peer_addr, "dropping incoming connection")
    }
}

/// Handler to a session.
#[derive(Debug)]
struct SessionHandler {
    event_tx: mpsc::Sender<InputEvent>,
    task: JoinHandle<()>,
    state: Arc<Mutex<State>>,
}

impl SessionHandler {
    /// Send input event to this session.
    async fn send_event(&mut self, event: InputEvent) -> Result<(), SendError<InputEvent>> {
        self.event_tx.send(event).await?;
        Ok(())
    }

    /// This method is cancel safe.
    async fn finished(&mut self) -> Result<(), JoinError> {
        (&mut self.task).await
    }

    fn is_connected(&self) -> bool {
        let state = self.state.lock().unwrap();
        match &*state {
            State::Handshaking => false,
            State::Idle => true,
            State::RelayingEvent { .. } => true,
        }
    }
}

#[derive(Debug)]
struct Session {
    server_tls_cert: Arc<Certificate>,
    server_tls_key: Arc<PrivateKey>,
    peer_addr: SocketAddr,
    transporter: ServerTransporter,
    event_rx: mpsc::Receiver<InputEvent>,
    state: Arc<Mutex<State>>,
}

#[derive(Clone, Copy, Default, Debug)]
enum State {
    #[default]
    Handshaking,
    Idle,
    RelayingEvent {
        event: InputEvent,
    },
}

/// Creates a new session.
fn spawn_session(
    tls_cert: Arc<Certificate>,
    tls_key: Arc<PrivateKey>,
    peer_addr: SocketAddr,
    transporter: ServerTransporter,
) -> SessionHandler {
    let (event_tx, event_rx) = mpsc::channel(1);

    let state: Arc<Mutex<State>> = Default::default();

    let session = Session {
        server_tls_cert: tls_cert,
        server_tls_key: tls_key,
        peer_addr,
        transporter,
        event_rx,
        state: state.clone(),
    };

    let task = task::spawn(async move {
        // handle session error if any
        if let Err(err) = run_session(session).await {
            log_error!(err);
        };
    });

    SessionHandler {
        event_tx,
        task,
        state,
    }
}

/// The session loop.
async fn run_session(session: Session) -> Result<(), Error> {
    let Session {
        server_tls_cert,
        server_tls_key,
        peer_addr,
        mut transporter,
        mut event_rx,
        state: state_ref,
    } = session;

    loop {
        // copy state from the mutex
        let state = {
            let state = state_ref.lock().unwrap();
            *state
        };

        let new_state = match state {
            State::Handshaking => {
                let transport = transporter.plain()?;

                // wait for hello message
                let msg = transport.recv_msg().await?;
                let client_version =
                    if let ClientMessage::Hello(HelloMessage { client_version }) = msg {
                        client_version
                    } else {
                        bail!("expecting hello message, but was {:?}", msg);
                    };

                // check version
                let server_version = env!("CARGO_PKG_VERSION").to_owned();
                if client_version != server_version {
                    error!(?server_version, ?client_version, "version mismatch");

                    let msg: HelloReply = HelloReplyError::VersionMismatch.into();
                    transport.send_msg(msg.into()).await?;

                    break;
                }

                // request upgrade transport

                let server_tls_cert_hash = Sha256::from_bytes(server_tls_cert.as_ref().as_ref());
                let msg: HelloReply = UpgradeTransportRequest {
                    server_tls_cert_hash,
                }
                .into();
                transport.send_msg(msg.into()).await?;

                // wait for upgrade transport reply
                let msg = transport.recv_msg().await?;
                let client_tls_cert_hash =
                    if let ClientMessage::UpgradeTransportReply(UpgradeTransportResponse {
                        client_tls_cert_hash,
                    }) = msg
                    {
                        client_tls_cert_hash
                    } else {
                        bail!("expecting upgrade transport message, but was {:?}", msg);
                    };

                if !task::block_in_place(|| {
                    verify_client_cert(&peer_addr.ip(), &client_tls_cert_hash)
                })? {
                    debug!(?peer_addr, "client rejected");
                    break;
                }

                // upgrade to tls
                let server_tls_cert = server_tls_cert.as_ref().clone();
                let server_tls_key = server_tls_key.as_ref().clone();
                let client_tls_cert_hash = client_tls_cert_hash.clone();
                transporter = transporter
                    .upgrade(move |stream| {
                        upgrade_server_stream(
                            stream,
                            server_tls_cert,
                            server_tls_key,
                            client_tls_cert_hash,
                        )
                    })
                    .await?;

                info!("session established");

                println!("Connected to client at {}.", peer_addr.ip());

                State::Idle
            }

            State::Idle => {
                let transport = transporter.secure()?;

                select! { biased;
                    event = event_rx.recv() => {
                        match event {
                            Some(event) => {
                                State::RelayingEvent { event }
                            },
                            None => break,
                        }
                    }

                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        let closed = transport.is_closed().await;
                        if closed {
                            println!("Disconnected from client at {}.", peer_addr.ip());
                            break;
                        } else {
                            State::Idle
                        }
                    }
                }
            }

            State::RelayingEvent { event } => {
                let transport = transporter.secure()?;

                let msg = event.into();
                transport
                    .send_msg(msg)
                    .await
                    .context("failed to send message")?;

                State::Idle
            }
        };

        // replace state in the mutex with the new state
        {
            let mut state = state_ref.lock().unwrap();
            *state = new_state;
        }
    }

    Result::<_, Error>::Ok(())
}

fn verify_client_cert(client_addr: &IpAddr, client_cert: &Sha256) -> Result<bool, Error> {
    {
        let mut stdout = std::io::stdout();
        write!(
            stdout,
            "Accept client at {} with TLS certificate hash {}? y/(n): ",
            client_addr, client_cert
        )?;
        stdout.flush()?;
    }

    let mut buf = String::new();
    std::io::stdin().read_line(&mut buf)?;
    let answer = buf.trim();
    Ok(answer == "y" || answer == "Y")
}

async fn upgrade_server_stream<S>(
    stream: S,
    server_tls_cert: Certificate,
    server_tls_key: PrivateKey,
    client_tls_cert_hash: Sha256,
) -> Result<TlsStream<S>, Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tls: TlsAcceptor = {
        let client_cert_verifier = Arc::new(SingleCertVerifier::new(client_tls_cert_hash));

        let server_cert = rustls::Certificate(server_tls_cert.into());
        let server_private_key = rustls::PrivateKey(server_tls_key.into());

        let cfg = ServerConfig::builder()
            .with_safe_defaults()
            .with_client_cert_verifier(client_cert_verifier)
            .with_single_cert(vec![server_cert], server_private_key)
            .context("failed to create server config tls")?;
        Arc::new(cfg).into()
    };

    let stream = tls.accept(stream).await.context("tls accept failed")?;

    Ok(stream.into())
}
