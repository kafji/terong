use crate::protocol::message::InputEvent;
use anyhow::Error;
use bytes::{Buf, BufMut, BytesMut};
use std::{io::Read, net::TcpStream, sync::mpsc::Sender, time::Duration};

pub fn run(event_sink: Sender<InputEvent>) {
    let mut client = Client::new(event_sink);
    run_client(&mut client).unwrap();
}

struct Client {
    buffer: BytesMut,
    event_sink: Sender<InputEvent>,
}

impl Client {
    fn new(event_sink: Sender<InputEvent>) -> Self {
        let buffer = BytesMut::new();
        Self { buffer, event_sink }
    }
}

fn run_client(client: &mut Client) -> Result<(), Error> {
    let addr = "192.168.123.31:5000".parse()?;
    println!("connecting to {}", addr);
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
    println!("connected  to {}", addr);
    loop {
        let mut buf = [0; 4096];
        let read = stream.read(&mut buf)?;
        client.buffer.put(&buf[..read]);
        if buf.len() < 2 {
            continue;
        }
        let length = client.buffer.get_u16();
        while length > client.buffer.len() as _ {
            let mut buf = [0; 4096];
            let read = stream.read(&mut buf)?;
            client.buffer.put(&buf[..read]);
        }
        let bytes = client.buffer.copy_to_bytes(length as _);
        let event: InputEvent = bincode::deserialize(bytes.chunk())?;
        client.event_sink.send(event)?;
    }
}
