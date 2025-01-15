#[macro_export]
macro_rules! include_migration {
    ($name:expr) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            concat!(concat!("/migration/", $name), ".sql")
        ))
    };
}
