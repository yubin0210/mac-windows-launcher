//! stage_view
//! macOS Stage Manager shelf state machine, physics spring slide animations,
//! hover tracking, thumbnail stream bounds calculation, and click activation.

use mac_physics_core::{
    compute_stage_shelf_layout, hit_test_stage_shelf, Rect, Spring, StageCardLayout,
    StageShelfMetrics,
};

use crate::stage_tracker::StageTracker;

/// User actions triggered from the Stage Manager shelf.
#[derive(Debug, Clone, PartialEq)]
pub enum StageViewAction {
    ActivateGroup(String),
}

/// State machine and layout manager for the Stage Manager shelf flyout.
#[derive(Debug, Clone)]
pub struct StageView {
    pub screen_width: f32,
    pub screen_height: f32,
    pub metrics: StageShelfMetrics,
    pub slide_spring: Spring,
    pub slide_t: f32,
    pub is_hovered: bool,
    pub hovered_card_idx: Option<usize>,
    pub cards: Vec<StageCardLayout>,
    pub auto_hide: bool,
}

impl StageView {
    /// Creates a new `StageView` configured for the specified display dimensions.
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            screen_width,
            screen_height,
            metrics: StageShelfMetrics::default(),
            slide_spring: Spring::new(1.0, 180.0, 24.0),
            slide_t: 0.0,
            is_hovered: false,
            hovered_card_idx: None,
            cards: Vec::new(),
            auto_hide: false,
        }
    }

    /// Updates the display viewport dimensions.
    pub fn set_screen_dimensions(&mut self, width: f32, height: f32) {
        self.screen_width = width;
        self.screen_height = height;
    }

    /// Advances spring physics animation and recomputes shelf layout against tracked window groups.
    pub fn update(&mut self, dt: f32, tracker: &StageTracker) {
        let target = if self.auto_hide && !self.is_hovered {
            0.2
        } else {
            1.0
        };

        self.slide_spring.step(dt, target);
        self.slide_t = self.slide_spring.position().clamp(0.0, 1.0);

        self.cards = compute_stage_shelf_layout(
            tracker.groups(),
            tracker.active_group_id(),
            &self.metrics,
            self.screen_height,
            self.slide_t,
        );

        if let Some(hovered_idx) = self.hovered_card_idx {
            if let Some(card) = self.cards.get_mut(hovered_idx) {
                card.is_hovered = true;
            } else {
                self.hovered_card_idx = None;
            }
        }
    }

    /// Handles cursor movement across the shelf and updates card hover states.
    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        self.is_hovered = true;
        self.hovered_card_idx = hit_test_stage_shelf(&self.cards, x, y);
        for (i, card) in self.cards.iter_mut().enumerate() {
            card.is_hovered = self.hovered_card_idx == Some(i);
        }
    }

    /// Clears hover states when cursor exits the shelf window.
    pub fn on_mouse_leave(&mut self) {
        self.is_hovered = false;
        self.hovered_card_idx = None;
        for card in &mut self.cards {
            card.is_hovered = false;
        }
    }

    /// Handles mouse clicks on shelf cards, returning an activation action if a card was hit.
    pub fn on_mouse_click(&mut self, x: f32, y: f32) -> Option<StageViewAction> {
        if let Some(idx) = hit_test_stage_shelf(&self.cards, x, y) {
            if let Some(card) = self.cards.get(idx) {
                return Some(StageViewAction::ActivateGroup(card.group_id.clone()));
            }
        }
        None
    }

    /// Returns the currently computed card layouts on the shelf.
    pub fn cards(&self) -> &[StageCardLayout] {
        &self.cards
    }

    /// Returns the normalized spring slide progress (0.0 = retracted, 1.0 = fully deployed).
    pub fn slide_progress(&self) -> f32 {
        self.slide_t
    }

    /// Returns active window id, destination thumbnail bounds, and opacity for all displayed shelf cards.
    pub fn desired_thumbnail_streams(&self, tracker: &StageTracker) -> Vec<(u64, Rect, u8)> {
        let mut streams = Vec::new();
        for card in &self.cards {
            if let Some(group) = tracker.groups().iter().find(|g| g.group_id == card.group_id) {
                if let Some(active_win) = group.active_window() {
                    let opacity = (card.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
                    streams.push((active_win.id, card.thumbnail_bounds, opacity));
                }
            }
        }
        streams
    }
}
