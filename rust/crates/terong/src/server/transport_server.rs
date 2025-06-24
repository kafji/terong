use crate::{
    tls::create_tls_acceptor,
    transport::{
        Certificate, PrivateKey, Transport,
        protocol::{ClientMessage, HeartbeatTimers, InputEvent, Ping, ServerMessage},
    },
};
use anyhow::{Context, Error};
use futures::{FutureExt, future};
use std::{fmt::Debug, net::SocketAddr};
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc::{self, error::SendError},
    task::{self, JoinError, JoinHandle},
};
use tokio_rustls::TlsAcceptor;
use tracing::{error, info};

type ServerTransport = Transport<ClientMessage, ServerMessage>;

#[derive(Debug, Clone)]
pub struct TransportServer {
    pub port: u16,
    pub tls_certs: Vec<Certificate>,
    pub tls_key: PrivateKey,
    pub tls_root_certs: Vec<Certificate>,
}

pub fn start(args: TransportServer, event_rx: mpsc::Receiver<InputEvent>) -> JoinHandle<()> {
    task::spawn(run_transport(args, event_rx))
}

async fn run_transport(args: TransportServer, mut event_rx: mpsc::Receiver<InputEvent>) {
    let tls_acceptor = create_tls_acceptor(
        &args.tls_certs[0].0,
        &args.tls_key.0,
        &args.tls_root_certs[0].0,
    );

    let server_addr = format!("0.0.0.0:{}", args.port);

    info!(server_address = server_addr, "listening");

    let listener = TcpListener::bind(server_addr)
        .await
        .expect("failed to bind to server address");

    let mut session_handle: Option<SessionHandle> = None;
    loop {
        let finished = session_handle
            .as_mut()
            .map(|session| session.finished().boxed())
            .unwrap_or_else(|| future::pending().boxed());

        select! {
            // check if session is finished if it exists
            Ok(()) = finished => {
                session_handle.take();
            }

            // propagate to session if it exists
            event = event_rx.recv() => {
                match (event, &mut session_handle) {
                    // propagate event to session
                    (Some(event), Some(session)) => {
                        session.send_event(event).await.ok();
                    },
                    // stop server if channel is closed
                    (None, _) => break,
                    // drop event if we didn't have active session
                    _ => (),
                }
            }

            Ok((connection, peer_addr)) = listener.accept() => {
                match handle_incoming_connection(
                    &mut session_handle,
                    connection,
                    peer_addr,
                    &tls_acceptor,
                ).await {
                    Ok(_) => (),
                    Err(err) => error!(
                        peer_address = %peer_addr,
                        error = %err,
                        "failed to handle incoming connection",
                    ),
                }
            },
        }
    }
}

// Handle incoming connection, create a new session if it's not exist, otherwise
// drop the connection.
async fn handle_incoming_connection(
    session_handle: &mut Option<SessionHandle>,
    connection: TcpStream,
    peer_addr: SocketAddr,
    tls_acceptor: &TlsAcceptor,
) -> Result<(), anyhow::Error> {
    info!(peer_address = %peer_addr, "received incoming connection");
    if session_handle.is_none() {
        let stream = tls_acceptor.accept(connection).await?;
        let transport = Transport::new(stream);

        let handler = spawn_session(peer_addr, transport);
        *session_handle = Some(handler);
    } else {
        info!(peer_address = %peer_addr, "dropping incoming connection");
    }
    Ok(())
}

/// Handler to a session.
#[derive(Debug)]
struct SessionHandle {
    event_tx: mpsc::Sender<InputEvent>,
    task: JoinHandle<()>,
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
}

#[derive(Debug)]
struct Session {
    transport: ServerTransport,
    event_rx: mpsc::Receiver<InputEvent>,
}

/// Creates a new session.
fn spawn_session(peer_addr: SocketAddr, transport: ServerTransport) -> SessionHandle {
    let (event_tx, event_rx) = mpsc::channel(1);

    let session = Session {
        transport,
        event_rx,
    };

    let task = task::spawn(async move {
        // handle session error if any
        if let Err(err) = run_session(session).await {
            error!(error = ?err);
        };
        info!("session terminated");
        info!(peer_address = %peer_addr, "disconnected from client");
    });

    SessionHandle { event_tx, task }
}

/// The session loop.
async fn run_session(session: Session) -> Result<(), Error> {
    let Session {
        mut transport,
        mut event_rx,
    } = session;

    let mut timers = HeartbeatTimers::new();

    loop {
        select! {

            // recv heartbeat deadline
            _ = timers.recv_deadline() => {
                info!("haven't heard any message from client for {} secs, terminating session", timers.timeout().as_secs());
                break;
            }

            // send heartbeat deadline
            _ = timers.send_deadline() => {
                transport
                    .send_msg(ServerMessage::Ping(Ping {}))
                    .await
                    .context("failed to send ping message")?;
                // reset send heartbeat deadline after receiving any message
                timers.reset_send_deadline();
            }

            // receive and handle client messages
            Ok(msg) = transport.recv_msg() => {
                // reset recv heartbeat deadline after receiving any message
                timers.reset_recv_deadline();
                match msg {
                    ClientMessage::Ping(Ping {}) => {
                    },
                }
            }

            // forward events
            event = event_rx.recv() => {
                match event {
                    Some(event) => {
                        transport
                            .send_msg(event.into())
                            .await
                            .context("failed to send event message")?;
                        // reset send heartbeat deadline after receiving any message
                        timers.reset_send_deadline();
                    },
                    None => {
                        info!("terminating session, event channel was closed");
                        break;
                    },
                }
            }
        }
    }

    Ok(())
}
