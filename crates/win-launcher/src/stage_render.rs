//! stage_render
//! Direct2D 1.1 surface rasterizer for the macOS Stage Manager left flyout shelf.
//!
//! Windows: Direct2D path geometries, DirectWrite text layouts, and brush caching
//! for frosted squircle cards, stacked multi-window fan-out, app title badges,
//! and hover highlights.
//! Non-Windows: Mock rasterizer context recording card layouts for headless verification.

use mac_physics_core::StageCardLayout;

/// Trait defining the Stage Manager shelf surface rendering contract.
pub trait StageRenderContext: Send + Sync {
    /// Renders the collection of staged window cards on the shelf surface.
    fn draw_stage_shelf(&mut self, cards: &[StageCardLayout]) -> Result<(), String>;

    /// Clears the rendered shelf surface or tracked draw commands.
    fn clear(&mut self);

    /// Convenience helper for rendering a single staged card with custom title and window count.
    fn draw_stage_card(
        &mut self,
        card: &StageCardLayout,
        app_name: &str,
        window_count: usize,
    ) -> Result<(), String> {
        let mut staged = card.clone();
        staged.app_name = app_name.to_string();
        staged.stack_depth = window_count;
        self.draw_stage_shelf(&[staged])
    }
}

/// Validates card bounds and opacity for finite, non-negative dimensions.
pub fn validate_cards(cards: &[StageCardLayout]) -> Result<(), String> {
    for card in cards {
        if !card.bounds.x.is_finite()
            || !card.bounds.y.is_finite()
            || !card.bounds.width.is_finite()
            || !card.bounds.height.is_finite()
        {
            return Err("Card bounds coordinates must be finite numbers".to_string());
        }
        if card.bounds.width < 0.0 || card.bounds.height < 0.0 {
            return Err("Card bounds dimensions must be non-negative".to_string());
        }
        if !card.opacity.is_finite() || card.opacity < 0.0 {
            return Err("Card opacity must be a non-negative finite number".to_string());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-Platform Mock Implementation
// ---------------------------------------------------------------------------

/// In-memory mock renderer implementing `StageRenderContext` for cross-platform testing.
#[derive(Debug, Clone, Default)]
pub struct MockStageRenderer {
    pub drawn_cards: Vec<StageCardLayout>,
}

impl MockStageRenderer {
    /// Creates a new empty `MockStageRenderer`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new mock renderer (alias for `new()`).
    pub fn new_mock() -> Self {
        Self::new()
    }

    /// Returns a slice of all cards drawn since creation or last `clear()`.
    pub fn cards_drawn(&self) -> &[StageCardLayout] {
        &self.drawn_cards
    }

    /// Clears recorded card draw commands.
    pub fn clear(&mut self) {
        self.drawn_cards.clear();
    }
}

impl StageRenderContext for MockStageRenderer {
    fn draw_stage_shelf(&mut self, cards: &[StageCardLayout]) -> Result<(), String> {
        validate_cards(cards)?;
        self.drawn_cards.extend_from_slice(cards);
        Ok(())
    }

    fn clear(&mut self) {
        self.drawn_cards.clear();
    }
}

#[cfg(not(windows))]
pub type StageRenderer = MockStageRenderer;

// ---------------------------------------------------------------------------
// Windows Implementation (Direct2D 1.1 / DirectWrite Shelf Rasterizer)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32 {
    use super::*;
    use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
    use windows::Win32::Graphics::Direct2D::{
        D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT, ID2D1DeviceContext,
        ID2D1SolidColorBrush,
    };
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
        DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
        DWRITE_TEXT_ALIGNMENT_LEADING,
    };

    /// Hardware-accelerated Direct2D 1.1 surface rasterizer for Stage Manager shelf on Windows.
    pub struct Direct2DStageRenderer {
        pub d2d_context: ID2D1DeviceContext,
        pub dwrite_factory: IDWriteFactory,
        pub title_format: IDWriteTextFormat,
        pub badge_format: IDWriteTextFormat,
        pub card_fill_brush: ID2D1SolidColorBrush,
        pub card_border_brush: ID2D1SolidColorBrush,
        pub hover_glow_brush: ID2D1SolidColorBrush,
        pub stack_card_brush: ID2D1SolidColorBrush,
        pub text_brush: ID2D1SolidColorBrush,
        pub badge_brush: ID2D1SolidColorBrush,
    }

    impl Direct2DStageRenderer {
        /// Creates a new `Direct2DStageRenderer` bound to the provided Direct2D device context.
        pub fn new(d2d_context: ID2D1DeviceContext) -> Result<Self, String> {
            let create_brush = |r: f32, g: f32, b: f32, a: f32| -> Result<ID2D1SolidColorBrush, String> {
                let color = D2D1_COLOR_F { r, g, b, a };
                unsafe {
                    d2d_context
                        .CreateSolidColorBrush(&color, None)
                        .map_err(|e| format!("Failed to create brush: {e}"))
                }
            };

            let card_fill_brush = create_brush(0.12, 0.12, 0.15, 0.75)?;
            let card_border_brush = create_brush(1.0, 1.0, 1.0, 0.18)?;
            let hover_glow_brush = create_brush(1.0, 1.0, 1.0, 0.45)?;
            let stack_card_brush = create_brush(0.10, 0.10, 0.12, 0.40)?;
            let text_brush = create_brush(1.0, 1.0, 1.0, 0.95)?;
            let badge_brush = create_brush(0.20, 0.20, 0.24, 0.85)?;

            let dwrite_factory: IDWriteFactory = unsafe {
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)
                    .map_err(|e| format!("Failed to create DirectWrite factory: {e}"))?
            };

            let title_format: IDWriteTextFormat = unsafe {
                let format = dwrite_factory
                    .CreateTextFormat(
                        windows::core::w!("Segoe UI Variable Text"),
                        None,
                        DWRITE_FONT_WEIGHT_NORMAL,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        11.0,
                        windows::core::w!("en-US"),
                    )
                    .map_err(|e| format!("Failed to create title text format: {e}"))?;
                let _ = format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING);
                let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
                format
            };

            let badge_format: IDWriteTextFormat = unsafe {
                let format = dwrite_factory
                    .CreateTextFormat(
                        windows::core::w!("Segoe UI Variable Text"),
                        None,
                        DWRITE_FONT_WEIGHT_SEMI_BOLD,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        10.0,
                        windows::core::w!("en-US"),
                    )
                    .map_err(|e| format!("Failed to create badge text format: {e}"))?;
                let _ = format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER);
                let _ = format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
                format
            };

            Ok(Self {
                d2d_context,
                dwrite_factory,
                title_format,
                badge_format,
                card_fill_brush,
                card_border_brush,
                hover_glow_brush,
                stack_card_brush,
                text_brush,
                badge_brush,
            })
        }
    }

    impl StageRenderContext for Direct2DStageRenderer {
        fn draw_stage_shelf(&mut self, cards: &[StageCardLayout]) -> Result<(), String> {
            validate_cards(cards)?;

            unsafe {
                self.d2d_context.BeginDraw();
            }

            let draw_res = (|| -> Result<(), String> {
                for card in cards {
                    let bx = card.bounds.x;
                    let by = card.bounds.y;
                    let bw = card.bounds.width;
                    let bh = card.bounds.height;
                    let op = card.opacity.clamp(0.0, 1.0);

                    // 1. Render stacked background cards (if stack_depth > 1)
                    if card.stack_depth > 1 {
                        let depth_to_draw = card.stack_depth.min(4) - 1;
                        for k in (1..=depth_to_draw).rev() {
                            let offset_x = bx - (k as f32 * 3.0);
                            let offset_y = by - (k as f32 * 3.0);
                            let stack_opacity = 0.4 * op;

                            let stack_rect = D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: offset_x,
                                    top: offset_y,
                                    right: offset_x + bw,
                                    bottom: offset_y + bh,
                                },
                                radiusX: 12.0,
                                radiusY: 12.0,
                            };

                            unsafe {
                                self.stack_card_brush.SetOpacity(stack_opacity);
                                self.d2d_context.FillRoundedRectangle(&stack_rect, &self.stack_card_brush);
                                self.card_border_brush.SetOpacity(0.15 * op);
                                self.d2d_context.DrawRoundedRectangle(&stack_rect, &self.card_border_brush, 1.0, None);
                            }
                        }
                    }

                    // 2. Render primary card
                    let primary_rect = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: bx,
                            top: by,
                            right: bx + bw,
                            bottom: by + bh,
                        },
                        radiusX: 12.0,
                        radiusY: 12.0,
                    };

                    unsafe {
                        self.card_fill_brush.SetOpacity(0.75 * op);
                        self.d2d_context.FillRoundedRectangle(&primary_rect, &self.card_fill_brush);

                        let border_alpha = if card.is_hovered { 0.45 } else { 0.18 } * op;
                        let border_brush = if card.is_hovered {
                            &self.hover_glow_brush
                        } else {
                            &self.card_border_brush
                        };
                        border_brush.SetOpacity(border_alpha);
                        let stroke_width = if card.is_hovered { 1.5 } else { 1.0 };
                        self.d2d_context.DrawRoundedRectangle(&primary_rect, border_brush, stroke_width, None);
                    }

                    // 3. Render app badge / title
                    let title_rect = D2D_RECT_F {
                        left: bx + 10.0,
                        top: by + 4.0,
                        right: bx + bw - (if card.stack_depth > 1 { 28.0 } else { 10.0 }),
                        bottom: by + 22.0,
                    };

                    if !card.app_name.is_empty() {
                        let text_utf16: Vec<u16> = card.app_name.encode_utf16().collect();
                        unsafe {
                            self.text_brush.SetOpacity(0.95 * op);
                            self.d2d_context.DrawText(
                                &text_utf16,
                                &self.title_format,
                                &title_rect,
                                &self.text_brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                                DWRITE_MEASURING_MODE_NATURAL,
                            );
                        }
                    }

                    // 4. Render stack count badge if stack_depth > 1
                    if card.stack_depth > 1 {
                        let badge_rect = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: bx + bw - 24.0,
                                top: by + 4.0,
                                right: bx + bw - 6.0,
                                bottom: by + 20.0,
                            },
                            radiusX: 6.0,
                            radiusY: 6.0,
                        };
                        let badge_text_rect = D2D_RECT_F {
                            left: bx + bw - 24.0,
                            top: by + 4.0,
                            right: bx + bw - 6.0,
                            bottom: by + 20.0,
                        };

                        let count_str = card.stack_depth.to_string();
                        let count_utf16: Vec<u16> = count_str.encode_utf16().collect();

                        unsafe {
                            self.badge_brush.SetOpacity(0.85 * op);
                            self.d2d_context.FillRoundedRectangle(&badge_rect, &self.badge_brush);
                            self.text_brush.SetOpacity(1.0 * op);
                            self.d2d_context.DrawText(
                                &count_utf16,
                                &self.badge_format,
                                &badge_text_rect,
                                &self.text_brush,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                                DWRITE_MEASURING_MODE_NATURAL,
                            );
                        }
                    }
                }
                Ok(())
            })();

            let end_res = unsafe { self.d2d_context.EndDraw(None, None) }
                .map_err(|e| format!("Direct2D EndDraw failed: {e}"));

            draw_res.and(end_res)
        }

        fn clear(&mut self) {
            unsafe {
                self.d2d_context.Clear(Some(&D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                }));
            }
        }
    }
}

