use crate::protocol::message::InputEvent;
use std::{
    io::{self, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc::{Receiver, TryRecvError},
};

pub fn run(event_source: Receiver<InputEvent>) {
    let mut server = Server::new(event_source);
    run_server(&mut server).unwrap()
}

struct Server {
    clients: Vec<(TcpStream, SocketAddr)>,
    event_source: Receiver<InputEvent>,
}

impl Server {
    fn new(event_source: Receiver<InputEvent>) -> Self {
        Self {
            clients: Vec::new(),
            event_source,
        }
    }
}

fn run_server(server: &mut Server) -> Result<(), anyhow::Error> {
    let listener = TcpListener::bind("127.0.0.1:5000")?;
    listener.set_nonblocking(true)?;

    loop {
        let conn = match listener.accept() {
            Ok(x) => Some(x),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => None,
            Err(err) => Err(err)?,
        };
        if let Some(conn) = conn {
            println!("received connection from {}", conn.1);
            server.clients.push(conn);
        }

        let event = match server.event_source.try_recv() {
            Ok(x) => Some(x),
            Err(TryRecvError::Empty) => None,
            Err(err) => Err(err)?,
        };
        if let Some(event) = event {
            println!("received input event {:?}", event);
            let m = bincode::serialize(&event)?;
            for (stream, peer_addr) in &mut server.clients {
                println!("sending input event to {}", peer_addr);
                stream.write(&(m.len() as u16).to_be_bytes())?;
                stream.write(&m)?;
            }
        }
    }
}
