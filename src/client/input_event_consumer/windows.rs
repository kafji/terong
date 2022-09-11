use crate::input_event::InputEvent;
use tokio::sync::mpsc;

pub fn run(event_source: mpsc::Receiver<InputEvent>, stop_signal: mpsc::Receiver<()>) {
    unimplemented!()
}
