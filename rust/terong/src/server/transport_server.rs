use crate::{
    log_error,
    transport::{
        protocol::{ClientMessage, InputEvent, Ping, Pong, ServerMessage},
        Certificate, PrivateKey, Transport,
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
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc::{self, error::SendError},
    task::{self, JoinError, JoinHandle},
    time::{interval_at, Instant, MissedTickBehavior},
};
use tokio_native_tls::native_tls;
use tracing::{debug, error, info};

type ServerTransport = Transport<ClientMessage, ServerMessage>;

#[derive(Debug, Clone)]
pub struct TransportServer {
    pub port: u16,
    pub tls_certs: Vec<Certificate>,
    pub tls_key: PrivateKey,

    #[allow(unused)]
    pub client_tls_certs: Vec<Certificate>,
}

pub fn start(args: TransportServer, event_rx: mpsc::Receiver<InputEvent>) -> JoinHandle<()> {
    task::spawn(run_transport(args, event_rx))
}

async fn run_transport(args: TransportServer, mut event_rx: mpsc::Receiver<InputEvent>) {
    let identity = native_tls::Identity::from_pkcs8(&args.tls_certs[0].0, &args.tls_key.0).unwrap();
    // schannel doesn't provide client cert verification https://learn.microsoft.com/en-us/windows/win32/secauthn/performing-authentication-using-schannel
    // and native-tls doesn't allow injecting peer verification
    let tls_acceptor = native_tls::TlsAcceptor::builder(identity)
        .build()
        .unwrap()
        .into();

    let server_addr = SocketAddrV4::new([0, 0, 0, 0].into(), args.port);

    info!("listening at {}", server_addr);
    let listener = TcpListener::bind(server_addr)
        .await
        .expect("failed to bind server");

    let mut session_handler: Option<SessionHandle> = None;

    loop {
        let finished = session_handler
            .as_mut()
            .map(|x| x.finished().boxed())
            .unwrap_or_else(|| future::pending().boxed());

        select! { biased;
            // check if session is finished if it exists
            Ok(()) = finished => {
                session_handler.take();
            }

            // propagate to session if it exists
            event = event_rx.recv() => {
                match (event, &mut session_handler) {
                    // propagate event to session
                    (Some(event), Some(session)) if session.is_connected() => { session.send_event(event).await.ok(); },
                    // stop server if channel is closed
                    (None, _) => break,
                    // drop event if we didn't have active session
                    _ => (),
                }
            }

            Ok((stream, peer_addr)) = listener.accept() => {
                handle_incoming_connection(
                    &mut session_handler,
                    stream,
                    peer_addr,
                    &tls_acceptor,
                ).await
            },
        }
    }
}

// Handle incoming connection, create a new session if it's not exist, otherwise
// drop the connection.
async fn handle_incoming_connection(
    session_handler: &mut Option<SessionHandle>,
    stream: TcpStream,
    peer_addr: SocketAddr,
    tls_acceptor: &tokio_native_tls::TlsAcceptor,
) {
    info!(?peer_addr, "received incoming connection");
    if session_handler.is_none() {
        let stream = tls_acceptor.accept(stream).await.unwrap();
        let transport = Transport::new(stream);

        let handler = spawn_session(peer_addr, transport);
        *session_handler = Some(handler);
    } else {
        info!(?peer_addr, "dropping incoming connection")
    }
}

/// Handler to a session.
#[derive(Debug)]
struct SessionHandle {
    event_tx: mpsc::Sender<InputEvent>,
    task: JoinHandle<()>,
    state: Arc<Mutex<SessionState>>,
}

impl SessionHandle {
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
            SessionState::Idle => true,
            SessionState::RelayingEvent { .. } => true,
        }
    }
}

#[derive(Debug)]
struct Session {
    transport: ServerTransport,
    event_rx: mpsc::Receiver<InputEvent>,
    state: Arc<Mutex<SessionState>>,
}

#[derive(Clone, Copy, Default, Debug)]
enum SessionState {
    #[default]
    Idle,
    RelayingEvent {
        event: InputEvent,
    },
}

/// Creates a new session.
fn spawn_session(peer_addr: SocketAddr, transport: ServerTransport) -> SessionHandle {
    let (event_tx, event_rx) = mpsc::channel(1);

    let state: Arc<Mutex<SessionState>> = Default::default();

    let session = Session {
        transport,
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

    SessionHandle {
        event_tx,
        task,
        state,
    }
}

/// The session loop.
async fn run_session(session: Session) -> Result<(), Error> {
    let Session {
        mut transport,
        mut event_rx,
        state: state_ref,
    } = session;

    let ping_ticker_interval = Duration::from_secs(20);
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
            SessionState::Idle => {
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
                                            error!(?err, "failed to send pong");
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
