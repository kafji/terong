#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "windows")]
pub use windows::run;

#[cfg(target_os = "linux")]
pub fn run() {
    panic!("noop");
}
