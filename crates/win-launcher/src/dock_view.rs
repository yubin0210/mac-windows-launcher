//! dock_view
//! Dock state machine, magnification layout, squircle tray, and bounce physics engine.

use mac_physics_core::magnification::{
    calculate_dock_layout_at, calculate_icon_scales, MagnificationConfig,
};
use mac_physics_core::spring::DampedSpring;
use mac_physics_core::squircle::{generate_squircle_contour, APPLE_SQUIRCLE_EXPONENT};
use serde::{Deserialize, Serialize};

use crate::process_launcher::LaunchTarget;

/// Represents an item on the Dock with identity, label, process mapping, and bounce spring state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockItem {
    pub id: String,
    pub label: String,
    pub icon_path: Option<String>,
    pub process_name: Option<String>,
    pub launch_target: Option<LaunchTarget>,
    pub bounce_spring: DampedSpring,
    pub is_running: bool,
    pub is_focused: bool,
}

impl DockItem {
    /// Creates a new `DockItem` with calibrated macOS bounce physics.
    pub fn new(id: impl Into<String>, label: impl Into<String>, is_running: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon_path: None,
            process_name: None,
            launch_target: None,
            bounce_spring: DampedSpring::with_calibrated_macos(),
            is_running,
            is_focused: false,
        }
    }

    /// Sets an optional icon image or asset path.
    pub fn with_icon_path(mut self, path: impl Into<String>) -> Self {
        self.icon_path = Some(path.into());
        self
    }

    /// Sets an explicit executable or process name binding (e.g. "explorer.exe").
    pub fn with_process_name(mut self, process_name: impl Into<String>) -> Self {
        self.process_name = Some(process_name.into());
        self
    }

    /// Sets an explicit launch target for process spawning or window activation.
    pub fn with_launch_target(mut self, target: LaunchTarget) -> Self {
        self.launch_target = Some(target);
        self
    }

    /// Sets running and focus indicators.
    pub fn with_running(mut self, is_running: bool, is_focused: bool) -> Self {
        self.is_running = is_running;
        self.is_focused = is_focused;
        self
    }
}

/// Represents the rendered transform and state of a dock item at the current frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockRenderItem {
    pub id: String,
    pub index: usize,
    pub center_x: f32,
    pub center_y: f32,
    pub base_size: f32,
    pub current_size: f32,
    pub scale: f32,
    pub bounce_offset_y: f32,
    pub is_bouncing: bool,
    pub is_running: bool,
    pub is_focused: bool,
    pub label: String,
}

/// Default exponential decay rate for smooth scale recovery on mouse leave (~150-200ms settling).
pub const DEFAULT_SCALE_DECAY_RATE: f32 = 42.0;

/// Layout and geometry configuration for `DockView`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockViewConfig {
    pub magnification: MagnificationConfig,
    pub icon_padding: f32,
    pub tray_padding_x: f32,
    pub tray_padding_y: f32,
    pub window_width: f32,
    pub window_height: f32,
    pub dock_anchor_x: f32,
    pub dock_baseline_y: f32,
    pub squircle_exponent: f32,
    pub points_per_quadrant: usize,
    pub hover_headroom: f32,
    pub scale_decay_rate: f32,
}

impl Default for DockViewConfig {
    fn default() -> Self {
        let base_size = 48.0;
        let max_size = 128.0;
        let spread = 150.0;
        let window_width = 1200.0;
        let window_height = 180.0;
        let dock_baseline_y = 160.0;

        Self {
            magnification: MagnificationConfig::new(base_size, max_size, spread),
            icon_padding: 10.0,
            tray_padding_x: 14.0,
            tray_padding_y: 8.0,
            window_width,
            window_height,
            dock_anchor_x: window_width / 2.0,
            dock_baseline_y,
            squircle_exponent: APPLE_SQUIRCLE_EXPONENT,
            points_per_quadrant: 16,
            hover_headroom: 12.0,
            scale_decay_rate: DEFAULT_SCALE_DECAY_RATE,
        }
    }
}

