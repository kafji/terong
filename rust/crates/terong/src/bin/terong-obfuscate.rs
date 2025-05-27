use std::{path::PathBuf, str::FromStr, time::Instant};
use terong::{EVENT_LOG_FILE_PATH, event_logger::LocalInputEventObfuscator};
use tracing::info;

#[tokio::main]
async fn main() {
    use terong::event_logger::obfuscate;
    use tokio::{fs::File, try_join};

    tracing_subscriber::fmt().init();

    let t = Instant::now();
    info!("obfuscating log events");

    let input_path = PathBuf::from_str(EVENT_LOG_FILE_PATH).unwrap();
    let input_file = File::open(&input_path);

    let mut output_path = input_path.clone();
    output_path.set_file_name(format!(
        "{}.obfuscated.{}",
        input_path.file_stem().expect("missing file stem").to_string_lossy(),
        input_path.extension().expect("missing extension").to_string_lossy()
    ));
    let output_file = File::create(&output_path);

    let (input_file, output_file) = try_join!(input_file, output_file).unwrap();

    obfuscate(input_file, output_file, LocalInputEventObfuscator::new())
        .await
        .unwrap();

    let d = Instant::now() - t;
    info!(takes = ?d, "finished");
}
