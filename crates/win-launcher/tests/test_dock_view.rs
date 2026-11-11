//! Integration tests for win-launcher dock view, layout, hover, and physics.

use win_launcher::dock_view::DockView;
use win_launcher::window::{DockWindow, DockWindowConfig, WindowEvent};

#[test]
fn test_dock_view_initialization_defaults() {
    let dock = DockView::new();
    assert_eq!(dock.items().len(), 10);
    assert!(!dock.is_hover_active());
    assert_eq!(dock.cursor_pos(), None);

    let render_items = dock.get_render_items();
    assert_eq!(render_items.len(), 10);

    for (i, item) in render_items.iter().enumerate() {
        assert_eq!(item.index, i);
        assert_eq!(item.scale, 1.0);
        assert_eq!(item.current_size, 48.0);
        assert_eq!(item.bounce_offset_y, 0.0);
        assert!(!item.is_bouncing);
    }

    let (tx, ty, tw, th) = dock.get_tray_geometry();
    let (rx, ry, rw, rh) = dock.resting_tray_geometry();
    assert_eq!((tx, ty, tw, th), (rx, ry, rw, rh));
    assert!(tw > 0.0);
    assert_eq!(th, 48.0 + 16.0); // base_size + 2 * tray_padding_y
}

#[test]
fn test_mouse_hover_triggers_magnification() {
    let mut dock = DockView::new();
    let resting_items = dock.get_render_items();

    // Hover directly over icon 3 (Messages)
    let target_icon = &resting_items[3];
    let hover_x = target_icon.center_x;
    let hover_y = target_icon.center_y;

    dock.on_mouse_move(hover_x, hover_y);
    assert!(dock.is_hover_active());

    let active_items = dock.get_render_items();
    let hovered = &active_items[3];

    // Peak magnification should approach max_size / base_size (128 / 48 ≈ 2.667)
    assert!(
        hovered.scale > 2.5,
        "Hovered icon scale was {}, expected > 2.5",
        hovered.scale
    );
    assert!(
        hovered.current_size > 120.0,
        "Hovered icon size was {}, expected > 120.0",
        hovered.current_size
    );

    // Neighbors should be magnified with Gaussian falloff
    let left_neighbor = &active_items[2];
    let right_neighbor = &active_items[4];
    assert!(left_neighbor.scale > 1.0 && left_neighbor.scale < hovered.scale);
    assert!(right_neighbor.scale > 1.0 && right_neighbor.scale < hovered.scale);

    // Distant icon (e.g. index 9) should remain near 1.0
    let distant = &active_items[9];
    assert!(
        (distant.scale - 1.0).abs() < 0.1,
        "Distant icon scale was {}",
        distant.scale
    );
}

#[test]
fn test_mouse_outside_hover_band_or_leave_resets_scales() {
    let mut dock = DockView::new();
    let resting_items = dock.get_render_items();
    let center_x = resting_items[5].center_x;

    // Move mouse far above the dock (e.g. top of screen at y = 10.0)
    dock.on_mouse_move(center_x, 10.0);
    assert!(!dock.is_hover_active());

    let render_items = dock.get_render_items();
    for item in render_items {
        assert_eq!(item.scale, 1.0);
        assert_eq!(item.current_size, 48.0);
    }

    // Now move inside dock hover zone
    dock.on_mouse_move(center_x, 140.0);
    assert!(dock.is_hover_active());
    assert!(dock.get_render_items()[5].scale > 2.0);

    // Now cursor leaves the window
    dock.on_mouse_leave();
    assert!(!dock.is_hover_active());
    assert_eq!(dock.cursor_pos(), None);

    // After animation steps, scales fully settle to resting 1.0
    for _ in 0..30 {
        dock.update(1.0 / 120.0);
    }

    for item in dock.get_render_items() {
        assert_eq!(item.scale, 1.0);
        assert_eq!(item.current_size, 48.0);
    }
}

#[test]
fn test_launch_bounce_animation_and_rest_convergence() {
    let mut dock = DockView::new();
    assert!(!dock.get_render_items()[2].is_bouncing);

    // Trigger launch bounce on icon 2
    dock.trigger_launch(2);
    assert!(dock.get_render_items()[2].is_bouncing);

    // Step physics forward in time
    let mut min_bounce_offset = 0.0f32;
    for _ in 0..10 {
        dock.update(0.016); // ~60Hz delta time
        let offset = dock.get_render_items()[2].bounce_offset_y;
        if offset < min_bounce_offset {
            min_bounce_offset = offset;
        }
    }

    // Icon must have traveled upward into the air (negative Y)
    assert!(
        min_bounce_offset < -10.0,
        "Min bounce offset was {min_bounce_offset}, expected upward displacement < -10.0"
    );

    // Let the spring oscillator run until settling to rest
    for _ in 0..300 {
        dock.update(0.016);
    }

    let settled_item = &dock.get_render_items()[2];
    assert!(
        !settled_item.is_bouncing,
        "Spring did not settle to rest after 300 steps"
    );
    assert_eq!(settled_item.bounce_offset_y, 0.0);
}

