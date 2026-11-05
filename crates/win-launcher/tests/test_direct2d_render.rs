//! test_direct2d_render
//! Tests for Direct2D surface rasterization engine, squircle tray, and running indicator pips.

use win_launcher::directx::MockRenderContext;
use win_launcher::dock_view::DockView;

#[test]
fn test_render_context_draws_tray_and_pips() {
    let mut dock = DockView::new();
    for i in 0..dock.items().len() {
        dock.set_app_running(i, false, false);
    }
    dock.set_app_running(0, true, true); // First item running and focused
    let items = dock.get_render_items();
    let tray = dock.get_tray_geometry();
    let contour = dock.get_tray_squircle_contour();

    let mut mock_ctx = MockRenderContext::new();
    mock_ctx.render_dock(&items, tray, &contour).expect("Render failed");

    assert!(mock_ctx.tray_drawn);
    assert_eq!(mock_ctx.running_pips_count, 1);
    assert_eq!(mock_ctx.icons_drawn_count, items.len());
}

#[test]
fn test_running_and_focused_pips_count_matches_dock_state() {
    let mut dock = DockView::new();
    for i in 0..dock.items().len() {
        dock.set_app_running(i, false, false);
    }

    // Set item 0 running + focused
    dock.set_app_running(0, true, true);
    // Set item 2 running (not focused)
    dock.set_app_running(2, true, false);
    // Set item 4 running (not focused)
    dock.set_app_running(4, true, false);

    let items = dock.get_render_items();
    let tray = dock.get_tray_geometry();
    let contour = dock.get_tray_squircle_contour();

    let mut mock_ctx = MockRenderContext::new();
    mock_ctx.render_dock(&items, tray, &contour).expect("Render succeeds");

    assert_eq!(mock_ctx.running_pips_count, 3);
    assert_eq!(mock_ctx.focused_pips_count, 1);
    assert_eq!(mock_ctx.drawn_pip_positions.len(), 3);

    // Verify pip horizontal center matches each running item's center_x
    let pip_0 = &mock_ctx.drawn_pip_positions[0];
    assert!((pip_0.0 - items[0].center_x).abs() < 1e-3);
    assert!(pip_0.2); // item 0 is focused

    let pip_1 = &mock_ctx.drawn_pip_positions[1];
    assert!((pip_1.0 - items[2].center_x).abs() < 1e-3);
    assert!(!pip_1.2); // item 2 is not focused

    let pip_2 = &mock_ctx.drawn_pip_positions[2];
    assert!((pip_2.0 - items[4].center_x).abs() < 1e-3);
    assert!(!pip_2.2); // item 4 is not focused

    // Pip vertical position should sit on tray floor (within tray vertical bounds)
    let (_, tray_y, _, tray_h) = tray;
    let tray_bottom = tray_y + tray_h;
    assert!(pip_0.1 > tray_y && pip_0.1 <= tray_bottom);
}

#[test]
fn test_squircle_tray_contour_geometry() {
    let dock = DockView::new();
    let (tx, ty, tw, th) = dock.get_tray_geometry();
    let contour = dock.get_tray_squircle_contour();

    // With points_per_quadrant = 16, total contour points = 16 * 4 = 64
    assert_eq!(contour.len(), dock.config.points_per_quadrant * 4);

    // Verify all contour points lie within the bounding box of the tray (with epsilon for rounding)
    let margin = 0.5;
    for &(px, py) in &contour {
        assert!(px >= tx - margin && px <= tx + tw + margin, "px {px} out of tray X [{tx}, {}]", tx + tw);
        assert!(py >= ty - margin && py <= ty + th + margin, "py {py} out of tray Y [{ty}, {}]", ty + th);
    }
}

#[test]
fn test_render_context_geometry_validation_errors() {
    let dock = DockView::new();
    let items = dock.get_render_items();
    let contour = dock.get_tray_squircle_contour();

    let mut ctx = MockRenderContext::new();

    // Invalid zero or negative tray dimensions
    let err = ctx.render_dock(&items, (0.0, 0.0, 0.0, 50.0), &contour);
    assert!(err.is_err());

    let err2 = ctx.render_dock(&items, (0.0, 0.0, 100.0, -10.0), &contour);
    assert!(err2.is_err());

    // Non-finite tray coordinates
    let err3 = ctx.render_dock(&items, (f32::NAN, 0.0, 100.0, 50.0), &contour);
    assert!(err3.is_err());

    // Too few squircle contour points (< 4)
    let few_points = vec![(0.0, 0.0), (10.0, 0.0)];
    let err4 = ctx.render_dock(&items, (0.0, 0.0, 100.0, 50.0), &few_points);
    assert!(err4.is_err());
}

#[test]
fn test_mock_render_context_clear_and_reuse() {
    let mut dock = DockView::new();
    dock.set_app_running(0, true, true);
    let items = dock.get_render_items();
    let tray = dock.get_tray_geometry();
    let contour = dock.get_tray_squircle_contour();

    let mut ctx = MockRenderContext::with_size(800, 600);
    assert_eq!(ctx.width, 800);
    assert_eq!(ctx.height, 600);

    ctx.render_dock(&items, tray, &contour).unwrap();
    assert!(ctx.tray_drawn);
    assert!(ctx.icons_drawn_count > 0);

    ctx.clear();
    assert!(!ctx.tray_drawn);
    assert_eq!(ctx.running_pips_count, 0);
    assert_eq!(ctx.icons_drawn_count, 0);
    assert_eq!(ctx.focused_pips_count, 0);
    assert!(ctx.drawn_pip_positions.is_empty());
}

#[test]
fn test_brushes_are_cached_and_reused() {
    let dock = DockView::new();
    let items = dock.get_render_items();
    let tray = dock.get_tray_geometry();
    let contour = dock.get_tray_squircle_contour();

    let mut ctx = MockRenderContext::new();
    ctx.render_dock(&items, tray, &contour).expect("First frame render succeeds");
    assert!(ctx.brushes_cached);

    // Subsequent frame render reuses cached brushes
    ctx.render_dock(&items, tray, &contour).expect("Second frame render succeeds");
    assert!(ctx.brushes_cached);
}

#[test]
fn test_surface_end_draw_guaranteed_on_success_and_error() {
    let dock = DockView::new();
    let items = dock.get_render_items();
    let tray = dock.get_tray_geometry();
    let contour = dock.get_tray_squircle_contour();

    let mut ctx = MockRenderContext::new();

    // 1. Success case: EndDraw is called, surface is not left locked
    let res = ctx.render_dock(&items, tray, &contour);
    assert!(res.is_ok());
    assert!(!ctx.surface_locked, "Surface must be unlocked after successful render");
    assert!(ctx.end_draw_called, "EndDraw must be called after successful render");

    // 2. Error case (invalid item size): EndDraw is still called, surface unlocked
    let mut invalid_items = items.clone();
    invalid_items[0].current_size = -5.0; // Invalid geometry inside drawing loop
    let err_res = ctx.render_dock(&invalid_items, tray, &contour);
    assert!(err_res.is_err(), "Must return error for invalid item");
    assert!(!ctx.surface_locked, "Surface must be unlocked even when render error occurs");
    assert!(ctx.end_draw_called, "EndDraw must be executed on error to prevent surface leak");
}
