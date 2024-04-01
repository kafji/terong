use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(env_filter())
        .init();
}

fn env_filter() -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
}

#[macro_export]
macro_rules! log_error {
    ($err:expr) => {{
        #[allow(unused)]
        use std::error::Error;
        let cause = $err.source();
        if let Some(cause) = cause {
            tracing::error!(?cause, "{}", $err);
        } else {
            tracing::error!("{}", $err);
        }
    }};
}
