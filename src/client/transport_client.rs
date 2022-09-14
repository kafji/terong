use self::connection_session::ConnectionSession;
use crate::{
    newtype,
    protocol::{self, ClientMessage, HelloMessage, HelloReply, InputEvent, ServerMessage},
    transport::{Certificate, PrivateKey, Transport},
};
use anyhow::{bail, Context, Error};
use rustls::{
    client::{ServerCertVerified, ServerCertVerifier},
    ClientConfig, ServerName,
};
use std::{net::SocketAddr, sync::Arc, time::SystemTime};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::mpsc,
    task::{self, JoinHandle},
};
use tracing::info;

pub fn start(mut event_tx: mpsc::UnboundedSender<InputEvent>) -> JoinHandle<()> {
    task::spawn(async move { run_client(&mut event_tx).await.unwrap() })
}

async fn get_client_tls_cert() -> Result<Certificate, Error> {
    todo!()
}

async fn get_client_tls_key() -> Result<PrivateKey, Error> {
    todo!()
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

    let client_tls_key = get_client_tls_key().await?;
    let client_tls_cert = get_client_tls_cert().await?;
    let mut session = ConnectionSession::new(&client_tls_key, &client_tls_cert, stream).await?;

    info!("connected to {}", server_addr);

    loop {
        // read event message
        let msg = session
            .transport()
            .recv_msg()
            .await
            .context("failed to read event message")?;
        if let ServerMessage::Event(event) = msg {
            if let Err(_) = event_tx.send(event) {
                break;
            }
        } else {
            bail!("expecting event message, but was {:?}", msg);
        }
    }

    Ok(())
}

mod connection_session {
    use crate::{
        protocol::{ClientMessage, HelloMessage, HelloReply, HelloReplyMessage, ServerMessage},
        transport::{Certificate, PrivateKey, SingleCertVerifier, Transport},
    };
    use anyhow::{bail, Error};
    use rustls::{
        client::{ServerCertVerified, ServerCertVerifier},
        ClientConfig, ServerName,
    };
    use std::{net::IpAddr, sync::Arc, time::SystemTime};
    use tokio::io::{AsyncRead, AsyncWrite};
    use tokio_rustls::{TlsConnector, TlsStream};

    #[derive(Debug)]
    pub struct ConnectionSession<'a, S> {
        client_tls_cert: &'a Certificate,
        transport: Transport<S, ServerMessage, ClientMessage>,
    }

    impl<'a, S> ConnectionSession<'a, S>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        /// Initiates new session with the server.
        pub async fn new(
            client_tls_key: &PrivateKey,
            client_tls_cert: &'a Certificate,
            stream: S,
        ) -> Result<ConnectionSession<'a, TlsStream<S>>, Error> {
            let transport = Transport::new(stream);
            let s = Self {
                client_tls_cert,
                transport,
            };
            let s = s.handshake(client_tls_key).await?;
            Ok(s)
        }

        /// Initiates client-server handshake.
        ///
        /// On success will return a session with secure transport.
        async fn handshake(
            mut self,
            client_tls_key: &PrivateKey,
        ) -> Result<ConnectionSession<'a, TlsStream<S>>, Error> {
            let client_tls_cert = self.client_tls_cert.clone().into();
            let msg = HelloMessage {
                client_version: env!("CARGO_PKG_VERSION").into(),
                client_tls_cert,
            };
            self.transport.send_msg(msg).await?;

            let msg = self.transport.recv_msg().await?;
            let s = match msg {
                ServerMessage::HelloReply(reply) => match reply {
                    HelloReply::Ok(msg) => {
                        let HelloReplyMessage { server_tls_cert } = msg;
                        self.upgrade_transport(client_tls_key, &server_tls_cert.into())
                            .await?
                    }
                    HelloReply::Err(err) => {
                        bail!("handshake failure, {:?}", err)
                    }
                },
                _ => bail!("was expecting hello reply, but was{:?}", msg),
            };

            Ok(s)
        }

        async fn upgrade_transport<'b>(
            self,
            client_tls_key: &'b PrivateKey,
            server_tls_cert: &'b Certificate,
        ) -> Result<ConnectionSession<'a, TlsStream<S>>, Error>
        where
            'a: 'b,
        {
            let Self {
                client_tls_cert,
                transport,
            } = self;
            let transport = transport
                .try_map_stream(|stream| async move {
                    upgrade_stream(
                        stream,
                        client_tls_key.clone(),
                        server_tls_cert.clone(),
                        todo!(),
                    )
                    .await
                })
                .await?;
            let s = ConnectionSession {
                client_tls_cert,
                transport,
            };
            Ok(s)
        }
    }

    impl<S> ConnectionSession<'_, S> {
        pub fn transport(&mut self) -> &mut Transport<S, ServerMessage, ClientMessage> {
            &mut self.transport
        }
    }

    async fn upgrade_stream<S>(
        stream: S,
        client_key: PrivateKey,
        server_cert: Certificate,
        server_addr: IpAddr,
    ) -> Result<TlsStream<S>, Error>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let tls: TlsConnector = {
            let cfg = ClientConfig::builder()
                .with_safe_defaults()
                .with_custom_certificate_verifier(Arc::new(SingleCertVerifier::new(server_cert)))
                .with_single_cert(vec![], rustls::PrivateKey(client_key.into()))?;
            Arc::new(cfg).into()
        };
        let stream = tls
            .connect(ServerName::IpAddress(server_addr), stream)
            .await?;
        Ok(stream.into())
    }
}
