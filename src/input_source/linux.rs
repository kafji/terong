use super::{controller::InputController, event::LocalInputEvent};
use crate::transport::protocol::{InputEvent, KeyCode, MouseButton, MouseScrollDirection};
use anyhow::Error;
use evdev_rs::{
    enums::{EventCode, EV_REL},
    Device, GrabMode, InputEvent as LinuxInputEvent, ReadFlag,
};
use futures::future;
use std::{
    cmp::Ordering,
    fs::File,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    sync::mpsc,
    task::{self, JoinHandle},
    try_join,
};
use tracing::warn;

pub fn start(
    keyboard_device: Option<PathBuf>,
    mouse_device: Option<PathBuf>,
    touchpad_device: Option<PathBuf>,
    event_tx: mpsc::Sender<InputEvent>,
) -> JoinHandle<()> {
    run(keyboard_device, mouse_device, touchpad_device, event_tx).unwrap()
}

/// RAII ensuring the device's grab mode will be set to ungrab
/// when it is dropped.
#[derive(Debug)]
struct Ungrabber(Device);

impl Drop for Ungrabber {
    fn drop(&mut self) {
        self.0
            .grab(GrabMode::Ungrab)
            .expect("failed to ungrab device");
    }
}

impl From<Device> for Ungrabber {
    fn from(x: Device) -> Self {
        Self(x)
    }
}

impl Deref for Ungrabber {
    type Target = Device;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Ungrabber {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn read_input_source<F>(
    device: &mut Device,
    controller: Arc<Mutex<InputController>>,
    mut map: F,
) -> Result<(), Error>
where
    F: FnMut(&LinuxInputEvent) -> Option<LocalInputEvent>,
{
    loop {
        let (_, event) = device.next_event(ReadFlag::NORMAL | ReadFlag::BLOCKING)?;
        let event = map(&event);
        if let Some(event) = event {
            let mut controller = controller.lock().unwrap();
            let consume_input = controller.on_input_event(event)?;
            set_consume_input(device, consume_input)?;
        }
    }
}

fn set_consume_input(device: &mut Device, flag: bool) -> Result<(), Error> {
    let mode = if flag {
        GrabMode::Grab
    } else {
        GrabMode::Ungrab
    };
    device.grab(mode).map_err(Into::into)
}

fn run(
    keyboard_device: Option<PathBuf>,
    mouse_device: Option<PathBuf>,
    touchpad_device: Option<PathBuf>,
    event_tx: mpsc::Sender<InputEvent>,
) -> Result<JoinHandle<()>, Error> {
    let controller = Arc::new(Mutex::new(InputController::new(event_tx)));

    let handle = task::spawn(async move {
        let keyboard = keyboard_device
            .map(|x| spawn_listener(x, controller.clone(), map_keyboard_event))
            .transpose()
            .unwrap()
            .unwrap_or_else(|| task::spawn(future::ready(())));

        let mouse = mouse_device
            .map(|x| spawn_listener(x, controller.clone(), map_mouse_event))
            .transpose()
            .unwrap()
            .unwrap_or_else(|| task::spawn(future::ready(())));

        let touchpad = touchpad_device
            .map(|x| spawn_listener(x, controller.clone(), |_| None))
            .transpose()
            .unwrap()
            .unwrap_or_else(|| task::spawn(future::ready(())));

        try_join!(keyboard, mouse, touchpad).unwrap();
    });

    Ok(handle)
}

fn spawn_listener<F>(
    device: PathBuf,
    controller: Arc<Mutex<InputController>>,
    map: F,
) -> Result<JoinHandle<()>, Error>
where
    F: FnMut(&LinuxInputEvent) -> Option<LocalInputEvent> + Send + 'static,
{
    let mut device = {
        let file = File::open(device)?;
        let dev = Device::new_from_file(file)?;
        Ungrabber::from(dev)
    };

    let handle = task::spawn_blocking(move || {
        read_input_source(&mut device, controller, map).unwrap();
    });

    Ok(handle)
}

fn map_keyboard_event(x: &LinuxInputEvent) -> Option<LocalInputEvent> {
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
        _ => None,
    }
}

fn map_mouse_event(x: &LinuxInputEvent) -> Option<LocalInputEvent> {
    let LinuxInputEvent {
        event_code, value, ..
    } = x;
    match event_code {
        EventCode::EV_REL(ev_rel) => match ev_rel {
            EV_REL::REL_WHEEL => match value.cmp(&0) {
                Ordering::Less => LocalInputEvent::MouseScroll {
                    direction: MouseScrollDirection::Down {
                        clicks: *value as _,
                    },
                }
                .into(),
                Ordering::Equal => None,
                Ordering::Greater => LocalInputEvent::MouseScroll {
                    direction: MouseScrollDirection::Up {
                        clicks: *value as _,
                    },
                }
                .into(),
            },
            _ => None,
        },
        _ => None,
    }
}
