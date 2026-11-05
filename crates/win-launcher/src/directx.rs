//! directx
//! Direct3D 11 device, DirectComposition visual tree pipeline, and Direct2D 1.1 surface rasterization engine.

use crate::dock_view::DockRenderItem;

#[cfg(windows)]
use windows::core::Interface;
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_END_CLOSED, D2D_POINT_2F, D2D_RECT_F,
};
#[cfg(windows)]
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_ELLIPSE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_ROUNDED_RECT, ID2D1Device, ID2D1DeviceContext,
    ID2D1Factory1, ID2D1SolidColorBrush,
};
#[cfg(windows)]
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
#[cfg(windows)]
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_SDK_VERSION,
};
#[cfg(windows)]
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionSurface, IDCompositionTarget,
    IDCompositionVisual,
};
#[cfg(windows)]
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM,
};
#[cfg(windows)]
use windows::Win32::Graphics::Dxgi::IDXGIDevice;

/// Error type covering Direct2D and DirectComposition surface rendering operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    InvalidGeometry(&'static str),
    DeviceError(String),
    DrawingFailed(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGeometry(msg) => write!(f, "Invalid geometry: {msg}"),
            Self::DeviceError(msg) => write!(f, "Device error: {msg}"),
            Self::DrawingFailed(msg) => write!(f, "Drawing failed: {msg}"),
        }
    }
}

impl std::error::Error for RenderError {}

#[cfg(windows)]
impl From<windows::core::Error> for RenderError {
    fn from(err: windows::core::Error) -> Self {
        Self::DeviceError(err.to_string())
    }
}

/// Container for cached Direct2D solid color brushes to eliminate COM allocation in the 120Hz render loop.
#[cfg(windows)]
pub struct CachedBrushes {
    pub tray_fill: ID2D1SolidColorBrush,
    pub tray_stroke: ID2D1SolidColorBrush,
    pub icon_fill: ID2D1SolidColorBrush,
    pub icon_stroke: ID2D1SolidColorBrush,
    pub running_pip: ID2D1SolidColorBrush,
    pub focused_pip_halo: ID2D1SolidColorBrush,
    pub focused_pip_core: ID2D1SolidColorBrush,
}

#[cfg(windows)]
impl CachedBrushes {
    /// Creates and initializes the brush cache using the specified device context.
    pub fn new(context: &ID2D1DeviceContext) -> Result<Self, RenderError> {
        let create_brush = |r: f32, g: f32, b: f32, a: f32| -> Result<ID2D1SolidColorBrush, RenderError> {
            let color = D2D1_COLOR_F { r, g, b, a };
            unsafe { context.CreateSolidColorBrush(&color, None).map_err(RenderError::from) }
        };

        Ok(Self {
            tray_fill: create_brush(1.0, 1.0, 1.0, 0.12)?,
            tray_stroke: create_brush(1.0, 1.0, 1.0, 0.25)?,
            icon_fill: create_brush(1.0, 1.0, 1.0, 0.15)?,
            icon_stroke: create_brush(1.0, 1.0, 1.0, 0.30)?,
            running_pip: create_brush(1.0, 1.0, 1.0, 0.80)?,
            focused_pip_halo: create_brush(1.0, 1.0, 1.0, 0.40)?,
            focused_pip_core: create_brush(1.0, 1.0, 1.0, 1.00)?,
        })
    }
}

/// Mock render context for cross-platform simulation, unit testing, and headless verification.
#[derive(Debug, Clone, Default)]
pub struct MockRenderContext {
    pub tray_drawn: bool,
    pub running_pips_count: usize,
    pub icons_drawn_count: usize,
    pub focused_pips_count: usize,
    pub width: u32,
    pub height: u32,
    pub last_tray_geom: Option<(f32, f32, f32, f32)>,
    pub last_contour_points: usize,
    pub drawn_pip_positions: Vec<(f32, f32, bool)>,
    pub drawn_icon_bounds: Vec<(f32, f32, f32)>,
    pub brushes_cached: bool,
    pub surface_locked: bool,
    pub end_draw_called: bool,
}

impl MockRenderContext {
    /// Creates a new mock render context with default dock dimensions.
    pub fn new() -> Self {
        Self::with_size(1200, 180)
    }

