use crate::protocol::{
    self, ClientMessage, HelloMessage, HelloReply, InputEvent, MessageInbox, ServerMessage,
};
use anyhow::{bail, Context, Error};
use std::net::SocketAddr;
use tokio::{
    net::TcpStream,
    sync::mpsc,
    task::{self, JoinHandle},
};
use tracing::info;

pub fn start(event_tx: mpsc::UnboundedSender<InputEvent>) -> JoinHandle<()> {
    task::spawn(run(event_tx))
}

async fn run(event_tx: mpsc::UnboundedSender<InputEvent>) {
    run_client(event_tx).await.unwrap();
}

async fn run_client(event_tx: mpsc::UnboundedSender<InputEvent>) -> Result<(), Error> {
    let addr: SocketAddr = "192.168.123.31:3000"
        .parse()
        .context("server address was invalid")?;

    // open connection with the server
    info!("connecting to {}", addr);
    let mut stream = TcpStream::connect(addr)
        .await
        .context("failed to connect to the server")?;
    info!("connected to {}", addr);

    let (mut source, mut sink) = stream.split();
    let mut inbox = MessageInbox::new(&mut source);

    // send handshake message
    let hello_msg = HelloMessage { version: "".into() };
    {
        let msg: ClientMessage = hello_msg.into();
        protocol::send_msg(&mut sink, &msg).await
    }
    .context("failed to send hello message")?;

    // read handshake reply
    let msg = inbox
        .recv_msg()
        .await
        .context("failed to read hello reply")?;
    if let ServerMessage::HelloReply(reply) = msg {
        if let HelloReply::Err(err) = reply {
            bail!("handshake failure, {:?}", err)
        }
    } else {
        bail!("expecting hello reply, but was {:?}", msg);
    }

    // handshake successful

    loop {
        // read event message
        let msg = inbox
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
