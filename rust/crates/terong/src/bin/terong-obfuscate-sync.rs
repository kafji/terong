use std::{path::PathBuf, str::FromStr, time::Instant};
use terong::{EVENT_LOG_FILE_PATH, event_logger::LocalInputEventObfuscator};

fn main() {
    use std::fs::File;
    use terong::event_logger::sync::obfuscate;

    let start = Instant::now();

    let mut path = PathBuf::from_str(EVENT_LOG_FILE_PATH).unwrap();
    let input = File::open(&path).unwrap();

    path.set_file_name(format!(
        "{}.obfuscated.{}",
        path.file_stem().expect("missing file stem").to_string_lossy(),
        path.extension().expect("missing extension").to_string_lossy()
    ));
    let output = File::create(&path).unwrap();

    let records = obfuscate(input, output, LocalInputEventObfuscator::new()).unwrap();

    let d = Instant::now() - start;
    println!("processed {} in {:?}", records, d);
}