    /// Creates a new mock render context with explicit dimensions.
    pub fn with_size(width: u32, height: u32) -> Self {
        Self {
            tray_drawn: false,
            running_pips_count: 0,
            icons_drawn_count: 0,
            focused_pips_count: 0,
            width,
            height,
            last_tray_geom: None,
            last_contour_points: 0,
            drawn_pip_positions: Vec::new(),
            drawn_icon_bounds: Vec::new(),
            brushes_cached: true,
            surface_locked: false,
            end_draw_called: false,
        }
    }

    /// Resets all recorded rasterization state.
    pub fn clear(&mut self) {
        self.tray_drawn = false;
        self.running_pips_count = 0;
        self.icons_drawn_count = 0;
        self.focused_pips_count = 0;
        self.last_tray_geom = None;
        self.last_contour_points = 0;
        self.drawn_pip_positions.clear();
        self.drawn_icon_bounds.clear();
        self.surface_locked = false;
        self.end_draw_called = false;
    }

    /// Simulates dock rasterization, validating geometries and updating draw statistics.
    pub fn render_dock(
        &mut self,
        render_items: &[DockRenderItem],
        tray_geom: (f32, f32, f32, f32),
        squircle_pts: &[(f32, f32)],
    ) -> Result<(), RenderError> {
        let (tx, ty, tw, th) = tray_geom;
        if !tx.is_finite() || !ty.is_finite() || !tw.is_finite() || !th.is_finite() {
            return Err(RenderError::InvalidGeometry("Tray coordinates must be finite numbers"));
        }
        if tw <= 0.0 || th <= 0.0 {
            return Err(RenderError::InvalidGeometry("Tray width and height must be strictly positive"));
        }
        if squircle_pts.len() < 4 {
            return Err(RenderError::InvalidGeometry("Squircle contour requires at least 4 perimeter points"));
        }
        for &(px, py) in squircle_pts {
            if !px.is_finite() || !py.is_finite() {
                return Err(RenderError::InvalidGeometry("Contour coordinates must be finite numbers"));
            }
        }

        // Simulate surface BeginDraw locking
        self.surface_locked = true;
        self.end_draw_called = false;

        let draw_result = (|| -> Result<(), RenderError> {
            self.tray_drawn = true;
            self.last_tray_geom = Some(tray_geom);
            self.last_contour_points = squircle_pts.len();
            self.running_pips_count = 0;
            self.icons_drawn_count = 0;
            self.focused_pips_count = 0;
            self.drawn_pip_positions.clear();
            self.drawn_icon_bounds.clear();

            let tray_floor_y = ty + th - 4.0;

            for item in render_items {
                if !item.center_x.is_finite() || !item.center_y.is_finite() || item.current_size <= 0.0 {
                    return Err(RenderError::InvalidGeometry("Render item position and size must be valid"));
                }

                self.icons_drawn_count += 1;
                self.drawn_icon_bounds.push((item.center_x, item.center_y, item.current_size));

                if item.is_running {
                    self.running_pips_count += 1;
                    if item.is_focused {
                        self.focused_pips_count += 1;
                    }
                    let pip_x = item.center_x;
                    let pip_y = tray_floor_y;
                    self.drawn_pip_positions.push((pip_x, pip_y, item.is_focused));
                }
            }

            Ok(())
        })();

        // Guaranteed EndDraw unlocking even if drawing encounters an error
        self.surface_locked = false;
        self.end_draw_called = true;

        draw_result
    }
}

/// On non-Windows platforms, `Direct2DRenderer` is an alias to `MockRenderContext`.
#[cfg(not(windows))]
pub type Direct2DRenderer = MockRenderContext;

/// Direct2D 1.1 surface rasterization engine drawing onto DirectComposition surfaces on Windows.
#[cfg(windows)]
pub struct Direct2DRenderer {
    factory: ID2D1Factory1,
    d2d_device: ID2D1Device,
    d2d_context: ID2D1DeviceContext,
    surface: IDCompositionSurface,
    width: u32,
    height: u32,
    cached_brushes: Option<CachedBrushes>,
}

#[cfg(windows)]
impl Direct2DRenderer {
    /// Creates a new `Direct2DRenderer` using the provided Direct3D 11 device and DirectComposition device.
    pub fn new(
        d3d_device: &ID3D11Device,
        comp_device: &IDCompositionDevice,
    ) -> Result<Self, RenderError> {
        Self::with_size(d3d_device, comp_device, 1920, 200)
    }

