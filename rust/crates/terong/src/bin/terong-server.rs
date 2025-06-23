use std::env::args;

#[tokio::main]
async fn main() {
    let should_log = args()
        .skip(1)
        .next()
        .as_deref()
        .map(|arg| arg == "--log")
        .unwrap_or_default();

    terong::server::run(should_log).await
}
