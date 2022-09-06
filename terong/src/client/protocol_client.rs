mod client {
    use crate::protocol::message::Message;
    use anyhow::Error;
    use bytes::{Buf, BufMut, BytesMut};
    use std::{
        convert::TryInto,
        io::{self, Read},
        net::{SocketAddr, TcpStream},
        time::Duration,
    };

    pub struct Client {
        stream: TcpStream,
        buffer: BytesMut,
    }

    impl Client {
        /// Establish connection to the server.
        pub fn connect(addr: SocketAddr) -> Result<Self, Error> {
            let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
            stream.set_nodelay(true)?;

            let s = Self {
                stream,
                buffer: Default::default(),
            };
            Ok(s)
        }

        fn fill_buffer_at_least(&mut self, size: usize) -> Result<usize, Error> {
            let mut read = 0;
            while self.buffer.len() < size {
                let mut buf = [0; 4096];
                read = self.stream.read(&mut buf)?;
                if read == 0 {
                    let err = io::Error::from(io::ErrorKind::UnexpectedEof);
                    return Err(err.into());
                }
                self.buffer.put(&buf[..read]);
            }
            Ok(read)
        }

        pub fn read_msg_len(&mut self) -> Result<u16, Error> {
            if self.buffer.len() < 2 {
                self.fill_buffer_at_least(2)?;
            }
            let length = self.buffer.get_u16();
            Ok(length)
        }

        pub fn read_msg(&mut self, len: u16) -> Result<Message, Error> {
            let len: usize = len.try_into()?;
            if self.buffer.len() < len {
                self.fill_buffer_at_least(len)?;
            }
            let bytes = self.buffer.copy_to_bytes(len);
            let msg = bincode::deserialize(bytes.chunk())?;
            Ok(msg)
        }
    }
}

use self::client::Client;
use crate::protocol::message::{InputEvent, Message};
use anyhow::Error;
use crossbeam::channel::{Receiver, Sender, TryRecvError};
use log::{debug, info};

pub fn run(event_sink: Sender<InputEvent>, stop_signal: Receiver<()>) {
    let addr = "192.168.123.31:5000"
        .parse()
        .expect("server address was invalid");
    debug!("connecting to {}", addr);
    let mut client = Client::connect(addr).expect("failed to connect to the server");
    info!("connected to {}", addr);
    run_client(&mut client, &event_sink, &stop_signal).unwrap();
}

fn run_client(
    client: &mut Client,
    event_sink: &Sender<InputEvent>,
    stop_signal: &Receiver<()>,
) -> Result<(), Error> {
    loop {
        match stop_signal.try_recv() {
            Ok(_) => break,
            Err(TryRecvError::Empty) => (),
            Err(err) => return Err(err.into()),
        }

        let msg_len = client.read_msg_len()?;
        debug!("received message length {}", msg_len);
        let msg = client.read_msg(msg_len)?;
        debug!("received message {:?}", msg);
        match msg {
            Message::InputEvent(x) => {
                event_sink.send(x)?;
            }
        }
    }
    Ok(())
}
