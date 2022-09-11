use crate::protocol::message::InputEvent;
use crossbeam::channel::{Receiver, Sender};
use tokio::task::JoinHandle;

pub fn start(event_sink: Sender<InputEvent>, stop_signal: Receiver<()>) -> JoinHandle<()> {
    unimplemented!()
}
