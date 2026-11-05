//! Tests for StageTracker window enumeration, filtering, grouping, and activation.

use mac_physics_core::StageWindowInfo;
use win_launcher::stage_tracker::{
    fallback_app_name_from_title, is_stage_candidate, MockWindowEnumerator, StageTracker,
};

#[test]
fn test_grouping_windows_by_application() {
    let mock = MockWindowEnumerator::new();
    mock.add_window(StageWindowInfo::new(101, "Google - Chrome", "Chrome", 1200, 800, false));
    mock.add_window(StageWindowInfo::new(102, "YouTube - Chrome", "Chrome", 1200, 800, false));
    mock.add_window(StageWindowInfo::new(103, "Docs - Chrome", "Chrome", 1200, 800, false));
    mock.add_window(StageWindowInfo::new(201, "main.rs - VS Code", "Code", 1400, 900, false));
    mock.add_window(StageWindowInfo::new(202, "lib.rs - VS Code", "Code", 1400, 900, false));

    let mut tracker = StageTracker::new(Box::new(mock));
    tracker.refresh();

    assert_eq!(tracker.groups().len(), 2);

    let chrome_group = tracker.groups().iter().find(|g| g.group_id == "group_chrome");
    assert!(chrome_group.is_some());
    let chrome_group = chrome_group.unwrap();
    assert_eq!(chrome_group.app_name, "Chrome");
    assert_eq!(chrome_group.windows.len(), 3);

    let code_group = tracker.groups().iter().find(|g| g.group_id == "group_code");
    assert!(code_group.is_some());
    let code_group = code_group.unwrap();
    assert_eq!(code_group.app_name, "Code");
    assert_eq!(code_group.windows.len(), 2);

    // First group should be active by default
    assert_eq!(tracker.active_group_id(), Some("group_chrome"));
}

#[test]
fn test_active_group_selection_and_retrieval() {
    let mock = MockWindowEnumerator::new();
    mock.add_window(StageWindowInfo::new(10, "Terminal", "Terminal", 800, 600, false));
    mock.add_window(StageWindowInfo::new(20, "Notes", "Notes", 600, 500, false));

    let mut tracker = StageTracker::new(Box::new(mock));
    tracker.refresh();

    let active = tracker.active_group();
    assert!(active.is_some());
    let active = active.unwrap();
    assert_eq!(active.group_id, "group_terminal");
    assert_eq!(active.active_window().map(|w| w.id), Some(10));
}

#[test]
fn test_set_active_group_minimizes_previous_and_activates_target() {
    let mock = MockWindowEnumerator::new();
    let win1 = StageWindowInfo::new(101, "Chrome 1", "Chrome", 1000, 700, false);
    let win2 = StageWindowInfo::new(102, "Chrome 2", "Chrome", 1000, 700, false);
    let win3 = StageWindowInfo::new(201, "Code Editor", "Code", 1200, 800, false);

    mock.add_window(win1);
    mock.add_window(win2);
    mock.add_window(win3);

    let mut tracker = StageTracker::new(Box::new(mock.clone()));
    tracker.refresh();

    assert_eq!(tracker.active_group_id(), Some("group_chrome"));

    // Switch to Code
    let res = tracker.set_active_group("group_code");
    assert!(res.is_ok());

    assert_eq!(tracker.active_group_id(), Some("group_code"));
    assert_eq!(tracker.previous_group_id(), Some("group_chrome"));

    // Previous active group windows (101, 102) should have been minimized
    let minimized = mock.minimized_ids();
    assert!(minimized.contains(&101));
    assert!(minimized.contains(&102));

    // Target active window (201) should have been restored and activated
    let restored = mock.restored_ids();
    assert!(restored.contains(&201));
    let activated = mock.activated_ids();
    assert!(activated.contains(&201));
}

#[test]
fn test_refresh_updating_groups_on_add_and_remove() {
    let mock = MockWindowEnumerator::new();
    mock.add_window(StageWindowInfo::new(1, "Browser", "Firefox", 1024, 768, false));

    let mut tracker = StageTracker::new(Box::new(mock.clone()));
    tracker.refresh();
    assert_eq!(tracker.groups().len(), 1);

    // Add a window for another app
    mock.add_window(StageWindowInfo::new(2, "Music", "Spotify", 800, 600, false));
    tracker.refresh();
    assert_eq!(tracker.groups().len(), 2);

    // Remove the first window
    mock.remove_window(1);
    tracker.refresh();
    assert_eq!(tracker.groups().len(), 1);
    assert_eq!(tracker.groups()[0].group_id, "group_spotify");
}

#[test]
fn test_set_active_group_error_handling() {
    let mut tracker = StageTracker::new_mock();
    tracker.refresh();

    let res = tracker.set_active_group("group_nonexistent");
    assert!(res.is_err());
    let err_msg = res.unwrap_err();
    assert!(err_msg.contains("not found"));
}

