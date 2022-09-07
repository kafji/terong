mod client {
    use crate::protocol::{Event, ServerMessage};
    use anyhow::Error;
    use bytes::{Buf, BufMut, BytesMut};
    use crossbeam::channel::Sender;
    use log::debug;
    use std::{
        convert::TryInto,
        io::{self, Read},
        net::{SocketAddr, TcpStream},
        time::Duration,
    };

    enum State {
        Idle,
        ReadMsgLen { msg_len: u16 },
        ReadMsg { msg: ServerMessage },
    }

    pub struct Client {
        state: State,
        stream: TcpStream,
        buffer: BytesMut,
        event_sink: Sender<Event>,
    }

    impl Client {
        /// Establish connection to the server.
        pub fn connect(addr: SocketAddr, event_sink: Sender<Event>) -> Result<Self, Error> {
            let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;

            let s = Self {
                state: State::Idle,
                stream,
                buffer: Default::default(),
                event_sink,
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

        fn read_msg_len(&mut self) -> Result<u16, Error> {
            if self.buffer.len() < 2 {
                self.fill_buffer_at_least(2)?;
            }
            let length = self.buffer.get_u16();
            Ok(length)
        }

        fn read_msg(&mut self, len: u16) -> Result<ServerMessage, Error> {
            let len: usize = len.try_into()?;
            if self.buffer.len() < len {
                self.fill_buffer_at_least(len)?;
            }
            let bytes = self.buffer.copy_to_bytes(len);
            let msg = bincode::deserialize(bytes.chunk())?;
            Ok(msg)
        }

        pub fn drive_state(&mut self) -> Result<(), Error> {
            self.state = match &self.state {
                State::Idle => {
                    let msg_len = self.read_msg_len()?;
                    debug!("received message length {}", msg_len);
                    State::ReadMsgLen { msg_len }
                }
                State::ReadMsgLen { msg_len } => {
                    let msg = self.read_msg(*msg_len)?;
                    debug!("received message {:?}", msg);
                    State::ReadMsg { msg }
                }
                State::ReadMsg { msg } => {
                    match msg {
                        ServerMessage::Event(x) => {
                            self.event_sink.send(*x)?;
                        }
                        _ => todo!(),
                    };
                    State::Idle
                }
            };
            Ok(())
        }
    }
}

use self::client::Client;
use crate::protocol::Event;
use anyhow::Error;
use crossbeam::channel::{Receiver, Sender, TryRecvError};
use log::{debug, info};

pub fn run(event_sink: Sender<Event>, stop_signal: Receiver<()>) {
    let addr = "192.168.123.31:5000"
        .parse()
        .expect("server address was invalid");
    debug!("connecting to {}", addr);
    let mut client = Client::connect(addr, event_sink).expect("failed to connect to the server");
    info!("connected to {}", addr);
    run_client(&mut client, &stop_signal).unwrap();
}

fn run_client(client: &mut Client, stop_signal: &Receiver<()>) -> Result<(), Error> {
    loop {
        match stop_signal.try_recv() {
            Ok(_) => {
                debug!("received stop signal");
                break;
            }
            Err(TryRecvError::Empty) => (),
            Err(err) => return Err(err.into()),
        }

        client.drive_state()?;
    }
    Ok(())
}
