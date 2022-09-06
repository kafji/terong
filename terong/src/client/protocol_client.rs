use crate::protocol::message::InputEvent;
use anyhow::Error;
use bytes::{Buf, BytesMut};
use std::{io::Read, net::TcpStream};

pub fn run() {
    let mut client = Client::new();
    run_client(&mut client).unwrap();
}

struct Client {}

impl Client {
    fn new() -> Self {
        Self {}
    }
}

fn run_client(client: &mut Client) -> Result<(), Error> {
    let mut stream = TcpStream::connect("127.0.0.1:5000")?;
    let mut buf = BytesMut::with_capacity(1024);
    let read = stream.read(&mut buf)?;
    assert!(read != 0);
    let length = buf.get_u16();
    let bytes = buf.copy_to_bytes(length as _);
    let event: InputEvent = bincode::deserialize(&*bytes)?;
    todo!()
}
