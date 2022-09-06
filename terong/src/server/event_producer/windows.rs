use crate::protocol::message::{InputEvent, Key};
use log::{debug, warn};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use windows::Win32::{
    Foundation::{GetLastError, BOOL, LPARAM, LRESULT, WPARAM},
    System::{
        Console::{SetConsoleCtrlHandler, CTRL_C_EVENT},
        LibraryLoader::GetModuleHandleW,
        Threading::ExitProcess,
    },
    UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
        HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL,
        WM_APP, WM_KEYDOWN, WM_KEYUP, WM_MOUSEMOVE,
    },
};

struct Unhooker(HHOOK);

impl Drop for Unhooker {
    fn drop(&mut self) {
        unsafe { UnhookWindowsHookEx(self.0) };
    }
}

pub fn run(event_sink: Sender<InputEvent>, stop_signal: Receiver<()>) {
    if {
        let b: bool = unsafe { SetConsoleCtrlHandler(Some(ctrl_handler), true) }.into();
        !b
    } {
        panic!("failed to set ctrl handler");
    }

    let module = unsafe { GetModuleHandleW(None) }.unwrap();
    assert!(!module.is_invalid());

    let _mouse_hook = Unhooker(
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), module, 0) }
            .expect("failed to set mouse hook"),
    );

    let _keyboard_hook = Unhooker(
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), module, 0) }
            .expect("failed to set keyboard hook"),
    );

    let mut msg = MSG::default();
    loop {
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match r.0 {
            -1 => {
                let error = unsafe { GetLastError() };
                panic!("{:?}", error);
            }
            0 => {
                debug!("quitting");
                break;
            }
            _ => match msg.message {
                WM_APP => {
                    let ptr_event = msg.lParam.0 as *mut InputEvent;
                    let event = *unsafe { Box::from_raw(ptr_event) };
                    event_sink.send(event).unwrap();
                }
                _ => {
                    warn!("unhandled message {:?}", msg);
                }
            },
        }
        match stop_signal.try_recv() {
            Ok(_) => break,
            Err(TryRecvError::Empty) => (),
            Err(err) => panic!("{}", err),
        }
    }
}

extern "system" fn ctrl_handler(ctrl_type: u32) -> BOOL {
    match ctrl_type {
        CTRL_C_EVENT => unsafe { ExitProcess(0) },
        _ => BOOL(0),
    }
}

extern "system" fn mouse_hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    assert_eq!(ncode, HC_ACTION as _);
    let hook_event = unsafe {
        let ptr = lparam.0 as *const MSLLHOOKSTRUCT;
        &*ptr
    };
    debug!("received mouse hook event {:?}", hook_event);
    match wparam.0 as u32 {
        WM_MOUSEMOVE => {
            // dbg!(hook);
        }
        _ => (),
    }
    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}

extern "system" fn keyboard_hook_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    assert_eq!(ncode, HC_ACTION as _);
    let hook_event = unsafe {
        let ptr = lparam.0 as *const KBDLLHOOKSTRUCT;
        &*ptr
    };
    debug!("received keyboard hook event {:?}", hook_event);
    let key = VkCode(hook_event.vkCode).into();
    let event = match wparam.0 as u32 {
        WM_KEYDOWN => InputEvent::KeyDown { key }.into(),
        WM_KEYUP => InputEvent::KeyUp { key }.into(),
        _ => None,
    };
    if let Some(event) = event {
        let event = Box::new(event);
        let event: &mut InputEvent = Box::leak(event);
        let ptr_event = event as *mut _;
        unsafe {
            let b = PostMessageW(None, WM_APP, WPARAM::default(), LPARAM(ptr_event as isize));
            let b: bool = b.into();
            assert_eq!(b, true);
        }
    }
    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}

struct VkCode(u32);

impl Into<Key> for VkCode {
    fn into(self) -> Key {
        let vk_code = self.0;
        // https://docs.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
        match vk_code {
            0x41..=0x5A => {
                let key_a = Key::A as u32;
                let key = if key_a < 0x41 {
                    let d = 0x41 - key_a;
                    vk_code - d
                } else {
                    let d = key_a - 0x41;
                    vk_code + d
                };
                unsafe { Key::from_u32(key) }
            }
            _ => todo!(),
        }
    }
}
