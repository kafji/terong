use crate::protocol::{InputEvent, KeyCode, MouseButton};
use anyhow::{anyhow, Error};
use evdev_rs::{
    enums::{BusType, EventCode, EventType, EV_KEY, EV_REL, EV_SYN},
    DeviceWrapper, InputEvent as LinuxInputEvent, UInputDevice, UninitDevice,
};
use std::{convert::TryInto, iter, time::SystemTime};
use strum::IntoEnumIterator;
use tokio::{
    sync::mpsc,
    task::{self, JoinHandle},
};

pub fn start(proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) -> JoinHandle<()> {
    task::spawn(run(proto_event_rx))
}

async fn run(proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) {
    run_consumer(proto_event_rx).await.unwrap()
}

// useful documentations:
// https://www.kernel.org/doc/html/v5.15/input/uinput.html
// https://www.kernel.org/doc/html/v5.15/input/event-codes.html
// https://github.com/torvalds/linux/blob/master/include/linux/input.h
// https://www.freedesktop.org/software/libevdev/doc/latest/
// https://ndesh26.github.io/evdev-rs/evdev_rs/
// https://github.com/ndesh26/evdev-rs/issues/75
// https://github.com/ndesh26/evdev-rs/pull/76

async fn run_consumer(
    mut proto_event_rx: mpsc::UnboundedReceiver<InputEvent>,
) -> Result<(), Error> {
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

    let uidev = UInputDevice::create_from_device(&dev)?;

    while let Some(ie) = proto_event_rx.recv().await {
        let es: Vec<LinuxInputEvent> = ie.try_into()?;
        task::block_in_place(|| {
            for e in &es {
                uidev.write_event(&e)?;
            }
            Result::<_, Error>::Ok(())
        })?;
    }

    Ok(())
}

impl TryInto<Vec<LinuxInputEvent>> for InputEvent {
    type Error = anyhow::Error;
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
            InputEvent::MouseScroll {} => todo!(),
            InputEvent::KeyDown { key } => vec![(EventCode::EV_KEY(key.into()), 1)],
            InputEvent::KeyUp { key } => vec![(EventCode::EV_KEY(key.into()), 0)],
        };

        let es = es
            .into_iter()
            .map(|(event_code, value)| LinuxInputEvent {
                time,
                event_code,
                value,
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
