use win_launcher::menu_bar_window::{MenuBarWindow, MenuBarWindowConfig, MenuBarWindowEvent};

#[test]
fn test_menu_bar_window_lifecycle_and_mock() {
    let win = MenuBarWindow::new_mock();
    assert!(win.is_open());
    assert_eq!(win.height(), 28);
    assert_eq!(win.width(), 1920);
    assert!(win.is_appbar_registered());
}

#[test]
fn test_menu_bar_window_event_queue() {
    let mut win = MenuBarWindow::new_mock();
    assert!(win.poll_events().is_empty());

    win.push_mock_event(MenuBarWindowEvent::MouseMove { x: 100.0, y: 14.0 });
    win.push_mock_event(MenuBarWindowEvent::MouseDown { x: 100.0, y: 14.0 });

    let events = win.poll_events();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], MenuBarWindowEvent::MouseMove { x: 100.0, y: 14.0 });
    assert_eq!(events[1], MenuBarWindowEvent::MouseDown { x: 100.0, y: 14.0 });

    assert!(win.poll_events().is_empty());
}

#[test]
fn test_menu_bar_window_height_expansion() {
    let mut win = MenuBarWindow::new_mock();
    assert_eq!(win.height(), 28);

    win.set_height(250);
    assert_eq!(win.height(), 250);

    win.reset_height();
    assert_eq!(win.height(), 28);
}

#[test]
fn test_menu_bar_window_kill_focus() {
    let mut win = MenuBarWindow::new_mock();
    win.push_mock_event(MenuBarWindowEvent::KillFocus);

    let events = win.poll_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0], MenuBarWindowEvent::KillFocus);
}

#[test]
fn test_menu_bar_window_custom_config() {
    let config = MenuBarWindowConfig {
        width: 2560,
        height: 28,
        title: "Test Bar".to_string(),
        class_name: "TestBarClass".to_string(),
        reserve_appbar: false,
    };
    let win = MenuBarWindow::new_mock_with_config(config.clone());
    assert_eq!(win.width(), 2560);
    assert_eq!(win.height(), 28);
    assert!(!win.is_appbar_registered());

    // Verify unified create() constructor
    let created = MenuBarWindow::create(config).expect("create should succeed");
    assert_eq!(created.width(), 2560);
    assert_eq!(created.height(), 28);
}