#[test]
fn test_window_filtering_criteria() {
    // Normal window
    assert!(is_stage_candidate(
        "Document - Word",
        "WordWndClass",
        1200,
        800,
        false,
        false,
        true
    ));

    // Invisible window
    assert!(!is_stage_candidate(
        "Document - Word",
        "WordWndClass",
        1200,
        800,
        false,
        false,
        false
    ));

    // Small dimensions (<= 100)
    assert!(!is_stage_candidate("App", "Wnd", 100, 500, false, false, true));
    assert!(!is_stage_candidate("App", "Wnd", 500, 100, false, false, true));
    assert!(!is_stage_candidate("App", "Wnd", 50, 50, false, false, true));

    // Tool window
    assert!(!is_stage_candidate("App", "Wnd", 800, 600, true, false, true));

    // Cloaked window
    assert!(!is_stage_candidate("App", "Wnd", 800, 600, false, true, true));

    // Empty or whitespace title
    assert!(!is_stage_candidate("", "Wnd", 800, 600, false, false, true));
    assert!(!is_stage_candidate("   ", "Wnd", 800, 600, false, false, true));

    // Program Manager
    assert!(!is_stage_candidate("Program Manager", "Progman", 1920, 1080, false, false, true));

    // Launcher's own class names and titles
    for name in ["MacDockWindow", "MacMenuBarWindow", "MacSpotlightWindow", "MacStageWindow"] {
        assert!(!is_stage_candidate(name, "GenericWnd", 800, 600, false, false, true));
        assert!(!is_stage_candidate("Some Title", name, 800, 600, false, false, true));
    }
}

#[test]
fn test_fallback_app_name_from_title() {
    assert_eq!(fallback_app_name_from_title("Document - Word"), "Document");
    assert_eq!(fallback_app_name_from_title("Google - Chrome"), "Google");
    assert_eq!(fallback_app_name_from_title("Calculator"), "Calculator");
    assert_eq!(fallback_app_name_from_title(""), "App");
    assert_eq!(fallback_app_name_from_title("   "), "App");
}

#[test]
fn test_active_group_preserved_if_still_present_on_refresh() {
    let mock = MockWindowEnumerator::new();
    mock.add_window(StageWindowInfo::new(1, "Tab 1", "Safari", 1000, 700, false));
    mock.add_window(StageWindowInfo::new(2, "Code", "Editor", 1200, 800, false));

    let mut tracker = StageTracker::new(Box::new(mock.clone()));
    tracker.refresh();

    // Default active is Safari
    assert_eq!(tracker.active_group_id(), Some("group_safari"));

    // Switch to Editor
    tracker.set_active_group("group_editor").unwrap();
    assert_eq!(tracker.active_group_id(), Some("group_editor"));

    // Refresh should preserve Editor as active
    tracker.refresh();
    assert_eq!(tracker.active_group_id(), Some("group_editor"));

    // Remove Editor window; refresh should fallback to Safari
    mock.remove_window(2);
    tracker.refresh();
    assert_eq!(tracker.active_group_id(), Some("group_safari"));
}

#[test]
fn test_set_active_group_restores_all_windows_in_multi_window_group() {
    let mock = MockWindowEnumerator::new();
    let win1 = StageWindowInfo::new(101, "Chrome 1", "Chrome", 1000, 700, false);
    let win2 = StageWindowInfo::new(102, "Chrome 2", "Chrome", 1000, 700, false);
    let win3 = StageWindowInfo::new(201, "Editor", "Code", 1200, 800, false);

    mock.add_window(win1);
    mock.add_window(win2);
    mock.add_window(win3);

    let mut tracker = StageTracker::new(Box::new(mock.clone()));
    tracker.refresh();

    assert_eq!(tracker.active_group_id(), Some("group_chrome"));

    // Switch to Code
    tracker.set_active_group("group_code").unwrap();
    assert_eq!(tracker.active_group_id(), Some("group_code"));
    assert!(mock.minimized_ids().contains(&101));
    assert!(mock.minimized_ids().contains(&102));

    mock.clear_records();

    // Switch back to Chrome - all windows in Chrome must be restored
    tracker.set_active_group("group_chrome").unwrap();
    assert_eq!(tracker.active_group_id(), Some("group_chrome"));
    let restored = mock.restored_ids();
    assert!(restored.contains(&101), "Window 101 must be restored");
    assert!(restored.contains(&102), "Window 102 must be restored");
    assert!(mock.activated_ids().contains(&101));
}

#[test]
fn test_set_active_group_idempotent_on_current_active() {
    let mock = MockWindowEnumerator::new();
    let win1 = StageWindowInfo::new(101, "Doc 1", "Word", 1000, 700, false);
    let win2 = StageWindowInfo::new(102, "Doc 2", "Word", 1000, 700, false);

    mock.add_window(win1);
    mock.add_window(win2);

    let mut tracker = StageTracker::new(Box::new(mock.clone()));
    tracker.refresh();

    assert_eq!(tracker.active_group_id(), Some("group_word"));
    mock.clear_records();

    let res = tracker.set_active_group("group_word");
    assert!(res.is_ok());
    assert!(mock.minimized_ids().is_empty(), "Should not minimize windows of current group");
}
