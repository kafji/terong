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
