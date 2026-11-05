//! stage_manager
//! macOS Stage Manager layout geometry, models, and shelf calculations.

use serde::{Deserialize, Serialize};

use crate::menu_bar::Rect;

/// Standard padding inside a stage card around the thumbnail preview.
pub const CARD_PADDING: f32 = 8.0;

/// Vertical space reserved for the application title and icon badge at the top of a stage card.
pub const BADGE_RESERVED_HEIGHT: f32 = 20.0;

/// Snapshot information for a single window tracked by Stage Manager.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageWindowInfo {
    pub id: u64,
    pub title: String,
    pub app_name: String,
    pub width: u32,
    pub height: u32,
    pub is_minimized: bool,
}

impl StageWindowInfo {
    pub fn new(
        id: u64,
        title: impl Into<String>,
        app_name: impl Into<String>,
        width: u32,
        height: u32,
        is_minimized: bool,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            app_name: app_name.into(),
            width,
            height,
            is_minimized,
        }
    }
}

/// A logical group of windows belonging to an application or user space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageGroup {
    pub group_id: String,
    pub app_name: String,
    pub windows: Vec<StageWindowInfo>,
    pub active_window_index: usize,
}

impl StageGroup {
    pub fn new(
        group_id: impl Into<String>,
        app_name: impl Into<String>,
        windows: Vec<StageWindowInfo>,
        active_window_index: usize,
    ) -> Self {
        Self {
            group_id: group_id.into(),
            app_name: app_name.into(),
            windows,
            active_window_index,
        }
    }

    /// Returns a reference to the currently active window in this group, if valid.
    pub fn active_window(&self) -> Option<&StageWindowInfo> {
        self.windows.get(self.active_window_index)
    }

    /// Returns the total number of windows contained in this group.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }
}

/// Layout and geometry metrics for the Stage Manager left flyout shelf.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StageShelfMetrics {
    pub shelf_x: f32,
    pub shelf_width: f32,
    pub card_width: f32,
    pub card_height: f32,
    pub card_spacing: f32,
    pub max_groups: usize,
    pub corner_radius: f32,
}

impl Default for StageShelfMetrics {
    fn default() -> Self {
        Self {
            shelf_x: 16.0,
            shelf_width: 180.0,
            card_width: 150.0,
            card_height: 100.0,
            card_spacing: 16.0,
            max_groups: 5,
            corner_radius: 12.0,
        }
    }
}

/// Computed layout geometry and state for an individual staged group card on the shelf.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageCardLayout {
    pub group_id: String,
    pub app_name: String,
    pub bounds: Rect,
    pub thumbnail_bounds: Rect,
    pub stack_depth: usize,
    pub opacity: f32,
    pub is_hovered: bool,
}

/// Computes a centered `Rect` within `(0.0, 0.0, target_w, target_h)` that fits
/// `(source_w, source_h)` while preserving the source aspect ratio.
///
/// Falls back safely to `(0.0, 0.0, target_w, target_h)` if width or height is zero.
pub fn compute_thumbnail_aspect_fit(
    source_w: u32,
    source_h: u32,
    target_w: f32,
    target_h: f32,
) -> Rect {
    if source_w == 0
        || source_h == 0
        || !target_w.is_finite()
        || !target_h.is_finite()
        || target_w <= 0.0
        || target_h <= 0.0
    {
        let safe_w = if target_w.is_finite() { target_w.max(0.0) } else { 0.0 };
        let safe_h = if target_h.is_finite() { target_h.max(0.0) } else { 0.0 };
        return Rect::new(0.0, 0.0, safe_w, safe_h);
    }

    let src_w = source_w as f32;
    let src_h = source_h as f32;
    let src_aspect = src_w / src_h;
    let target_aspect = target_w / target_h;

    let (fit_w, fit_h) = if src_aspect > target_aspect {
        // Wider than target: match width, letterbox vertically
        (target_w, target_w / src_aspect)
    } else {
        // Taller than or equal to target: match height, pillarbox horizontally
        (target_h * src_aspect, target_h)
    };

    let x = (target_w - fit_w) / 2.0;
    let y = (target_h - fit_h) / 2.0;

    Rect::new(x, y, fit_w, fit_h)
}

/// Computes layout positions, bounding rectangles, and thumbnail targets for the Stage Manager shelf.
///
/// - Excludes `active_group_id` (since the active group is displayed on Center Stage).
/// - Takes up to `metrics.max_groups` staged groups.
/// - Centers the shelf cards vertically on the screen (clamped to below the Menu Bar).
/// - Interpolates card X position and opacity based on `shelf_slide_t` (0.0 = hidden off-screen, 1.0 = visible).
pub fn compute_stage_shelf_layout(
    groups: &[StageGroup],
    active_group_id: Option<&str>,
    metrics: &StageShelfMetrics,
    screen_height: f32,
    shelf_slide_t: f32,
) -> Vec<StageCardLayout> {
    let staged_groups: Vec<&StageGroup> = groups
        .iter()
        .filter(|g| match active_group_id {
            Some(active_id) => g.group_id != active_id,
            None => true,
        })
        .take(metrics.max_groups)
        .collect();

    if staged_groups.is_empty() {
        return Vec::new();
    }

    let n = staged_groups.len();
    let total_height = n as f32 * metrics.card_height + (n - 1) as f32 * metrics.card_spacing;
    let start_y = ((screen_height - total_height) / 2.0).max(28.0 + 10.0);

    let t = if shelf_slide_t.is_finite() {
        shelf_slide_t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let hidden_x = -metrics.card_width - 10.0;
    let visible_x = metrics.shelf_x;
    let card_x = hidden_x + (visible_x - hidden_x) * t;
    let opacity = t;

    staged_groups
        .into_iter()
        .enumerate()
        .map(|(i, group)| {
            let card_y = start_y + i as f32 * (metrics.card_height + metrics.card_spacing);
            let bounds = Rect::new(card_x, card_y, metrics.card_width, metrics.card_height);

            let thumb_avail_x = bounds.x + CARD_PADDING;
            let thumb_avail_y = bounds.y + CARD_PADDING + BADGE_RESERVED_HEIGHT;
            let thumb_avail_w = (bounds.width - 2.0 * CARD_PADDING).max(0.0);
            let thumb_avail_h = (bounds.height - 2.0 * CARD_PADDING - BADGE_RESERVED_HEIGHT).max(0.0);

            let (src_w, src_h) = match group.active_window() {
                Some(win) => (win.width, win.height),
                None => (0, 0),
            };

            let fit = compute_thumbnail_aspect_fit(src_w, src_h, thumb_avail_w, thumb_avail_h);
            let thumbnail_bounds = Rect::new(
                thumb_avail_x + fit.x,
                thumb_avail_y + fit.y,
                fit.width,
                fit.height,
            );

            let stack_depth = group.windows.len().clamp(1, 4);

            StageCardLayout {
                group_id: group.group_id.clone(),
                app_name: group.app_name.clone(),
                bounds,
                thumbnail_bounds,
                stack_depth,
                opacity,
                is_hovered: false,
            }
        })
        .collect()
}

/// Performs a hit-test against the shelf cards, returning the index of the first card
/// whose bounds contain the point `(cursor_x, cursor_y)`.
pub fn hit_test_stage_shelf(
    cards: &[StageCardLayout],
    cursor_x: f32,
    cursor_y: f32,
) -> Option<usize> {
    cards.iter().position(|card| card.bounds.contains(cursor_x, cursor_y))
}