/// State machine managing dock icons, cursor hover, magnification layout, and launch bounce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockView {
    pub items: Vec<DockItem>,
    pub config: DockViewConfig,
    cursor_pos: Option<(f32, f32)>,
    current_scales: Vec<f32>,
}

impl Default for DockView {
    fn default() -> Self {
        Self::new()
    }
}

impl DockView {
    /// Creates a new `DockView` pre-populated with standard default applications.
    pub fn new() -> Self {
        let items = vec![
            DockItem::new("finder", "Finder", true)
                .with_process_name("explorer.exe")
                .with_launch_target(LaunchTarget::AppExecutable {
                    process_name: "explorer.exe".to_string(),
                    command: "explorer.exe".to_string(),
                    args: Vec::new(),
                }),
            DockItem::new("launchpad", "Launchpad", false)
                .with_launch_target(LaunchTarget::ShellAction("launchpad".to_string())),
            DockItem::new("browser", "Safari", true)
                .with_process_name("msedge.exe")
                .with_launch_target(LaunchTarget::AppExecutable {
                    process_name: "msedge.exe".to_string(),
                    command: "msedge.exe".to_string(),
                    args: Vec::new(),
                }),
            DockItem::new("messages", "Messages", false)
                .with_process_name("phoneexperiencehost.exe")
                .with_launch_target(LaunchTarget::AppExecutable {
                    process_name: "phoneexperiencehost.exe".to_string(),
                    command: "phoneexperiencehost.exe".to_string(),
                    args: Vec::new(),
                }),
            DockItem::new("mail", "Mail", false)
                .with_process_name("hxoutlook.exe")
                .with_launch_target(LaunchTarget::AppExecutable {
                    process_name: "hxoutlook.exe".to_string(),
                    command: "hxoutlook.exe".to_string(),
                    args: Vec::new(),
                }),
            DockItem::new("maps", "Maps", false)
                .with_launch_target(LaunchTarget::ProtocolUri("bingmaps:".to_string())),
            DockItem::new("photos", "Photos", false)
                .with_launch_target(LaunchTarget::ProtocolUri("ms-photos:".to_string())),
            DockItem::new("music", "Music", false)
                .with_process_name("spotify.exe")
                .with_launch_target(LaunchTarget::AppExecutable {
                    process_name: "spotify.exe".to_string(),
                    command: "spotify.exe".to_string(),
                    args: Vec::new(),
                }),
            DockItem::new("terminal", "Terminal", true)
                .with_process_name("wt.exe")
                .with_launch_target(LaunchTarget::AppExecutable {
                    process_name: "wt.exe".to_string(),
                    command: "wt.exe".to_string(),
                    args: Vec::new(),
                }),
            DockItem::new("settings", "System Settings", false)
                .with_process_name("systemsettings.exe")
                .with_launch_target(LaunchTarget::AppExecutable {
                    process_name: "systemsettings.exe".to_string(),
                    command: "systemsettings.exe".to_string(),
                    args: Vec::new(),
                }),
        ];

        Self::with_items(items)
    }

    /// Returns the configured launch target for the item at `index`, if available.
    pub fn get_launch_target(&self, index: usize) -> Option<&LaunchTarget> {
        self.items.get(index).and_then(|item| item.launch_target.as_ref())
    }

    /// Creates a `DockView` with custom items and default configuration.
    pub fn with_items(items: Vec<DockItem>) -> Self {
        Self::with_config(items, DockViewConfig::default())
    }

    /// Creates a `DockView` with custom items and explicit layout configuration.
    pub fn with_config(items: Vec<DockItem>, config: DockViewConfig) -> Self {
        let n = items.len();
        Self {
            items,
            config,
            cursor_pos: None,
            current_scales: vec![1.0; n],
        }
    }

