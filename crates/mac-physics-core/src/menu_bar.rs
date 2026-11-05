//! menu_bar
//! macOS Top Menu Bar layout geometry, metrics, and system status models.

use serde::{Deserialize, Serialize};

/// Standard macOS Menu Bar dimensions and typography metrics.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MenuBarMetrics {
    pub height: f32,
    pub padding_h: f32,
    pub item_spacing: f32,
    pub font_size: f32,
    pub apple_icon_width: f32,
}

impl Default for MenuBarMetrics {
    fn default() -> Self {
        Self {
            height: 28.0,
            padding_h: 12.0,
            item_spacing: 16.0,
            font_size: 13.0,
            apple_icon_width: 16.0,
        }
    }
}

/// Alignment group for items in the Menu Bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MenuBarGroup {
    Left,
    Right,
}

/// A 2D rectangular bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

/// An individual item or menu trigger in the Menu Bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuBarItem {
    pub id: String,
    pub label: String,
    pub group: MenuBarGroup,
    pub is_bold: bool,
    pub has_dropdown: bool,
}

impl MenuBarItem {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        group: MenuBarGroup,
        is_bold: bool,
        has_dropdown: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            group,
            is_bold,
            has_dropdown,
        }
    }

    /// Creates standard Apple logo menu item.
    pub fn apple_logo() -> Self {
        Self::new("apple", "", MenuBarGroup::Left, true, true)
    }
}

/// System status snapshot captured from hardware and OS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemStatusSnapshot {
    pub battery_percent: Option<u8>,
    pub is_charging: bool,
    pub wifi_connected: bool,
    pub volume_percent: u8,
    pub time_text: String,
}

impl SystemStatusSnapshot {
    pub fn new(
        battery_percent: Option<u8>,
        is_charging: bool,
        wifi_connected: bool,
        volume_percent: u8,
        time_text: impl Into<String>,
    ) -> Self {
        Self {
            battery_percent,
            is_charging,
            wifi_connected,
            volume_percent,
            time_text: time_text.into(),
        }
    }
}

/// A positioned menu bar item with an explicit bounding box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionedMenuBarItem {
    pub id: String,
    pub label: String,
    pub bounds: Rect,
    pub group: MenuBarGroup,
    pub is_bold: bool,
    pub has_dropdown: bool,
}

/// Complete horizontal layout of the Menu Bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuBarLayout {
    pub width: f32,
    pub height: f32,
    pub left_items: Vec<PositionedMenuBarItem>,
    pub right_items: Vec<PositionedMenuBarItem>,
}

impl MenuBarLayout {
    /// Hit-tests a coordinate against all positioned menu bar items.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&PositionedMenuBarItem> {
        if y < 0.0 || y > self.height {
            return None;
        }
        for item in &self.left_items {
            if item.bounds.contains(x, y) {
                return Some(item);
            }
        }
        for item in &self.right_items {
            if item.bounds.contains(x, y) {
                return Some(item);
            }
        }
        None
    }
}

/// Estimates width of a menu bar item based on text and icon type.
fn estimate_item_width(item: &MenuBarItem) -> f32 {
    if item.id == "apple" {
        return 24.0;
    }
    let char_count = item.label.chars().count();
    let text_w = char_count as f32 * 7.5;
    (text_w + 14.0).max(28.0)
}

/// Computes the complete layout of left-aligned app menus and right-aligned system status indicators.
pub fn compute_menu_bar_layout(
    items: &[MenuBarItem],
    status: &SystemStatusSnapshot,
    screen_width: f32,
) -> MenuBarLayout {
    let metrics = MenuBarMetrics::default();
    let mut left_items = Vec::new();

    // 1. Position Left-Aligned Menu Items
    let mut current_x = metrics.padding_h;
    for item in items.iter().filter(|i| i.group == MenuBarGroup::Left) {
        let width = estimate_item_width(item);
        let bounds = Rect::new(current_x, 0.0, width, metrics.height);
        left_items.push(PositionedMenuBarItem {
            id: item.id.clone(),
            label: item.label.clone(),
            bounds,
            group: MenuBarGroup::Left,
            is_bold: item.is_bold,
            has_dropdown: item.has_dropdown,
        });
        current_x += width + metrics.item_spacing;
    }

    // 2. Build Right-Aligned Status Items
    let mut right_specs = Vec::new();

    // Clock
    right_specs.push((
        "clock".to_string(),
        status.time_text.clone(),
        (status.time_text.chars().count() as f32 * 7.5 + 12.0).max(64.0),
        false,
    ));

    // Control Center icon
    right_specs.push(("control_center".to_string(), "⑆".to_string(), 24.0, true));

    // Spotlight search trigger icon
    right_specs.push(("spotlight".to_string(), "🔍".to_string(), 24.0, false));

    // Battery status
    if let Some(pct) = status.battery_percent {
        let label = if status.is_charging {
            format!("⚡{pct}%")
        } else {
            format!("{pct}%")
        };
        right_specs.push(("battery".to_string(), label, 54.0, true));
    }

    // Wi-Fi status
    let wifi_label = if status.wifi_connected { "􀙇" } else { "􀙈" };
    right_specs.push(("wifi".to_string(), wifi_label.to_string(), 24.0, true));

    // Audio status
    let vol_label = if status.volume_percent == 0 { "􀊢" } else { "􀊩" };
    right_specs.push(("volume".to_string(), vol_label.to_string(), 24.0, true));

    // Position Right-Aligned Items flowing right-to-left
    let mut right_items = Vec::new();
    let mut current_right = screen_width - metrics.padding_h;

    for (id, label, width, has_dd) in right_specs {
        current_right -= width;
        let bounds = Rect::new(current_right, 0.0, width, metrics.height);
        right_items.push(PositionedMenuBarItem {
            id,
            label,
            bounds,
            group: MenuBarGroup::Right,
            is_bold: false,
            has_dropdown: has_dd,
        });
        current_right -= metrics.item_spacing;
    }

    MenuBarLayout {
        width: screen_width,
        height: metrics.height,
        left_items,
        right_items,
    }
}
