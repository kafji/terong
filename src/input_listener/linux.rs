use super::event::LocalInputEvent;
use evdev_rs::{
    enums::{BusType, EventCode, EventType, EV_KEY, EV_REL, EV_SYN},
    Device, DeviceWrapper, InputEvent, ReadFlag, UInputDevice, UninitDevice,
};
use nix::fcntl::OFlag;
use std::os::unix::prelude::OpenOptionsExt;
use tokio::{
    sync::{mpsc, watch},
    task::{self, JoinHandle},
};

pub fn start(
    input_event_tx: mpsc::UnboundedSender<LocalInputEvent>,
    capture_input_rx: watch::Receiver<bool>,
) -> JoinHandle<()> {
    task::spawn(run())
}

async fn run() {
    let uinput = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(OFlag::O_NONBLOCK.bits())
        .open("/dev/uinput")
        .unwrap();
    let dev = Device::new_from_file(uinput).unwrap();
    let event = dev.next_event(ReadFlag::NORMAL).unwrap();
    dbg!(&event.1);
}
