use crate::transport::protocol::{InputEvent, KeyCode, MouseButton, MouseScrollDirection};
use anyhow::{anyhow, Error};
use evdev_rs::{
    enums::{BusType, EventCode, EventType, EV_REL, EV_SYN},
    DeviceWrapper, InputEvent as LinuxInputEvent, UInputDevice, UninitDevice,
};
use std::{convert::TryInto, time::SystemTime};
use strum::IntoEnumIterator;
use tokio::{
    sync::mpsc,
    task::{self, JoinHandle},
};

pub fn start(event_rx: mpsc::Receiver<InputEvent>) -> JoinHandle<()> {
    task::spawn(async {
        run_input_sink(event_rx).await.unwrap();
        ()
    })
}

// relevant links:
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
    dev.set_bustype(BusType::BUS_VIRTUAL as _);

    dev.enable_event_type(&EventType::EV_SYN)?;
    dev.enable_event_code(&EventCode::EV_SYN(EV_SYN::SYN_REPORT), None)?;

    dev.enable_event_type(&EventType::EV_KEY)?;

    // register mouse button events
    for btn in MouseButton::iter() {
        let key = btn.into();
        dev.enable_event_code(&EventCode::EV_KEY(key), None)?;
    }

    // register keyboard events
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

async fn run_input_sink(mut event_rx: mpsc::Receiver<InputEvent>) -> Result<(), Error> {
    let dev = create_virtual_device()?;
    let uidev = UInputDevice::create_from_device(&dev)?;
    let mut events = Vec::new();
    while let Some(event) = event_rx.recv().await {
        to_linux_events(event, &mut events)?;
        for event in &events {
            uidev.write_event(&event)?;
        }
        events.clear();
    }
    Ok(())
}

fn to_linux_events(event: InputEvent, out: &mut Vec<LinuxInputEvent>) -> Result<(), Error> {
    let time = SystemTime::now().try_into()?;

    let events: &[LinuxInputEvent] = match event {
        // mouse move
        InputEvent::MouseMove { dx, dy } => &[
            LinuxInputEvent {
                time,
                event_code: EventCode::EV_REL(EV_REL::REL_X),
                value: dx as _,
            },
            LinuxInputEvent {
                time,
                event_code: EventCode::EV_REL(EV_REL::REL_Y),
                value: dy as _,
            },
        ],

        // mouse click
        InputEvent::MouseButtonDown { button } => &[LinuxInputEvent {
            time,
            event_code: EventCode::EV_KEY(button.into()),
            value: 1,
        }],
        InputEvent::MouseButtonUp { button } => &[LinuxInputEvent {
            time,
            event_code: EventCode::EV_KEY(button.into()),
            value: 0,
        }],

        // mouse scroll
        InputEvent::MouseScroll {
            direction: MouseScrollDirection::Up { clicks },
        } => &[LinuxInputEvent {
            time,
            event_code: EventCode::EV_REL(EV_REL::REL_WHEEL),
            value: clicks as _,
        }],
        InputEvent::MouseScroll {
            direction: MouseScrollDirection::Down { clicks },
        } => &[LinuxInputEvent {
            time,
            event_code: EventCode::EV_REL(EV_REL::REL_WHEEL),
            value: -(clicks as i32),
        }],

        // keypress
        InputEvent::KeyDown { key } => &[LinuxInputEvent {
            time,
            event_code: EventCode::EV_KEY(key.into()),
            value: 1,
        }],
        InputEvent::KeyRepeat { key } => &[LinuxInputEvent {
            time,
            event_code: EventCode::EV_KEY(key.into()),
            value: 2,
        }],
        InputEvent::KeyUp { key } => &[LinuxInputEvent {
            time,
            event_code: EventCode::EV_KEY(key.into()),
            value: 0,
        }],
    };

    out.extend_from_slice(events);
    out.push(LinuxInputEvent {
        time,
        event_code: EventCode::EV_SYN(EV_SYN::SYN_REPORT),
        value: 0,
    });

    Ok(())
}
