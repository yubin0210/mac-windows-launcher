use win_launcher::menu_bar_render::{MenuBarRenderContext, MenuBarRenderer};
use win_launcher::menu_bar_view::MenuBarView;

#[test]
fn test_menu_bar_renderer_new_and_trait() {
    let mut renderer = MenuBarRenderer::new().expect("MenuBarRenderer::new should succeed");
    let view = MenuBarView::new();
    let res = renderer.render_menu_bar(&view, 1920.0);
    assert!(res.is_ok());
    let ctx = renderer.mock_context().expect("Mock context should exist");
    assert_eq!(ctx.rendered_frames, 1);
}

#[test]
fn test_menu_bar_renderer_mock_mode() {
    let mut renderer = MenuBarRenderer::new_mock();
    let mut view = MenuBarView::new();
    view.update_active_app("Xcode");

    let result = renderer.render(&view, 1920.0);
    assert!(result.is_ok());

    let ctx = renderer.mock_context().expect("Mock context should exist");
    assert_eq!(ctx.rendered_frames, 1);
    assert!(ctx.drawn_elements.contains(&"top_bar_background".to_string()));
    assert!(ctx.drawn_elements.contains(&"apple_logo".to_string()));
    assert!(ctx.drawn_elements.contains(&"app_title".to_string()));
    assert!(ctx.drawn_elements.contains(&"clock".to_string()));
    assert_eq!(ctx.last_app_title, "Xcode");
    assert!(!ctx.dropdown_open);
}

#[test]
fn test_menu_bar_renderer_with_open_dropdown() {
    let mut renderer = MenuBarRenderer::new_mock();
    let mut view = MenuBarView::new();
    view.open_dropdown("apple");

    let result = renderer.render(&view, 1920.0);
    assert!(result.is_ok());

    let ctx = renderer.mock_context().expect("Mock context should exist");
    assert_eq!(ctx.rendered_frames, 1);
    assert!(ctx.dropdown_open);
    assert_eq!(ctx.dropdown_menu_id, Some("apple".to_string()));
    assert!(ctx.dropdown_items_count > 0);
    assert!(ctx.drawn_elements.contains(&"dropdown_container".to_string()));
    assert!(ctx.drawn_elements.contains(&"dropdown_item".to_string()));
}

#[test]
fn test_menu_bar_renderer_invalid_geometry() {
    let mut renderer = MenuBarRenderer::new_mock();
    let view = MenuBarView::new();

    let result = renderer.render(&view, -100.0);
    assert!(result.is_err());

    let result_zero = renderer.render(&view, 0.0);
    assert!(result_zero.is_err());
}
