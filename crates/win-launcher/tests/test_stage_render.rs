//! Tests for Stage Shelf Direct2D Surface Rasterizer & Mock Renderer.

use mac_physics_core::{Rect, StageCardLayout};
use win_launcher::stage_render::{MockStageRenderer, StageRenderContext};
#[cfg(not(windows))]
use win_launcher::stage_render::StageRenderer;

#[test]
fn test_mock_renderer_empty_cards_list() {
    let mut renderer = MockStageRenderer::new();
    let res = renderer.draw_stage_shelf(&[]);
    assert!(res.is_ok(), "Empty cards list should render cleanly");
    assert!(renderer.cards_drawn().is_empty());
    assert_eq!(renderer.drawn_cards.len(), 0);
}

#[test]
fn test_single_card_rendering_records_attributes() {
    let mut renderer = MockStageRenderer::new();
    let card = StageCardLayout {
        group_id: "group-safari".to_string(),
        app_name: "Safari".to_string(),
        bounds: Rect::new(16.0, 120.0, 150.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 148.0, 134.0, 64.0),
        stack_depth: 1,
        opacity: 0.95,
        is_hovered: false,
    };

    let res = renderer.draw_stage_shelf(&[card.clone()]);
    assert!(res.is_ok());

    let drawn = renderer.cards_drawn();
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].group_id, "group-safari");
    assert_eq!(drawn[0].app_name, "Safari");
    assert_eq!(drawn[0].bounds.x, 16.0);
    assert_eq!(drawn[0].bounds.y, 120.0);
    assert_eq!(drawn[0].bounds.width, 150.0);
    assert_eq!(drawn[0].bounds.height, 100.0);
    assert_eq!(drawn[0].opacity, 0.95);
    assert!(!drawn[0].is_hovered);
    assert_eq!(drawn[0].stack_depth, 1);
}

#[test]
fn test_multi_window_stacked_card_layout_stack_depth() {
    let mut renderer = MockStageRenderer::new();
    let card = StageCardLayout {
        group_id: "group-xcode".to_string(),
        app_name: "Xcode".to_string(),
        bounds: Rect::new(16.0, 240.0, 150.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 268.0, 134.0, 64.0),
        stack_depth: 3,
        opacity: 1.0,
        is_hovered: false,
    };

    renderer.draw_stage_shelf(&[card]).expect("Failed to draw multi-window card");

    let drawn = renderer.cards_drawn();
    assert_eq!(drawn.len(), 1);
    assert!(drawn[0].stack_depth >= 2);
    assert_eq!(drawn[0].stack_depth, 3);
    assert_eq!(drawn[0].app_name, "Xcode");
}

#[test]
fn test_hover_state_styling_recorded() {
    let mut renderer = MockStageRenderer::new();
    let card = StageCardLayout {
        group_id: "group-terminal".to_string(),
        app_name: "Terminal".to_string(),
        bounds: Rect::new(16.0, 360.0, 150.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 388.0, 134.0, 64.0),
        stack_depth: 2,
        opacity: 1.0,
        is_hovered: true,
    };

    renderer.draw_stage_shelf(&[card]).expect("Failed to draw hovered card");

    let drawn = renderer.cards_drawn();
    assert_eq!(drawn.len(), 1);
    assert!(drawn[0].is_hovered, "Card must record hovered state");
}

#[test]
fn test_invalid_and_non_finite_bounds_rejection() {
    let mut renderer = MockStageRenderer::new();

    // NaN x
    let nan_x = StageCardLayout {
        group_id: "nan-x".to_string(),
        app_name: "Err".to_string(),
        bounds: Rect::new(f32::NAN, 100.0, 150.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 128.0, 134.0, 64.0),
        stack_depth: 1,
        opacity: 1.0,
        is_hovered: false,
    };
    assert!(renderer.draw_stage_shelf(&[nan_x]).is_err());

    // Infinity y
    let inf_y = StageCardLayout {
        group_id: "inf-y".to_string(),
        app_name: "Err".to_string(),
        bounds: Rect::new(16.0, f32::INFINITY, 150.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 128.0, 134.0, 64.0),
        stack_depth: 1,
        opacity: 1.0,
        is_hovered: false,
    };
    assert!(renderer.draw_stage_shelf(&[inf_y]).is_err());

    // Negative width
    let neg_w = StageCardLayout {
        group_id: "neg-w".to_string(),
        app_name: "Err".to_string(),
        bounds: Rect::new(16.0, 100.0, -10.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 128.0, 134.0, 64.0),
        stack_depth: 1,
        opacity: 1.0,
        is_hovered: false,
    };
    assert!(renderer.draw_stage_shelf(&[neg_w]).is_err());

    // Negative height
    let neg_h = StageCardLayout {
        group_id: "neg-h".to_string(),
        app_name: "Err".to_string(),
        bounds: Rect::new(16.0, 100.0, 150.0, -50.0),
        thumbnail_bounds: Rect::new(24.0, 128.0, 134.0, 64.0),
        stack_depth: 1,
        opacity: 1.0,
        is_hovered: false,
    };
    assert!(renderer.draw_stage_shelf(&[neg_h]).is_err());

    // Negative opacity
    let neg_opacity = StageCardLayout {
        group_id: "neg-op".to_string(),
        app_name: "Err".to_string(),
        bounds: Rect::new(16.0, 100.0, 150.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 128.0, 134.0, 64.0),
        stack_depth: 1,
        opacity: -0.5,
        is_hovered: false,
    };
    assert!(renderer.draw_stage_shelf(&[neg_opacity]).is_err());
}

#[test]
fn test_mock_renderer_clear_lifecycle() {
    let mut renderer = MockStageRenderer::new();
    let card = StageCardLayout {
        group_id: "group-1".to_string(),
        app_name: "Finder".to_string(),
        bounds: Rect::new(16.0, 100.0, 150.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 128.0, 134.0, 64.0),
        stack_depth: 1,
        opacity: 1.0,
        is_hovered: false,
    };

    renderer.draw_stage_shelf(&[card]).unwrap();
    assert_eq!(renderer.cards_drawn().len(), 1);

    renderer.clear();
    assert_eq!(renderer.cards_drawn().len(), 0);
    assert!(renderer.drawn_cards.is_empty());
}

#[test]
fn test_draw_stage_card_convenience_method() {
    let mut renderer = MockStageRenderer::new();
    let card = StageCardLayout {
        group_id: "group-notes".to_string(),
        app_name: "Placeholder".to_string(),
        bounds: Rect::new(16.0, 100.0, 150.0, 100.0),
        thumbnail_bounds: Rect::new(24.0, 128.0, 134.0, 64.0),
        stack_depth: 1,
        opacity: 0.8,
        is_hovered: false,
    };

    renderer.draw_stage_card(&card, "Notes", 4).expect("draw_stage_card should succeed");
    let drawn = renderer.cards_drawn();
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].app_name, "Notes");
    assert_eq!(drawn[0].stack_depth, 4);
    assert_eq!(drawn[0].opacity, 0.8);
}

#[cfg(not(windows))]
#[test]
fn test_stage_renderer_type_interface() {
    let mut renderer = StageRenderer::new();
    let res = renderer.draw_stage_shelf(&[]);
    assert!(res.is_ok());
}
