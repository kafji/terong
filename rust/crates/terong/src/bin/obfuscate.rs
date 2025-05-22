use terong::{
    EVENT_LOG_FILE_PATH,
    event_logger::{LocalInputEventObfuscator, obfuscate},
};
use tokio::fs::File;

#[tokio::main]
async fn main() {
    let input = File::open(EVENT_LOG_FILE_PATH).await.unwrap();
    let output = File::create(&format!("{}.obfuscated", EVENT_LOG_FILE_PATH)).await.unwrap();
    obfuscate(input, output, LocalInputEventObfuscator::new())
        .await
        .unwrap();
}
