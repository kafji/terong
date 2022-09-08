use super::event::{LocalInputEvent, MousePosition};
use crate::input_event::{KeyCode, MouseButton};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
};
use tokio::{
    select,
    sync::{
        mpsc::{self, UnboundedSender},
        watch,
    },
};
use tracing::{debug, warn};
use windows::Win32::{
    Foundation::{GetLastError, BOOL, LPARAM, LRESULT, WPARAM},
    System::{
        Console::{SetConsoleCtrlHandler, CTRL_C_EVENT},
        LibraryLoader::GetModuleHandleW,
        Threading::ExitProcess,
    },
    UI::{
        Input::KeyboardAndMouse::{
            VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_RCONTROL, VK_RETURN, VK_RMENU, VK_SPACE,
        },
        WindowsAndMessaging::{
            CallNextHookEx, GetMessageW, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
            HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL,
            WM_APP, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_QUIT,
        },
    },
};

/// Guard for unhooking hook.
///
/// Calls [UnhookWindowsHookEx] on drop.
struct Unhooker(HHOOK);

impl Drop for Unhooker {
    fn drop(&mut self) {
        let ok: bool = unsafe { UnhookWindowsHookEx(self.0) }.into();
        if !ok {
            let err = unsafe { GetLastError() };
            panic!("{:?}", err);
        }
    }
}

pub async fn run(
    event_sink: mpsc::UnboundedSender<LocalInputEvent>,
    mut capture_input_flag_source: watch::Receiver<bool>,
) {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    let listener = thread::spawn(move || run_listener(event_tx));

    loop {
        select! {
            _ = event_sink.closed() => {
                break;
            }
            _ = capture_input_flag_source.changed() => {
                let flag = *capture_input_flag_source.borrow();
                debug!("setting should capture flag to {}", flag);
                set_should_capture_flag(flag);
            }
            x = event_rx.recv() => {
                match x {
                    Some(event) => {
                        if let Err(_) = event_sink.send(event) {
                            break;
                        }
                    }
                    None => break,
                }

            }
        }
    }

    drop(event_rx);

    listener.join().unwrap();
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Debug)]
enum MessageCode {
    InputEvent = WM_APP,
}

fn run_listener(event_sink: mpsc::UnboundedSender<LocalInputEvent>) {
    // get module handle for this application
    let module = unsafe { GetModuleHandleW(None) }.unwrap();
    assert!(!module.is_invalid());

    // set low level mouse hook
    let _mouse_hook = Unhooker(
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), module, 0) }
            .expect("failed to set mouse hook"),
    );

    // set low level keyboard hook
    let _keyboard_hook = Unhooker(
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), module, 0) }
            .expect("failed to set keyboard hook"),
    );

    loop {
        // process message
        let mut msg = MSG::default();
        // blocking wait for message
        let ok = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match ok.0 {
            -1 => {
                let err = unsafe { GetLastError() };
                panic!("{:?}", err);
            }
            0 => {
                debug!("received quit message");
                break;
            }
            _ => {
                match msg.message {
                    WM_QUIT => {
                        debug!("received quit message");
                        break;
                    }
                    n if n == MessageCode::InputEvent as _ => {
                        // get pointer to input event from lparam
                        let ptr_event = msg.lParam.0 as *mut LocalInputEvent;
                        // acquire input event, the box will ensure it will be freed
                        let event = *unsafe { Box::from_raw(ptr_event) };
                        // propagate input event to the sink
                        if let Err(_) = event_sink.send(event) {
                            debug!("event sink was closed");
                            break;
                        }
                    }
                    _ => {
                        warn!("unhandled message {:?}", msg);
                    }
                }
            }
        }
    }
}

/// If the hooks should capture user inputs.
static SHOULD_CAPTURE: AtomicBool = AtomicBool::new(false);

fn should_capture() -> bool {
    SHOULD_CAPTURE.load(Ordering::Relaxed)
}

fn set_should_capture_flag(x: bool) {
    SHOULD_CAPTURE.store(x, Ordering::Relaxed)
}

