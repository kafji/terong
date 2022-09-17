use crate::{
    protocol::{
        ClientMessage, HelloMessage, HelloReply, InputEvent, ServerMessage,
        UpgradeTransportRequest, UpgradeTransportResponse,
    },
    transport::{Certificate, PrivateKey, SingleCertVerifier, Transport, Transporter},
};
use anyhow::{bail, Context, Error};
use rustls::{ClientConfig, ServerName};
use std::{
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
use tracing::info;

pub fn start(mut event_tx: mpsc::UnboundedSender<InputEvent>) -> JoinHandle<()> {
    task::spawn(async move { run_client(&mut event_tx).await.unwrap() })
}

async fn run_client(event_tx: &mut mpsc::UnboundedSender<InputEvent>) -> Result<(), Error> {
    let server_addr: SocketAddr = "192.168.123.31:3000"
        .parse()
        .context("invalid server address")?;

    // open connection with the server
    info!("connecting to {}", server_addr);
    let stream = TcpStream::connect(server_addr)
        .await
        .context("failed to connect to the server")?;

    info!("connected to {}", server_addr);

    let mut transporter: Transporter<_, _, ServerMessage, ClientMessage> =
        Transporter::PlainText(Transport::new(stream));

    let mut state = State::Handshaking {
        client_version: env!("CARGO_PKG_VERSION").into(),
    };

    loop {
        state = match state {
            State::Handshaking { client_version } => {
                let transport = transporter.plain()?;

                let msg = HelloMessage { client_version };
                transport.send_msg(msg).await?;

                let msg = transport.recv_msg().await?;
                let server_tls_cert = match msg {
                    ServerMessage::HelloReply(reply) => match reply {
                        HelloReply::Ok(UpgradeTransportRequest { server_tls_cert }) => {
                            server_tls_cert
                        }
                        HelloReply::Err(err) => {
                            bail!("handshake fail, {:?}", err)
                        }
                    },
                    _ => bail!("received unexpected message, {:?}", msg),
                };

                let client_tls_cert = get_client_tls_cert().await?;
                let msg = UpgradeTransportResponse {
                    client_tls_cert: client_tls_cert.clone(),
                };
                transport.send_msg(msg).await?;

                State::UpgradingTransport {
                    server_tls_cert,
                    client_tls_cert,
                }
            }

            State::UpgradingTransport {
                server_tls_cert,
                client_tls_cert,
            } => {
                let client_tls_key = get_client_tls_key().await?;

                transporter = transporter
                    .upgrade(|stream| async move {
                        upgrade_stream(stream, client_tls_key, server_tls_cert, server_addr.ip())
                            .await
                    })
                    .await?;

                State::Idle
            }

            State::Idle => {
                let transport = transporter.secure()?;

                let msg = transport.recv_msg().await?;
                match msg {
                    ServerMessage::Event(event) => State::ReceivedEvent { event },
                    _ => bail!("received unexpected message, {:?}", msg),
                }
            }

            State::ReceivedEvent { event } => {
                if let Err(_) = event_tx.send(event) {
                    break;
                }

                State::Idle
            }
        };
    }

    Ok(())
}

#[derive(Clone, Debug)]
pub enum State {
    Handshaking {
        client_version: String,
    },
    UpgradingTransport {
        server_tls_cert: Certificate,
        client_tls_cert: Certificate,
    },
    Idle,
    ReceivedEvent {
        event: InputEvent,
    },
}

async fn get_client_tls_cert() -> Result<Certificate, Error> {
    todo!()
}

async fn get_client_tls_key() -> Result<PrivateKey, Error> {
    todo!()
}

pub async fn upgrade_stream<S>(
    stream: S,
    client_tls_key: PrivateKey,
    server_tls_cert: Certificate,
    server_addr: IpAddr,
) -> Result<TlsStream<S>, Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tls: TlsConnector = {
        let cert_verifier = Arc::new(SingleCertVerifier::new(server_tls_cert));
        let cfg = ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(cert_verifier)
            .with_single_cert(vec![], rustls::PrivateKey(client_tls_key.into()))?;
        Arc::new(cfg).into()
    };
    let stream = tls
        .connect(ServerName::IpAddress(server_addr), stream)
        .await?;
    Ok(stream.into())
}
