use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let type_ = args
        .next()
        .expect("missing argument for type (client/server)");
    let config_file = args.next().map(PathBuf::from);
    match type_.as_str() {
        "client" => terong::client::run(config_file),
        "server" => terong::server::run(config_file),
        _ => println!("unexpected arguments"),
    }
}
