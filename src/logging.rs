use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;

pub fn init_logger() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_thread_ids(true)
        .init();
}

#[macro_export]
macro_rules! log_error {
    ($err:expr) => {{
        let cause = $err.source();
        if let Some(cause) = cause {
            tracing::error!(?cause, "{}", $err);
        } else {
            tracing::error!("{}", $err);
        }
    }};
}
