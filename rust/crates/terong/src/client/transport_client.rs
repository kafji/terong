use crate::{
    tls::create_tls_connector,
    transport::{
        Certificate, PrivateKey, Transport,
        protocol::{ClientMessage, HeartbeatTimers, InputEvent, Ping, ServerMessage},
    },
    typing::impl_from,
};
use anyhow::{Context, Error};
use std::{fmt, net::SocketAddr, time::Duration};
use tokio::{
    net::TcpStream,
    select,
    sync::mpsc,
    task::{self, JoinHandle},
    time::sleep,
};
use tracing::{debug, error, info};

/// Time it takes before client giving up on connecting to the server.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Interval between reconnecting attempt.
const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);

type ClientTransport = Transport<ServerMessage, ClientMessage>;

#[derive(Debug, Clone)]
pub struct TransportClient {
    pub server_addr: SocketAddr,
    pub tls_certs: Vec<Certificate>,
    pub tls_key: PrivateKey,
    pub tls_root_certs: Vec<Certificate>,
}

pub fn start(args: TransportClient, event_tx: mpsc::Sender<InputEvent>) -> JoinHandle<()> {
    task::spawn(run_transport(args, event_tx))
}

async fn run_transport(args: TransportClient, event_tx: mpsc::Sender<InputEvent>) {
    let tls_connector = create_tls_connector(
        &args.tls_certs[0].0,
        &args.tls_key.0,
        &args.tls_root_certs[0].0,
    );

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
            error!(error = ?err);

            if retry_count >= 5 {
                info!("giving up after {} retries", retry_count);
                break;
            }

            retry_count += 1;
            debug!("retry count incremented to {}", retry_count);

            info!(
                "reconnecting in {} secs ({})",
                RECONNECT_INTERVAL.as_secs(),
                retry_count
            );
            sleep(RECONNECT_INTERVAL).await;
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
    tls_connector: &tokio_rustls::TlsConnector,
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

    let stream = tls_connector
        .connect(
            rustls_pki_types::ServerName::IpAddress(server_addr.ip().into()),
            stream,
        )
        .await
        .unwrap();
    let transport: ClientTransport = Transport::new(stream);

    let session = Session {
        event_tx,
        transporter: transport,
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
}

async fn run_session(session: Session<'_>) -> Result<(), Error> {
    let Session {
        event_tx,
        transporter: mut transport,
    } = session;

    let mut timers = HeartbeatTimers::new();

    loop {
        select! {

            // recv heartbeat deadline
            _ = timers.recv_deadline() => {
                info!("haven't heard any message from server for {} secs, terminating session", timers.timeout().as_secs());
                break;
            }

            // send heartbeat deadline
            _ = timers.send_deadline() => {
                transport
                    .send_msg(ClientMessage::Ping(Ping {}))
                    .await
                    .context("failed to send ping message")?;
                // reset send heartbeat deadline after receiving any message
                timers.reset_send_deadline();
            }

            Ok(msg) = transport.recv_msg() => {
                // reset recv heartbeat deadline after receiving any message
                timers.reset_recv_deadline();
                match msg {
                    ServerMessage::Event(event) => {
                        event_tx.send(event).await?;
                    },
                    ServerMessage::Ping(Ping {}) => {
                    },
                };
            }
        }
    }

    Ok(())
}
