use mac_physics_core::stage_manager::{
    compute_stage_shelf_layout, compute_thumbnail_aspect_fit, hit_test_stage_shelf,
    StageCardLayout, StageGroup, StageShelfMetrics, StageWindowInfo,
};
use mac_physics_core::Rect;

#[test]
fn test_stage_window_info_and_group_models() {
    let win1 = StageWindowInfo::new(101, "Document 1", "TextEdit", 1200, 800, false);
    let win2 = StageWindowInfo::new(102, "Document 2", "TextEdit", 1200, 800, false);

    assert_eq!(win1.id, 101);
    assert_eq!(win1.title, "Document 1");
    assert_eq!(win1.app_name, "TextEdit");
    assert_eq!(win1.width, 1200);
    assert_eq!(win1.height, 800);
    assert!(!win1.is_minimized);

    let group = StageGroup::new("textedit_grp", "TextEdit", vec![win1.clone(), win2], 0);
    assert_eq!(group.group_id, "textedit_grp");
    assert_eq!(group.app_name, "TextEdit");
    assert_eq!(group.window_count(), 2);
    assert_eq!(group.active_window(), Some(&win1));

    // Test active window when index out of bounds
    let mut invalid_group = group.clone();
    invalid_group.active_window_index = 99;
    assert_eq!(invalid_group.active_window(), None);

    // Serialization round-trip
    let json = serde_json::to_string(&group).expect("serialize StageGroup");
    let deserialized: StageGroup = serde_json::from_str(&json).expect("deserialize StageGroup");
    assert_eq!(group, deserialized);
}

#[test]
fn test_stage_card_layout_serde() {
    let card = StageCardLayout {
        group_id: "grp_1".to_string(),
        app_name: "Finder".to_string(),
        bounds: Rect::new(16.0, 100.0, 150.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 128.0, 134.0, 64.0),
        stack_depth: 2,
        opacity: 0.8,
        is_hovered: true,
    };

    let json = serde_json::to_string(&card).expect("serialize StageCardLayout");
    let deserialized: StageCardLayout = serde_json::from_str(&json).expect("deserialize StageCardLayout");
    assert_eq!(card, deserialized);
}

#[test]
fn test_stage_shelf_metrics_defaults() {
    let metrics = StageShelfMetrics::default();
    assert_eq!(metrics.shelf_x, 16.0);
    assert_eq!(metrics.shelf_width, 180.0);
    assert_eq!(metrics.card_width, 150.0);
    assert_eq!(metrics.card_height, 100.0);
    assert_eq!(metrics.card_spacing, 16.0);
    assert_eq!(metrics.max_groups, 5);
    assert_eq!(metrics.corner_radius, 12.0);
}

#[test]
fn test_compute_thumbnail_aspect_fit_wide() {
    // 16:9 source (1920x1080) into 100x100 target box
    let rect = compute_thumbnail_aspect_fit(1920, 1080, 100.0, 100.0);

    // Wide aspect ratio: width should fill target_w (100.0)
    assert!((rect.width - 100.0).abs() < 1e-3);
    let expected_height = 100.0 / (1920.0 / 1080.0); // 56.25
    assert!((rect.height - expected_height).abs() < 1e-3);

    // Centered vertically
    let expected_y = (100.0 - expected_height) / 2.0; // 21.875
    assert!((rect.x - 0.0).abs() < 1e-3);
    assert!((rect.y - expected_y).abs() < 1e-3);
}

#[test]
fn test_compute_thumbnail_aspect_fit_tall() {
    // 9:16 source (1080x1920) into 100x100 target box
    let rect = compute_thumbnail_aspect_fit(1080, 1920, 100.0, 100.0);

    // Tall aspect ratio: height should fill target_h (100.0)
    assert!((rect.height - 100.0).abs() < 1e-3);
    let expected_width = 100.0 * (1080.0 / 1920.0); // 56.25
    assert!((rect.width - expected_width).abs() < 1e-3);

    // Centered horizontally
    let expected_x = (100.0 - expected_width) / 2.0; // 21.875
    assert!((rect.x - expected_x).abs() < 1e-3);
    assert!((rect.y - 0.0).abs() < 1e-3);
}

