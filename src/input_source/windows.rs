use super::event::{LocalInputEvent, MousePosition};
use crate::{
    input_source::controller::InputController,
    protocol::{windows::VirtualKey, InputEvent, KeyCode, MouseButton, MouseScrollDirection},
};
use std::{cell::Cell, cmp, ffi::c_void, time::Duration};
use tokio::{sync::mpsc, task};
use tracing::{debug, error, warn};
use windows::Win32::{
    Foundation::{GetLastError, LPARAM, LRESULT, RECT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, PostMessageW, SetCursorPos,
        SetWindowsHookExW, SystemParametersInfoW, UnhookWindowsHookEx, HC_ACTION, HHOOK,
        KBDLLHOOKSTRUCT, MOUSEHOOKSTRUCTEX_MOUSE_DATA, MSG, MSLLHOOKSTRUCT, SPI_GETWORKAREA,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WHEEL_DELTA, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_APP,
        WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
        WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
        WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
    },
};

/// RAII for unhooking hook.
///
/// Calls [UnhookWindowsHookEx] on drop.
struct Unhooker(HHOOK);

impl Drop for Unhooker {
    fn drop(&mut self) {
        let ok: bool = unsafe { UnhookWindowsHookEx(self.0) }.into();
        if !ok {
            error!("failed to unhook {:?}", self.0);
        }
    }
}

pub fn start(event_tx: mpsc::Sender<InputEvent>) -> task::JoinHandle<()> {
    task::spawn_blocking(|| run_input_source(event_tx))
}

/// Application defined message code.
///
/// https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-app
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Debug)]
enum MessageCode {
    InputEvent = WM_APP,
}

fn run_input_source(event_tx: mpsc::Sender<InputEvent>) {
    let mut controller = InputController::new(event_tx);

    // get module handle for this application
    let module = unsafe { GetModuleHandleW(None) }.expect("failed to get current module handle");
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

    let mut msg = MSG::default();
    let mut prev_event = None;

    loop {
        // set cursor position to its locked position if we're grabbing input
        if get_grab_input() {
            let MousePosition { x, y } = get_cursor_locked_pos();
            unsafe { SetCursorPos(x as _, y as _) };
        }

        // wait for message
        let ok = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match ok.0 {
            -1 => unsafe {
                let err = GetLastError();
                error!(?err);
                break;
            },
            0 => {
                debug!("received quit message");
                break;
            }
            _ => {
                match msg.message {
                    n if n == MessageCode::InputEvent as _ => {
                        // get pointer to input event from lparam
                        let ptr_event = msg.lParam.0 as *mut (LocalInputEvent, Duration);
                        // acquire input event, the box will ensure it will be freed
                        let (new_event, _) = *unsafe { Box::from_raw(ptr_event) };

                        // maps repeated key down events into key repeat event
                        let event = match (prev_event, &new_event) {
                            (
                                Some(LocalInputEvent::KeyDown { key: prev_key }),
                                LocalInputEvent::KeyDown { key },
                            ) if key == &prev_key => LocalInputEvent::KeyRepeat { key: *key },
                            _ => new_event,
                        };

                        prev_event = Some(new_event);

                        // propagate input event to the sink
                        let grab_input = controller.on_input_event(event).unwrap();
                        if grab_input != get_grab_input() {
                            set_grab_input(grab_input);
                        }
                    }
                    _ => unsafe {
                        DispatchMessageW(&msg);
                    },
                }
            }
        }
    }
}

fn get_screen_center() -> (i16 /* x */, i16 /* y */) {
    let mut rect = RECT::default();
    let ptr_rect = &mut rect as *mut _ as *mut c_void;
    let b = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            ptr_rect,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        )
    };
    assert!(b == true);
    let x = (rect.right / 2) as _;
    let y = (rect.bottom / 2) as _;
    (x, y)
}

thread_local! {
    static GRAB_INPUT: Cell<bool> = Cell::new(false);

    static CURSOR_LOCKED_POS: MousePosition = get_screen_center().into();
}

fn get_grab_input() -> bool {
    GRAB_INPUT.with(|x| x.get())
}

fn set_grab_input(value: bool) {
    GRAB_INPUT.with(|x| x.set(value));
}

fn get_cursor_locked_pos() -> MousePosition {
    CURSOR_LOCKED_POS.with(|x| *x)
}

