use mac_physics_core::magnification::{
    calculate_dock_layout, calculate_dock_layout_at, calculate_icon_scales, MagnificationConfig,
};

#[test]
fn test_no_magnification_when_cursor_out_of_bounds() {
    let config = MagnificationConfig {
        base_size: 48.0,
        max_size: 128.0,
        spread: 150.0,
    };
    let centers = vec![24.0, 76.0, 128.0];
    let scales = calculate_icon_scales(-1000.0, &centers, &config);
    for scale in scales {
        assert!((scale - 1.0).abs() < 1e-4);
    }
}

#[test]
fn test_peak_magnification_at_cursor_center() {
    let config = MagnificationConfig {
        base_size: 48.0,
        max_size: 128.0,
        spread: 150.0,
    };
    let centers = vec![50.0, 100.0, 150.0];
    let scales = calculate_icon_scales(100.0, &centers, &config);
    assert!((scales[1] - (128.0 / 48.0)).abs() < 1e-3);
    assert!(scales[0] < scales[1]);
    assert!(scales[2] < scales[1]);
}

#[test]
fn test_dock_layout_unmagnified() {
    let scales = vec![1.0, 1.0, 1.0];
    let layout = calculate_dock_layout(&scales, 48.0, 4.0);
    assert_eq!(layout.len(), 3);
    assert!((layout[0].0 - 24.0).abs() < 1e-4);
    assert!((layout[0].1 - 48.0).abs() < 1e-4);
    assert!((layout[1].0 - 76.0).abs() < 1e-4);
    assert!((layout[1].1 - 48.0).abs() < 1e-4);
    assert!((layout[2].0 - 128.0).abs() < 1e-4);
    assert!((layout[2].1 - 48.0).abs() < 1e-4);
}

#[test]
fn test_dock_layout_preserves_symmetric_expansion() {
    let scales = vec![1.5, 2.5, 1.5];
    let base_size = 48.0;
    let padding = 10.0;
    let layout = calculate_dock_layout(&scales, base_size, padding);
    assert_eq!(layout.len(), 3);

    // The center icon should remain at the midpoint of the resting dock
    let resting_center = (3.0 * base_size + 2.0 * padding) / 2.0;
    assert!((layout[1].0 - resting_center).abs() < 1e-4);

    // Symmetric distances from center icon to outer icons
    let dist_left = (layout[1].0 - layout[0].0).abs();
    let dist_right = (layout[2].0 - layout[1].0).abs();
    assert!((dist_left - dist_right).abs() < 1e-4);
}

#[test]
fn test_dock_layout_at_custom_center() {
    let scales = vec![1.0, 2.0, 1.0];
    let base_size = 50.0;
    let padding = 10.0;
    let custom_center = 500.0;
    let layout = calculate_dock_layout_at(&scales, base_size, padding, custom_center);

    // Total magnified width = (50 + 100 + 50) + 20 = 220
    // Expected center of center icon should be exactly at custom_center
    assert!((layout[1].0 - custom_center).abs() < 1e-4);
    assert_eq!(layout[1].1, 100.0);
}

#[test]
fn test_smooth_taper_at_boundary() {
    let config = MagnificationConfig {
        base_size: 48.0,
        max_size: 128.0,
        spread: 100.0,
    };
    // Test distances around 2*sigma (200), 2.5*sigma (250), 3.0*sigma (300), and beyond (350)
    let centers = vec![0.0, 200.0, 250.0, 299.0, 300.0, 350.0];
    let scales = calculate_icon_scales(0.0, &centers, &config);

    assert!(scales[0] > scales[1]);
    assert!(scales[1] > scales[2]);
    assert!(scales[2] > scales[3]);
    assert!(scales[3] > 1.0); // Slightly magnified right before cutoff
    assert_eq!(scales[4], 1.0); // Exactly at 3*sigma
    assert_eq!(scales[5], 1.0); // Beyond 3*sigma
}

#[test]
fn test_symmetric_inter_icon_magnification() {
    let config = MagnificationConfig::default();
    let centers = vec![40.0, 80.0];
    // Cursor exactly midway between icon 0 and icon 1 (60.0)
    let scales = calculate_icon_scales(60.0, &centers, &config);
    assert!((scales[0] - scales[1]).abs() < 1e-6);
    assert!(scales[0] > 1.0);
}

#[test]
fn test_dock_metrics_json_compatibility() {
    // Read the specs/dock-metrics.json produced by Task 2
    let json_data = include_str!("../../../specs/dock-metrics.json");
    let config: MagnificationConfig = serde_json::from_str(json_data).expect("Must parse dock-metrics.json");
    assert_eq!(config.base_size, 48.0);
    assert_eq!(config.max_size, 128.0);
    assert_eq!(config.spread, 150.0);

    // Verify default fallback works on corrupted JSON
    let fallback = MagnificationConfig::from_json_str_or_default("invalid-json");
    assert_eq!(fallback, MagnificationConfig::default());
}

#[test]
fn test_zero_spread_or_base_size_guards() {
    let bad_config1 = MagnificationConfig {
        base_size: 48.0,
        max_size: 128.0,
        spread: 0.0,
    };
    let bad_config2 = MagnificationConfig {
        base_size: 0.0,
        max_size: 128.0,
        spread: 150.0,
    };
    let centers = vec![50.0, 100.0, 150.0];
    let scales1 = calculate_icon_scales(100.0, &centers, &bad_config1);
    assert_eq!(scales1, vec![1.0, 1.0, 1.0]);

    let scales2 = calculate_icon_scales(100.0, &centers, &bad_config2);
    assert_eq!(scales2, vec![1.0, 1.0, 1.0]);
}

#[test]
fn test_empty_inputs() {
    let config = MagnificationConfig::default();
    assert!(calculate_icon_scales(100.0, &[], &config).is_empty());
    assert!(calculate_dock_layout(&[], 48.0, 10.0).is_empty());
}
