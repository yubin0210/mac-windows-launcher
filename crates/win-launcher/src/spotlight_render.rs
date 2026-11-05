//! spotlight_render
//! Direct2D 1.1 rasterizer for the Spotlight floating squircle bar and results list.
//!
//! Windows: Direct2D path geometries for the frosted squircle bar ($n=4.5$),
//! search magnifying glass icon, selection highlight, category pills, and divider lines.
//! Non-Windows: High-fidelity mock rasterizer context for unit testing.

use crate::directx::RenderError;
use mac_physics_core::spotlight::SpotlightItem;
use mac_physics_core::squircle::generate_squircle_contour;

/// Trait defining the Spotlight surface rendering contract.
pub trait SpotlightRenderContext {
    fn render_spotlight(
        &mut self,
        query: &str,
        results: &[SpotlightItem],
        selected_idx: usize,
        width: f32,
        height: f32,
    ) -> Result<(), RenderError>;
}

// ---------------------------------------------------------------------------
// Cross-Platform Mock Implementation
// ---------------------------------------------------------------------------

/// Mock rendering context tracking drawn items, query, and dimensions.
#[derive(Debug, Clone, Default)]
pub struct MockSpotlightRenderContext {
    pub bar_drawn: bool,
    pub query_drawn: String,
    pub items_drawn_count: usize,
    pub selected_index: usize,
    pub last_dimensions: (f32, f32),
    pub squircle_points_count: usize,
}

impl MockSpotlightRenderContext {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SpotlightRenderContext for MockSpotlightRenderContext {
    fn render_spotlight(
        &mut self,
        query: &str,
        results: &[SpotlightItem],
        selected_idx: usize,
        width: f32,
        height: f32,
    ) -> Result<(), RenderError> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(RenderError::InvalidGeometry(
                "Width and height must be positive, finite numbers",
            ));
        }

