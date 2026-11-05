use win_launcher::system_status::SystemStatusTracker;

#[test]
fn test_system_status_tracker_clock_and_battery() {
    let mut tracker = SystemStatusTracker::new_mock();
    let snapshot = tracker.refresh();
    assert!(!snapshot.time_text.is_empty());
    assert!(snapshot.volume_percent <= 100);
}

#[test]
fn test_system_status_tracker_snapshot_caching() {
    let mut tracker = SystemStatusTracker::new_mock();
    let s1 = tracker.refresh();
    let s2 = tracker.snapshot();
    assert_eq!(&s1, s2);
}

#[test]
fn test_system_status_tracker_custom_mock_values() {
    let mut tracker = SystemStatusTracker::new_mock();
    tracker.set_mock_battery(Some(45), true);
    tracker.set_mock_wifi(false);
    tracker.set_mock_volume(0);

    let snapshot = tracker.refresh();
    assert_eq!(snapshot.battery_percent, Some(45));
    assert!(snapshot.is_charging);
    assert!(!snapshot.wifi_connected);
    assert_eq!(snapshot.volume_percent, 0);
}

#[test]
fn test_system_status_tracker_format_clock() {
    let clock = SystemStatusTracker::format_clock();
    assert!(!clock.is_empty());
    assert!(clock.contains("AM") || clock.contains("PM"));
}

#[test]
fn test_system_status_tracker_ac_desktop_without_battery() {
    let mut tracker = SystemStatusTracker::new_mock();
    tracker.set_mock_battery(None, false);
    let snapshot = tracker.refresh();
    assert_eq!(snapshot.battery_percent, None);
    assert!(!snapshot.is_charging);
}
