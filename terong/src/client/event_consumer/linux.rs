use crate::event::InputEvent;
use crossbeam::channel::Receiver;

pub fn run(event_source: Receiver<InputEvent>, stop_signal: Receiver<()>) {
    todo!()
}
