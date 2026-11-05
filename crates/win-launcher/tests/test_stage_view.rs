//! Tests for StageView shelf state machine, animations, hover, thumbnails, and click activation.

use mac_physics_core::StageWindowInfo;
use win_launcher::stage_tracker::{MockWindowEnumerator, StageTracker};
use win_launcher::stage_view::{StageView, StageViewAction};

fn create_test_tracker() -> StageTracker {
    let mock = MockWindowEnumerator::new();
    mock.add_window(StageWindowInfo::new(101, "Safari - Browsing", "Safari", 1200, 800, false));
    mock.add_window(StageWindowInfo::new(201, "Code - main.rs", "Code", 1400, 900, false));
    mock.add_window(StageWindowInfo::new(301, "Music - Playing", "Music", 900, 600, false));

    let mut tracker = StageTracker::new(Box::new(mock));
    tracker.refresh();
    tracker
}

#[test]
fn test_view_update_produces_layout_for_staged_groups() {
    let tracker = create_test_tracker();
    // Default active group is group_safari (101)
    assert_eq!(tracker.active_group_id(), Some("group_safari"));

    let mut view = StageView::new(1920.0, 1080.0);
    assert_eq!(view.cards().len(), 0);

    // Update with time step
    view.update(0.016, &tracker);

    // Active group (Safari) should be on Center Stage and excluded from shelf
    // Staged groups should be Code and Music (2 cards)
    assert_eq!(view.cards().len(), 2);
    assert_eq!(view.cards()[0].group_id, "group_code");
    assert_eq!(view.cards()[1].group_id, "group_music");
}

#[test]
fn test_mouse_move_updates_hovered_card_idx() {
    let tracker = create_test_tracker();
    let mut view = StageView::new(1920.0, 1080.0);

    // Step spring to fully visible position
    for _ in 0..60 {
        view.update(0.016, &tracker);
    }
    assert_eq!(view.cards().len(), 2);

    let card0_bounds = view.cards()[0].bounds;
    let card1_bounds = view.cards()[1].bounds;

    // Move cursor over card 0
    view.on_mouse_move(card0_bounds.x + 5.0, card0_bounds.y + 5.0);
    assert_eq!(view.hovered_card_idx, Some(0));

    view.update(0.016, &tracker);
    assert!(view.cards()[0].is_hovered);
    assert!(!view.cards()[1].is_hovered);

    // Move cursor over card 1
    view.on_mouse_move(card1_bounds.x + 5.0, card1_bounds.y + 5.0);
    assert_eq!(view.hovered_card_idx, Some(1));

    view.update(0.016, &tracker);
    assert!(!view.cards()[0].is_hovered);
    assert!(view.cards()[1].is_hovered);

    // Mouse leaves
    view.on_mouse_leave();
    assert_eq!(view.hovered_card_idx, None);

    view.update(0.016, &tracker);
    assert!(!view.cards()[0].is_hovered);
    assert!(!view.cards()[1].is_hovered);
}

#[test]
fn test_mouse_click_returns_activate_group() {
    let tracker = create_test_tracker();
    let mut view = StageView::new(1920.0, 1080.0);

    for _ in 0..60 {
        view.update(0.016, &tracker);
    }
    assert_eq!(view.cards().len(), 2);

    let card1 = &view.cards()[1];
    let card1_x = card1.bounds.x + 10.0;
    let card1_y = card1.bounds.y + 10.0;

    // Click inside card 1
    let action = view.on_mouse_click(card1_x, card1_y);
    assert_eq!(action, Some(StageViewAction::ActivateGroup("group_music".to_string())));

    // Click outside any card
    let outside_action = view.on_mouse_click(1000.0, 500.0);
    assert_eq!(outside_action, None);
}

#[test]
fn test_desired_thumbnail_streams() {
    let tracker = create_test_tracker();
    let mut view = StageView::new(1920.0, 1080.0);

    for _ in 0..60 {
        view.update(0.016, &tracker);
    }

    let streams = view.desired_thumbnail_streams(&tracker);
    assert_eq!(streams.len(), 2);

    // Stream 0 corresponds to Code (window id 201)
    assert_eq!(streams[0].0, 201);
    assert_eq!(streams[0].1, view.cards()[0].thumbnail_bounds);
    assert_eq!(streams[0].2, 255);

    // Stream 1 corresponds to Music (window id 301)
    assert_eq!(streams[1].0, 301);
    assert_eq!(streams[1].1, view.cards()[1].thumbnail_bounds);
    assert_eq!(streams[1].2, 255);
}

#[test]
fn test_spring_convergence_over_time() {
    let tracker = create_test_tracker();
    let mut view = StageView::new(1920.0, 1080.0);

    assert_eq!(view.slide_progress(), 0.0);

    // Advance spring with dt = 0.016 over 60 frames (~1.0s)
    for _ in 0..60 {
        view.update(0.016, &tracker);
    }
    // Should converge close to 1.0
    assert!((view.slide_progress() - 1.0).abs() < 0.01);

    // Enable auto_hide while not hovered
    view.auto_hide = true;
    view.is_hovered = false;
    for _ in 0..60 {
        view.update(0.016, &tracker);
    }
    // Target is 0.2 when auto_hide and not hovered
    assert!((view.slide_progress() - 0.2).abs() < 0.01);

    // Hover reveals the shelf
    view.on_mouse_move(50.0, 500.0);
    for _ in 0..60 {
        view.update(0.016, &tracker);
    }
    // Converges back to 1.0
    assert!((view.slide_progress() - 1.0).abs() < 0.01);
}
