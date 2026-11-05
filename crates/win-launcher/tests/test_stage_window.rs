//! Tests for StageWindow (Win32 layered window and mock mode).

use win_launcher::stage_window::{StageWindow, StageWindowConfig, StageWindowEvent};

#[test]
fn test_stage_window_default_config() {
    let config = StageWindowConfig::default();
    assert_eq!(config.x, 0);
    assert_eq!(config.y, 28);
    assert_eq!(config.width, 220);
    assert_eq!(config.height, 1000);
}

#[test]
fn test_stage_window_lifecycle_and_mock_queue() {
    let mut win = StageWindow::new_mock();
    assert_eq!(win.width(), 220);
    assert_eq!(win.height(), 1000);
    assert_eq!(win.hwnd(), 0);

    // Initial event queue is empty
    assert!(win.poll_events().is_empty());

    // Push mock events
    win.push_mock_event(StageWindowEvent::MouseMove { x: 50.0, y: 150.0 });
    win.push_mock_event(StageWindowEvent::MouseDown { x: 50.0, y: 150.0 });
    win.push_mock_event(StageWindowEvent::MouseUp { x: 50.0, y: 150.0 });
    win.push_mock_event(StageWindowEvent::MouseLeave);
    win.push_mock_event(StageWindowEvent::KillFocus);
    win.push_mock_event(StageWindowEvent::Paint);
    win.push_mock_event(StageWindowEvent::Destroy);

    let events = win.poll_events();
    assert_eq!(events.len(), 7);
    assert_eq!(events[0], StageWindowEvent::MouseMove { x: 50.0, y: 150.0 });
    assert_eq!(events[1], StageWindowEvent::MouseDown { x: 50.0, y: 150.0 });
    assert_eq!(events[2], StageWindowEvent::MouseUp { x: 50.0, y: 150.0 });
    assert_eq!(events[3], StageWindowEvent::MouseLeave);
    assert_eq!(events[4], StageWindowEvent::KillFocus);
    assert_eq!(events[5], StageWindowEvent::Paint);
    assert_eq!(events[6], StageWindowEvent::Destroy);

    // Polling again drains the queue
    assert!(win.poll_events().is_empty());
}

#[test]
fn test_stage_window_custom_config() {
    let config = StageWindowConfig {
        x: 10,
        y: 40,
        width: 250,
        height: 900,
    };
    let win = StageWindow::new(config.clone()).expect("Failed to create StageWindow with custom config");
    assert_eq!(win.width(), 250);
    assert_eq!(win.height(), 900);

    let win_via_create = StageWindow::create(config).expect("Failed to create via StageWindow::create");
    assert_eq!(win_via_create.width(), 250);
    assert_eq!(win_via_create.height(), 900);
}
