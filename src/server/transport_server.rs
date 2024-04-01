use crate::{
    log_error,
    transport::{
        protocol::{ClientMessage, InputEvent, Ping, Pong, ServerMessage},
        Certificate, PrivateKey, SingleCertVerifier, Transport, Transporter,
    },
};
use anyhow::{Context, Error};
use futures::{future, FutureExt};
use std::{
    fmt::Debug,
    net::{SocketAddr, SocketAddrV4},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc::{self, error::SendError},
    task::{self, JoinError, JoinHandle},
    time::{interval_at, Instant, MissedTickBehavior},
};
use tokio_rustls::{rustls::ServerConfig, TlsAcceptor, TlsStream};
use tracing::{debug, error, info};

type ServerTransporter = Transporter<TcpStream, TlsStream<TcpStream>, ClientMessage, ServerMessage>;

#[derive(Debug)]
pub struct TransportServer {
    pub port: u16,

    pub tls_certs: Vec<Certificate>,
    pub tls_key: PrivateKey,

    pub event_rx: mpsc::Receiver<InputEvent>,

    pub client_tls_certs: Vec<Certificate>,
}

pub fn start(args: TransportServer) -> JoinHandle<()> {
    task::spawn(async move { run(args).await })
}

async fn run(args: TransportServer) {
    let TransportServer {
        port,
        tls_certs,
        tls_key,
        mut event_rx,
        client_tls_certs,
    } = args;

    let tls_config = {
        let tls = create_server_tls_config(
            tls_certs,
            tls_key,
            client_tls_certs.into_iter().last().unwrap(),
        )
        .unwrap();
        Arc::new(tls)
    };

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
                    tls_config.clone(),
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
    tls_config: Arc<ServerConfig>,
    session_handler: &mut Option<SessionHandler>,
    stream: TcpStream,
    peer_addr: SocketAddr,
) {
    info!(?peer_addr, "received incoming connection");
    if session_handler.is_none() {
        let transporter = Transporter::Plain(Transport::new(stream));
        let handler = spawn_session(tls_config, peer_addr, transporter);
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
    state: Arc<Mutex<SessionState>>,
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
            SessionState::Handshaking => false,
            SessionState::Idle => true,
            SessionState::RelayingEvent { .. } => true,
        }
    }
}

struct Session {
    tls_config: Arc<ServerConfig>,

    peer_addr: SocketAddr,

    transporter: ServerTransporter,

    event_rx: mpsc::Receiver<InputEvent>,

    state: Arc<Mutex<SessionState>>,
}

#[derive(Clone, Copy, Default, Debug)]
enum SessionState {
    #[default]
    Handshaking,
    Idle,
    RelayingEvent {
        event: InputEvent,
    },
}

/// Creates a new session.
fn spawn_session(
    tls_config: Arc<ServerConfig>,
    peer_addr: SocketAddr,
    transporter: ServerTransporter,
) -> SessionHandler {
    let (event_tx, event_rx) = mpsc::channel(1);

    let state: Arc<Mutex<SessionState>> = Default::default();

    let session = Session {
        tls_config,
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

        info!("session terminated");

        info!(?peer_addr, "disconnected from client");
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
        tls_config,
        peer_addr,
        mut transporter,
        mut event_rx,
        state: state_ref,
    } = session;

    let ping_ticker_interval = Duration::from_secs(60);
    let mut ping_ticker = {
        let mut ticker = interval_at(Instant::now() + ping_ticker_interval, ping_ticker_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker
    };
    let mut local_ping_counter = 1;

    loop {
        // copy state from the mutex
        let state = {
            let state = state_ref.lock().unwrap();
            *state
        };

        let new_state = match state {
            SessionState::Handshaking => {
                debug!(?peer_addr, "upgrading to secure transport");

                // upgrade to tls
                transporter = {
                    let tls_config = tls_config.clone();
                    transporter
                        .upgrade(move |stream| upgrade_server_stream(stream, tls_config))
                        .await?
                };

                debug!(?peer_addr, "connection upgraded");

                info!(?peer_addr, "session established");

                SessionState::Idle
            }

            SessionState::Idle => {
                let transport = transporter.secure()?;

                select! { biased;

                    _ = ping_ticker.tick() => {
                        debug!("ping ticker ticks");

                        if local_ping_counter % 2 == 1 {
                            // it has been a tick since last ping-pong or since the session was established
                            // yet server has not receive ping from client
                            info!("haven't heard ping from client for {} secs, terminating session", ping_ticker_interval.as_secs());
                            break;
                        }

                        SessionState::Idle
                    }

                    Ok(msg) = transport.recv_msg() => {
                        match msg {
                            ClientMessage::Ping(Ping { counter }) => {
                                if counter == local_ping_counter {
                                    debug!("received ping, incrementing local counter");
                                    local_ping_counter += 1;

                                    let msg = ServerMessage::Pong(Pong { counter: local_ping_counter });
                                    match transport.send_msg(msg).await {
                                        Ok(_) => (),
                                        Err(err) => {
                                            error!("failed to send pong, {:?}", err);
                                            break;
                                        },
                                    }
                                    debug!("pong sent successfully, incrementing local counter, resetting ticker");
                                    local_ping_counter +=1;
                                    ping_ticker.reset();

                                    SessionState::Idle
                                } else {
                                    // received ping from client, but counter is mismatch
                                    info!("terminating session, ping counter mismatch");
                                    break;
                                }
                            },
                        }
                    }

                    event = event_rx.recv() => {
                        match event {
                            Some(event) => SessionState::RelayingEvent { event },
                            None => {
                                info!("terminating session, event channel was closed");
                                break;
                            },
                        }
                    }
                }
            }

            SessionState::RelayingEvent { event } => {
                let transport = transporter.secure()?;

                transport
                    .send_msg(event.into())
                    .await
                    .context("failed to send message")?;

                SessionState::Idle
            }
        };

        // replace state in the mutex with the new state
        {
            let mut state = state_ref.lock().unwrap();
            *state = new_state;
        }
    }

    Ok(())
}

async fn upgrade_server_stream<S>(
    stream: S,
    tls_config: Arc<ServerConfig>,
) -> Result<TlsStream<S>, Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tls: TlsAcceptor = tls_config.into();

    let stream = tls.accept(stream).await.context("tls accept failed")?;

    Ok(stream.into())
}

fn create_server_tls_config(
    server_certs: Vec<Certificate>,
    server_key: PrivateKey,
    client_cert: Certificate,
) -> Result<ServerConfig, Error> {
    let cert_verifier = Arc::new(SingleCertVerifier::new(client_cert));

    let cfg = ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(cert_verifier)
        .with_single_cert(
            server_certs
                .into_iter()
                .map(|x| rustls::Certificate(x.into()))
                .collect(),
            rustls::PrivateKey(server_key.into()),
        )
        .context("failed to create server config tls")?;

    Ok(cfg)
}
