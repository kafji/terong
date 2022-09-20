use std::env;

/// Get value of hidden configuration to disable TLS.
pub fn no_tls() -> bool {
    env::var("TERONG_NO_TLS")
        .ok()
        .and_then(|x| x.parse::<u8>().ok())
        .map(|x| x == 1)
        .unwrap_or_default()
}
