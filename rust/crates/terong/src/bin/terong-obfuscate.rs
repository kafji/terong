use std::{path::PathBuf, str::FromStr};
use terong::{
    EVENT_LOG_FILE_PATH,
    event_logger::{LocalInputEventObfuscator, obfuscate},
};
use tokio::fs::File;

#[tokio::main]
async fn main() {
    let mut path = PathBuf::from_str(EVENT_LOG_FILE_PATH).unwrap();
    let input = File::open(&path).await.unwrap();

    path.set_file_name(format!(
        "{}.obfuscated.{}",
        path.file_stem().expect("missing file stem").to_string_lossy(),
        path.extension().expect("missing extension").to_string_lossy()
    ));
    let output = File::create(&path).await.unwrap();

    obfuscate(input, output, LocalInputEventObfuscator::new())
        .await
        .unwrap();
}