/// Procedure for low level mouse hook.
extern "system" fn mouse_hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // per documentation, ncode will always be HC_ACTION
    assert_eq!(ncode, HC_ACTION as _);

    // pointer dance to get MSLLHOOKSTRUCT from lparam
    let ptr_hook_event = lparam.0 as *const MSLLHOOKSTRUCT;
    let hook_event = unsafe { *ptr_hook_event };

    // debug!("received mouse hook event {:?}", hook_event);

    // map hook event to input event
    let event = match wparam.0 as u32 {
        WM_MOUSEMOVE => {
            let x = hook_event.pt.x;
            let y = hook_event.pt.y;
            LocalInputEvent::MousePosition(MousePosition { x, y }).into()
        }
        WM_LBUTTONDOWN => LocalInputEvent::MouseButtonDown {
            button: MouseButton::Left,
        }
        .into(),
        WM_LBUTTONUP => LocalInputEvent::MouseButtonUp {
            button: MouseButton::Left,
        }
        .into(),
        _ => None,
    };

    // send input event in a message to the mq
    if let Some(event) = event {
        let event = Box::new(event);
        let event: &mut LocalInputEvent = Box::leak(event);
        let ptr_event = event as *mut _;
        unsafe {
            let b = PostMessageW(
                None,
                MessageCode::InputEvent as _,
                WPARAM::default(),
                LPARAM(ptr_event as isize),
            );
            let b: bool = b.into();
            assert_eq!(b, true);
        }

        // if should capture, capture the event instead of passing it through
        if should_capture() {
            return LRESULT::default();
        }
    }

    // passthrough
    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}

/// Procedure for low level keyboard hook.
extern "system" fn keyboard_hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // per documentation, ncode will always be HC_ACTION
    assert_eq!(ncode, HC_ACTION as _);

    // pointer dance to get KBDLLHOOKSTRUCT from lparam
    let ptr_hook_event = lparam.0 as *const KBDLLHOOKSTRUCT;
    let hook_event = unsafe { *ptr_hook_event };

    // debug!("received keyboard hook event {:?}", hook_event);

    // map hook event to input event
    let key = VkCode(hook_event.vkCode).into();
    let event = match wparam.0 as u32 {
        WM_KEYDOWN => LocalInputEvent::KeyDown { key }.into(),
        WM_KEYUP => LocalInputEvent::KeyUp { key }.into(),
        _ => None,
    };

    // send input event in a message to the mq
    if let Some(event) = event {
        let event = Box::new(event);
        let event: &mut LocalInputEvent = Box::leak(event);
        let ptr_event = event as *mut _;
        unsafe {
            let b = PostMessageW(
                None,
                MessageCode::InputEvent as _,
                WPARAM::default(),
                LPARAM(ptr_event as isize),
            );
            let b: bool = b.into();
            assert_eq!(b, true);
        }

        // if should capture, capture the event instead of passing it through
        if should_capture() {
            return LRESULT::default();
        }
    }

    // passthrough
    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}

/// Type to aid conversion from Windows' virtual key code to app's key code.
struct VkCode(u32);

impl Into<KeyCode> for VkCode {
    fn into(self) -> KeyCode {
        let vk_code = self.0 as u16;
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
        match vk_code {
            n if n == VK_SPACE.0 => KeyCode::Space,
            n if n == VK_RETURN.0 => KeyCode::Enter,
            0x41..=0x5A => {
                let key_a = KeyCode::A as u16;
                let key = if key_a < 0x41 {
                    let d = 0x41 - key_a;
                    vk_code - d
                } else {
                    let d = key_a - 0x41;
                    vk_code + d
                };
                unsafe { KeyCode::from_u16(key as _) }
            }
            n if n == VK_LCONTROL.0 => KeyCode::LeftCtrl,
            n if n == VK_RCONTROL.0 => KeyCode::RightCtrl,
            n if n == VK_LMENU.0 => KeyCode::LeftAlt,
            n if n == VK_RMENU.0 => KeyCode::RightAlt,
            n => todo!("{:?}", n),
        }
    }
}
