use win_launcher::app_tracker::AppTracker;
use win_launcher::dock_view::{DockItem, DockView};

#[test]
fn test_app_tracker_running_query_and_mock() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("explorer.exe", 1001, true);
    tracker.register_mock_running_app("msedge.exe", 1002, false);

    assert!(tracker.is_app_running("explorer.exe"));
    assert!(tracker.is_app_focused("explorer.exe"));
    assert!(tracker.is_app_running("msedge.exe"));
    assert!(!tracker.is_app_focused("msedge.exe"));
    assert!(!tracker.is_app_running("nonexistent.exe"));
}

#[test]
fn test_app_tracker_case_insensitivity_and_extension_normalization() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("EXPLORER.EXE", 1001, true);

    // Queries with different cases and with/without .exe
    assert!(tracker.is_app_running("explorer.exe"));
    assert!(tracker.is_app_running("explorer"));
    assert!(tracker.is_app_running("EXPLORER"));
    assert!(tracker.is_app_focused("explorer"));
    assert!(tracker.is_app_focused("explorer.exe"));

    let pids = tracker.get_running_pids("explorer");
    assert_eq!(pids, vec![1001]);
}

#[test]
fn test_app_tracker_pids_query() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("code.exe", 2001, false);
    tracker.register_mock_running_app("code.exe", 2002, true);

    let pids = tracker.get_running_pids("code.exe");
    assert_eq!(pids.len(), 2);
    assert!(pids.contains(&2001));
    assert!(pids.contains(&2002));
    assert!(tracker.is_app_focused("code.exe"));

    assert_eq!(tracker.get_running_pids("notepad.exe").len(), 0);
}

#[test]
fn test_app_tracker_clear_and_refresh() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("app1.exe", 5001, false);
    assert!(tracker.is_app_running("app1.exe"));

    tracker.refresh_running_apps();
    tracker.clear_mock_apps();
    assert!(!tracker.is_app_running("app1.exe"));
    assert_eq!(tracker.get_running_pids("app1.exe").len(), 0);

    #[cfg(not(windows))]
    assert_eq!(tracker.all_apps().len(), 0);
}

#[test]
fn test_app_tracker_full_path_query_normalization() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("notepad.exe", 3001, true);

    // Queries with full directory path prefixes (both Windows backslash and Unix forward slash)
    assert!(tracker.is_app_running(r"C:\Windows\System32\notepad.exe"));
    assert!(tracker.is_app_running("/usr/bin/notepad.exe"));
    assert!(tracker.is_app_running(r"C:\Windows\notepad"));
    assert!(tracker.is_app_focused(r"C:\Windows\System32\notepad.exe"));
    assert_eq!(tracker.get_running_pids(r"C:\Windows\notepad.exe"), vec![3001]);

    // Also register with full path and query by executable basename
    tracker.register_mock_running_app(r"C:\Program Files\App\custom.exe", 3002, false);
    assert!(tracker.is_app_running("custom.exe"));
    assert!(tracker.is_app_running("custom"));
    assert_eq!(tracker.get_running_pids("custom"), vec![3002]);
}

#[test]
fn test_dock_view_running_and_focused_status_update() {
    let mut dock = DockView::new();
    let mut tracker = AppTracker::new();

    // Register active mock apps: explorer (Finder), wt.exe (Terminal focused)
    tracker.register_mock_running_app("explorer.exe", 1001, false);
    tracker.register_mock_running_app("wt.exe", 1002, true);

    dock.update_running_status(&tracker);

    let render_items = dock.get_render_items();
    // Finder -> explorer.exe: running = true, focused = false
    let finder = render_items.iter().find(|i| i.id == "finder").unwrap();
    assert!(finder.is_running);
    assert!(!finder.is_focused);

    // Terminal -> wt.exe: running = true, focused = true
    let terminal = render_items.iter().find(|i| i.id == "terminal").unwrap();
    assert!(terminal.is_running);
    assert!(terminal.is_focused);

    // Messages -> not running
    let messages = render_items.iter().find(|i| i.id == "messages").unwrap();
    assert!(!messages.is_running);
    assert!(!messages.is_focused);
}

#[test]
fn test_dock_view_set_app_running_manual() {
    let mut dock = DockView::new();
    assert_eq!(dock.items().len(), 10);

    // Set item 3 (Messages) to running and focused manually
    dock.set_app_running(3, true, true);
    assert!(dock.items()[3].is_running);
    assert!(dock.items()[3].is_focused);

    let render_items = dock.get_render_items();
    assert!(render_items[3].is_running);
    assert!(render_items[3].is_focused);
}

#[test]
fn test_dock_item_builder_methods() {
    let item = DockItem::new("custom", "Custom App", false)
        .with_icon_path("assets/custom.png")
        .with_process_name("custom_app.exe")
        .with_running(true, true);

    assert_eq!(item.id, "custom");
    assert_eq!(item.label, "Custom App");
    assert_eq!(item.icon_path.as_deref(), Some("assets/custom.png"));
    assert_eq!(item.process_name.as_deref(), Some("custom_app.exe"));
    assert!(item.is_running);
    assert!(item.is_focused);
}

#[test]
fn test_resolve_process_name_mappings() {
    assert_eq!(DockView::resolve_process_name("finder", "Finder"), Some("explorer.exe"));
    assert_eq!(DockView::resolve_process_name("browser", "Safari"), Some("msedge.exe"));
    assert_eq!(DockView::resolve_process_name("terminal", "Terminal"), Some("wt.exe"));
    assert_eq!(DockView::resolve_process_name("settings", "System Settings"), Some("systemsettings.exe"));
    assert_eq!(DockView::resolve_process_name("mail", "Mail"), Some("hxoutlook.exe"));
    assert_eq!(DockView::resolve_process_name("music", "Music"), Some("spotify.exe"));
    assert_eq!(DockView::resolve_process_name("unknown", "Unknown"), None);
}