#[cfg(windows)]
pub use win32::Direct2DStageRenderer;

#[cfg(windows)]
pub struct StageRenderer {
    inner: Option<Direct2DStageRenderer>,
    mock: MockStageRenderer,
    is_mock: bool,
}

#[cfg(windows)]
impl StageRenderer {
    pub fn new(d2d_context: windows::Win32::Graphics::Direct2D::ID2D1DeviceContext) -> Result<Self, String> {
        let renderer = Direct2DStageRenderer::new(d2d_context)?;
        Ok(Self {
            inner: Some(renderer),
            mock: MockStageRenderer::new(),
            is_mock: false,
        })
    }

    pub fn new_mock() -> Self {
        Self {
            inner: None,
            mock: MockStageRenderer::new(),
            is_mock: true,
        }
    }

    pub fn cards_drawn(&self) -> &[StageCardLayout] {
        self.mock.cards_drawn()
    }
}

#[cfg(windows)]
impl StageRenderContext for StageRenderer {
    fn draw_stage_shelf(&mut self, cards: &[StageCardLayout]) -> Result<(), String> {
        if self.is_mock {
            self.mock.draw_stage_shelf(cards)
        } else if let Some(r) = &mut self.inner {
            r.draw_stage_shelf(cards)
        } else {
            self.mock.draw_stage_shelf(cards)
        }
    }

    fn clear(&mut self) {
        if self.is_mock {
            self.mock.clear();
        } else if let Some(r) = &mut self.inner {
            r.clear();
        } else {
            self.mock.clear();
        }
    }
}
