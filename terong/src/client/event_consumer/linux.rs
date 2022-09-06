use crate::protocol::message::InputEvent;
use crossbeam::channel::Receiver;

pub fn run(event_source: Receiver<InputEvent>, stop_signal: Receiver<()>) {
    todo!()
}