    /// Creates a `Direct2DRenderer` with explicit pixel dimensions.
    pub fn with_size(
        d3d_device: &ID3D11Device,
        comp_device: &IDCompositionDevice,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let dxgi_device: IDXGIDevice = d3d_device.cast()?;
        Self::from_dxgi_and_comp(&dxgi_device, comp_device, width, height)
    }

    /// Creates a `Direct2DRenderer` from an existing DXGI device and DirectComposition device.
    pub fn from_dxgi_and_comp(
        dxgi_device: &IDXGIDevice,
        comp_device: &IDCompositionDevice,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let d2d_device: ID2D1Device = unsafe { factory.CreateDevice(dxgi_device)? };
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

        let cached_brushes = CachedBrushes::new(&d2d_context).ok();

        Ok(Self {
            factory,
            d2d_device,
            d2d_context,
            surface,
            width,
            height,
            cached_brushes,
        })
    }

    /// Creates a `Direct2DRenderer` and immediately binds its surface to the provided root visual.
    pub fn with_visual(
        d3d_device: &ID3D11Device,
        comp_device: &IDCompositionDevice,
        root_visual: &IDCompositionVisual,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let renderer = Self::with_size(d3d_device, comp_device, width, height)?;
        renderer.bind_to_visual(root_visual)?;
        Ok(renderer)
    }

    /// Binds the composition surface to the specified visual target.
    pub fn bind_to_visual(&self, visual: &IDCompositionVisual) -> Result<(), RenderError> {
        unsafe { visual.SetContent(&self.surface)? };
        Ok(())
    }

    /// Returns a reference to the underlying `IDCompositionSurface`.
    pub fn surface(&self) -> &IDCompositionSurface {
        &self.surface
    }

    /// Returns a reference to the `ID2D1DeviceContext`.
    pub fn d2d_context(&self) -> &ID2D1DeviceContext {
        &self.d2d_context
    }

    /// Returns a reference to the `ID2D1Device`.
    pub fn d2d_device(&self) -> &ID2D1Device {
        &self.d2d_device
    }

    /// Returns a reference to the `ID2D1Factory1`.
    pub fn d2d_factory(&self) -> &ID2D1Factory1 {
        &self.factory
    }

    /// Returns a reference to the cached brushes, if initialized.
    pub fn cached_brushes(&self) -> Option<&CachedBrushes> {
        self.cached_brushes.as_ref()
    }

