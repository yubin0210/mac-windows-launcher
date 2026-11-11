//! Shared physical geometry for the native input control, result painting and hit testing.
use crate::display::WorkArea;
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotlightGeometry {
    pub x: i32, pub y: i32, pub width: i32, pub height: i32,
    pub scale: f32, pub header: f32, pub row_height: f32,
    pub capacity: usize, pub visible_rows: usize,
}
impl SpotlightGeometry {
    pub fn fit(area: WorkArea, dpi: u32, count: usize, has_query: bool) -> Result<Self, String> {
        let width = area.right - area.left;
        let height = area.bottom - area.top;
        if width < 240 || height < 160 || !(48..=768).contains(&dpi) { return Err("No usable Spotlight work area".into()); }
        let scale = (dpi as f32 / 96.0).min((width-24) as f32 / 640.0).min((height-24) as f32 / 400.0);
        let header = 56.0 * scale;
        let row_height = 48.0 * scale;
        let footer = 24.0 * scale;
        let capacity = (((height-24) as f32 - header - footer) / row_height).floor().clamp(1.0, 5.0) as usize;
        let visible_rows = count.min(capacity);
        let body_rows = if visible_rows == 0 && has_query { 1 } else { visible_rows };
        let panel_width = (640.0 * scale).round() as i32;
        let panel_height = (header + body_rows as f32 * row_height + footer).ceil() as i32;
        let x = area.left + (width-panel_width)/2;
        let y = (area.top + (height as f32 * 0.22) as i32).min(area.bottom-panel_height-12).max(area.top+12);
        Ok(Self { x, y, width: panel_width, height: panel_height, scale, header, row_height, capacity, visible_rows })
    }
    pub fn row_at(&self, y: f32) -> Option<usize> {
        if !y.is_finite() || y < self.header { return None; }
        let row = ((y-self.header) / self.row_height).floor() as usize;
        (row < self.visible_rows).then_some(row)
    }
}
