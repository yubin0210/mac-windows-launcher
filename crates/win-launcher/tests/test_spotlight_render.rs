use mac_physics_core::spotlight::SpotlightCategory;
use mac_physics_core::squircle::generate_squircle_contour;
use win_launcher::indexer::AppIndexer;
use win_launcher::spotlight_render::{
    MockSpotlightRenderContext, SpotlightRenderContext, SpotlightRenderer,
};
use win_launcher::spotlight_view::SpotlightView;

#[test]
fn test_spotlight_renderer_draws_bar_and_results() {
    let mut indexer = AppIndexer::new();
    indexer.register_mock_item("Terminal", "CLI", SpotlightCategory::Application, "wt.exe");
    let mut view = SpotlightView::new(indexer);
    view.input_text("term");

    let mut mock_ctx = MockSpotlightRenderContext::new();
    mock_ctx
        .render_spotlight(
            view.get_query(),
            view.get_results(),
            view.selected_index(),
            640.0,
            180.0,
        )
        .expect("Render failed");

    assert!(mock_ctx.bar_drawn);
    assert_eq!(mock_ctx.items_drawn_count, 1);
    assert_eq!(mock_ctx.selected_index, 0);
    assert_eq!(mock_ctx.query_drawn, "term");
    assert_eq!(mock_ctx.last_dimensions, (640.0, 180.0));
    assert!(mock_ctx.squircle_points_count >= 64);
}

#[test]
fn test_spotlight_renderer_empty_query_draws_resting_bar() {
    let indexer = AppIndexer::new();
    let view = SpotlightView::new(indexer);

    let mut mock_ctx = MockSpotlightRenderContext::new();
    mock_ctx
        .render_spotlight(
            view.get_query(),
            view.get_results(),
            view.selected_index(),
            640.0,
            56.0,
        )
        .expect("Render failed");

    assert!(mock_ctx.bar_drawn);
    assert_eq!(mock_ctx.items_drawn_count, 0);
    assert_eq!(mock_ctx.query_drawn, "");
}

#[test]
fn test_spotlight_renderer_dimensions_validation() {
    let mut mock_ctx = MockSpotlightRenderContext::new();
    let err = mock_ctx.render_spotlight("", &[], 0, -10.0, 56.0);
    assert!(err.is_err());

    let err_zero = mock_ctx.render_spotlight("", &[], 0, 640.0, 0.0);
    assert!(err_zero.is_err());
}

#[test]
fn test_spotlight_contour_dimensions_span_full_width_and_height() {
    let pts = generate_squircle_contour(640.0, 180.0, 4.5, 64);
    let mut max_x = 0.0f32;
    let mut max_y = 0.0f32;
    for (x, y) in pts {
        max_x = max_x.max(x.abs());
        max_y = max_y.max(y.abs());
    }
    // Expected semi-axes are 320.0 and 90.0
    assert!((max_x - 320.0).abs() < 1e-3);
    assert!((max_y - 90.0).abs() < 1e-3);
}

#[test]
fn test_unified_spotlight_renderer_mock_mode() {
    let mut renderer = SpotlightRenderer::new_mock();
    let res = renderer.render_spotlight("test", &[], 0, 640.0, 56.0);
    assert!(res.is_ok());
}
