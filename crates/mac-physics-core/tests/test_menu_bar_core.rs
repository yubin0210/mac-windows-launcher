use mac_physics_core::menu_bar::{
    compute_menu_bar_layout, MenuBarGroup, MenuBarItem, MenuBarMetrics, SystemStatusSnapshot,
};

#[test]
fn test_menu_bar_layout_left_and_right_distribution() {
    let items = vec![
        MenuBarItem::apple_logo(),
        MenuBarItem::new("app", "Finder", MenuBarGroup::Left, true, true),
        MenuBarItem::new("file", "File", MenuBarGroup::Left, false, true),
        MenuBarItem::new("edit", "Edit", MenuBarGroup::Left, false, true),
    ];
    let status = SystemStatusSnapshot::new(Some(85), false, true, 60, "12:00 PM");
    let layout = compute_menu_bar_layout(&items, &status, 1920.0);

    assert_eq!(layout.height, MenuBarMetrics::default().height);
    assert!(!layout.left_items.is_empty());
    assert!(!layout.right_items.is_empty());
    assert!(layout.right_items.last().unwrap().bounds.x > 1500.0);
}

#[test]
fn test_menu_bar_metrics_defaults() {
    let metrics = MenuBarMetrics::default();
    assert_eq!(metrics.height, 28.0);
    assert_eq!(metrics.padding_h, 12.0);
    assert_eq!(metrics.item_spacing, 16.0);
    assert_eq!(metrics.font_size, 13.0);
}

#[test]
fn test_system_status_snapshot_battery_and_clock() {
    let status = SystemStatusSnapshot::new(Some(92), true, true, 80, "Thu Nov 5 12:23 PM");
    assert_eq!(status.battery_percent, Some(92));
    assert!(status.is_charging);
    assert!(status.wifi_connected);
    assert_eq!(status.volume_percent, 80);
    assert_eq!(status.time_text, "Thu Nov 5 12:23 PM");
}

#[test]
fn test_menu_bar_layout_hit_testing() {
    let items = vec![
        MenuBarItem::apple_logo(),
        MenuBarItem::new("app", "Terminal", MenuBarGroup::Left, true, true),
    ];
    let status = SystemStatusSnapshot::new(None, false, false, 50, "1:00 PM");
    let layout = compute_menu_bar_layout(&items, &status, 1000.0);

    // Hit test Apple logo
    let hit_apple = layout.hit_test(16.0, 14.0);
    assert!(hit_apple.is_some());
    assert_eq!(hit_apple.unwrap().id, "apple");

    // Outside bounds
    let hit_miss = layout.hit_test(500.0, 14.0);
    assert!(hit_miss.is_none());

    // Y outside menu bar
    let hit_outside_y = layout.hit_test(16.0, 50.0);
    assert!(hit_outside_y.is_none());
}