#[test]
fn test_compute_thumbnail_aspect_fit_ultrawide_21_9() {
    // 21:9 source (2560x1080) into 150x100 box
    let rect = compute_thumbnail_aspect_fit(2560, 1080, 150.0, 100.0);

    assert!((rect.width - 150.0).abs() < 1e-3);
    let expected_h = 150.0 / (2560.0 / 1080.0);
    assert!((rect.height - expected_h).abs() < 1e-3);
    assert!(rect.y > 0.0);
}

#[test]
fn test_compute_thumbnail_aspect_fit_square() {
    // Square source (800x800) into rectangular box (150x100) -> should pillarbox horizontally
    let rect = compute_thumbnail_aspect_fit(800, 800, 150.0, 100.0);
    assert!((rect.height - 100.0).abs() < 1e-3);
    assert!((rect.width - 100.0).abs() < 1e-3);
    assert!((rect.x - 25.0).abs() < 1e-3);
    assert!((rect.y - 0.0).abs() < 1e-3);
}

#[test]
fn test_compute_thumbnail_aspect_fit_zero_guards() {
    // Zero source width fallback
    let r1 = compute_thumbnail_aspect_fit(0, 800, 150.0, 100.0);
    assert_eq!(r1, Rect::new(0.0, 0.0, 150.0, 100.0));

    // Zero source height fallback
    let r2 = compute_thumbnail_aspect_fit(1200, 0, 150.0, 100.0);
    assert_eq!(r2, Rect::new(0.0, 0.0, 150.0, 100.0));

    // Zero target bounds fallback
    let r3 = compute_thumbnail_aspect_fit(1200, 800, 0.0, 0.0);
    assert_eq!(r3, Rect::new(0.0, 0.0, 0.0, 0.0));

    // NaN target width/height fallback
    let r_nan = compute_thumbnail_aspect_fit(1200, 800, f32::NAN, 100.0);
    assert_eq!(r_nan, Rect::new(0.0, 0.0, 0.0, 100.0));

    let r_nan2 = compute_thumbnail_aspect_fit(1200, 800, 150.0, f32::NAN);
    assert_eq!(r_nan2, Rect::new(0.0, 0.0, 150.0, 0.0));
}

#[test]
fn test_stage_shelf_layout_nan_slide_guard() {
    let metrics = StageShelfMetrics::default();
    let g1 = StageGroup::new("g1", "App", vec![StageWindowInfo::new(1, "W", "App", 800, 600, false)], 0);
    let layout = compute_stage_shelf_layout(&[g1], None, &metrics, 1080.0, f32::NAN);
    assert_eq!(layout.len(), 1);
    assert_eq!(layout[0].opacity, 0.0);
    assert_eq!(layout[0].bounds.x, -metrics.card_width - 10.0);
}

#[test]
fn test_stage_shelf_layout_active_group_excluded() {
    let metrics = StageShelfMetrics::default();
    let g1 = StageGroup::new("g1", "Safari", vec![StageWindowInfo::new(1, "Tab", "Safari", 1000, 700, false)], 0);
    let g2 = StageGroup::new("g2", "Code", vec![StageWindowInfo::new(2, "Main", "Code", 1200, 800, false)], 0);
    let g3 = StageGroup::new("g3", "Music", vec![StageWindowInfo::new(3, "Song", "Music", 800, 600, false)], 0);

    let groups = vec![g1, g2, g3];

    // Exclude g2 (active group in Center Stage)
    let layout = compute_stage_shelf_layout(&groups, Some("g2"), &metrics, 1080.0, 1.0);

    assert_eq!(layout.len(), 2);
    assert_eq!(layout[0].group_id, "g1");
    assert_eq!(layout[1].group_id, "g3");
}

#[test]
fn test_stage_shelf_layout_max_groups_clamping() {
    let metrics = StageShelfMetrics {
        max_groups: 3,
        ..Default::default()
    };

    let groups: Vec<StageGroup> = (0..6)
        .map(|i| {
            StageGroup::new(
                format!("g{}", i),
                format!("App{}", i),
                vec![StageWindowInfo::new(i as u64, "Title", "App", 1000, 700, false)],
                0,
            )
        })
        .collect();

    let layout = compute_stage_shelf_layout(&groups, None, &metrics, 1080.0, 1.0);
    assert_eq!(layout.len(), 3);
    assert_eq!(layout[0].group_id, "g0");
    assert_eq!(layout[1].group_id, "g1");
    assert_eq!(layout[2].group_id, "g2");
}

