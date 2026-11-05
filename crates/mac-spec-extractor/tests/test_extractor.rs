use mac_spec_extractor::extractor::DockMetricsExtractor;
use mac_spec_extractor::models::DockMetrics;

#[test]
fn test_default_dock_metrics_are_valid() {
    let metrics = DockMetrics::default_macos_spec();
    assert_eq!(metrics.base_icon_size, 48.0);
    assert_eq!(metrics.max_icon_size, 128.0);
    assert!(metrics.magnification_spread > 0.0);
    assert!(metrics.squircle_exponent >= 4.0);
}

#[test]
fn test_dock_metrics_json_roundtrip() {
    let metrics = DockMetrics::default_macos_spec();
    let json = serde_json::to_string_pretty(&metrics).expect("Failed to serialize");
    let deserialized: DockMetrics = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(metrics, deserialized);
}

#[test]
fn test_extractor_produces_valid_metrics() {
    let extractor = DockMetricsExtractor::new();
    let metrics = extractor.extract();
    assert!(metrics.base_icon_size > 0.0);
    assert!(metrics.max_icon_size >= metrics.base_icon_size);
    assert!(metrics.magnification_spread > 0.0);
    assert!(metrics.squircle_exponent >= 4.0);
    assert!(metrics.border_radius > 0.0);
    assert!(metrics.icon_padding >= 0.0);
}

#[test]
fn test_extractor_custom_json_values() {
    let json_str = r#"{
        "tilesize": 64,
        "largesize": 140,
        "magnification_spread": 160.0,
        "squircle_exponent": 5.0,
        "border_radius": 20.0,
        "icon_padding": 12.0
    }"#;
    let val: serde_json::Value = serde_json::from_str(json_str).unwrap();
    let metrics = DockMetricsExtractor::extract_from_json_value(&val);
    assert_eq!(metrics.base_icon_size, 64.0);
    assert_eq!(metrics.max_icon_size, 140.0);
    assert_eq!(metrics.magnification_spread, 160.0);
    assert_eq!(metrics.squircle_exponent, 5.0);
    assert_eq!(metrics.border_radius, 20.0);
    assert_eq!(metrics.icon_padding, 12.0);
}

#[test]
fn test_extractor_fallback_on_missing_path() {
    let extractor = DockMetricsExtractor::new().with_plist_path("/non/existent/path/Info.plist");
    let metrics = extractor.extract();
    assert_eq!(metrics, DockMetrics::default_macos_spec());
}
