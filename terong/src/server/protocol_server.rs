//! The TCP server that will transmits events to clients.

use crate::protocol::{InputEvent, ServerMessage};
use anyhow::Error;
use std::{convert::TryInto, net::SocketAddr};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc,
};
use tracing::{debug, info};

/// Run the server.
pub async fn run(event_source: mpsc::UnboundedReceiver<InputEvent>) {
    let mut server = Server::new(event_source);
    run_server(&mut server).await.unwrap();
}

struct Server {
    clients: Vec<(TcpStream, SocketAddr)>,
    event_source: mpsc::UnboundedReceiver<InputEvent>,
}

impl Server {
    fn new(event_source: mpsc::UnboundedReceiver<InputEvent>) -> Self {
        Self {
            clients: Vec::new(),
            event_source,
        }
    }

    async fn send_input_event(&mut self, event: InputEvent) -> Result<(), Error> {
        debug!("sending input event");
        let msg: ServerMessage = event.into();
        let msg = bincode::serialize(&msg)?;
        let msg_len = msg.len();
        for (stream, addr) in &mut self.clients {
            debug!("sending message {:?} length {} to {}", msg, msg_len, addr);
            stream.write_all(&msg_len.to_be_bytes()).await?;
            stream.write_all(&msg).await?;
        }
        Ok(())
    }
}

async fn run_server(server: &mut Server) -> Result<(), Error> {
    let addr = "0.0.0.0:5000";

    info!("listening at {}", addr);

    let listener = TcpListener::bind(addr).await?;

    loop {
        select! {
            // accept incoming connections
            conn = listener.accept() => {
                let (stream, addr) = conn?;
                info!("received connection from {}", addr);
                server.clients.push((stream, addr));
            }
            // send input events to connected clients
            x = server.event_source.recv() => {
                if let Some(event) = x {
                    server.send_input_event(event).await?;
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}