#[test]
fn test_stage_shelf_layout_vertical_centering_and_spacing() {
    let metrics = StageShelfMetrics::default();
    let groups: Vec<StageGroup> = (0..3)
        .map(|i| {
            StageGroup::new(
                format!("g{}", i),
                format!("App{}", i),
                vec![StageWindowInfo::new(i as u64, "Title", "App", 1000, 700, false)],
                0,
            )
        })
        .collect();

    let screen_height = 1080.0;
    // 3 cards: 3 * 100 + 2 * 16 = 332
    // start_y = (1080 - 332) / 2 = 374.0
    let layout = compute_stage_shelf_layout(&groups, None, &metrics, screen_height, 1.0);

    assert_eq!(layout.len(), 3);
    assert!((layout[0].bounds.y - 374.0).abs() < 1e-3);
    assert!((layout[1].bounds.y - (374.0 + 100.0 + 16.0)).abs() < 1e-3);
    assert!((layout[2].bounds.y - (374.0 + 2.0 * (100.0 + 16.0))).abs() < 1e-3);

    // Height of each card
    for card in &layout {
        assert_eq!(card.bounds.width, metrics.card_width);
        assert_eq!(card.bounds.height, metrics.card_height);
    }
}

#[test]
fn test_stage_shelf_layout_vertical_clamp_to_menu_bar() {
    let metrics = StageShelfMetrics::default();
    let groups: Vec<StageGroup> = (0..5)
        .map(|i| {
            StageGroup::new(
                format!("g{}", i),
                format!("App{}", i),
                vec![StageWindowInfo::new(i as u64, "Title", "App", 1000, 700, false)],
                0,
            )
        })
        .collect();

    // Small screen height: 400.0
    // Total height = 5 * 100 + 4 * 16 = 564
    // (400 - 564) / 2 = -82.0 -> clamped to 28.0 + 10.0 = 38.0
    let layout = compute_stage_shelf_layout(&groups, None, &metrics, 400.0, 1.0);
    assert_eq!(layout[0].bounds.y, 38.0);
}

#[test]
fn test_stage_shelf_layout_slide_interpolation() {
    let metrics = StageShelfMetrics::default();
    let groups = vec![StageGroup::new(
        "g1",
        "Finder",
        vec![StageWindowInfo::new(1, "Files", "Finder", 800, 600, false)],
        0,
    )];

    // shelf_slide_t = 0.0 -> fully hidden: x = -card_width - 10.0 = -160.0
    let hidden = compute_stage_shelf_layout(&groups, None, &metrics, 1080.0, 0.0);
    assert!((hidden[0].bounds.x - (-160.0)).abs() < 1e-3);
    assert!((hidden[0].opacity - 0.0).abs() < 1e-3);

    // shelf_slide_t = 1.0 -> fully visible: x = shelf_x = 16.0
    let visible = compute_stage_shelf_layout(&groups, None, &metrics, 1080.0, 1.0);
    assert!((visible[0].bounds.x - 16.0).abs() < 1e-3);
    assert!((visible[0].opacity - 1.0).abs() < 1e-3);

    // shelf_slide_t = 0.5 -> midway: -160.0 + 176.0 * 0.5 = -72.0
    let midway = compute_stage_shelf_layout(&groups, None, &metrics, 1080.0, 0.5);
    assert!((midway[0].bounds.x - (-72.0)).abs() < 1e-3);
    assert!((midway[0].opacity - 0.5).abs() < 1e-3);

    // Clamping out-of-range t
    let clamped_neg = compute_stage_shelf_layout(&groups, None, &metrics, 1080.0, -0.5);
    assert!((clamped_neg[0].bounds.x - (-160.0)).abs() < 1e-3);
    assert_eq!(clamped_neg[0].opacity, 0.0);

    let clamped_pos = compute_stage_shelf_layout(&groups, None, &metrics, 1080.0, 1.5);
    assert!((clamped_pos[0].bounds.x - 16.0).abs() < 1e-3);
    assert_eq!(clamped_pos[0].opacity, 1.0);
}

