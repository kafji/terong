use crate::{
    log_error,
    transport::{
        protocol::{ClientMessage, InputEvent, Ping, Pong, ServerMessage},
        Certificate, PrivateKey, Transport,
    },
};
use anyhow::Error;
use macross::impl_from;
use std::{fmt, net::SocketAddr, time::Duration};
use tokio::{
    net::TcpStream,
    select,
    sync::mpsc,
    task::{self, JoinHandle},
    time::{interval_at, sleep, Instant, MissedTickBehavior},
};
use tokio_native_tls::native_tls;
use tracing::{debug, error, info};

/// Time it takes before client giving up on connecting to the server.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

type ClientTransport = Transport<ServerMessage, ClientMessage>;

#[derive(Debug, Clone)]
pub struct TransportClient {
    pub server_addr: SocketAddr,
    pub tls_certs: Vec<Certificate>,
    pub tls_key: PrivateKey,
    pub server_tls_certs: Vec<Certificate>,
}

pub fn start(args: TransportClient, event_tx: mpsc::Sender<InputEvent>) -> JoinHandle<()> {
    task::spawn(run_transport(args, event_tx))
}

async fn run_transport(args: TransportClient, event_tx: mpsc::Sender<InputEvent>) {
    let identity = native_tls::Identity::from_pkcs8(&args.tls_certs[0].0, &args.tls_key.0).unwrap();
    let server_cert = native_tls::Certificate::from_pem(&args.server_tls_certs[0].0).unwrap();
    let tls_connector = native_tls::TlsConnector::builder()
        .identity(identity)
        .disable_built_in_roots(true)
        .add_root_certificate(server_cert)
        .build()
        .unwrap()
        .into();

    let mut retry_count = 0;

    loop {
        if let Err(err) = connect(
            &args.server_addr,
            &event_tx,
            &mut retry_count,
            &tls_connector,
        )
        .await
        {
            log_error!(err);

            if retry_count >= 5 {
                info!("giving up after {} retries", retry_count);
                break;
            }

            retry_count += 1;
            debug!("retry count incremented to {}", retry_count);

            let delay = Duration::from_secs(10);
            info!("reconnecting in {} secs ({})", delay.as_secs(), retry_count);
            sleep(delay).await;
        }
    }
}

#[derive(Debug)]
enum ConnectError {
    Timeout { msg: String },
    Other(Error),
}

impl_from!(ConnectError, {
    Self::Other => Error,
});

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectError::Timeout { msg } => f.write_str(msg),
            ConnectError::Other(err) => f.write_str(&err.to_string()),
        }
    }
}

impl std::error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConnectError::Other(err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

async fn connect(
    server_addr: &SocketAddr,
    event_tx: &mpsc::Sender<InputEvent>,
    retry_count: &mut u8,
    tls_connector: &tokio_native_tls::TlsConnector,
) -> Result<(), ConnectError> {
    info!(?server_addr, "connecting to server");

    let stream = select! { biased;
        Ok(stream) = TcpStream::connect(server_addr) => {
            stream
        }

        _ = tokio::time::sleep(CONNECT_TIMEOUT) => {
            let msg = format!("failed to connect to the server after {} secs", CONNECT_TIMEOUT.as_secs());
            return Err(ConnectError::Timeout{ msg });
        }
    };

    info!(?server_addr, "connected to server");

    *retry_count = 0;
    debug!("retry count reset to zero");

    let stream = tls_connector.connect("", stream).await.unwrap();
    let transport: ClientTransport = Transport::new(stream);

    let session = Session {
        event_tx,
        transporter: transport,
        state: Default::default(),
    };
    let result = run_session(session).await;

    info!(?server_addr, "disconnected from server");

    result?;

    Ok(())
}

#[derive(Debug)]
struct Session<'a> {
    event_tx: &'a mpsc::Sender<InputEvent>,
    transporter: ClientTransport,
    state: SessionState,
}

#[derive(Clone, Copy, Default, Debug)]
pub enum SessionState {
    #[default]
    Idle,
    EventRelayed {
        event: InputEvent,
    },
}

async fn run_session(session: Session<'_>) -> Result<(), Error> {
    let Session {
        event_tx,
        transporter: mut transport,
        mut state,
    } = session;

    let ping_ticker_interval = Duration::from_secs(15);
    let mut ping_ticker = {
        let mut ticker = interval_at(Instant::now() + ping_ticker_interval, ping_ticker_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker
    };

    let mut local_ping_counter = 1;

    loop {
        state = match state {
            SessionState::Idle => {
                select! { biased;

                    _ = ping_ticker.tick() => {
                        debug!("ping ticker ticks");

                        if local_ping_counter % 2 == 1 {
                            // odd, client send ping
                            let msg = Ping { counter: local_ping_counter }.into();
                            match transport.send_msg(msg).await {
                                Ok(_) => {
                                    debug!("ping sent successfully, incrementing local counter");
                                    local_ping_counter += 1;
                                    SessionState::Idle
                                },
                                Err(err) => {
                                    error!(?err, "failed to send ping");
                                    break;
                                },
                            }
                        } else {
                            // client has sent ping a tick before
                            // but client has not receive pong from server
                            info!("haven't heard pong from server for {} secs, terminating session", ping_ticker_interval.as_secs());
                            break;
                        }
                    }

                    Ok(msg) = transport.recv_msg() => {
                        debug!("received message, {:?}", msg);

                        let event = match msg {
                            ServerMessage::Event(event) => Some(event),
                            ServerMessage::Pong(Pong { counter })=> {
                                if counter == local_ping_counter {
                                    debug!("received pong, incrementing local counter, resetting ticker");
                                    local_ping_counter += 1;
                                    ping_ticker.reset();
                                    None
                                } else {
                                    // received pong from server, but counter is mismatch
                                    info!("terminating session, ping counter mismatch");
                                    break;
                                }
                            },
                        };

                        match event {
                            Some(event) => SessionState::EventRelayed { event },
                            None => SessionState::Idle
                        }
                    }
                }
            }

            SessionState::EventRelayed { event } => {
                // propagate event to input sink
                event_tx.send(event).await?;

                SessionState::Idle
            }
        };
    }

    Ok(())
}
