use crate::{
    log_error,
    transport::{
        protocol::{ClientMessage, HelloMessage, HelloReply, InputEvent, ServerMessage},
        Certificate, PrivateKey, SingleCertVerifier, Transport, Transporter,
    },
};
use anyhow::{bail, Context, Error};
use rustls::{ClientConfig, ServerName};
use std::{
    env,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    select,
    sync::mpsc,
    task::{self, JoinHandle},
    time::sleep,
};
use tokio_rustls::{TlsConnector, TlsStream};
use tracing::{debug, info};

type ClientTransporter = Transporter<TcpStream, TlsStream<TcpStream>, ServerMessage, ClientMessage>;

#[derive(Debug)]
pub struct TransportClient {
    pub server_addr: SocketAddr,

    pub tls_certs: Vec<Certificate>,
    pub tls_key: PrivateKey,

    pub server_tls_certs: Vec<Certificate>,

    pub event_tx: mpsc::Sender<InputEvent>,
}

pub fn start(args: TransportClient) -> JoinHandle<()> {
    task::spawn(async move { run_transport_client(args).await })
}

async fn run_transport_client(args: TransportClient) {
    let TransportClient {
        server_addr,
        event_tx,
        tls_certs,
        tls_key,
        server_tls_certs,
    } = args;

    let tls_config = {
        let tls = create_client_tls_config(
            tls_certs,
            tls_key,
            server_tls_certs.into_iter().last().unwrap(),
        )
        .unwrap();
        Arc::new(tls)
    };

    loop {
        if let Err(err) = connect(&server_addr, tls_config.clone(), &event_tx).await {
            log_error!(err);

            sleep(Duration::from_secs(5)).await;
        }
    }
}

async fn connect(
    server_addr: &SocketAddr,
    tls_config: Arc<ClientConfig>,
    event_tx: &mpsc::Sender<InputEvent>,
) -> Result<(), Error> {
    info!(?server_addr, "connecting to server");

    let stream = TcpStream::connect(server_addr)
        .await
        .context("failed to connect to the server")?;

    info!(?server_addr, "connected to server");

    let transporter: ClientTransporter = Transporter::Plain(Transport::new(stream));

    let session = Session {
        server_addr,
        tls_config,
        event_tx,
        transporter,
        state: Default::default(),
    };

    let r = run_session(session).await;

    info!(?server_addr, "disconnected from server");

    r
}

struct Session<'a> {
    server_addr: &'a SocketAddr,
    tls_config: Arc<ClientConfig>,
    event_tx: &'a mpsc::Sender<InputEvent>,
    transporter: ClientTransporter,
    state: SessionState,
}

#[derive(Clone, Copy, Default, Debug)]
pub enum SessionState {
    #[default]
    Handshaking,
    Idle,
    EventRelayed {
        event: InputEvent,
    },
}

async fn run_session(session: Session<'_>) -> Result<(), Error> {
    let Session {
        server_addr,
        tls_config,
        event_tx,
        mut transporter,
        mut state,
    } = session;

    loop {
        state = match state {
            SessionState::Handshaking => {
                let client_version = env!("CARGO_PKG_VERSION").into();

                debug!(?server_addr, ?client_version, "handshaking");

                let transport = transporter.plain()?;

                // send hello message
                let msg = HelloMessage { client_version };
                transport.send_msg(msg.into()).await?;

                // wait for hello reply
                let msg = transport.recv_msg().await?;
                match msg {
                    ServerMessage::HelloReply(reply) => {
                        if let HelloReply::Err(err) = reply {
                            bail!("handshake fail, {:?}", err)
                        }
                    }
                    _ => bail!("received unexpected message, {:?}", msg),
                }

                debug!(?server_addr, "upgrading to secure transport");

                // upgrade to tls
                transporter = {
                    let tls_config = tls_config.clone();
                    transporter
                        .upgrade(move |stream| async move {
                            upgrade_client_stream(stream, tls_config, server_addr.ip()).await
                        })
                        .await?
                };

                debug!(?server_addr, "connection upgraded");

                info!(?server_addr, "session established");

                SessionState::Idle
            }

            SessionState::Idle => {
                let transport = transporter.secure()?;

                select! { biased;
                    Ok(msg) = transport.recv_msg() => {
                        let event = match msg {
                            ServerMessage::Event(event) => event,
                            _ => bail!("received unexpected message, {:?}", msg),
                        };

                        SessionState::EventRelayed { event }
                    }

                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        let closed = transport.is_closed().await;

                        debug!(?closed, "client connection status");

                        if closed {
                            break;
                        } else {
                            SessionState::Idle
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

async fn upgrade_client_stream<S>(
    stream: S,
    tls_config: Arc<ClientConfig>,
    server_addr: IpAddr,
) -> Result<TlsStream<S>, Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tls: TlsConnector = tls_config.into();

    let stream = tls
        .connect(ServerName::IpAddress(server_addr), stream)
        .await
        .context("tls connect failed")?;

    Ok(stream.into())
}

fn create_client_tls_config(
    client_certs: Vec<Certificate>,
    client_key: PrivateKey,
    server_cert: Certificate,
) -> Result<ClientConfig, Error> {
    let cert_verifier = Arc::new(SingleCertVerifier::new(server_cert));

    let mut cfg = ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(cert_verifier)
        .with_single_cert(
            client_certs
                .into_iter()
                .map(|x| rustls::Certificate(x.into()))
                .collect(),
            rustls::PrivateKey(client_key.into()),
        )
        .context("failed to create client config tls")?;

    cfg.enable_sni = false;

    Ok(cfg)
}
