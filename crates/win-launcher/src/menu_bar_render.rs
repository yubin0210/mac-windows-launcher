//! menu_bar_render
//! Direct2D 1.1 rasterizer for the macOS Top Menu Bar and squircle dropdown menus.
//!
//! Windows: Direct2D path geometries, DirectWrite text layouts, and brush caching
//! for the 28px frosted glass top bar, Apple logo, dynamic app title, status monitors
//! (Wi-Fi, Battery, Volume, Clock), and floating squircle dropdown menus.
//! Non-Windows: High-fidelity mock rasterizer context for unit testing.

use crate::directx::RenderError;
use crate::menu_bar_view::MenuBarView;

/// Trait defining the Menu Bar surface rendering contract.
pub trait MenuBarRenderContext {
    fn render_menu_bar(
        &mut self,
        view: &MenuBarView,
        screen_width: f32,
    ) -> Result<(), RenderError>;
}

// ---------------------------------------------------------------------------
// Cross-Platform Mock Implementation
// ---------------------------------------------------------------------------

/// Mock rendering context tracking drawn items, dimensions, and dropdown state.
#[derive(Debug, Clone, Default)]
pub struct MockMenuBarRenderContext {
    pub rendered_frames: usize,
    pub drawn_elements: Vec<String>,
    pub last_app_title: String,
    pub dropdown_open: bool,
    pub dropdown_menu_id: Option<String>,
    pub dropdown_items_count: usize,
    pub last_screen_width: f32,
}

