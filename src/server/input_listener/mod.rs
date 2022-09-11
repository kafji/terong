pub mod event;

use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(target_os = "linux")] {
        mod linux;
        pub use self::linux::{start};
    }
}

cfg_if! {
    if #[cfg(target_os = "windows")] {
        mod windows;
        pub use self::windows::{start};
    }
}
