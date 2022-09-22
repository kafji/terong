use cfg_if::cfg_if;
use serde::Deserialize;
use std::net::IpAddr;

#[derive(Clone, Deserialize, Debug)]
pub struct ServerConfig {
    pub port: u16,
    pub addr: IpAddr,

    #[cfg(target_os = "linux")]
    pub linux: LinuxConfig,
}

cfg_if! {
    if #[cfg(target_os = "linux")] {
        use std::path::PathBuf;

        #[derive(Clone, Deserialize, Debug)]
        pub struct LinuxConfig {
            pub keyboard_device: Option<PathBuf>,
            pub mouse_device: Option<PathBuf>,
            pub touchpad_device: Option<PathBuf>,
        }
    }
}
