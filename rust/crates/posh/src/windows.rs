mod process;

use self::process::get_process_info;
use std::{ffi::c_void, sync::OnceLock};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
            SPI_GETWORKAREA, SWP_ASYNCWINDOWPOS, SWP_NOOWNERZORDER, SWP_NOZORDER,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SetWindowPos, SystemParametersInfoW,
        },
    },
    core::BOOL,
};

#[derive(Copy, Clone, Debug)]
enum Constraint {
    Width(u32),
    Height(u32),
}

#[derive(Copy, Clone, Debug)]
enum HorizontalPosition {
    Left,
    Right,
}

struct App {
    screen_rect: RECT,
    horizontal_position: HorizontalPosition,
    constraint: Constraint,
}

static APP: OnceLock<App> = OnceLock::new();

pub fn run() {
    unsafe {
        APP.get_or_init(|| {
            let screen_rect = {
                let mut rect = RECT::default();
                SystemParametersInfoW(
                    SPI_GETWORKAREA,
                    0,
                    Some(&mut rect as *mut RECT as *mut c_void),
                    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS::default(),
                )
                .unwrap();
                rect
            };
            App {
                screen_rect,
                constraint: Constraint::Width({
                    let width = screen_rect.right - screen_rect.left;
                    (width / 5) as _
                }),
                horizontal_position: HorizontalPosition::Right,
            }
        });

        EnumWindows(Some(enum_windows_proc), LPARAM::default()).unwrap();
    }
}

unsafe extern "system" fn enum_windows_proc(window: HWND, _l_param: LPARAM) -> BOOL {
    (|| unsafe {
        if !IsWindowVisible(window).as_bool() {
            return;
        }

        let pid = {
            let mut pid = 0;
            GetWindowThreadProcessId(window, Some(&mut pid));
            pid
        };

        if let Some(info) = get_process_info(pid) {
            if info.cmd != "firefox.exe" {
                return;
            }

            let title = {
                let mut buf = [0; 1024];
                let len = GetWindowTextW(window, &mut buf);
                String::from_utf16(&buf[..len as usize]).unwrap()
            };
            if title != "Picture-in-Picture" {
                return;
            }

            let mut rect = RECT::default();
            GetWindowRect(window, &mut rect).unwrap();
            if rect == RECT::default() {
                return;
            }

            let ratio = {
                let w = rect.right - rect.left;
                let h = rect.bottom - rect.top;
                w as f64 / h as f64
            };

            let app = APP.get().unwrap();

            let (width, height) = match app.constraint {
                Constraint::Width(width) => (width as _, (width as f64 * ratio).round() as _),
                Constraint::Height(height) => ((height as f64 * ratio).round() as _, height as _),
            };

            let x_pos = match app.horizontal_position {
                HorizontalPosition::Left => 0,
                HorizontalPosition::Right => app.screen_rect.right - width,
            };

            SetWindowPos(
                window,
                None,
                x_pos,
                800,
                width,
                height,
                SWP_ASYNCWINDOWPOS | SWP_NOOWNERZORDER | SWP_NOZORDER,
            )
            .unwrap();
        }
    })();
    true.into()
}
