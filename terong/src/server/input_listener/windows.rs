use crate::{
    event::{InputEvent, KeyCode, MouseButton, MousePosition},
    server::input_listener::Signal,
};
use crossbeam::channel::{Receiver, Sender, TryRecvError};
use log::{debug, warn};
use once_cell::sync::OnceCell;
use windows::Win32::{
    Foundation::{BOOL, LPARAM, LRESULT, WPARAM},
    System::{
        Console::{SetConsoleCtrlHandler, CTRL_C_EVENT},
        LibraryLoader::GetModuleHandleW,
        Threading::{ExitProcess, GetCurrentThreadId},
    },
    UI::WindowsAndMessaging::{
        CallNextHookEx, PeekMessageW, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
        HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, PM_REMOVE, WH_KEYBOARD_LL,
        WH_MOUSE_LL, WM_APP, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
        WM_QUIT,
    },
};

/// Guard for unhooking hook.
///
/// Calls [UnhookWindowsHookEx] on drop.
struct Unhooker(HHOOK);

impl Drop for Unhooker {
    fn drop(&mut self) {
        unsafe { UnhookWindowsHookEx(self.0) };
    }
}

/// Message queue thread id.
///
/// This is the thread id where the message queue and hook procedures are executed.
static MQ_THREAD_ID: OnceCell<u32> = OnceCell::new();

/// Assert current thread is the message queue thread.
macro_rules! assert_in_mq_thread {
    () => {
        let current_thread_id = unsafe { GetCurrentThreadId() };
        let mq_thread_id = *MQ_THREAD_ID.get().expect("mq thread id is unset");
        assert!(current_thread_id == mq_thread_id);
    };
}

const WM_APP_INPUT_EVENT: u32 = WM_APP;

/// Run the Windows input event listener.
///
/// This function must not be called more than once.
pub fn run(
    event_sink: Sender<InputEvent>,
    signal_source: Receiver<Signal>,
    stop_signal_source: Receiver<()>,
) {
    // set mq thread id
    let thread_id = unsafe { GetCurrentThreadId() };
    MQ_THREAD_ID
        .set(thread_id)
        .expect("mq thread id already set");

    // set ctrl+c trap
    if {
        let b: bool = unsafe { SetConsoleCtrlHandler(Some(ctrl_handler), true) }.into();
        !b
    } {
        panic!("failed to set ctrl handler");
    }

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
        // handle stop signal
        match stop_signal_source.try_recv() {
            Ok(_) => {
                debug!("received stop signal");
                break;
            }
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => {
                debug!("stop signal channel was disconnected");
                // channel disconnected. app terminating? exiting loop.
                break;
            }
        }

        // handle general signal
        match signal_source.try_recv() {
            Ok(signal) => {
                debug!("received signal {:?}", signal);
                match signal {
                    Signal::SetShouldCapture(should_capture) => set_should_capture(should_capture),
                }
            }
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => {
                debug!("signal channel was disconnected");
                // channel disconnected. app terminating? exiting loop.
                break;
            }
        }

        // process message
        let mut msg = MSG::default();
        let has_msg = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE) }.into();
        if has_msg {
            match msg.message {
                WM_QUIT => {
                    debug!("received quit message");
                    break;
                }
                WM_APP_INPUT_EVENT => {
                    // get pointer to input event from lparam
                    let ptr_event = msg.lParam.0 as *mut InputEvent;
                    // acquire input event, the box will ensure it will be freed
                    let event = *unsafe { Box::from_raw(ptr_event) };
                    // propagate input event to the sink
                    event_sink.send(event).unwrap();
                }
                _ => {
                    warn!("unhandled message {:?}", msg);
                }
            }
        }
    }
}

/// Procedure for ctrl+c trap.
extern "system" fn ctrl_handler(ctrl_type: u32) -> BOOL {
    match ctrl_type {
        CTRL_C_EVENT => {
            debug!("received ctrl+c event, exiting process");
            unsafe { ExitProcess(0) }
        }
        _ => BOOL(0),
    }
}

/// If the hooks should capture user inputs.
static mut SHOULD_CAPTURE: bool = false;

/// Get the should capture flag safely.
fn should_capture() -> bool {
    assert_in_mq_thread!();
    unsafe { SHOULD_CAPTURE }
}

/// Safely set the should capture flag value.
fn set_should_capture(x: bool) {
    assert_in_mq_thread!();
    unsafe { SHOULD_CAPTURE = x }
}

/// Procedure for low level mouse hook.
extern "system" fn mouse_hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // per documentation, ncode will always be HC_ACTION
    assert_eq!(ncode, HC_ACTION as _);

    // pointer dance to get MSLLHOOKSTRUCT from lparam
    let ptr_hook_event = lparam.0 as *const MSLLHOOKSTRUCT;
    let hook_event = unsafe { *ptr_hook_event };

    debug!("received mouse event {:?}", hook_event);

    // map hook event to input event
    let event = match wparam.0 as u32 {
        WM_MOUSEMOVE => {
            let x = hook_event.pt.x;
            let y = hook_event.pt.y;
            InputEvent::MousePosition(MousePosition { x, y }).into()
        }
        WM_LBUTTONDOWN => InputEvent::MouseButtonDown {
            button: MouseButton::Left,
        }
        .into(),
        WM_LBUTTONUP => InputEvent::MouseButtonUp {
            button: MouseButton::Left,
        }
        .into(),
        _ => {
            debug!("unhandled mouse event {:?}", hook_event);
            None
        }
    };

    // send input event in a message to the mq
    if let Some(event) = event {
        let event = Box::new(event);
        let event: &mut InputEvent = Box::leak(event);
        let ptr_event = event as *mut _;
        unsafe {
            let b = PostMessageW(
                None,
                WM_APP_INPUT_EVENT,
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

    debug!("received keyboard event {:?}", hook_event);

    // map hook event to input event
    let key = VkCode(hook_event.vkCode).into();
    let event = match wparam.0 as u32 {
        WM_KEYDOWN => InputEvent::KeyDown { key }.into(),
        WM_KEYUP => InputEvent::KeyUp { key }.into(),
        _ => {
            debug!("unhandled keyboard event {:?}", hook_event);
            None
        }
    };

    // send input event in a message to the mq
    if let Some(event) = event {
        let event = Box::new(event);
        let event: &mut InputEvent = Box::leak(event);
        let ptr_event = event as *mut _;
        unsafe {
            let b = PostMessageW(
                None,
                WM_APP_INPUT_EVENT,
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
        let vk_code = self.0;
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
        match vk_code {
            0x41..=0x5A => {
                let key_a = KeyCode::A as u32;
                let key = if key_a < 0x41 {
                    let d = 0x41 - key_a;
                    vk_code - d
                } else {
                    let d = key_a - 0x41;
                    vk_code + d
                };
                unsafe { KeyCode::from_u16(key as _) }
            }
            _ => todo!(),
        }
    }
}
