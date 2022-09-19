use crate::{
    config::no_tls,
    protocol::{
        ClientMessage, HelloMessage, HelloReply, InputEvent, ServerMessage,
        UpgradeTransportRequest, UpgradeTransportResponse,
    },
    transport::{Certificate, PrivateKey, SingleCertVerifier, Transport, Transporter},
};
use anyhow::{bail, Context, Error};
use futures::pin_mut;
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
use tracing::{debug, info, warn};

pub fn start(mut event_tx: mpsc::Sender<InputEvent>) -> JoinHandle<()> {
    task::spawn(async move { run_client(&mut event_tx).await.unwrap() })
}

async fn run_client(event_tx: &mut mpsc::Sender<InputEvent>) -> Result<(), Error> {
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
        Transporter::Plain(Transport::new(stream));

    let mut state = State::Handshaking;

    let cert = {
        let mut params = rcgen::CertificateParams::default();
        params.subject_alt_names.push(rcgen::SanType::IpAddress(
            "192.168.123.205".parse().unwrap(),
        ));
        let cert = rcgen::Certificate::from_params(params).unwrap();
        cert
    };

    loop {
        debug!(?state);
        state = match state {
            State::Handshaking => {
                let transport = transporter.plain()?;

                // send hello message
                let client_version = env!("CARGO_PKG_VERSION").into();
                let msg = HelloMessage { client_version };
                transport.send_msg(msg.into()).await?;

                // wait for hello reply
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

                // send client tls certificate
                let client_tls_cert: Certificate = {
                    let x = cert.serialize_der().unwrap();
                    x.into()
                };
                let msg = UpgradeTransportResponse {
                    client_tls_cert: client_tls_cert.clone(),
                };
                transport.send_msg(msg.into()).await?;

                // upgrade to tls
                let no_tls = no_tls();
                if no_tls {
                    warn!("tls disabled")
                } else {
                    let client_tls_key = { cert.serialize_private_key_der().into() };
                    transporter = transporter
                        .upgrade(|stream| async move {
                            upgrade_stream(
                                stream,
                                client_tls_cert,
                                client_tls_key,
                                server_tls_cert,
                                server_addr.ip(),
                            )
                            .await
                        })
                        .await?;
                }

                State::Idle
            }

            State::Idle => {
                let messenger = transporter.any();

                debug!(?state, "waiting for message");
                let msg = messenger.recv_msg().await?;

                debug!(?state, ?msg, "received message");

                match msg {
                    ServerMessage::Event(event) => State::ReceivedEvent { event },
                    _ => bail!("received unexpected message, {:?}", msg),
                }
            }

            State::ReceivedEvent { event } => {
                if let Err(_) = event_tx.send(event).await {
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
    Handshaking,
    Idle,
    ReceivedEvent { event: InputEvent },
}

pub async fn upgrade_stream<S>(
    stream: S,
    client_tls_cert: Certificate,
    client_tls_key: PrivateKey,
    server_tls_cert: Certificate,
    server_addr: IpAddr,
) -> Result<TlsStream<S>, Error>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let tls: TlsConnector = {
        let server_cert_verifier = Arc::new(SingleCertVerifier::new(server_tls_cert));

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