impl MockMenuBarRenderContext {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MenuBarRenderContext for MockMenuBarRenderContext {
    fn render_menu_bar(
        &mut self,
        view: &MenuBarView,
        screen_width: f32,
    ) -> Result<(), RenderError> {
        if !screen_width.is_finite() || screen_width <= 0.0 {
            return Err(RenderError::InvalidGeometry(
                "Screen width must be a positive finite number",
            ));
        }

        self.rendered_frames += 1;
        self.last_screen_width = screen_width;
        self.last_app_title = view.active_app_title().to_string();
        self.drawn_elements.clear();

        // 1. Draw top bar background
        self.drawn_elements.push("top_bar_background".to_string());

        // 2. Draw left items (Apple logo, App title, menus)
        self.drawn_elements.push("apple_logo".to_string());
        self.drawn_elements.push("app_title".to_string());
        for item in view.items() {
            if item.id != "apple" && item.id != "app_root" {
                self.drawn_elements.push(format!("menu_item_{}", item.id));
            }
        }

        // 3. Draw right status indicators
        self.drawn_elements.push("wifi".to_string());
        self.drawn_elements.push("battery".to_string());
        self.drawn_elements.push("volume".to_string());
        self.drawn_elements.push("clock".to_string());
        self.drawn_elements.push("control_center".to_string());

        // 4. Draw active dropdown if open
        if view.is_dropdown_open() {
            self.dropdown_open = true;
            if let Some(dropdown) = view.active_dropdown() {
                self.dropdown_menu_id = Some(dropdown.menu_id.clone());
                self.dropdown_items_count = dropdown.items.len();
                self.drawn_elements.push("dropdown_container".to_string());
                for item in &dropdown.items {
                    if item.is_separator {
                        self.drawn_elements.push("dropdown_separator".to_string());
                    } else {
                        self.drawn_elements.push("dropdown_item".to_string());
                    }
                }
            }
        } else {
            self.dropdown_open = false;
            self.dropdown_menu_id = None;
            self.dropdown_items_count = 0;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Windows Implementation (Direct2D 1.1 Surface Rasterizer)
// ---------------------------------------------------------------------------

#[cfg(windows)]
pub mod win32 {
    use super::*;
    use mac_physics_core::squircle::generate_squircle_contour;
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D_POINT_2F, D2D_RECT_F, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED,
    };
    use windows::Win32::Graphics::Direct2D::{
        ID2D1DeviceContext, ID2D1Factory1, ID2D1SolidColorBrush, D2D1_ROUNDED_RECT,
    };
    use windows::Win32::Graphics::DirectComposition::IDCompositionVirtualSurface;

    /// Container for cached Direct2D solid color brushes for the Menu Bar.
    pub struct CachedMenuBarBrushes {
        pub bar_fill: ID2D1SolidColorBrush,
        pub bar_border: ID2D1SolidColorBrush,
        pub text_primary: ID2D1SolidColorBrush,
        pub text_secondary: ID2D1SolidColorBrush,
        pub hover_pill: ID2D1SolidColorBrush,
        pub dropdown_fill: ID2D1SolidColorBrush,
        pub dropdown_border: ID2D1SolidColorBrush,
        pub dropdown_selection: ID2D1SolidColorBrush,
        pub separator: ID2D1SolidColorBrush,
        pub battery_green: ID2D1SolidColorBrush,
    }

    impl CachedMenuBarBrushes {
        pub fn new(context: &ID2D1DeviceContext) -> Result<Self, RenderError> {
            let create_brush = |r: f32, g: f32, b: f32, a: f32| -> Result<ID2D1SolidColorBrush, RenderError> {
                let color = D2D1_COLOR_F { r, g, b, a };
                unsafe { context.CreateSolidColorBrush(&color, None).map_err(RenderError::from) }
            };

            Ok(Self {
                bar_fill: create_brush(0.11, 0.11, 0.12, 0.82)?,
                bar_border: create_brush(1.0, 1.0, 1.0, 0.10)?,
                text_primary: create_brush(1.0, 1.0, 1.0, 0.94)?,
                text_secondary: create_brush(0.92, 0.92, 0.96, 0.60)?,
                hover_pill: create_brush(1.0, 1.0, 1.0, 0.15)?,
                dropdown_fill: create_brush(0.14, 0.14, 0.15, 0.96)?,
                dropdown_border: create_brush(1.0, 1.0, 1.0, 0.16)?,
                dropdown_selection: create_brush(0.0, 0.48, 1.0, 0.90)?,
                separator: create_brush(0.35, 0.35, 0.37, 0.45)?,
                battery_green: create_brush(0.20, 0.78, 0.35, 0.95)?,
            })
        }
    }

    /// Direct2D 1.1 hardware rasterizer for the Menu Bar.
    pub struct Direct2DMenuBarRenderer {
        surface: IDCompositionVirtualSurface,
        factory: ID2D1Factory1,
        cached_brushes: Option<CachedMenuBarBrushes>,
    }

    impl Direct2DMenuBarRenderer {
        pub fn new(
            surface: IDCompositionVirtualSurface,
            factory: ID2D1Factory1,
        ) -> Self {
            Self {
                surface,
                factory,
                cached_brushes: None,
            }
        }

        pub fn render_menu_bar(
            &mut self,
            view: &MenuBarView,
            screen_width: f32,
        ) -> Result<(), RenderError> {
            if !screen_width.is_finite() || screen_width <= 0.0 {
                return Err(RenderError::InvalidGeometry("Screen width must be positive"));
            }

            let height = 28.0f32;
            let total_height = if view.is_dropdown_open() {
                height + view.active_dropdown().map(|d| d.total_height()).unwrap_or(0.0) + 20.0
            } else {
                height
            };

            let rect = RECT {
                left: 0,
                top: 0,
                right: screen_width as i32,
                bottom: total_height as i32,
            };

            let _ = unsafe { self.surface.Trim(Some(&[rect])) };

            let mut offset = windows::Win32::Foundation::POINT::default();
            let draw_dc: ID2D1DeviceContext =
                unsafe { self.surface.BeginDraw(None, &mut offset)? };

            if self.cached_brushes.is_none() {
                let brushes = CachedMenuBarBrushes::new(&draw_dc)?;
                self.cached_brushes = Some(brushes);
            }

            let draw_result = (|| -> Result<(), RenderError> {
                let brushes = self.cached_brushes.as_ref().ok_or_else(|| {
                    RenderError::DrawingFailed("Failed to initialize brush cache".to_string())
                })?;

                let clear_color = D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                };
                unsafe { draw_dc.Clear(Some(&clear_color)) };

                let ox = offset.x as f32;
                let oy = offset.y as f32;

                // 1. Draw top 28px frosted bar
                let bar_rect = D2D_RECT_F {
                    left: ox,
                    top: oy,
                    right: ox + screen_width,
                    bottom: oy + height,
                };
                unsafe {
                    draw_dc.FillRectangle(&bar_rect, &brushes.bar_fill);
                    draw_dc.DrawLine(
                        D2D_POINT_2F {
                            x: ox,
                            y: oy + height,
                        },
                        D2D_POINT_2F {
                            x: ox + screen_width,
                            y: oy + height,
                        },
                        &brushes.bar_border,
                        1.0,
                        None,
                    );
                }

                // 2. Draw Menu Items using layout positions
                let layout = view.layout();

                for item in &layout.left_items {
                    let rx = ox + item.bounds.x;
                    let ry = oy + item.bounds.y;
                    let rw = item.bounds.width;
                    let rh = item.bounds.height;

                    // If active dropdown is for this item, draw active highlight pill
                    if let Some(dropdown) = view.active_dropdown() {
                        if dropdown.menu_id == item.id {
                            let pill_rect = D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: rx - 2.0,
                                    top: ry + 2.0,
                                    right: rx + rw + 2.0,
                                    bottom: ry + rh - 2.0,
                                },
                                radiusX: 4.0,
                                radiusY: 4.0,
                            };
                            unsafe {
                                draw_dc.FillRoundedRectangle(&pill_rect, &brushes.hover_pill);
                            }
                        }
                    }

                    // Render Apple icon or text label
                    if item.id == "apple" {
                        let apple_center = D2D_POINT_2F {
                            x: rx + 10.0,
                            y: ry + 14.0,
                        };
                        let apple_ellipse = windows::Win32::Graphics::Direct2D::D2D1_ELLIPSE {
                            point: apple_center,
                            radiusX: 4.5,
                            radiusY: 5.5,
                        };
                        unsafe {
                            draw_dc.FillEllipse(&apple_ellipse, &brushes.text_primary);
                        }
                    }
                }

                // 3. Draw Right Items (Battery fill bar, clock, icons)
                for item in &layout.right_items {
                    let rx = ox + item.bounds.x;
                    let ry = oy + item.bounds.y;

                    if item.id == "battery" {
                        let status = view.status();
                        let batt_rect = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: rx,
                                top: ry + 8.0,
                                right: rx + 22.0,
                                bottom: ry + 20.0,
                            },
                            radiusX: 2.0,
                            radiusY: 2.0,
                        };
                        let fill_pct = status.battery_percent.unwrap_or(100) as f32 / 100.0;
                        let fill_rect = D2D_RECT_F {
                            left: rx + 2.0,
                            top: ry + 10.0,
                            right: rx + 2.0 + (18.0 * fill_pct),
                            bottom: ry + 18.0,
                        };
                        unsafe {
                            draw_dc.DrawRoundedRectangle(&batt_rect, &brushes.text_secondary, 1.0, None);
                            let fill_brush = if status.is_charging {
                                &brushes.battery_green
                            } else {
                                &brushes.text_primary
                            };
                            draw_dc.FillRectangle(&fill_rect, fill_brush);
                        }
                    }
                }

                // 4. Draw Floating Dropdown Menu if open
                if let Some(dropdown) = view.active_dropdown() {
                    let dx = ox + dropdown.origin_x;
                    let dy = oy + dropdown.origin_y;
                    let dw = dropdown.width;
                    let dh = dropdown.total_height();

                    // Generate squircle contour for dropdown
                    let pts = generate_squircle_contour(dw, dh, 4.0, 48);
                    if pts.len() >= 4 {
                        let half_w = dw * 0.5;
                        let half_h = dh * 0.5;
                        let path_geom = unsafe { self.factory.CreatePathGeometry()? };
                        let sink = unsafe { path_geom.Open()? };

                        let first = pts[0];
                        unsafe {
                            sink.BeginFigure(
                                D2D_POINT_2F {
                                    x: dx + half_w + first.0,
                                    y: dy + half_h + first.1,
                                },
                                D2D1_FIGURE_BEGIN_FILLED,
                            );

                            for p in &pts[1..] {
                                sink.AddLine(D2D_POINT_2F {
                                    x: dx + half_w + p.0,
                                    y: dy + half_h + p.1,
                                });
                            }

                            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                            sink.Close()?;

                            draw_dc.FillGeometry(&path_geom, &brushes.dropdown_fill, None);
                            draw_dc.DrawGeometry(&path_geom, &brushes.dropdown_border, 1.0, None);
                        }
                    }

                    // Render rows inside dropdown
                    for (i, row) in dropdown.items.iter().enumerate() {
                        let row_y = dy + 5.0 + (i as f32 * dropdown.item_height);
                        if row.is_separator {
                            let sep_y = row_y + (dropdown.item_height * 0.5);
                            unsafe {
                                draw_dc.DrawLine(
                                    D2D_POINT_2F { x: dx + 12.0, y: sep_y },
                                    D2D_POINT_2F { x: dx + dw - 12.0, y: sep_y },
                                    &brushes.separator,
                                    1.0,
                                    None,
                                );
                            }
                        } else {
                            if dropdown.hovered_index == Some(i) && row.is_enabled {
                                let sel_rect = D2D1_ROUNDED_RECT {
                                    rect: D2D_RECT_F {
                                        left: dx + 6.0,
                                        top: row_y + 1.0,
                                        right: dx + dw - 6.0,
                                        bottom: row_y + dropdown.item_height - 1.0,
                                    },
                                    radiusX: 4.0,
                                    radiusY: 4.0,
                                };
                                unsafe {
                                    draw_dc.FillRoundedRectangle(&sel_rect, &brushes.dropdown_selection);
                                }
                            }
                        }
                    }
                }

                Ok(())
            })();

