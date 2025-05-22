mod config;
mod input_event;
mod logging;
mod tls;
mod transport;
mod typing;

pub mod client;
pub mod event_logger;
pub mod server;

#[cfg(feature = "bench")]
pub mod event_buffer;

#[cfg(not(feature = "bench"))]
mod event_buffer;

pub const EVENT_LOG_FILE_PATH: &'static str = "./events.log";