    /// Width of the raster surface in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the raster surface in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Renders the dock tray squircle background, app icons, and running indicator pips
    /// onto the DirectComposition surface.
    pub fn render_dock(
        &mut self,
        render_items: &[DockRenderItem],
        tray_geom: (f32, f32, f32, f32),
        squircle_pts: &[(f32, f32)],
    ) -> Result<(), RenderError> {
        let (tx, ty, tw, th) = tray_geom;
        if !tx.is_finite() || !ty.is_finite() || !tw.is_finite() || !th.is_finite() {
            return Err(RenderError::InvalidGeometry("Tray coordinates must be finite numbers"));
        }
        if tw <= 0.0 || th <= 0.0 {
            return Err(RenderError::InvalidGeometry("Tray dimensions must be positive"));
        }
        if squircle_pts.len() < 4 {
            return Err(RenderError::InvalidGeometry("Squircle contour requires at least 4 points"));
        }

        let mut offset = windows::Win32::Foundation::POINT::default();
        let draw_dc: ID2D1DeviceContext = unsafe { self.surface.BeginDraw(None, &mut offset)? };

        // Lazily initialize or refresh brushes on draw_dc if not already present
        if self.cached_brushes.is_none() {
            if let Ok(brushes) = CachedBrushes::new(&draw_dc) {
                self.cached_brushes = Some(brushes);
            }
        }

        // Execute drawing commands with guaranteed EndDraw invocation even on error
        let draw_result = (|| -> Result<(), RenderError> {
            let brushes = self
                .cached_brushes
                .as_ref()
                .ok_or_else(|| RenderError::DrawingFailed("Failed to initialize brush cache".to_string()))?;

            // 1. Clear background with transparent black
            let clear_color = D2D1_COLOR_F {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            };
            unsafe { draw_dc.Clear(Some(&clear_color)) };

            let ox = offset.x as f32;
            let oy = offset.y as f32;

            // 2. Render frosted glass squircle tray background
            let path_geom = unsafe { self.factory.CreatePathGeometry()? };
            let sink = unsafe { path_geom.Open()? };

            if let Some(&(first_x, first_y)) = squircle_pts.first() {
                unsafe {
                    sink.BeginFigure(
                        D2D_POINT_2F {
                            x: first_x + ox,
                            y: first_y + oy,
                        },
                        D2D1_FIGURE_BEGIN_FILLED,
                    );
                    for &(px, py) in &squircle_pts[1..] {
                        sink.AddLine(D2D_POINT_2F {
                            x: px + ox,
                            y: py + oy,
                        });
                    }
                    sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                    sink.Close()?;
                }

                unsafe {
                    draw_dc.FillGeometry(&path_geom, &brushes.tray_fill, None);
                    draw_dc.DrawGeometry(&path_geom, &brushes.tray_stroke, 1.0, None);
                }
            }

            // 3. Render dock icon bounding squircles
            for item in render_items {
                let half = item.current_size / 2.0;
                let corner_radius = item.current_size * 0.225; // Standard continuous squircle curvature approximation
                let rounded_rect = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: item.center_x - half + ox,
                        top: item.center_y - half + oy,
                        right: item.center_x + half + ox,
                        bottom: item.center_y + half + oy,
                    },
                    radiusX: corner_radius,
                    radiusY: corner_radius,
                };

                unsafe {
                    draw_dc.FillRoundedRectangle(&rounded_rect, &brushes.icon_fill);
                    draw_dc.DrawRoundedRectangle(&rounded_rect, &brushes.icon_stroke, 1.0, None);
                }
            }

            // 4. Render running and focused indicator pips
            // Running pip: 4px diameter circle (radius 2px) centered below icon on tray floor
            let tray_floor_y = ty + th - 4.0;

            for item in render_items {
                if item.is_running {
                    let pip_x = item.center_x + ox;
                    let pip_y = tray_floor_y + oy;

                    let pip_ellipse = D2D1_ELLIPSE {
                        point: D2D_POINT_2F {
                            x: pip_x,
                            y: pip_y,
                        },
                        radiusX: 2.0,
                        radiusY: 2.0,
                    };

                    if item.is_focused {
                        // Distinct illuminated pip with subtle glowing halo
                        let halo_ellipse = D2D1_ELLIPSE {
                            point: D2D_POINT_2F {
                                x: pip_x,
                                y: pip_y,
                            },
                            radiusX: 3.5,
                            radiusY: 3.5,
                        };
                        unsafe {
                            draw_dc.FillEllipse(&halo_ellipse, &brushes.focused_pip_halo);
                            draw_dc.FillEllipse(&pip_ellipse, &brushes.focused_pip_core);
                        }
                    } else {
                        unsafe {
                            draw_dc.FillEllipse(&pip_ellipse, &brushes.running_pip);
                        }
                    }
                }
            }

            Ok(())
        })();

        // 5. Guaranteed surface.EndDraw() execution to avoid leaving DirectComposition surface locked
        let end_draw_result = unsafe { self.surface.EndDraw().map_err(RenderError::from) };

        draw_result.and(end_draw_result)
    }
}

/// Encapsulates the Direct3D 11 device and DirectComposition visual pipeline on Windows.
#[cfg(windows)]
pub struct DirectCompositionPipeline {
    d3d_device: ID3D11Device,
    d3d_context: ID3D11DeviceContext,
    dxgi_device: IDXGIDevice,
    dcomp_device: IDCompositionDevice,
    target: IDCompositionTarget,
    root_visual: IDCompositionVisual,
}

