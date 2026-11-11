//! spotlight_render
//! Native composition background and DirectWrite result list; EDIT owns query painting.
//!
//! Windows: shared-device surface, rounded panel, app icons, selection and text.
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
    use crate::{directx::DirectCompositionPipeline, icon_cache::IconPixels, spotlight_layout::SpotlightGeometry};
    use std::collections::HashMap;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::DirectWrite::*;
    use windows::Win32::Graphics::DirectComposition::IDCompositionSurface;
    use windows::Win32::Graphics::Dxgi::Common::*;

    pub struct CachedSpotlightBrushes {
        pub bar_fill: ID2D1SolidColorBrush, pub bar_stroke: ID2D1SolidColorBrush,
        pub search_icon: ID2D1SolidColorBrush, pub selection_fill: ID2D1SolidColorBrush,
        pub category_badge: ID2D1SolidColorBrush, pub divider_stroke: ID2D1SolidColorBrush,
    }
    impl CachedSpotlightBrushes {
        pub fn new(context: &ID2D1DeviceContext) -> Result<Self, RenderError> {
            let brush = |r, g, b, a| unsafe { context.CreateSolidColorBrush(&D2D1_COLOR_F { r, g, b, a }, None).map_err(RenderError::from) };
            Ok(Self {
                bar_fill: brush(32.0/255.0, 36.0/255.0, 41.0/255.0, 1.0)?, bar_stroke: brush(1.0, 1.0, 1.0, 0.2)?,
                search_icon: brush(0.96, 0.96, 0.96, 1.0)?, selection_fill: brush(0.0, 0.36, 0.74, 0.95)?,
                category_badge: brush(0.72, 0.75, 0.80, 1.0)?, divider_stroke: brush(1.0, 1.0, 1.0, 0.12)?,
            })
        }
    }
    struct Fonts { scale: f32, title: IDWriteTextFormat, detail: IDWriteTextFormat }
    pub struct Direct2DSpotlightRenderer {
        device: ID2D1Device, surface: IDCompositionSurface,
        text_factory: IDWriteFactory, fonts: Option<Fonts>, brushes: Option<CachedSpotlightBrushes>,
        icons: HashMap<String, ID2D1Bitmap1>, size: (u32, u32),
    }
    impl Direct2DSpotlightRenderer {
        pub fn with_pipeline(pipeline: &DirectCompositionPipeline, width: u32, height: u32) -> Result<Self, RenderError> {
            let surface = unsafe { pipeline.dcomp_device().CreateSurface(width, height, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_ALPHA_MODE_PREMULTIPLIED)? };
            unsafe { pipeline.root_visual().SetContent(&surface)?; }
            Ok(Self { device: pipeline.d2d_device().clone(), surface,
                text_factory: unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? }, fonts: None, brushes: None,
                icons: HashMap::new(), size: (width, height) })
        }
        pub fn surface(&self) -> &IDCompositionSurface { &self.surface }
        pub fn resize(&mut self, pipeline: &DirectCompositionPipeline, width: u32, height: u32) -> Result<(), RenderError> {
            if self.size == (width, height) { return Ok(()); }
            let surface = unsafe { pipeline.dcomp_device().CreateSurface(width, height, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_ALPHA_MODE_PREMULTIPLIED)? };
            unsafe { pipeline.root_visual().SetContent(&surface)?; }
            self.surface = surface; self.size = (width, height);
            Ok(())
        }
        pub fn retain_icons(&mut self, ids: &[String]) { self.icons.retain(|id, _| ids.contains(id)); }
        pub fn set_icon(&mut self, id: &str, pixels: &IconPixels) -> Result<(), RenderError> {
            let context = unsafe { self.device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)? };
            let bitmap = crate::directx::upload_icon(&context, pixels)?;
            self.icons.insert(id.into(), bitmap);
            Ok(())
        }
        fn fonts(&mut self, scale: f32) -> Result<(), RenderError> {
            if self.fonts.as_ref().is_some_and(|fonts| fonts.scale == scale) { return Ok(()); }
            let create = |size: f32| -> Result<IDWriteTextFormat, RenderError> {
                let format = unsafe { self.text_factory.CreateTextFormat(windows::core::w!("Segoe UI"), None,
                    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, size * scale, windows::core::w!(""))? };
                unsafe { format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?; format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?; }
                Ok(format)
            };
            self.fonts = Some(Fonts { scale, title: create(16.0)?, detail: create(11.0)? });
            Ok(())
        }
        fn text(&self, context: &ID2D1DeviceContext, text: &str, format: &IDWriteTextFormat, brush: &ID2D1SolidColorBrush, bounds: D2D_RECT_F) -> Result<(), RenderError> {
            let units: Vec<u16> = text.encode_utf16().collect();
            let layout = unsafe { self.text_factory.CreateTextLayout(&units, format, (bounds.right-bounds.left).max(1.0), (bounds.bottom-bounds.top).max(1.0))? };
            let ellipsis = unsafe { self.text_factory.CreateEllipsisTrimmingSign(format)? };
            unsafe {
                layout.SetTrimming(&DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, ..Default::default() }, &ellipsis)?;
                context.DrawTextLayout(D2D_POINT_2F { x: bounds.left, y: bounds.top }, &layout, brush, D2D1_DRAW_TEXT_OPTIONS_CLIP);
            }
            Ok(())
        }
        pub fn draw(&mut self, query: &str, results: &[SpotlightItem], selected: usize, geometry: SpotlightGeometry) -> Result<(), RenderError> {
            if geometry.width <= 0 || geometry.height <= 0 || !geometry.scale.is_finite() || geometry.scale <= 0.0 || self.size != (geometry.width as u32, geometry.height as u32) {
                return Err(RenderError::InvalidGeometry("Spotlight geometry does not match its surface"));
            }
            self.fonts(geometry.scale)?;
            let mut offset = POINT::default();
            let context: ID2D1DeviceContext = unsafe { self.surface.BeginDraw(None, &mut offset)? };
            let draw = (|| {
                if self.brushes.is_none() { self.brushes = Some(CachedSpotlightBrushes::new(&context)?); }
                let brushes = self.brushes.as_ref().ok_or(RenderError::DrawingFailed("Spotlight brushes unavailable".into()))?;
                let fonts = self.fonts.as_ref().ok_or(RenderError::DrawingFailed("Spotlight fonts unavailable".into()))?;
                let scale = geometry.scale;
                let ox = offset.x as f32; let oy = offset.y as f32;
                let width = geometry.width as f32; let height = geometry.height as f32;
                let bounds = |left, top, right, bottom| D2D_RECT_F { left: ox + left, top: oy + top, right: ox + right, bottom: oy + bottom };
                unsafe {
                    context.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
                    let panel = D2D1_ROUNDED_RECT { rect: bounds(0.5, 0.5, width-0.5, height-0.5), radiusX: 14.0*scale, radiusY: 14.0*scale };
                    context.FillRoundedRectangle(&panel, &brushes.bar_fill);
                    context.DrawRoundedRectangle(&panel, &brushes.bar_stroke, 1.0, None);
                    let lens = D2D1_ELLIPSE { point: D2D_POINT_2F { x: ox + 24.0*scale, y: oy + 26.0*scale }, radiusX: 6.0*scale, radiusY: 6.0*scale };
                    context.DrawEllipse(&lens, &brushes.category_badge, 1.8*scale, None);
                    context.DrawLine(D2D_POINT_2F { x: ox+28.0*scale, y: oy+30.0*scale }, D2D_POINT_2F { x: ox+34.0*scale, y: oy+36.0*scale }, &brushes.category_badge, 1.8*scale, None);
                }
                // The native child EDIT paints the query, caret and IME composition above this tree.
                for (index, item) in results.iter().take(geometry.visible_rows).enumerate() {
                    let y = geometry.header + index as f32 * geometry.row_height;
                    if index == selected {
                        let row = D2D1_ROUNDED_RECT { rect: bounds(8.0*scale, y+2.0*scale, width-8.0*scale, y+geometry.row_height-2.0*scale), radiusX: 7.0*scale, radiusY: 7.0*scale };
                        unsafe { context.FillRoundedRectangle(&row, &brushes.selection_fill); }
                    }
                    let icon = bounds(16.0*scale, y+8.0*scale, 48.0*scale, y+40.0*scale);
                    if let Some(bitmap) = self.icons.get(&item.id) {
                        let size = unsafe { bitmap.GetSize() };
                        let factor = (32.0*scale/size.width).min(32.0*scale/size.height);
                        let center_x = ox+32.0*scale; let center_y = oy+y+24.0*scale;
                        let target = D2D_RECT_F { left: center_x-size.width*factor/2.0, right: center_x+size.width*factor/2.0,
                            top: center_y-size.height*factor/2.0, bottom: center_y+size.height*factor/2.0 };
                        unsafe { context.DrawBitmap(bitmap, Some(&target), 1.0, D2D1_INTERPOLATION_MODE_LINEAR, None, None); }
                    } else {
                        let label = if item.category == mac_physics_core::spotlight::SpotlightCategory::Calculation { "=".into() } else { item.title.chars().take(2).collect::<String>() };
                        self.text(&context, &label, &fonts.title, &brushes.search_icon, icon)?;
                    }
                    self.text(&context, &item.title, &fonts.title, &brushes.search_icon, bounds(60.0*scale, y+3.0*scale, width-20.0*scale, y+26.0*scale))?;
                    self.text(&context, &item.subtitle, &fonts.detail, &brushes.category_badge, bounds(60.0*scale, y+25.0*scale, width-20.0*scale, y+43.0*scale))?;
                }
                if !query.trim().is_empty() && results.is_empty() {
                    self.text(&context, "No matching apps or settings", &fonts.title, &brushes.category_badge,
                        bounds(20.0*scale, geometry.header, width-20.0*scale, geometry.header+geometry.row_height))?;
                }
                let footer = if results.get(selected).is_some_and(|item| item.category == mac_physics_core::spotlight::SpotlightCategory::Calculation) {
                    "Enter to copy result · Esc to close"
                } else if query.trim().is_empty() { "Search apps, settings, or enter a calculation" }
                else { "↑ ↓ select · Enter open · Esc close" };
                self.text(&context, footer, &fonts.detail, &brushes.category_badge, bounds(16.0*scale, height-24.0*scale, width-16.0*scale, height-2.0*scale))?;
                Ok(())
            })();
            let end = unsafe { self.surface.EndDraw().map_err(RenderError::from) };
            draw.and(end)
        }
    }
    impl SpotlightRenderContext for Direct2DSpotlightRenderer {
        fn render_spotlight(&mut self, query: &str, results: &[SpotlightItem], selected: usize, width: f32, height: f32) -> Result<(), RenderError> {
            if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 { return Err(RenderError::InvalidGeometry("Invalid Spotlight dimensions")); }
            let scale = width/640.0;
            let geometry = SpotlightGeometry { x: 0, y: 0, width: width as i32, height: height as i32, scale,
                header: 56.0*scale, row_height: 48.0*scale, capacity: 5, visible_rows: results.len().min(5) };
            self.draw(query, results, selected, geometry)
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
