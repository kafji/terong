use super::event::LocalInputEvent;
use crate::protocol::{KeyCode, MouseButton};
use anyhow::Error;
use evdev_rs::{enums::EventCode, Device, GrabMode, InputEvent, ReadFlag};
use std::{
    fs::File,
    ops::{Deref, DerefMut},
    path::Path,
    sync::{Arc, Mutex},
};
use tokio::{
    select,
    sync::{mpsc, watch},
    task::{self, JoinHandle},
};
use tracing::{info, warn};

pub fn start(
    input_event_tx: mpsc::UnboundedSender<LocalInputEvent>,
    capture_input_rx: watch::Receiver<bool>,
) -> JoinHandle<()> {
    task::spawn(async { run(input_event_tx, capture_input_rx).await.unwrap() })
}

/// RAII structure ensuring the device's grab mode will be set to ungrab when it
/// is dropped.
#[derive(Debug)]
struct DeviceGuard(Device);

impl From<Device> for DeviceGuard {
    fn from(x: Device) -> Self {
        Self(x)
    }
}

impl Drop for DeviceGuard {
    fn drop(&mut self) {
        self.0.grab(GrabMode::Ungrab).unwrap();
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

#[derive(Debug)]
struct InputSource<D> {
    dev: Arc<Mutex<D>>,
}

impl InputSource<DeviceGuard> {
    async fn new(path: impl AsRef<Path>) -> Result<Self, Error> {
        let dev = task::block_in_place(|| {
            let file = File::open(path)?;
            let dev = Device::new_from_file(file)?;
            Result::<_, Error>::Ok(DeviceGuard::from(dev))
        })?;
        let dev = Arc::new(Mutex::new(dev));
        let s = Self { dev };
        Ok(s)
    }
}

impl<D> InputSource<D>
where
    D: DerefMut<Target = Device>,
{
    async fn set_capture_input(&mut self, x: bool) -> Result<(), Error> {
        task::block_in_place(|| {
            info!("setting capture input to {}", x);
            let mode = if x { GrabMode::Grab } else { GrabMode::Ungrab };
            let mut dev = self.dev.lock().unwrap();
            dev.grab(mode)
        })
        .map_err(Into::into)
    }

    async fn read_event(&self) -> Result<LocalInputEvent, Error> {
        task::block_in_place(|| {
            let dev = self.dev.lock().unwrap();
            let event = loop {
                let (_, event) = dev.next_event(ReadFlag::NORMAL | ReadFlag::BLOCKING)?;
                let event = linux_event_to_local_event(&event);
                if let Some(event) = event {
                    break event;
                }
            };
            Ok(event)
        })
    }
}

async fn run(
    input_event_tx: mpsc::UnboundedSender<LocalInputEvent>,
    mut capture_input_rx: watch::Receiver<bool>,
) -> Result<(), Error> {
    let mut source = InputSource::new("/dev/input/event3").await?;
    loop {
        select! { biased;
            x = capture_input_rx.changed() => {
                match x {
                    Ok(_) => {
                        let flag = *capture_input_rx.borrow();
                        source.set_capture_input(flag).await?;
                    },
                    Err(_) => break,
                }
            }
            x = source.read_event() => {
                let event = x?;
                if let Err(_) = input_event_tx.send(event) {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn linux_event_to_local_event(x: &InputEvent) -> Option<LocalInputEvent> {
    let InputEvent {
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
