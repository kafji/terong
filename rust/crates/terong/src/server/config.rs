use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Deserialize, Debug)]
pub struct ServerConfig {
    pub port: u16,

    pub tls_cert_path: PathBuf,
    pub tls_key_path: PathBuf,

    pub tls_root_cert_path: PathBuf,

    #[cfg(target_os = "linux")]
    pub linux: LinuxConfig,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Deserialize, Debug)]
pub struct LinuxConfig {
    pub keyboard_device: Option<PathBuf>,
    pub mouse_device: Option<PathBuf>,
    pub touchpad_device: Option<PathBuf>,
}