#[test]
fn test_stage_shelf_layout_stack_depth() {
    let metrics = StageShelfMetrics::default();

    let g_single = StageGroup::new(
        "single",
        "Calculator",
        vec![StageWindowInfo::new(1, "Calc", "Calculator", 400, 600, false)],
        0,
    );
    let g_triple = StageGroup::new(
        "triple",
        "Safari",
        vec![
            StageWindowInfo::new(2, "Tab 1", "Safari", 1000, 700, false),
            StageWindowInfo::new(3, "Tab 2", "Safari", 1000, 700, false),
            StageWindowInfo::new(4, "Tab 3", "Safari", 1000, 700, false),
        ],
        0,
    );
    let g_six = StageGroup::new(
        "six",
        "Terminal",
        (0..6)
            .map(|i| StageWindowInfo::new(i + 10, "Term", "Terminal", 800, 600, false))
            .collect(),
        0,
    );

    let groups = vec![g_single, g_triple, g_six];
    let layout = compute_stage_shelf_layout(&groups, None, &metrics, 1080.0, 1.0);

    assert_eq!(layout[0].stack_depth, 1);
    assert_eq!(layout[1].stack_depth, 3);
    assert_eq!(layout[2].stack_depth, 4); // Capped at 4
}

#[test]
fn test_stage_shelf_layout_thumbnail_bounds_contained() {
    let metrics = StageShelfMetrics::default();
    let group = StageGroup::new(
        "g1",
        "Browser",
        vec![StageWindowInfo::new(1, "Page", "Browser", 1920, 1080, false)],
        0,
    );

    let layout = compute_stage_shelf_layout(&[group], None, &metrics, 1080.0, 1.0);
    let card = &layout[0];

    // Thumbnail bounds must be contained inside the card bounds
    assert!(card.thumbnail_bounds.x >= card.bounds.x);
    assert!(card.thumbnail_bounds.y >= card.bounds.y);
    assert!(card.thumbnail_bounds.x + card.thumbnail_bounds.width <= card.bounds.x + card.bounds.width);
    assert!(card.thumbnail_bounds.y + card.thumbnail_bounds.height <= card.bounds.y + card.bounds.height);
}

#[test]
fn test_hit_test_stage_shelf() {
    let metrics = StageShelfMetrics::default();
    let groups: Vec<StageGroup> = (0..3)
        .map(|i| {
            StageGroup::new(
                format!("g{}", i),
                format!("App{}", i),
                vec![StageWindowInfo::new(i as u64, "Title", "App", 1000, 700, false)],
                0,
            )
        })
        .collect();

    let layout = compute_stage_shelf_layout(&groups, None, &metrics, 1080.0, 1.0);

    // Hit card 0 center
    let c0 = &layout[0];
    assert_eq!(
        hit_test_stage_shelf(&layout, c0.bounds.x + c0.bounds.width / 2.0, c0.bounds.y + c0.bounds.height / 2.0),
        Some(0)
    );

    // Hit card 2 center
    let c2 = &layout[2];
    assert_eq!(
        hit_test_stage_shelf(&layout, c2.bounds.x + c2.bounds.width / 2.0, c2.bounds.y + c2.bounds.height / 2.0),
        Some(2)
    );

    // Miss outside X
    assert_eq!(hit_test_stage_shelf(&layout, 500.0, c0.bounds.y + 10.0), None);

    // Miss between card 0 and card 1 (gap is 16px)
    let gap_y = c0.bounds.y + c0.bounds.height + 8.0;
    assert_eq!(hit_test_stage_shelf(&layout, c0.bounds.x + 10.0, gap_y), None);
}

#[test]
fn test_stage_shelf_layout_empty_and_all_active() {
    let metrics = StageShelfMetrics::default();

    // Empty input
    let empty_layout = compute_stage_shelf_layout(&[], None, &metrics, 1080.0, 1.0);
    assert!(empty_layout.is_empty());

    // Single group which is active
    let single = vec![StageGroup::new(
        "g_active",
        "App",
        vec![StageWindowInfo::new(1, "T", "A", 100, 100, false)],
        0,
    )];
    let active_layout = compute_stage_shelf_layout(&single, Some("g_active"), &metrics, 1080.0, 1.0);
    assert!(active_layout.is_empty());
}