#[test]
fn test_mouse_click_hit_testing() {
    let mut dock = DockView::new();
    let resting_items = dock.get_render_items();

    // Click icon 4 (Mail)
    let icon4 = &resting_items[4];
    let click_x = icon4.center_x;
    let click_y = icon4.center_y;

    let clicked_index = dock.on_mouse_click(click_x, click_y);
    assert_eq!(clicked_index, Some(4));
    assert!(dock.get_render_items()[4].is_bouncing);

    // Click far away from any icon (empty space)
    let miss_index = dock.on_mouse_click(10.0, 10.0);
    assert_eq!(miss_index, None);
}

#[test]
fn test_tray_geometry_and_squircle_contour() {
    let mut dock = DockView::new();
    let (rx, ry, rw, rh) = dock.resting_tray_geometry();
    let (tx, ty, tw, th) = dock.get_tray_geometry();
    assert_eq!((rx, ry, rw, rh), (tx, ty, tw, th));

    // When magnified, tray width expands
    dock.on_mouse_move(600.0, 140.0);
    let (_, _, mag_w, _) = dock.get_tray_geometry();
    assert!(
        mag_w > tw,
        "Magnified tray width ({mag_w}) should be wider than resting width ({tw})"
    );

    // Squircle contour generation
    let points_per_quadrant = 16;
    let contour = dock.get_tray_contour(points_per_quadrant);
    assert_eq!(contour.len(), points_per_quadrant * 4);

    let (active_x, active_y, active_w, active_h) = dock.get_tray_geometry();
    let x_min = active_x;
    let x_max = active_x + active_w;
    let y_min = active_y;
    let y_max = active_y + active_h;

    // All contour points must lie within the tray bounding box (allowing tiny float eps)
    for (px, py) in contour {
        assert!(
            px >= x_min - 0.01 && px <= x_max + 0.01,
            "Point px={px} outside [{x_min}, {x_max}]"
        );
        assert!(
            py >= y_min - 0.01 && py <= y_max + 0.01,
            "Point py={py} outside [{y_min}, {y_max}]"
        );
    }
}

#[test]
fn test_edge_cases_and_defensive_guards() {
    // Empty items
    let mut empty_dock = DockView::with_items(Vec::new());
    assert_eq!(empty_dock.get_render_items().len(), 0);
    assert_eq!(empty_dock.calculate_current_scales().len(), 0);
    assert_eq!(empty_dock.on_mouse_click(100.0, 100.0), None);
    empty_dock.trigger_launch(0); // Should not panic

    let (ex, ey, ew, eh) = empty_dock.get_tray_geometry();
    assert!(ew > 0.0 && eh > 0.0 && ex.is_finite() && ey.is_finite());
    let empty_contour = empty_dock.get_tray_contour(8);
    assert_eq!(empty_contour.len(), 32);

    // Non-finite coordinates and invalid dt
    let mut dock = DockView::new();
    dock.on_mouse_move(f32::NAN, 100.0);
    assert_eq!(dock.cursor_pos(), None);

    dock.on_mouse_move(100.0, f32::INFINITY);
    assert_eq!(dock.cursor_pos(), None);

    dock.update(-1.0); // Negative dt is a no-op
    dock.update(f32::NAN); // NaN dt is a no-op
}

#[test]
fn test_dock_window_cross_platform_event_queue() {
    let config = DockWindowConfig::default();
    let mut window = DockWindow::create(config.clone()).expect("Window creation succeeds");

    assert!(window.is_open());
    assert_eq!(window.config(), &config);

    // A real HWND queues its initial Paint during creation. Start the injection
    // assertion after those legitimate native startup events have been drained.
    let _ = window.poll_events();

    window.queue_event(WindowEvent::MouseMove { x: 500.0, y: 150.0 });
    window.queue_event(WindowEvent::MouseClick { x: 500.0, y: 150.0 });
    window.queue_event(WindowEvent::MouseLeave);

    let events = window.poll_events();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0], WindowEvent::MouseMove { x: 500.0, y: 150.0 });
    assert_eq!(events[1], WindowEvent::MouseClick { x: 500.0, y: 150.0 });
    assert_eq!(events[2], WindowEvent::MouseLeave);

    window.queue_event(WindowEvent::Destroy);
    let destroy_events = window.poll_events();
    assert_eq!(destroy_events, vec![WindowEvent::Destroy]);
    assert!(!window.is_open());
}

#[test]
fn test_mouse_leave_smooth_scale_decay() {
    let mut dock = DockView::new();
    dock.on_mouse_move(600.0, 136.0);
    let items_hovered = dock.get_render_items();
    let peak_scale_hover = items_hovered[5].scale;
    assert!(peak_scale_hover > 2.0);

    // Mouse leaves
    dock.on_mouse_leave();
    // After 1 frame (8.33ms at 120Hz), scale must have decayed partially, NOT snapped to 1.0
    dock.update(1.0 / 120.0);
    let items_frame1 = dock.get_render_items();
    assert!(items_frame1[5].scale > 1.0);
    assert!(items_frame1[5].scale < peak_scale_hover);

    // After 200ms, scale must have fully settled to 1.0
    for _ in 0..24 {
        dock.update(1.0 / 120.0);
    }
    let items_settled = dock.get_render_items();
    assert!((items_settled[5].scale - 1.0).abs() < 1e-3);
}


#[test]
fn empty_dock_retains_a_visible_context_menu_target() {
    let dock = DockView::with_items(vec![]);
    assert!(dock.get_tray_geometry().2 >= 200.0);
    assert_eq!(dock.get_tray_geometry(), dock.resting_tray_geometry());
}
