use crate::transport::protocol::{
    windows::VirtualKey, InputEvent, MouseButton, MouseScrollDirection,
};
use std::mem::size_of;
use tokio::{
    sync::mpsc,
    task::{self, JoinHandle},
};
use windows::Win32::UI::{
    Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
        MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
        MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, VIRTUAL_KEY,
    },
    WindowsAndMessaging::{WHEEL_DELTA, XBUTTON1, XBUTTON2},
};

pub fn start(mut event_rx: mpsc::Receiver<InputEvent>) -> JoinHandle<()> {
    task::spawn_blocking(move || loop {
        let event = match event_rx.blocking_recv() {
            Some(x) => x,
            None => break,
        };

        let input = match event {
            InputEvent::MouseMove { dx, dy } => INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: dx as _,
                        dy: dy as _,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: Default::default(),
                    },
                },
            },

            InputEvent::MouseButtonDown { button } => INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: Default::default(),
                        dy: Default::default(),
                        mouseData: match button {
                            MouseButton::Mouse4 => XBUTTON1 as _,
                            MouseButton::Mouse5 => XBUTTON2 as _,
                            _ => 0,
                        },
                        dwFlags: match button {
                            MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
                            MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,
                            MouseButton::Middle => MOUSEEVENTF_MIDDLEDOWN,
                            MouseButton::Mouse4 | MouseButton::Mouse5 => MOUSEEVENTF_XDOWN,
                        },
                        time: 0,
                        dwExtraInfo: Default::default(),
                    },
                },
            },

            InputEvent::MouseButtonUp { button } => INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: Default::default(),
                        dy: Default::default(),
                        mouseData: match button {
                            MouseButton::Mouse4 => XBUTTON1 as _,
                            MouseButton::Mouse5 => XBUTTON2 as _,
                            _ => 0,
                        },
                        dwFlags: match button {
                            MouseButton::Left => MOUSEEVENTF_LEFTUP,
                            MouseButton::Right => MOUSEEVENTF_RIGHTUP,
                            MouseButton::Middle => MOUSEEVENTF_MIDDLEUP,
                            MouseButton::Mouse4 | MouseButton::Mouse5 => MOUSEEVENTF_XUP,
                        },
                        time: 0,
                        dwExtraInfo: Default::default(),
                    },
                },
            },

            InputEvent::MouseScroll { direction } => INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: Default::default(),
                        dy: Default::default(),
                        mouseData: {
                            match direction {
                                MouseScrollDirection::Up { clicks } => {
                                    let v = WHEEL_DELTA * clicks as u32;
                                    assert_eq!(v >> 31, 0);
                                    v
                                }
                                MouseScrollDirection::Down { clicks } => {
                                    let mut v = WHEEL_DELTA * clicks as u32;
                                    assert_eq!(v >> 31, 0);
                                    // assume two's complement
                                    v = !v;
                                    v
                                }
                            }
                        },
                        dwFlags: MOUSEEVENTF_WHEEL,
                        time: 0,
                        dwExtraInfo: Default::default(),
                    },
                },
            },

            InputEvent::KeyDown { key } | InputEvent::KeyRepeat { key } => INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: {
                            let vk: VirtualKey = key.into();
                            VIRTUAL_KEY(vk.0)
                        },
                        wScan: Default::default(),
                        dwFlags: Default::default(),
                        time: 0,
                        dwExtraInfo: Default::default(),
                    },
                },
            },

            InputEvent::KeyUp { key } => INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: {
                            let vk: VirtualKey = key.into();
                            VIRTUAL_KEY(vk.0)
                        },
                        wScan: Default::default(),
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: Default::default(),
                    },
                },
            },
        };

        unsafe { SendInput(&[input], size_of::<INPUT>() as _) };
    })
}
