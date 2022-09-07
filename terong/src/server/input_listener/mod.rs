use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_os = "linux")] {
        mod linux;
        pub use self::linux::run;
    }
}

cfg_if! {
    if #[cfg(target_os = "windows")] {
        mod windows;
        pub use self::windows::run;
    }
}

#[derive(Debug)]
pub enum Signal {
    SetShouldCapture(bool),
}
