use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_thread_ids(true)
        .init();

    let mut args = std::env::args().skip(1);
    let type_ = args
        .next()
        .expect("missing argument for type (client/server)");
    let config_file = args.next().map(PathBuf::from);
    match type_.as_str() {
        "client" => terong::client::run(config_file),
        "server" => terong::server::run(config_file).await,
        _ => println!("unexpected arguments"),
    }
}
