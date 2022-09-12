use crate::protocol::InputEvent;
use tokio::{sync::mpsc, task::JoinHandle};

pub fn start(event_rx: mpsc::UnboundedReceiver<InputEvent>) -> JoinHandle<()> {
    unimplemented!()
}