/// Procedure for low level mouse hook.
extern "system" fn mouse_hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // per documentation, ncode will always be HC_ACTION
    assert_eq!(ncode, HC_ACTION as _);

    // pointer dance to get MSLLHOOKSTRUCT from lparam
    let ptr_hook_event = lparam.0 as *const MSLLHOOKSTRUCT;
    let hook_event = unsafe { *ptr_hook_event };

    // map hook event to input event
    let event = match wparam.0 as u32 {
        WM_MOUSEMOVE => {
            let x = hook_event.pt.x as _;
            let y = hook_event.pt.y as _;
            let pos = MousePosition { x, y };

            if get_grab_input() {
                let cpos = get_cursor_locked_pos();
                let mvment = cpos.delta_to(&pos);
                LocalInputEvent::MouseMove(mvment)
            } else {
                LocalInputEvent::MousePosition(pos)
            }
            .into()
        }

        WM_LBUTTONDOWN => LocalInputEvent::MouseButtonDown {
            button: MouseButton::Left,
        }
        .into(),
        WM_LBUTTONUP => LocalInputEvent::MouseButtonUp {
            button: MouseButton::Left,
        }
        .into(),

        WM_RBUTTONDOWN => LocalInputEvent::MouseButtonDown {
            button: MouseButton::Right,
        }
        .into(),
        WM_RBUTTONUP => LocalInputEvent::MouseButtonUp {
            button: MouseButton::Right,
        }
        .into(),

        WM_MBUTTONDOWN => LocalInputEvent::MouseButtonDown {
            button: MouseButton::Middle,
        }
        .into(),
        WM_MBUTTONUP => LocalInputEvent::MouseButtonUp {
            button: MouseButton::Middle,
        }
        .into(),

        WM_XBUTTONDOWN => get_mouse_button(hook_event.mouseData)
            .map(|button| LocalInputEvent::MouseButtonDown { button }),
        WM_XBUTTONUP => get_mouse_button(hook_event.mouseData)
            .map(|button| LocalInputEvent::MouseButtonUp { button }),

        WM_MOUSEWHEEL => {
            let delta = {
                let mut bytes = [0; 2];
                bytes.copy_from_slice(&hook_event.mouseData.0.to_be_bytes()[..2]);
                i16::from_be_bytes(bytes)
            };
            let clicks = delta / WHEEL_DELTA as i16;
            let direction = match clicks.cmp(&0) {
                cmp::Ordering::Less => MouseScrollDirection::Down {
                    clicks: clicks.abs() as _,
                }
                .into(),
                cmp::Ordering::Equal => None,
                cmp::Ordering::Greater => MouseScrollDirection::Up {
                    clicks: clicks.abs() as _,
                }
                .into(),
            };
            direction.map(|direction| LocalInputEvent::MouseScroll { direction })
        }

        action => {
            warn!(?action, "unhandled mouse event");
            None
        }
    };

    if let Some(event) = event {
        let time = Duration::from_millis(hook_event.time as _);
        post_input_event(event, time);
    }

    if get_grab_input() {
        LRESULT(1)
    } else {
        unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
    }
}

/// Procedure for low level keyboard hook.
extern "system" fn keyboard_hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // per documentation, ncode will always be HC_ACTION
    assert_eq!(ncode, HC_ACTION as _);

    // pointer dance to get KBDLLHOOKSTRUCT from lparam
    let ptr_hook_event = lparam.0 as *const KBDLLHOOKSTRUCT;
    let hook_event = unsafe { *ptr_hook_event };

    // map hook event to input event
    let key = KeyCode::from_virtual_key(VirtualKey(hook_event.vkCode as _)).unwrap();
    let event = match wparam.0 as u32 {
        WM_KEYDOWN | WM_SYSKEYDOWN => { LocalInputEvent::KeyDown { key } }.into(),

        WM_KEYUP | WM_SYSKEYUP => LocalInputEvent::KeyUp { key }.into(),

        action => {
            warn!(?action, "unhandled keyboard event");
            None
        }
    };

    if let Some(event) = event {
        let time = Duration::from_millis(hook_event.time as _);
        post_input_event(event, time);
    }

    if get_grab_input() {
        LRESULT(1)
    } else {
        unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
    }
}

/// Send input event to the message queue.
///
/// Retruns `true` if event should be consumed, `false` if event should be forwarded to the next hook.
fn post_input_event(event: LocalInputEvent, time: Duration) {
    let event = {
        let x = Box::new((event, time));
        Box::leak(x)
    };
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
}

fn get_mouse_button(data: MOUSEHOOKSTRUCTEX_MOUSE_DATA) -> Option<MouseButton> {
    let mut bytes = [0; 2];
    bytes.copy_from_slice(&data.0.to_be_bytes()[..2]);
    let value = u16::from_be_bytes(bytes);
    match value {
        n if n == XBUTTON1.0 as _ => MouseButton::Mouse4.into(),
        n if n == XBUTTON2.0 as _ => MouseButton::Mouse5.into(),
        _ => None,
    }
}
