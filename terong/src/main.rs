fn main() {
    let mut args = std::env::args().skip(1);
    let type_ = args
        .next()
        .expect("missing argument for type (client/server)");
    match type_.as_str() {
        "client" => terong::client::run(),
        "server" => terong::server::run(),
        _ => println!("unexpected arguments"),
    }
}