        let pts = generate_squircle_contour(width, height, 4.5, 64);
        self.squircle_points_count = pts.len();
        self.bar_drawn = true;
        self.query_drawn = query.to_string();
        self.items_drawn_count = results.len();
        self.selected_index = selected_idx;
        self.last_dimensions = (width, height);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Windows Direct2D Implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32 {
    use super::*;
    use windows::core::Interface;
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D_POINT_2F, D2D_RECT_F,
    };
    use windows::Win32::Graphics::Direct2D::{
        D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
        D2D1_ROUNDED_RECT, ID2D1Device, ID2D1DeviceContext, ID2D1Factory1, ID2D1SolidColorBrush,
    };
    use windows::Win32::Graphics::Direct3D11::ID3D11Device;
    use windows::Win32::Graphics::DirectComposition::{IDCompositionDevice, IDCompositionSurface};
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM,
    };
    use windows::Win32::Graphics::Dxgi::IDXGIDevice;

    /// Cached brushes for Spotlight rendering.
    pub struct CachedSpotlightBrushes {
        pub bar_fill: ID2D1SolidColorBrush,
        pub bar_stroke: ID2D1SolidColorBrush,
        pub search_icon: ID2D1SolidColorBrush,
        pub selection_fill: ID2D1SolidColorBrush,
        pub category_badge: ID2D1SolidColorBrush,
        pub divider_stroke: ID2D1SolidColorBrush,
    }

    impl CachedSpotlightBrushes {
        pub fn new(context: &ID2D1DeviceContext) -> Result<Self, RenderError> {
            let create_brush =
                |r: f32, g: f32, b: f32, a: f32| -> Result<ID2D1SolidColorBrush, RenderError> {
                    let color = D2D1_COLOR_F { r, g, b, a };
                    unsafe {
                        context
                            .CreateSolidColorBrush(&color, None)
                            .map_err(RenderError::from)
                    }
                };

            Ok(Self {
                // Frosted dark glass container fill
                bar_fill: create_brush(0.12, 0.12, 0.14, 0.88)?,
                // Subtle acrylic highlight stroke
                bar_stroke: create_brush(1.0, 1.0, 1.0, 0.15)?,
                // Magnifying glass icon
                search_icon: create_brush(0.9, 0.9, 0.9, 0.7)?,
                // Active row selection accent (macOS blue)
                selection_fill: create_brush(0.0, 0.48, 1.0, 0.85)?,
                // Category pill background
                category_badge: create_brush(0.22, 0.22, 0.25, 0.85)?,
                // Divider line between search bar and dropdown results
                divider_stroke: create_brush(1.0, 1.0, 1.0, 0.10)?,
            })
        }
    }

    /// Windows Direct2D Spotlight rasterizer.
    pub struct Direct2DSpotlightRenderer {
        factory: ID2D1Factory1,
        #[allow(dead_code)]
        d2d_device: ID2D1Device,
        #[allow(dead_code)]
        d2d_context: ID2D1DeviceContext,
        surface: IDCompositionSurface,
        cached_brushes: Option<CachedSpotlightBrushes>,
    }

    impl Direct2DSpotlightRenderer {
        pub fn new(
            d3d_device: &ID3D11Device,
            comp_device: &IDCompositionDevice,
            width: u32,
            height: u32,
        ) -> Result<Self, RenderError> {
            let dxgi_device: IDXGIDevice = d3d_device.cast()?;
            let factory: ID2D1Factory1 = unsafe {
                windows::Win32::Graphics::Direct2D::D2D1CreateFactory(
                    D2D1_FACTORY_TYPE_SINGLE_THREADED,
                    None,
                )?
            };
            let d2d_device: ID2D1Device = unsafe { factory.CreateDevice(&dxgi_device)? };
            let d2d_context: ID2D1DeviceContext =
                unsafe { d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)? };

            let surface: IDCompositionSurface = unsafe {
                comp_device.CreateSurface(
                    width,
                    height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_ALPHA_MODE_PREMULTIPLIED,
                )?
            };

            Ok(Self {
                factory,
                d2d_device,
                d2d_context,
                surface,
                cached_brushes: None,
            })
        }

        pub fn surface(&self) -> &IDCompositionSurface {
            &self.surface
        }
    }

    impl SpotlightRenderContext for Direct2DSpotlightRenderer {
        fn render_spotlight(
            &mut self,
            _query: &str,
            results: &[SpotlightItem],
            selected_idx: usize,
            width: f32,
            height: f32,
        ) -> Result<(), RenderError> {
            if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
                return Err(RenderError::InvalidGeometry(
                    "Width and height must be positive, finite numbers",
                ));
            }

            let mut offset = windows::Win32::Foundation::POINT::default();
            let draw_dc: ID2D1DeviceContext =
                unsafe { self.surface.BeginDraw(None, &mut offset)? };

            if self.cached_brushes.is_none() {
                if let Ok(brushes) = CachedSpotlightBrushes::new(&draw_dc) {
                    self.cached_brushes = Some(brushes);
                }
            }

            let draw_result = (|| -> Result<(), RenderError> {
                let brushes = self
                    .cached_brushes
                    .as_ref()
                    .ok_or_else(|| RenderError::DrawingFailed("Failed to initialize brush cache".to_string()))?;

                // Clear with transparent black
                let clear_color = D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                };
                unsafe { draw_dc.Clear(Some(&clear_color)) };

                let ox = offset.x as f32;
                let oy = offset.y as f32;

                // 1. Draw frosted squircle container bar
                let half_w = width * 0.5;
                let half_h = height * 0.5;
                let pts = generate_squircle_contour(width, height, 4.5, 64);
                if pts.len() >= 4 {
                    let path_geom = unsafe { self.factory.CreatePathGeometry()? };
                    let sink = unsafe { path_geom.Open()? };

                    let first = pts[0];
                    unsafe {
                        sink.BeginFigure(
                            D2D_POINT_2F {
                                x: ox + half_w + first.0,
                                y: oy + half_h + first.1,
                            },
                            D2D1_FIGURE_BEGIN_FILLED,
                        );

                        for p in &pts[1..] {
                            sink.AddLine(D2D_POINT_2F {
                                x: ox + half_w + p.0,
                                y: oy + half_h + p.1,
                            });
                        }

                        sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                        sink.Close()?;

                        draw_dc.FillGeometry(&path_geom, &brushes.bar_fill, None);
                        draw_dc.DrawGeometry(&path_geom, &brushes.bar_stroke, 1.0, None);
                    }
                }

                // 2. Draw search icon (magnifying glass)
                let lens_center = D2D_POINT_2F {
                    x: ox + 26.0,
                    y: oy + 28.0,
                };
                let ellipse = D2D1_ELLIPSE {
                    point: lens_center,
                    radiusX: 6.0,
                    radiusY: 6.0,
                };
                unsafe {
                    draw_dc.DrawEllipse(&ellipse, &brushes.search_icon, 2.0, None);
                    draw_dc.DrawLine(
                        D2D_POINT_2F {
                            x: ox + 30.5,
                            y: oy + 32.5,
                        },
                        D2D_POINT_2F {
                            x: ox + 36.0,
                            y: oy + 38.0,
                        },
                        &brushes.search_icon,
                        2.0,
                        None,
                    );
                }

                // 3. Draw divider line and results list if results exist
                if !results.is_empty() {
                    let divider_y = oy + 56.0;
                    unsafe {
                        draw_dc.DrawLine(
                            D2D_POINT_2F {
                                x: ox + 16.0,
                                y: divider_y,
                            },
                            D2D_POINT_2F {
                                x: ox + width - 16.0,
                                y: divider_y,
                            },
                            &brushes.divider_stroke,
                            1.0,
                            None,
                        );
                    }

                    for (i, _item) in results.iter().enumerate() {
                        let item_y = divider_y + i as f32 * 48.0;

                        // Draw selection highlight for active row
                        if i == selected_idx {
                            let sel_rect = D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left: ox + 12.0,
                                    top: item_y + 4.0,
                                    right: ox + width - 12.0,
                                    bottom: item_y + 44.0,
                                },
                                radiusX: 8.0,
                                radiusY: 8.0,
                            };
                            unsafe {
                                draw_dc.FillRoundedRectangle(&sel_rect, &brushes.selection_fill);
                            }
                        }

                        // Draw category badge pill
                        let badge_rect = D2D1_ROUNDED_RECT {
                            rect: D2D_RECT_F {
                                left: ox + width - 84.0,
                                top: item_y + 14.0,
                                right: ox + width - 16.0,
                                bottom: item_y + 34.0,
                            },
                            radiusX: 4.0,
                            radiusY: 4.0,
                        };
                        unsafe {
                            draw_dc.FillRoundedRectangle(&badge_rect, &brushes.category_badge);
                        }
                    }
                }

                Ok(())
            })();

            let end_draw_result = unsafe { self.surface.EndDraw().map_err(RenderError::from) };
            draw_result.and(end_draw_result)
        }
    }
}

