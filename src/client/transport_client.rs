use crate::{
    config::no_tls,
    transport::protocol::{
        ClientMessage, HelloMessage, HelloReply, InputEvent, ServerMessage, Sha256,
        UpgradeTransportRequest, UpgradeTransportResponse,
    },
    transport::{
        generate_tls_key_pair, Certificate, PrivateKey, SingleCertVerifier, Transport, Transporter,
    },
};
use anyhow::{bail, Context, Error};
use rustls::{ClientConfig, ServerName};
use std::{
    env,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::mpsc,
    task::{self, JoinHandle},
};
use tokio_rustls::{TlsConnector, TlsStream};
use tracing::{debug, error, info, warn};

type ClientTransporter = Transporter<TcpStream, TlsStream<TcpStream>, ServerMessage, ClientMessage>;

pub fn start(server_addr: SocketAddr, event_tx: mpsc::Sender<InputEvent>) -> JoinHandle<()> {
    task::spawn(async move { run_client(server_addr, event_tx).await })
}

async fn run_client(server_addr: SocketAddr, event_tx: mpsc::Sender<InputEvent>) {
    loop {
        if let Err(err) = connect(server_addr, &event_tx).await {
            error!("{}", err);
        }
    }
}

async fn connect(
    server_addr: SocketAddr,
    event_tx: &mpsc::Sender<InputEvent>,
) -> Result<(), Error> {
    // open connection with the server
    info!(?server_addr, "connecting to server");

    let stream = TcpStream::connect(server_addr)
        .await
        .context("failed to connect to the server")?;

    info!(?server_addr, "connected to server");

    let transporter: ClientTransporter = Transporter::Plain(Transport::new(stream));

    let session = Session {
        server_addr,
        transporter,
        event_tx,
        state: Default::default(),
    };
    run_session(session).await?;

    Ok(())
}

#[derive(Debug)]
struct Session<'a> {
    server_addr: SocketAddr,
    transporter: ClientTransporter,
    event_tx: &'a mpsc::Sender<InputEvent>,
    state: SessionState,
}

#[derive(Clone, Copy, Default, Debug)]
pub enum SessionState {
    #[default]
    Handshaking,
    Established,
}

async fn run_session(session: Session<'_>) -> Result<(), Error> {
    let Session {
        server_addr,
        mut transporter,
        event_tx,
        mut state,
    } = session;

    loop {
        debug!(?state);

        state = match state {
            SessionState::Handshaking => {
                // get transport
                let transport = transporter.plain()?;

                // send hello message
                let client_version = env!("CARGO_PKG_VERSION").into();
                let msg = HelloMessage { client_version };
                transport.send_msg(msg.into()).await?;

                // wait for hello reply
                let msg = transport.recv_msg().await?;
                let server_tls_cert = match msg {
                    ServerMessage::HelloReply(reply) => match reply {
                        HelloReply::Ok(UpgradeTransportRequest {
                            server_tls_cert_hash,
                        }) => server_tls_cert_hash,
                        HelloReply::Err(err) => {
                            bail!("handshake fail, {:?}", err)
                        }
                    },
                    _ => bail!("received unexpected message, {:?}", msg),
                };

                // generate tls key pair for this session
                debug!("generating tls key pair");
                let (client_tls_cert, client_tls_key) =
                    generate_tls_key_pair("192.168.123.205".parse().unwrap())
                        .context("failed to generate tls key pair")?;

                // send client tls certificate
                let msg = UpgradeTransportResponse {
                    client_tls_cert_hash: Sha256::from_bytes(client_tls_cert.as_ref()),
                };
                transport.send_msg(msg.into()).await?;

                // upgrade to tls
                let no_tls = no_tls();
                if no_tls {
                    warn!("tls disabled")
                } else {
                    transporter = transporter
                        .upgrade(move |stream| async move {
                            upgrade_client_stream(
                                stream,
                                client_tls_cert,
                                client_tls_key,
                                server_tls_cert,
                                server_addr.ip(),
                            )
                            .await
                        })
                        .await?;
                    info!(?server_addr, "connection upgraded");
                }

                info!("session established");
                SessionState::Established
            }

            SessionState::Established => {
                let messenger = transporter.any();

                debug!("waiting for message");
                let msg = messenger
                    .recv_msg()
                    .await
                    .context("failed to receive message")?;

                debug!(?msg, "received message");

                let event = match msg {
                    ServerMessage::Event(event) => event,
                    _ => bail!("received unexpected message, {:?}", msg),
                };

                // propagate event to input sink
                event_tx.send(event).await?;

                SessionState::Established
            }
        };
    }
}

async fn upgrade_client_stream<S>(
    stream: S,
    client_tls_cert: Certificate,
    client_tls_key: PrivateKey,
    server_tls_cert_hash: Sha256,
    server_addr: IpAddr,
) -> Result<TlsStream<S>, Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tls: TlsConnector = {
        let server_cert_verifier = Arc::new(SingleCertVerifier::new(server_tls_cert_hash));

        let client_cert = rustls::Certificate(client_tls_cert.into());
        let client_private_key = rustls::PrivateKey(client_tls_key.into());

        let cfg = ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(server_cert_verifier)
            .with_single_cert(vec![client_cert], client_private_key)
            .context("failed to create client config tls")?;
        Arc::new(cfg).into()
    };

    let stream = tls
        .connect(ServerName::IpAddress(server_addr), stream)
        .await
        .context("tls connect failed")?;

    Ok(stream.into())
}
