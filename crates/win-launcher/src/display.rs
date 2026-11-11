//! Physical desktop coordinates and DPI-scaled Dock geometry.
use crate::DockViewConfig;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkArea { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockPlacement { pub x: i32, pub y: i32, pub width: i32, pub height: i32, pub capacity: usize, scale: f32 }
impl DockPlacement {
    pub fn fit(area: WorkArea, dpi: u32) -> Result<Self, String> { Self::fit_with_size(area, dpi, 48) }
    pub fn fit_with_size(area: WorkArea, dpi: u32, icon_size: u32) -> Result<Self, String> {
        if !(24..=96).contains(&icon_size) { return Err("Invalid Dock icon size".into()); }
        let width = area.right - area.left;
        let height = area.bottom - area.top;
        if width < 160 || height < 100 || !(48..=768).contains(&dpi) { return Err("Invalid display work area or DPI".into()); }
        let scale = (dpi as f32 / 96.0 * icon_size as f32 / 48.0).min(width as f32 / 320.0).min(height as f32 / 180.0);
        let margin = (8.0 * scale).round() as i32;
        let width = width - margin * 2;
        let dock_height = (180.0 * scale).round() as i32;
        // Leave room for magnification expansion in addition to the resting icons.
        // Bound the sum of Gaussian growth by its integral plus one peak sample.
        // Reserving only one enlarged icon clips wide rows at maximum hover.
        let growth = 80.0 * scale * (1.0 + (2.0 * std::f32::consts::PI).sqrt() * 150.0 / 58.0);
        let capacity = ((width as f32 - growth - 28.0 * scale) / (58.0 * scale)).floor().max(1.0) as usize;
        Ok(Self { x: area.left + margin, y: area.bottom - dock_height, width, height: dock_height, capacity, scale })
    }
    pub fn view_config(&self) -> DockViewConfig {
        let mut config = DockViewConfig::default();
        config.window_width = self.width as f32;
        config.window_height = self.height as f32;
        config.dock_anchor_x = self.width as f32 / 2.0;
        config.dock_baseline_y = self.height as f32 - 20.0 * self.scale;
        config.magnification.base_size *= self.scale;
        config.magnification.max_size *= self.scale;
        config.magnification.spread *= self.scale;
        config.icon_padding *= self.scale;
        config.tray_padding_x *= self.scale;
        config.tray_padding_y *= self.scale;
        config.hover_headroom *= self.scale;
        config
    }
}

#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct Monitor { pub id: String, pub area: WorkArea, pub handle: windows::Win32::Graphics::Gdi::HMONITOR }
#[cfg(windows)]
pub fn monitors() -> Vec<Monitor> {
    use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::*;
    unsafe extern "system" fn collect(handle: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let list = &mut *(data.0 as *mut Vec<Monitor>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(handle, &mut info.monitorInfo).as_bool() {
            let rect = info.monitorInfo.rcWork;
            let count = info.szDevice.iter().position(|c| *c == 0).unwrap_or(info.szDevice.len());
            list.push(Monitor { id: String::from_utf16_lossy(&info.szDevice[..count]), area: WorkArea { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom }, handle });
        }
        BOOL(1)
    }
    let mut list = Vec::<Monitor>::new();
    unsafe { let _ = EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut list as *mut _ as isize)); }
    list
}
#[cfg(windows)]
pub fn configured(hwnd: windows::Win32::Foundation::HWND, settings: &crate::settings::Settings) -> Result<DockPlacement, String> {
    use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTOPRIMARY};
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
    let current = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY) };
    let monitors = monitors();
    let monitor = settings.monitor.as_ref().and_then(|id| monitors.iter().find(|monitor| &monitor.id == id))
        .or_else(|| monitors.iter().find(|monitor| monitor.handle == current)).ok_or("No available display")?;
    let dpi = if monitor.handle == current { unsafe { GetDpiForWindow(hwnd) } } else {
        let mut x = 0; let mut y = 0;
        unsafe { GetDpiForMonitor(monitor.handle, MDT_EFFECTIVE_DPI, &mut x, &mut y) }.map_err(|e| e.to_string())?;
        x
    };
    DockPlacement::fit_with_size(monitor.area, dpi, settings.icon_size)
}
