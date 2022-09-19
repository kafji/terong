use super::event::{LocalInputEvent, MousePosition};
use crate::{
    input_source::controller::InputController,
    protocol::{
        windows::{ScanCode, VirtualKey},
        InputEvent, KeyCode, MouseButton,
    },
};
use once_cell::sync::OnceCell;
use std::{
    ffi::c_void,
    mem::{self, size_of},
    ptr::null,
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::{sync::mpsc, task};
use tracing::{debug, error, warn};
use windows::{
    core::PCWSTR,
    w,
    Win32::{
        Devices::HumanInterfaceDevice::{
            HID_USAGE_GENERIC_KEYBOARD, HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
        },
        Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::HBRUSH,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::{
                GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
                RAWINPUTHEADER, RAWKEYBOARD, RAWMOUSE, RIDEV_EXINPUTSINK, RIDEV_INPUTSINK,
                RIDEV_NOLEGACY, RID_INPUT, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
            },
            WindowsAndMessaging::{
                CallNextHookEx, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
                PostMessageW, RegisterClassExW, SetCursorPos, SetWindowsHookExW, ShowWindow,
                SystemParametersInfoW, UnhookWindowsHookEx, CW_USEDEFAULT, HCURSOR, HC_ACTION,
                HHOOK, HICON, KBDLLHOOKSTRUCT, KF_REPEAT, MSG, MSLLHOOKSTRUCT, RI_KEY_BREAK,
                RI_KEY_E0, RI_KEY_E1, RI_KEY_MAKE, SHOW_WINDOW_CMD, SPI_GETWORKAREA,
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOW_EX_STYLE,
                WINDOW_STYLE, WM_APP, WM_CREATE, WM_DWMNCRENDERINGCHANGED, WM_GETMINMAXINFO,
                WM_INPUT, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
                WM_NCCALCSIZE, WM_NCCREATE, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSEXW,
                WNDCLASS_STYLES,
            },
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
            error!("failed to unhook {:?}", self.0);
        }
    }
}

pub fn start(event_tx: mpsc::Sender<InputEvent>) -> task::JoinHandle<()> {
    task::spawn_blocking(|| run_input_source(event_tx))
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Debug)]
enum MessageCode {
    InputEvent = WM_APP,
}

// https://stackoverflow.com/a/16565324

// https://learn.microsoft.com/en-us/windows/win32/ipc/interprocess-communications#using-pipes-for-ipc
// https://learn.microsoft.com/en-us/windows/win32/ipc/anonymous-pipes

fn run_input_source(event_tx: mpsc::Sender<InputEvent>) {
    let mut controller = InputController::new(event_tx);

    unsafe {
        let mut rect = RECT::default();
        let ptr_rect = &mut rect as *mut _ as *mut c_void;
        let b = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            ptr_rect,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
        );
        assert!(b == true);
        let x = rect.right / 2;
        let y = rect.bottom / 2;
        CENTRE_POS.set((x, y)).unwrap();
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

    let class_name = w!("terong-window-class");

    unsafe {
        let class = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as _,
            style: WNDCLASS_STYLES::default(),
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: module,
            hIcon: HICON::default(),
            hCursor: HCURSOR::default(),
            hbrBackground: HBRUSH::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: class_name.into(),
            hIconSm: HICON::default(),
        };
        RegisterClassExW(&class);
    };

    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("Terong"),
            WINDOW_STYLE::default(),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            None,
            None,
            module,
            null(),
        )
    };

    unsafe {
        ShowWindow(window, SHOW_WINDOW_CMD::default());
    }

    unsafe {
        let devices = [
            RAWINPUTDEVICE {
                usUsagePage: HID_USAGE_PAGE_GENERIC,
                usUsage: HID_USAGE_GENERIC_MOUSE,
                dwFlags: RIDEV_NOLEGACY | RIDEV_INPUTSINK | RIDEV_EXINPUTSINK,
                hwndTarget: window,
            },
            RAWINPUTDEVICE {
                usUsagePage: HID_USAGE_PAGE_GENERIC,
                usUsage: HID_USAGE_GENERIC_KEYBOARD,
                dwFlags: RIDEV_NOLEGACY | RIDEV_INPUTSINK | RIDEV_EXINPUTSINK,
                hwndTarget: window,
            },
        ];
        RegisterRawInputDevices(&devices, size_of::<RAWINPUTDEVICE>() as _);
    }

    let mut previous_event = None;

    loop {
        let mut msg = MSG::default();
        let ok = unsafe { GetMessageW(&mut msg, window, 0, 0) };
        match ok.0 {
            -1 => unsafe {
                let err = GetLastError();
                error!("{:?}", err);
                break;
            },
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

                        dbg!(event);

                        let event2 = match (previous_event, &event) {
                            (
                                Some(LocalInputEvent::KeyDown { key: prev_key }),
                                LocalInputEvent::KeyDown { key },
                            ) if prev_key == *key => LocalInputEvent::KeyRepeat { key: prev_key },
                            _ => event,
                        };

                        previous_event = Some(event);

                        // propagate input event to the sink
                        let should_capture = controller.on_input_event(event2).unwrap();
                        set_should_capture_flag(should_capture);
                    }
                    _ => unsafe {
                        DispatchMessageW(&msg);
                    },
                }
            }
        }
    }
}

