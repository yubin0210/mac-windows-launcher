use win_launcher::spotlight_window::{SpotlightEvent, SpotlightWindow};

#[test]
fn test_spotlight_window_open_hide_and_events() {
    let mut win = SpotlightWindow::new_mock();
    assert!(!win.is_visible());

    win.show();
    assert!(win.is_visible());

    win.hide();
    assert!(!win.is_visible());
}

#[test]
fn test_spotlight_window_toggle() {
    let mut win = SpotlightWindow::new_mock();
    assert!(!win.is_visible());

    win.toggle();
    assert!(win.is_visible());

    win.toggle();
    assert!(!win.is_visible());
}

#[test]
fn test_spotlight_window_event_queue() {
    let mut win = SpotlightWindow::new_mock();
    win.push_event(SpotlightEvent::Opened);
    win.push_event(SpotlightEvent::QueryChanged("terminal".to_string()));
    win.push_event(SpotlightEvent::Navigate(1));
    win.push_event(SpotlightEvent::Execute);
    win.push_event(SpotlightEvent::Closed);

    let events = win.poll_events();
    assert_eq!(events.len(), 5);
    assert_eq!(events[0], SpotlightEvent::Opened);
    assert_eq!(events[1], SpotlightEvent::QueryChanged("terminal".to_string()));
    assert_eq!(events[2], SpotlightEvent::Navigate(1));
    assert_eq!(events[3], SpotlightEvent::Execute);
    assert_eq!(events[4], SpotlightEvent::Closed);

    // Queue drained
    assert!(win.poll_events().is_empty());
}

#[test]
fn test_spotlight_window_height_expansion() {
    let mut win = SpotlightWindow::new_mock();
    assert_eq!(win.width(), 640);
    assert_eq!(win.height(), 56);

    win.set_height(380);
    assert_eq!(win.height(), 380);

    win.reset_height();
    assert_eq!(win.height(), 56);
}