    /// Handles cursor movement at coordinates `(x, y)` relative to window space.
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            self.cursor_pos = Some((x, y));
            self.ensure_scales_capacity();
            if self.is_in_hover_zone(x, y) {
                self.current_scales = self.target_scales_at(x);
            }
        }
    }

    /// Handles mouse cursor leaving the dock window area.
    pub fn on_mouse_leave(&mut self) {
        self.cursor_pos = None;
    }

    /// Returns `true` if mouse hover is active within the dock magnification sensitivity zone.
    pub fn is_hover_active(&self) -> bool {
        if let Some((x, y)) = self.cursor_pos {
            self.is_in_hover_zone(x, y)
        } else {
            false
        }
    }

    /// Returns the resting (unmagnified) geometry `(x, y, width, height)` of the dock tray.
    pub fn resting_tray_geometry(&self) -> (f32, f32, f32, f32) {
        let n = self.items.len() as f32;
        if n == 0.0 {
            let width = (self.config.magnification.base_size * 4.5).min(self.config.window_width - 16.0).max(self.config.tray_padding_x * 2.0);
            let height = self.config.magnification.base_size + self.config.tray_padding_y * 2.0;
            let x = self.config.dock_anchor_x - width / 2.0;
            let y = self.config.dock_baseline_y - self.config.magnification.base_size
                - self.config.tray_padding_y;
            return (x, y, width, height);
        }

        let resting_span = n * self.config.magnification.base_size
            + (n - 1.0).max(0.0) * self.config.icon_padding;
        let width = resting_span + self.config.tray_padding_x * 2.0;
        let height = self.config.magnification.base_size + self.config.tray_padding_y * 2.0;
        let x = self.config.dock_anchor_x - width / 2.0;
        let y = self.config.dock_baseline_y - self.config.magnification.base_size
            - self.config.tray_padding_y;

        (x, y, width, height)
    }

    /// Checks if coordinates `(x, y)` fall within the vertical and horizontal hover range.
    pub fn is_in_hover_zone(&self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }

        // Active vertical band: above baseline by max magnified height + headroom, down to window bottom
        let min_y = self.config.dock_baseline_y
            - self.config.magnification.max_size
            - self.config.hover_headroom;
        let max_y = self.config.window_height.max(self.config.dock_baseline_y + 20.0);

        if y < min_y || y > max_y {
            return false;
        }

        // Horizontal band: covers resting dock width plus 3x Gaussian spread
        let (tray_x, _, tray_w, _) = self.resting_tray_geometry();
        let buffer = 3.0 * self.config.magnification.spread;
        let min_x = tray_x - buffer;
        let max_x = tray_x + tray_w + buffer;

        x >= min_x && x <= max_x
    }

    /// Imparts an upward velocity bounce impulse to the icon at `index`.
    pub fn trigger_launch(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            // Negative Y velocity impulse imparts upward launch bounce
            item.bounce_spring.trigger_bounce(-360.0);
        }
    }

    /// Handles mouse click at `(x, y)`. If an icon was clicked, triggers launch and returns its index.
    pub fn on_mouse_click(&mut self, x: f32, y: f32) -> Option<usize> {
        let render_items = self.get_render_items();
        for item in render_items {
            let half = item.current_size / 2.0;
            let left = item.center_x - half;
            let right = item.center_x + half;
            let top = item.center_y - half;
            let bottom = item.center_y + half;

            if x >= left && x <= right && y >= top && y <= bottom {
                self.trigger_launch(item.index);
                return Some(item.index);
            }
        }
        None
    }

    /// Advances physics simulation (spring bounce oscillators and smooth scale decay) by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }

        self.ensure_scales_capacity();

        let target_scales = self.calculate_target_scales();
        let decay_rate = if self.config.scale_decay_rate.is_finite() && self.config.scale_decay_rate > 0.0 {
            self.config.scale_decay_rate
        } else {
            DEFAULT_SCALE_DECAY_RATE
        };

        let decay_factor = 1.0 - (-decay_rate * dt).exp();
        let mut all_settled = true;

        for (scale, &target) in self.current_scales.iter_mut().zip(target_scales.iter()) {
            *scale += (target - *scale) * decay_factor;
            if (*scale - target).abs() >= 1e-4 {
                all_settled = false;
            }
        }

        if all_settled {
            for (scale, &target) in self.current_scales.iter_mut().zip(target_scales.iter()) {
                *scale = target;
            }
        }

        for item in &mut self.items {
            item.bounce_spring.step(dt, 0.0);
        }
    }

    /// Returns resting horizontal centers for all items based on unmagnified layout.
    pub fn resting_icon_centers(&self) -> Vec<f32> {
        let n = self.items.len();
        if n == 0 {
            return Vec::new();
        }
        let unmagnified = vec![1.0; n];
        let resting_layout = calculate_dock_layout_at(
            &unmagnified,
            self.config.magnification.base_size,
            self.config.icon_padding,
            self.config.dock_anchor_x,
        );
        resting_layout.into_iter().map(|(c, _)| c).collect()
    }

    /// Calculates target icon scales given cursor x coordinate.
    pub fn target_scales_at(&self, cursor_x: f32) -> Vec<f32> {
        let centers = self.resting_icon_centers();
        calculate_icon_scales(cursor_x, &centers, &self.config.magnification)
    }

    /// Computes instantaneous target scales based on current hover status.
    pub fn calculate_target_scales(&self) -> Vec<f32> {
        let n = self.items.len();
        if n == 0 {
            return Vec::new();
        }
        if let Some((cx, cy)) = self.cursor_pos {
            if self.is_in_hover_zone(cx, cy) {
                return self.target_scales_at(cx);
            }
        }
        vec![1.0; n]
    }

    /// Read-only access to current animated scale factors.
    pub fn current_scales(&self) -> &[f32] {
        &self.current_scales
    }

    fn ensure_scales_capacity(&mut self) {
        if self.current_scales.len() != self.items.len() {
            self.current_scales.resize(self.items.len(), 1.0);
        }
    }

    /// Returns current animated scale factors for all dock items.
    pub fn calculate_current_scales(&self) -> Vec<f32> {
        let n = self.items.len();
        if n == 0 {
            return Vec::new();
        }
        if self.current_scales.len() == n {
            self.current_scales.clone()
        } else {
            let mut s = self.current_scales.clone();
            s.resize(n, 1.0);
            s
        }
    }

    /// Returns rendered items with updated coordinates, dimensions, and bounce displacements.
    pub fn get_render_items(&self) -> Vec<DockRenderItem> {
        let n = self.items.len();
        if n == 0 {
            return Vec::new();
        }

        let scales = self.calculate_current_scales();
        let layout = calculate_dock_layout_at(
            &scales,
            self.config.magnification.base_size,
            self.config.icon_padding,
            self.config.dock_anchor_x,
        );

        let mut render_items = Vec::with_capacity(n);
        for i in 0..n {
            let (center_x, rendered_size) = layout[i];
            let scale = scales[i];
            let raw_pos = self.items[i].bounce_spring.position();
            // Icon bounce offset floats upward (negative offset in Y-down window space)
            let bounce_offset_y = raw_pos.min(0.0);
            let is_bouncing = !self.items[i].bounce_spring.is_resting();

            // Icons are anchored at the baseline: bottom edge sits at dock_baseline_y
            let center_y = self.config.dock_baseline_y - rendered_size / 2.0 + bounce_offset_y;

            render_items.push(DockRenderItem {
                id: self.items[i].id.clone(),
                index: i,
                center_x,
                center_y,
                base_size: self.config.magnification.base_size,
                current_size: rendered_size,
                scale,
                bounce_offset_y,
                is_bouncing,
                is_running: self.items[i].is_running,
                is_focused: self.items[i].is_focused,
                label: self.items[i].label.clone(),
            });
        }

        render_items
    }

    /// Computes the bounding rectangle `(x, y, width, height)` of the dock tray background.
    pub fn get_tray_geometry(&self) -> (f32, f32, f32, f32) {
        let n = self.items.len();
        if n == 0 {
            let width = (self.config.magnification.base_size * 4.5).min(self.config.window_width - 16.0).max(self.config.tray_padding_x * 2.0);
            let height = self.config.magnification.base_size + self.config.tray_padding_y * 2.0;
            let x = self.config.dock_anchor_x - width / 2.0;
            let y = self.config.dock_baseline_y - self.config.magnification.base_size
                - self.config.tray_padding_y;
            return (x, y, width, height);
        }

        let scales = self.calculate_current_scales();
        let layout = calculate_dock_layout_at(
            &scales,
            self.config.magnification.base_size,
            self.config.icon_padding,
            self.config.dock_anchor_x,
        );

        let (leftmost_center, leftmost_size) = layout.first().copied().unwrap_or((0.0, 0.0));
        let (rightmost_center, rightmost_size) = layout.last().copied().unwrap_or((0.0, 0.0));

        let icon_span_left = leftmost_center - leftmost_size / 2.0;
        let icon_span_right = rightmost_center + rightmost_size / 2.0;
        let active_span = (icon_span_right - icon_span_left).max(0.0);

        let width = active_span + self.config.tray_padding_x * 2.0;
        let height = self.config.magnification.base_size + self.config.tray_padding_y * 2.0;

        let center_x = (icon_span_left + icon_span_right) / 2.0;
        let x = center_x - width / 2.0;
        let y = self.config.dock_baseline_y - self.config.magnification.base_size
            - self.config.tray_padding_y;

        (x, y, width, height)
    }

    /// Returns the closed squircle polygonal contour for the tray background,
    /// centered and translated to world window coordinates using configured quadrant resolution.
    pub fn get_tray_squircle_contour(&self) -> Vec<(f32, f32)> {
        self.get_tray_contour(self.config.points_per_quadrant)
    }

    /// Generates the closed squircle polygonal contour for the tray background,
    /// centered and translated to world window coordinates.
    pub fn get_tray_contour(&self, points_per_quadrant: usize) -> Vec<(f32, f32)> {
        let (x, y, w, h) = self.get_tray_geometry();
        let raw_contour = generate_squircle_contour(
            w,
            h,
            self.config.squircle_exponent,
            points_per_quadrant,
        );

        let center_x = x + w / 2.0;
        let center_y = y + h / 2.0;

        raw_contour
            .into_iter()
            .map(|(px, py)| (center_x + px, center_y + py))
            .collect()
    }

    /// Manually sets the running and focus state of the item at `index`.
    pub fn set_app_running(&mut self, index: usize, is_running: bool, is_focused: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.is_running = is_running;
            item.is_focused = is_focused;
        }
    }

    /// Updates running and focus status for all dock items from an `AppTracker`.
    pub fn update_running_status(&mut self, tracker: &crate::app_tracker::AppTracker) {
        for item in &mut self.items {
            let proc_target = if let Some(ref p) = item.process_name {
                Some(p.as_str())
            } else {
                Self::resolve_process_name(&item.id, &item.label)
            };

            if let Some(target) = proc_target {
                item.is_running = tracker.is_app_running(target);
                item.is_focused = tracker.is_app_focused(target);
            } else {
                item.is_running = tracker.is_app_running(&item.label) || tracker.is_app_running(&item.id);
                item.is_focused = tracker.is_app_focused(&item.label) || tracker.is_app_focused(&item.id);
            }
        }
    }

    /// Resolves canonical Windows process name for standard macOS dock applications.
    pub fn resolve_process_name(id: &str, label: &str) -> Option<&'static str> {
        let map_name = |name: &str| match name.trim().to_ascii_lowercase().as_str() {
            "finder" => Some("explorer.exe"),
            "safari" | "browser" => Some("msedge.exe"),
            "terminal" => Some("wt.exe"),
            "settings" | "system settings" => Some("systemsettings.exe"),
            "mail" => Some("hxoutlook.exe"),
            "messages" => Some("phoneexperiencehost.exe"),
            "music" => Some("spotify.exe"),
            _ => None,
        };

        map_name(id).or_else(|| map_name(label))
    }

    /// Read-only access to current dock items.
    pub fn items(&self) -> &[DockItem] {
        &self.items
    }

    /// Mutable access to dock items.
    pub fn items_mut(&mut self) -> &mut [DockItem] {
        &mut self.items
    }

    /// Returns current cursor position relative to window space.
    pub fn cursor_pos(&self) -> Option<(f32, f32)> {
        self.cursor_pos
    }
}