static CENTRE_POS: OnceCell<(i32, i32)> = OnceCell::new();

/// If the hooks should capture user inputs.
static SHOULD_CAPTURE: AtomicBool = AtomicBool::new(false);

fn should_capture() -> bool {
    SHOULD_CAPTURE.load(Ordering::SeqCst)
}

fn set_should_capture_flag(value: bool) {
    debug!(?value, "setting capture flag");
    SHOULD_CAPTURE.store(value, Ordering::SeqCst)
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let event = match msg {
        WM_INPUT => unsafe {
            let mut rawinput = RAWINPUT::default();
            let pdata = {
                let ptr = &mut rawinput as *mut _;
                ptr as *mut c_void
            };
            let pcbsize = {
                let mut x = size_of::<RAWINPUT>() as u32;
                &mut x as *mut _
            };
            GetRawInputData(
                HRAWINPUT(lparam.0),
                RID_INPUT,
                pdata,
                pcbsize,
                size_of::<RAWINPUTHEADER>() as _,
            );

            match rawinput.header.dwType {
                n if n == RIM_TYPEMOUSE.0 => None,
                n if n == RIM_TYPEKEYBOARD.0 => {
                    dbg!(rawinput.data.keyboard.Flags as u32 & RI_KEY_E0);
                    dbg!(rawinput.data.keyboard.Flags as u32 & RI_KEY_E1);

                    dbg!(rawinput.data.keyboard.MakeCode);

                    let key: KeyCode =
                        KeyCode::from_scancode(ScanCode(dbg!(rawinput.data.keyboard.MakeCode)))
                            .unwrap();

                    match rawinput.data.keyboard.Flags as u32 {
                        f if (f & RI_KEY_BREAK) == RI_KEY_MAKE => LocalInputEvent::KeyDown { key },
                        f if (f & RI_KEY_BREAK) == RI_KEY_BREAK => LocalInputEvent::KeyUp { key },
                        _ => todo!(),
                    }
                    .into()
                }
                _ => None,
            }
        },
        action => {
            debug!(?action, "unhandled message");
            None
        }
    };
    match event {
        Some(event) => {
            dbg!(event);

            let event = {
                let x = Box::new(event);
                Box::leak(x)
            };
            let ptr_event = event as *mut _;
            unsafe {
                let b = PostMessageW(
                    hwnd,
                    MessageCode::InputEvent as _,
                    WPARAM::default(),
                    LPARAM(ptr_event as isize),
                );
                let b: bool = b.into();
                assert_eq!(b, true);
            }
            LRESULT(0)
        }
        None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
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
        let event = {
            let x = Box::new(event);
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

        // if should capture, capture the event instead of passing it through
        if should_capture() {
            return LRESULT(1);
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
    let key = KeyCode::from_virtualkey(VirtualKey(hook_event.vkCode as _)).unwrap();
    let event = match wparam.0 as u32 {
        WM_KEYDOWN | WM_SYSKEYDOWN => LocalInputEvent::KeyDown { key }.into(),
        WM_KEYUP | WM_SYSKEYUP => LocalInputEvent::KeyUp { key }.into(),
        action => {
            warn!(?action, "unhandled keyboard event");
            None
        }
    };

    // send input event in a message to the mq
    if let Some(event) = event {
        let event = {
            let x = Box::new(event);
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

        // if should capture, capture the event instead of passing it through
        if should_capture() {
            return LRESULT(1);
        }
    }

    // passthrough
    unsafe { CallNextHookEx(None, ncode, wparam, lparam) }
}