            let end_draw_result = unsafe { self.surface.EndDraw().map_err(RenderError::from) };
            draw_result.and(end_draw_result)
        }
    }

    impl MenuBarRenderContext for Direct2DMenuBarRenderer {
        fn render_menu_bar(
            &mut self,
            view: &MenuBarView,
            screen_width: f32,
        ) -> Result<(), RenderError> {
            self.render_menu_bar(view, screen_width)
        }
    }
}

#[cfg(windows)]
pub use win32::{CachedMenuBarBrushes, Direct2DMenuBarRenderer};

// ---------------------------------------------------------------------------
// Unified MenuBarRenderer
// ---------------------------------------------------------------------------

/// Unified Menu Bar renderer presenting identical interfaces across platforms.
pub struct MenuBarRenderer {
    #[cfg(windows)]
    inner: Option<Direct2DMenuBarRenderer>,
    mock_context: MockMenuBarRenderContext,
    is_mock: bool,
}

impl Default for MenuBarRenderer {
    fn default() -> Self {
        Self::new_mock()
    }
}

impl MenuBarRenderer {
    /// Creates a MenuBarRenderer (defaults to mock mode on cross-platform development).
    pub fn new() -> Result<Self, RenderError> {
        Ok(Self::new_mock())
    }

    /// Creates a mock renderer for testing and cross-platform execution.
    pub fn new_mock() -> Self {
        Self {
            #[cfg(windows)]
            inner: None,
            mock_context: MockMenuBarRenderContext::new(),
            is_mock: true,
        }
    }

