use super::event::LocalInputEvent;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

pub fn start(
    input_event_tx: mpsc::UnboundedSender<LocalInputEvent>,
    capture_input_rx: watch::Receiver<bool>,
) -> JoinHandle<()> {
    unimplemented!()
}
