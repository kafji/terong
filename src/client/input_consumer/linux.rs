use crate::input_event::InputEvent;
use anyhow::Error;
use evdev_rs::{
    enums::{BusType, EventCode, EventType, EV_KEY, EV_REL, EV_SYN},
    Device, DeviceWrapper, InputEvent as LinuxInputEvent, ReadFlag, UInputDevice, UninitDevice,
};
use nix::fcntl::OFlag;
use std::{convert::TryInto, io::Read, os::unix::prelude::OpenOptionsExt, time::SystemTime};
use tokio::{sync::mpsc, task::JoinHandle};

pub fn start(proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) -> JoinHandle<()> {
    tokio::task::spawn(run(proto_event_rx))
}

async fn run(proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) {
    tokio::task::spawn_blocking(|| run_consumer(proto_event_rx))
        .await
        .unwrap()
        .unwrap()
}

// https://www.kernel.org/doc/html/v5.15/input/uinput.html

// https://www.kernel.org/doc/html/v5.15/input/event-codes.html

// https://github.com/torvalds/linux/blob/master/include/linux/input.h

// https://www.freedesktop.org/software/libevdev/doc/latest/

// https://github.com/ndesh26/evdev-rs/issues/75

// https://github.com/ndesh26/evdev-rs/pull/76

fn run_consumer(mut proto_event_rx: mpsc::UnboundedReceiver<InputEvent>) -> Result<(), Error> {
    {
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

    // let mut dev = Device::new_from_file(uinput).unwrap();

    let dev = UninitDevice::new().unwrap();

    dev.set_bustype(BusType::BUS_USB as _);
    dev.set_name("Terong Virtual Device");

    dev.enable_event_type(&EventType::EV_KEY)?;
    dev.enable_event_code(&EventCode::EV_KEY(EV_KEY::BTN_LEFT), None)?;
    dev.enable_event_code(&EventCode::EV_KEY(EV_KEY::BTN_RIGHT), None)?;
    dev.enable_event_code(&EventCode::EV_KEY(EV_KEY::KEY_SPACE), None)?;

    dev.enable_event_type(&EventType::EV_REL)?;
    dev.enable_event_code(&EventCode::EV_REL(EV_REL::REL_X), None)?;
    dev.enable_event_code(&EventCode::EV_REL(EV_REL::REL_Y), None)?;

    dev.enable_event_type(&EventType::EV_SYN)?;
    dev.enable_event_code(&EventCode::EV_SYN(EV_SYN::SYN_REPORT), None)?;

    let ui_dev = UInputDevice::create_from_device(&dev).unwrap();

    loop {
        let _ = std::io::stdin().read(&mut [0; 1]).unwrap();

        let pe = match proto_event_rx.blocking_recv() {
            Some(x) => x,
            None => break,
        };
        dbg!(&pe);

        let pe: Vec<LinuxInputEvent> = pe.try_into().unwrap();
        for e in &pe {
            dbg!(&e);
            ui_dev.write_event(&e).unwrap();
        }

        // let time = SystemTime::now().try_into()?;
        let ev = LinuxInputEvent {
            time: pe.first().unwrap().time,
            event_code: EventCode::EV_SYN(EV_SYN::SYN_REPORT),
            value: 0,
        };
        dbg!(&ev);
        ui_dev.write_event(&ev)?;
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
            InputEvent::MouseButtonDown { button } => todo!(),
            InputEvent::MouseButtonUp { button } => todo!(),
            InputEvent::MouseScroll {} => todo!(),
            InputEvent::KeyDown { key } => vec![(EventCode::EV_KEY(EV_KEY::KEY_SPACE), 1)],
            InputEvent::KeyUp { key } => todo!(),
        };

        let es = es
            .into_iter()
            .map(|(event_code, value)| LinuxInputEvent {
                time,
                event_code,
                value,
            })
            .collect();

        Ok(es)
    }
}
