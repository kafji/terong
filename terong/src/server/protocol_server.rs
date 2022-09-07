//! The TCP server that will transmits events to clients.

use crate::protocol::{Event, ServerMessage};
use anyhow::Error;
use crossbeam::channel::{Receiver, TryRecvError};
use log::{debug, info};
use std::{
    convert::TryInto,
    io::{self, Write},
    net::{SocketAddr, TcpListener, TcpStream},
};

/// Run the server.
pub fn run(event_source: Receiver<Event>, stop_signal: Receiver<()>) {
    let mut server = Server::new(event_source);
    run_server(&mut server, &stop_signal).unwrap();
}

struct Server {
    clients: Vec<(TcpStream, SocketAddr)>,
    event_source: Receiver<Event>,
}

impl Server {
    fn new(event_source: Receiver<Event>) -> Self {
        Self {
            clients: Vec::new(),
            event_source,
        }
    }
}

fn run_server(server: &mut Server, stop_signal: &Receiver<()>) -> Result<(), Error> {
    let addr = "0.0.0.0:5000";
    info!("listening at {}", addr);
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;

    loop {
        match stop_signal.try_recv() {
            Ok(_) => {
                debug!("received stop signal");
                break;
            }
            Err(TryRecvError::Empty) => (),
            Err(err) => return Err(err.into()),
        }

        let conn = match listener.accept() {
            Ok(x) => Some(x),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => None,
            Err(err) => Err(err)?,
        };
        if let Some((stream, addr)) = conn {
            info!("received connection from {}", addr);
            server.clients.push((stream, addr));
        }

        let event = match server.event_source.try_recv() {
            Ok(x) => Some(x),
            Err(TryRecvError::Empty) => None,
            Err(err) => Err(err)?,
        };
        if let Some(event) = event {
            let msg: ServerMessage = event.into();
            let msg_len: u16 = {
                let len = bincode::serialized_size(&msg)?;
                len.try_into()?
            };
            for (stream, addr) in &mut server.clients {
                debug!("sending message {:?} length {} to {}", msg, msg_len, addr);
                stream.write_all(&msg_len.to_be_bytes())?;
                bincode::serialize_into(stream, &msg)?;
            }
        }
    }

    Ok(())
}
