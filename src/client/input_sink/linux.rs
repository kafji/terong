use crate::protocol::{InputEvent, KeyCode, MouseButton, MouseScrollDirection};
use anyhow::{anyhow, Error};
use evdev_rs::{
    enums::{BusType, EventCode, EventType, EV_REL, EV_SYN},
    DeviceWrapper, InputEvent as LinuxInputEvent, UInputDevice, UninitDevice,
};
use std::{convert::TryInto, iter, time::SystemTime};
use strum::IntoEnumIterator;
use tokio::{
    sync::mpsc,
    task::{self, JoinHandle},
};

pub fn start(event_rx: mpsc::Receiver<InputEvent>) -> JoinHandle<()> {
    task::spawn_blocking(|| {
        run_input_sink(event_rx).unwrap();
    })
}

// useful documentations:
// https://www.kernel.org/doc/html/v5.15/input/uinput.html
// https://www.kernel.org/doc/html/v5.15/input/event-codes.html
// https://github.com/torvalds/linux/blob/master/include/linux/input.h
// https://www.freedesktop.org/software/libevdev/doc/latest/
// https://ndesh26.github.io/evdev-rs/evdev_rs/
// https://github.com/ndesh26/evdev-rs/issues/75
// https://github.com/ndesh26/evdev-rs/pull/76

fn create_virtual_device() -> Result<UninitDevice, Error> {
    let dev =
        UninitDevice::new().ok_or_else(|| anyhow!("failed to create virtual evdev device"))?;

    dev.set_name("Terong Virtual Input Device");
    dev.set_bustype(BusType::BUS_USB as _);

    dev.enable_event_type(&EventType::EV_SYN)?;
    dev.enable_event_code(&EventCode::EV_SYN(EV_SYN::SYN_REPORT), None)?;

    dev.enable_event_type(&EventType::EV_KEY)?;

    for btn in MouseButton::iter() {
        let key = btn.into();
        dev.enable_event_code(&EventCode::EV_KEY(key), None)?;
    }

    for key in KeyCode::iter() {
        let key = key.into();
        dev.enable_event_code(&EventCode::EV_KEY(key), None)?;
    }

    dev.enable_event_type(&EventType::EV_REL)?;
    dev.enable_event_code(&EventCode::EV_REL(EV_REL::REL_X), None)?;
    dev.enable_event_code(&EventCode::EV_REL(EV_REL::REL_Y), None)?;
    dev.enable_event_code(&EventCode::EV_REL(EV_REL::REL_WHEEL), None)?;

    Ok(dev)
}

fn run_input_sink(mut event_rx: mpsc::Receiver<InputEvent>) -> Result<(), Error> {
    let dev = create_virtual_device()?;

    let uidev = UInputDevice::create_from_device(&dev)?;

    while let Some(event) = event_rx.blocking_recv() {
        let events: Vec<LinuxInputEvent> = event.try_into()?;

        for e in &events {
            uidev.write_event(&e)?;
        }
    }

    Ok(())
}

impl TryInto<Vec<LinuxInputEvent>> for InputEvent {
    type Error = Error;
    fn try_into(self) -> Result<Vec<LinuxInputEvent>, Self::Error> {
        let time = SystemTime::now().try_into()?;

        let es = match self {
            InputEvent::MouseMove { dx, dy } => vec![
                (EventCode::EV_REL(EV_REL::REL_X), dx),
                (EventCode::EV_REL(EV_REL::REL_Y), dy),
            ],
            InputEvent::MouseButtonDown { button } => {
                vec![(EventCode::EV_KEY(button.into()), 1)]
            }
            InputEvent::MouseButtonUp { button } => {
                vec![(EventCode::EV_KEY(button.into()), 0)]
            }
            InputEvent::MouseScroll {
                direction: MouseScrollDirection::Up { clicks },
            } => vec![(EventCode::EV_REL(EV_REL::REL_WHEEL), clicks as i16)],
            InputEvent::MouseScroll {
                direction: MouseScrollDirection::Down { clicks },
            } => vec![(EventCode::EV_REL(EV_REL::REL_WHEEL), -(clicks as i16))],
            InputEvent::KeyDown { key } => vec![(EventCode::EV_KEY(key.into()), 1)],
            InputEvent::KeyRepeat { key } => vec![(EventCode::EV_KEY(key.into()), 2)],
            InputEvent::KeyUp { key } => vec![(EventCode::EV_KEY(key.into()), 0)],
        };

        let es = es
            .into_iter()
            .map(|(event_code, value)| LinuxInputEvent {
                time,
                event_code,
                value: value as _,
            })
            .chain(iter::once(LinuxInputEvent {
                time,
                event_code: EventCode::EV_SYN(EV_SYN::SYN_REPORT),
                value: 0,
            }))
            .collect();

        Ok(es)
    }
}
