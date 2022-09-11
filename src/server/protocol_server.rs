mod server {
    use crate::protocol::ServerMessage;
    use anyhow::Error;
    use tokio::{io::AsyncWriteExt, net::TcpStream};
    use tracing::{debug, warn};

    pub struct Server {
        client: Option<TcpStream>,
    }

    impl Server {
        pub fn new() -> Self {
            Self {
                client: Default::default(),
            }
        }

        pub fn add_client(&mut self, client: TcpStream) {
            if self.client.is_none() {
                self.client = Some(client);
            } else {
                let addr = client.peer_addr().unwrap();
                warn!("dropping multiple incoming connections from {}", addr);
            }
        }

        pub async fn send_message(&mut self, msg: ServerMessage) -> Result<(), Error> {
            if let Some(stream) = &mut self.client {
                let msg = bincode::serialize(&msg)?;
                let msg_len = msg.len();

                let addr = stream.peer_addr()?;
                debug!("sending message {:?} length {} to {}", msg, msg_len, addr);
                stream.write_all(&msg_len.to_be_bytes()).await?;
                stream.write_all(&msg).await?;
            }
            Ok(())
        }
    }
}

use self::server::Server;
use crate::protocol::{InputEvent, ServerMessage};
use anyhow::Error;
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc,
    task::{self, JoinHandle},
};
use tracing::info;

pub fn start(proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) -> JoinHandle<()> {
    task::spawn(run(proto_event_rx))
}

async fn run(mut proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) {
    let mut server = Server::new();
    run_server(&mut server, &mut proto_event_rx).await.unwrap();
}

#[derive(Debug)]
enum Action {
    IncomingConnection(TcpStream),
    SendMessage(ServerMessage),
}

async fn run_server(
    server: &mut Server,
    proto_event_rx: &mut mpsc::UnboundedReceiver<InputEvent>,
) -> Result<(), Error> {
    let addr = "0.0.0.0:3000";
    info!("listening at {}", addr);
    let listener = TcpListener::bind(addr).await?;

    loop {
        let action: Result<Action, Error> = select! {
            x = listener.accept() => {
                match x {
                    Ok((conn, addr)) => {
                        info!("received connection from {}", addr);
                        Ok(Action::IncomingConnection(conn))
                    },
                    Err(err) => Err(err.into()),
                }
            }
            x = proto_event_rx.recv() => {
                match x {
                    Some(event) => Ok(Action::SendMessage(event.into())),
                    None => break,
                }
            }
        };
        let action = action?;
        match action {
            Action::IncomingConnection(conn) => server.add_client(conn),
            Action::SendMessage(msg) => server.send_message(msg).await?,
        }
    }

    Ok(())
}
