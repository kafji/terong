use super::{controller::InputController, event::LocalInputEvent};
use crate::protocol::{InputEvent, KeyCode, MouseButton};
use anyhow::Error;
use evdev_rs::{enums::EventCode, Device, GrabMode, InputEvent as LinuxInputEvent, ReadFlag};
use std::{
    fs::File,
    ops::{Deref, DerefMut},
};
use tokio::{
    sync::mpsc,
    task::{self, JoinHandle},
};
use tracing::warn;

pub fn start(event_tx: mpsc::Sender<InputEvent>) -> JoinHandle<()> {
    run(event_tx)
}

/// RAII ensuring the device's grab mode will be set to ungrab
/// when it is dropped.
#[derive(Debug)]
struct DeviceGuard(Device);

impl Drop for DeviceGuard {
    fn drop(&mut self) {
        self.0.grab(GrabMode::Ungrab).unwrap();
    }
}

impl From<Device> for DeviceGuard {
    fn from(x: Device) -> Self {
        Self(x)
    }
}

impl Deref for DeviceGuard {
    type Target = Device;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DeviceGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn read_input_source(device: &mut Device, controller: &mut InputController) -> Result<(), Error> {
    loop {
        let (_, event) = device.next_event(ReadFlag::NORMAL | ReadFlag::BLOCKING)?;
        let event = linux_event_to_local_event(&event);
        if let Some(event) = event {
            let should_capture = controller.on_input_event(event)?;
            set_capture_input(device, should_capture)?;
        }
    }
}

fn set_capture_input(device: &mut Device, flag: bool) -> Result<(), Error> {
    let mode = if flag {
        GrabMode::Grab
    } else {
        GrabMode::Ungrab
    };
    device.grab(mode).map_err(Into::into)
}

fn run(event_tx: mpsc::Sender<InputEvent>) -> JoinHandle<()> {
    run2(event_tx).unwrap()
}

fn run2(event_tx: mpsc::Sender<InputEvent>) -> Result<JoinHandle<()>, Error> {
    let mut controller = InputController::new(event_tx);

    let mut device = {
        let file = File::open("/dev/input/event3")?;
        let dev = Device::new_from_file(file)?;
        DeviceGuard::from(dev)
    };

    let input_source_handler = task::spawn_blocking(move || {
        read_input_source(&mut device, &mut controller).unwrap();
    });

    Ok(input_source_handler)
}

fn linux_event_to_local_event(x: &LinuxInputEvent) -> Option<LocalInputEvent> {
    let LinuxInputEvent {
        event_code, value, ..
    } = x;
    match event_code {
        EventCode::EV_KEY(ev_key) => {
            let btn = MouseButton::from_ev_key(*ev_key);
            let key = KeyCode::from_ev_key(*ev_key);
            match (value, btn, key) {
                (1, Some(button), None) => LocalInputEvent::MouseButtonDown { button }.into(),
                (0, Some(button), None) => LocalInputEvent::MouseButtonUp { button }.into(),
                (1, None, Some(key)) => LocalInputEvent::KeyDown { key }.into(),
                (2, None, Some(key)) => LocalInputEvent::KeyRepeat { key }.into(),
                (0, None, Some(key)) => LocalInputEvent::KeyUp { key }.into(),
                _ => {
                    warn!(
                        "unexpected raw key {:?}, value {}, button {:?}, key {:?}",
                        ev_key, value, btn, key
                    );
                    None
                }
            }
        }
        EventCode::EV_REL(_) => todo!(),
        EventCode::EV_ABS(_) => todo!(),
        EventCode::EV_REP(_) => todo!(),
        _ => None,
    }
}