#[cfg(windows)]
pub use win32::{CachedSpotlightBrushes, Direct2DSpotlightRenderer};

// ---------------------------------------------------------------------------
// Unified SpotlightRenderer
// ---------------------------------------------------------------------------

/// Unified Spotlight renderer presenting identical interfaces across platforms.
pub struct SpotlightRenderer {
    #[cfg(windows)]
    inner: Option<Direct2DSpotlightRenderer>,
    #[cfg(not(windows))]
    inner: MockSpotlightRenderContext,
}

impl Default for SpotlightRenderer {
    fn default() -> Self {
        Self::new_mock()
    }
}

impl SpotlightRenderer {
    /// Creates a mock renderer for testing.
    pub fn new_mock() -> Self {
        Self {
            #[cfg(windows)]
            inner: None,
            #[cfg(not(windows))]
            inner: MockSpotlightRenderContext::new(),
        }
    }

    /// Attaches an active Windows Direct2D renderer.
    #[cfg(windows)]
    pub fn with_direct2d(renderer: Direct2DSpotlightRenderer) -> Self {
        Self {
            inner: Some(renderer),
        }
    }
}

impl SpotlightRenderContext for SpotlightRenderer {
    fn render_spotlight(
        &mut self,
        query: &str,
        results: &[SpotlightItem],
        selected_idx: usize,
        width: f32,
        height: f32,
    ) -> Result<(), RenderError> {
        #[cfg(windows)]
        {
            if let Some(r) = &mut self.inner {
                r.render_spotlight(query, results, selected_idx, width, height)
            } else {
                let mut mock = MockSpotlightRenderContext::new();
                mock.render_spotlight(query, results, selected_idx, width, height)
            }
        }
        #[cfg(not(windows))]
        {
            self.inner
                .render_spotlight(query, results, selected_idx, width, height)
        }
    }
}