#[cfg(windows)]
impl DirectCompositionPipeline {
    /// Initializes Direct3D 11 device with BGRA support and binds a DirectComposition
    /// visual tree to the specified window handle.
    pub fn new(hwnd: HWND) -> windows::core::Result<Self> {
        let mut d3d_device: Option<ID3D11Device> = None;
        let mut d3d_context: Option<ID3D11DeviceContext> = None;

        // Try hardware acceleration first, fall back to WARP for virtualized/headless environments
        let hardware_res = unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d_device),
                None,
                Some(&mut d3d_context),
            )
        };

        if hardware_res.is_err() {
            unsafe {
                D3D11CreateDevice(
                    None,
                    D3D_DRIVER_TYPE_WARP,
                    None,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    None,
                    D3D11_SDK_VERSION,
                    Some(&mut d3d_device),
                    None,
                    Some(&mut d3d_context),
                )?;
            }
        }

        let d3d_device = d3d_device.ok_or_else(|| {
            windows::core::Error::new(
                windows::core::HRESULT(-1),
                "Failed to initialize Direct3D 11 device",
            )
        })?;

        let d3d_context = d3d_context.ok_or_else(|| {
            windows::core::Error::new(
                windows::core::HRESULT(-1),
                "Failed to obtain Direct3D 11 immediate context",
            )
        })?;

        // Query DXGI device for DirectComposition interoperability
        let dxgi_device: IDXGIDevice = d3d_device.cast()?;

        // Create DirectComposition device attached to DXGI device
        let dcomp_device: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi_device)? };

        // Create topmost DirectComposition target for the layered window
        let target: IDCompositionTarget = unsafe { dcomp_device.CreateTargetForHwnd(hwnd, true)? };

        // Create and set root visual
        let root_visual: IDCompositionVisual = unsafe { dcomp_device.CreateVisual()? };
        unsafe {
            target.SetRoot(&root_visual)?;
            dcomp_device.Commit()?;
        }

        Ok(Self {
            d3d_device,
            d3d_context,
            dxgi_device,
            dcomp_device,
            target,
            root_visual,
        })
    }

    /// Commits all pending composition commands to the compositor engine.
    pub fn commit(&self) -> windows::core::Result<()> {
        unsafe { self.dcomp_device.Commit() }
    }

    /// Returns a reference to the root `IDCompositionVisual`.
    pub fn root_visual(&self) -> &IDCompositionVisual {
        &self.root_visual
    }

    /// Returns a reference to the `IDCompositionDevice`.
    pub fn dcomp_device(&self) -> &IDCompositionDevice {
        &self.dcomp_device
    }

    /// Returns a reference to the `IDXGIDevice`.
    pub fn dxgi_device(&self) -> &IDXGIDevice {
        &self.dxgi_device
    }

    /// Returns a reference to the `ID3D11Device`.
    pub fn d3d_device(&self) -> &ID3D11Device {
        &self.d3d_device
    }

    /// Returns a reference to the `ID3D11DeviceContext`.
    pub fn d3d_context(&self) -> &ID3D11DeviceContext {
        &self.d3d_context
    }

    /// Returns a reference to the `IDCompositionTarget`.
    pub fn target(&self) -> &IDCompositionTarget {
        &self.target
    }

    /// Creates a new child visual within this composition device.
    pub fn create_visual(&self) -> windows::core::Result<IDCompositionVisual> {
        unsafe { self.dcomp_device.CreateVisual() }
    }

    /// Appends a child visual to a parent visual.
    pub fn add_visual(
        &self,
        parent: &IDCompositionVisual,
        child: &IDCompositionVisual,
    ) -> windows::core::Result<()> {
        unsafe { parent.AddVisual(child, false, None) }
    }

    /// Removes a child visual from its parent visual.
    pub fn remove_visual(
        &self,
        parent: &IDCompositionVisual,
        child: &IDCompositionVisual,
    ) -> windows::core::Result<()> {
        unsafe { parent.RemoveVisual(child) }
    }

    /// Clears all child visuals from a parent visual.
    pub fn remove_all_visuals(&self, parent: &IDCompositionVisual) -> windows::core::Result<()> {
        unsafe { parent.RemoveAllVisuals() }
    }
}

/// Cross-platform mock/stub pipeline for non-Windows development and testing.
#[cfg(not(windows))]
#[derive(Debug)]
pub struct DirectCompositionPipeline {
    is_mock: bool,
    committed_count: std::sync::atomic::AtomicUsize,
}

#[cfg(not(windows))]
impl Default for DirectCompositionPipeline {
    fn default() -> Self {
        Self::new_mock()
    }
}

#[cfg(not(windows))]
impl DirectCompositionPipeline {
    /// Creates a mock DirectComposition pipeline for cross-platform simulation and unit tests.
    pub fn new_mock() -> Self {
        Self {
            is_mock: true,
            committed_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Simulates a visual composition commit.
    pub fn commit(&self) -> Result<(), &'static str> {
        self.committed_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Returns `true` for mock implementation.
    pub fn is_mock(&self) -> bool {
        self.is_mock
    }

    /// Returns total number of commit operations performed.
    pub fn commit_count(&self) -> usize {
        self.committed_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
