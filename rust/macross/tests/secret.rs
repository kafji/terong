use macross::secret::Secret;

#[test]
fn test_display_format() {
    let secret = Secret("sekret");
    assert_eq!(format!("{secret}"), "█████");
}

#[test]
fn test_debug_format() {
    let secret = Secret("sekret");
    assert_eq!(format!("{secret:?}"), r#"Secret("█████")"#);
}

#[test]
fn test_pretty_debug_format() {
    let secret = Secret("sekret");
    assert!(!format!("{secret:#?}").contains("sekret"));
}

#[cfg(feature = "serde")]
#[test]
fn test_secret_is_deserializable() {
    let deserialized: Secret<String> = serde_json::from_str("\"sekret\"").unwrap();
    assert_eq!(deserialized, Secret("sekret".to_owned()));
}

#[cfg(feature = "serde")]
#[test]
fn test_secret_is_serializable() {
    let serialized = serde_json::to_value(&Secret("sekret")).unwrap();
    assert!(matches!(serialized, serde_json::Value::String(x) if x == "sekret"));
}

#[test]
fn test_into() {
    let _: Secret<&str> = "sekret".into();
}
