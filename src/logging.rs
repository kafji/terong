use cfg_if::cfg_if;
use tracing_subscriber::EnvFilter;

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(env_filter())
        .with_thread_ids(true)
        .init();
}

fn env_filter() -> EnvFilter {
    let builder = EnvFilter::builder();

    cfg_if! {
        if #[cfg(debug_assertions)] {
            use tracing::metadata::LevelFilter;
            let builder = builder.with_default_directive(LevelFilter::INFO.into());
        }
    }

    builder.from_env_lossy()
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
