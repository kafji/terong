use crate::protocol::{InputEvent, KeyCode, MouseButton};
use anyhow::Error;
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
    let dev = UninitDevice::new().expect("failed to create virtual evdev device");

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

impl Into<EV_KEY> for KeyCode {
    fn into(self) -> EV_KEY {
        use KeyCode::*;
        use EV_KEY::*;
        match self {
            Escape => KEY_ESC,

            F1 => KEY_F1,
            F2 => KEY_F2,
            F3 => KEY_F3,
            F4 => KEY_F4,
            F5 => KEY_F5,
            F6 => KEY_F6,
            F7 => KEY_F7,
            F8 => KEY_F8,
            F9 => KEY_F9,
            F10 => KEY_F10,
            F11 => KEY_F11,
            F12 => KEY_F12,

            PrintScreen => KEY_PRINT,
            ScrollLock => KEY_SCROLLLOCK,
            PauseBreak => KEY_PAUSE,

            Grave => KEY_GRAVE,

            D1 => KEY_1,
            D2 => KEY_2,
            D3 => KEY_3,
            D4 => KEY_4,
            D5 => KEY_5,
            D6 => KEY_6,
            D7 => KEY_7,
            D8 => KEY_8,
            D9 => KEY_9,
            D0 => KEY_0,

            Minus => KEY_MINUS,
            Equal => KEY_EQUAL,

            A => KEY_A,
            B => KEY_B,
            C => KEY_C,
            D => KEY_D,
            E => KEY_E,
            F => KEY_F,
            G => KEY_G,
            H => KEY_H,
            I => KEY_I,
            J => KEY_J,
            K => KEY_K,
            L => KEY_L,
            M => KEY_M,
            N => KEY_N,
            O => KEY_O,
            P => KEY_P,
            Q => KEY_Q,
            R => KEY_R,
            S => KEY_S,
            T => KEY_T,
            U => KEY_U,
            V => KEY_V,
            W => KEY_W,
            X => KEY_X,
            Y => KEY_Y,
            Z => KEY_Z,

            LeftBrace => KEY_LEFTBRACE,
            RightBrace => KEY_RIGHTBRACE,

            SemiColon => KEY_SEMICOLON,
            Apostrophe => KEY_APOSTROPHE,

            Comma => KEY_COMMA,
            Dot => KEY_DOT,
            Slash => KEY_SLASH,

            Backspace => KEY_BACKSPACE,
            BackSlash => KEY_BACKSLASH,
            Enter => KEY_ENTER,

            Space => KEY_SPACE,

            Tab => EV_KEY::KEY_TAB,
            CapsLock => KEY_CAPSLOCK,

            LeftShift => KEY_LEFTSHIFT,
            RightShift => KEY_RIGHTSHIFT,

            LeftCtrl => KEY_LEFTALT,
            RightCtrl => KEY_RIGHTCTRL,

            LeftAlt => KEY_LEFTALT,
            RightAlt => KEY_RIGHTALT,

            LeftMeta => KEY_LEFTMETA,
            RightMeta => KEY_RIGHTMETA,

            Insert => KEY_INSERT,
            Delete => KEY_DELETE,

            Home => KEY_HOME,
            End => KEY_END,

            PageUp => KEY_PAGEUP,
            PageDown => KEY_PAGEDOWN,

            Up => KEY_UP,
            Left => KEY_LEFT,
            Down => KEY_DOWN,
            Right => KEY_RIGHT,
        }
    }
}

impl Into<EV_KEY> for MouseButton {
    fn into(self) -> EV_KEY {
        match self {
            MouseButton::Left => EV_KEY::BTN_LEFT,
            MouseButton::Right => EV_KEY::BTN_RIGHT,
            MouseButton::Middle => EV_KEY::BTN_MIDDLE,
            MouseButton::Mouse4 => EV_KEY::BTN_4,
            MouseButton::Mouse5 => EV_KEY::BTN_5,
        }
    }
}
