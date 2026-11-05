//! mac-physics-core
//! Platform-agnostic math and physics engine for macOS UI/UX reconstruction.

pub mod magnification;
pub mod menu_bar;
pub mod spotlight;
pub mod spring;
pub mod squircle;
pub mod stage_manager;

pub use menu_bar::{
    compute_menu_bar_layout, MenuBarGroup, MenuBarItem, MenuBarLayout, MenuBarMetrics,
    PositionedMenuBarItem, Rect, SystemStatusSnapshot,
};
pub use spring::{DampedSpring, Spring};
pub use stage_manager::{
    compute_stage_shelf_layout, compute_thumbnail_aspect_fit, hit_test_stage_shelf,
    StageCardLayout, StageGroup, StageShelfMetrics, StageWindowInfo, BADGE_RESERVED_HEIGHT,
    CARD_PADDING,
};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(version(), "0.1.0");
    }
}
