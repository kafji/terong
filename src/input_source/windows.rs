use super::event::{LocalInputEvent, MousePosition};
use crate::{
    input_source::controller::InputController,
    protocol::{InputEvent, KeyCode, MouseButton},
};
use once_cell::sync::OnceCell;
use std::{
    ffi::c_void,
    mem,
    ptr::null,
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::{sync::mpsc, task};
use tracing::{debug, error, warn};
use windows::{
    core::PCWSTR,
    w,
    Win32::{
        Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::HBRUSH,
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CallNextHookEx, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
            PostMessageW, RegisterClassExW, SetCursorPos, SetWindowsHookExW, ShowWindow,
            SystemParametersInfoW, UnhookWindowsHookEx, CW_USEDEFAULT, HCURSOR, HC_ACTION, HHOOK,
            HICON, KBDLLHOOKSTRUCT, KF_REPEAT, MSG, MSLLHOOKSTRUCT, SHOW_WINDOW_CMD,
            SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WH_KEYBOARD_LL, WH_MOUSE_LL,
            WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CREATE, WM_DWMNCRENDERINGCHANGED,
            WM_GETMINMAXINFO, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
            WM_NCCALCSIZE, WM_NCCREATE, WM_QUIT, WNDCLASSEXW, WNDCLASS_STYLES,
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

    let class = unsafe {
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
            lpszClassName: PCWSTR::null(),
            hIconSm: HICON::default(),
        };
        let ptr = RegisterClassExW(&class);
        PCWSTR::from_raw(ptr as *const _)
    };

    let window = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
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
                        debug!("dispatching message {:?}", msg);
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

fn set_should_capture_flag(x: bool) {
    SHOULD_CAPTURE.store(x, Ordering::SeqCst)
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => (),
        WM_GETMINMAXINFO => (),
        WM_NCCREATE => (),
        WM_NCCALCSIZE => (),
        WM_DWMNCRENDERINGCHANGED => (),
        n => warn!("unhandled message {}", n),
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
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
            unsafe {
                let (x, y) = *CENTRE_POS.get().unwrap();
                SetCursorPos(x, y);
            };
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
    let key = KeyCode::from_u16(hook_event.vkCode as _).unwrap();
    let event = match wparam.0 as u32 {
        WM_KEYDOWN => LocalInputEvent::KeyDown { key }.into(),
        WM_KEYUP => LocalInputEvent::KeyUp { key }.into(),
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