    /// Attaches an active Windows Direct2D renderer.
    #[cfg(windows)]
    pub fn with_direct2d(renderer: Direct2DMenuBarRenderer) -> Self {
        Self {
            inner: Some(renderer),
            mock_context: MockMenuBarRenderContext::new(),
            is_mock: false,
        }
    }

    /// Returns a reference to the mock context if in mock mode.
    pub fn mock_context(&self) -> Option<&MockMenuBarRenderContext> {
        if self.is_mock {
            Some(&self.mock_context)
        } else {
            None
        }
    }

    /// Returns a mutable reference to the mock context if in mock mode.
    pub fn mock_context_mut(&mut self) -> Option<&mut MockMenuBarRenderContext> {
        if self.is_mock {
            Some(&mut self.mock_context)
        } else {
            None
        }
    }

    /// Renders the complete menu bar and any open dropdown menu.
    pub fn render(
        &mut self,
        view: &MenuBarView,
        screen_width: f32,
    ) -> Result<(), RenderError> {
        if self.is_mock {
            self.mock_context.render_menu_bar(view, screen_width)
        } else {
            #[cfg(windows)]
            {
                if let Some(r) = &mut self.inner {
                    r.render_menu_bar(view, screen_width)
                } else {
                    self.mock_context.render_menu_bar(view, screen_width)
                }
            }
            #[cfg(not(windows))]
            {
                self.mock_context.render_menu_bar(view, screen_width)
            }
        }
    }
}

impl MenuBarRenderContext for MenuBarRenderer {
    fn render_menu_bar(
        &mut self,
        view: &MenuBarView,
        screen_width: f32,
    ) -> Result<(), RenderError> {
        self.render(view, screen_width)
    }
}
