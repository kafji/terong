mod process;

use self::process::get_process_info;
use crate::cli::{Cli, Command, HorizontalPosition};
use std::{ffi::c_void, sync::OnceLock};
use windows::{
    core::BOOL,
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
            SetWindowPos, SystemParametersInfoW, SPI_GETWORKAREA, SWP_ASYNCWINDOWPOS,
            SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        },
    },
};

#[derive(Copy, Clone, Debug)]
enum Constraint {
    Width(u32),
    Height(u32),
}

struct App {
    command: Command,
    screen_rect: RECT,
    constraint: Constraint,
}

static APP: OnceLock<App> = OnceLock::new();

pub fn run() {
    let cli: Cli = argh::from_env();

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
                command: cli.command,
                screen_rect,
                constraint: Constraint::Width({
                    let width = screen_rect.right - screen_rect.left;
                    (width / 4) as _
                }),
            }
        });

        EnumWindows(Some(enum_windows_proc), LPARAM::default()).unwrap();
    }
}

fn get_window_title(window: HWND) -> String {
    let mut buf = [0; 1024];
    let len = unsafe { GetWindowTextW(window, &mut buf) };
    String::from_utf16(&buf[..len as usize]).unwrap()
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

        let process_info = if let Some(info) = get_process_info(pid) {
            info
        } else {
            return;
        };

        let title = get_window_title(window);

        let app = APP.get().unwrap();

        match &app.command {
            Command::Center(args) => {
                if !title.starts_with(&args.title) {
                    return;
                }

                let mut rect = RECT::default();
                GetWindowRect(window, &mut rect).unwrap();
                if rect == RECT::default() {
                    return;
                }

                let screen_width = app.screen_rect.right - app.screen_rect.left;
                let screen_height = app.screen_rect.bottom - app.screen_rect.top;

                let window_width = rect.right - rect.left;
                let window_height = rect.bottom - rect.top;

                let x = screen_width / 2 - window_width / 2;
                let y = screen_height / 2 - window_height / 2;

                SetWindowPos(
                    window,
                    None,
                    x,
                    y,
                    0,
                    0,
                    SWP_ASYNCWINDOWPOS | SWP_NOOWNERZORDER | SWP_NOSIZE | SWP_NOZORDER,
                )
                .unwrap();
            }
            Command::Pip(args) => {
                if process_info.cmd != "firefox.exe" {
                    return;
                }

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

                let (width, height) = match app.constraint {
                    Constraint::Width(width) => (width as _, (width as f64 * ratio).round() as _),
                    Constraint::Height(height) => {
                        ((height as f64 * ratio).round() as _, height as _)
                    }
                };

                let x_pos = match args.horizontal {
                    HorizontalPosition::Left => 0,
                    HorizontalPosition::Right => app.screen_rect.right - width,
                };

                SetWindowPos(
                    window,
                    None,
                    x_pos,
                    200,
                    width,
                    height,
                    SWP_ASYNCWINDOWPOS | SWP_NOOWNERZORDER | SWP_NOZORDER,
                )
                .unwrap();
            }
        }
    })();

    true.into()
}
