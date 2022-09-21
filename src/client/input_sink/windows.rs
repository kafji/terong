use crate::transport::protocol::InputEvent;
use tokio::{sync::mpsc, task::JoinHandle};

pub fn start(event_rx: mpsc::Receiver<InputEvent>) -> JoinHandle<()> {
    unimplemented!()
}
