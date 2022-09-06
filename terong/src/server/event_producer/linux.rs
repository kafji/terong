use crate::protocol::message::InputEvent;
use crossbeam::channel::{Receiver, Sender};

pub fn run(event_sink: Sender<InputEvent>, stop_signal: Receiver<()>) {
    todo!()
}
