//! win-launcher
//! DirectComposition 120Hz system launcher for Windows replicating macOS Dock fidelity.

pub mod app_tracker;
pub mod directx;
pub mod dock_view;
pub mod dwm_thumbnail;
pub mod indexer;
pub mod menu_bar_render;
pub mod menu_bar_view;
pub mod menu_bar_window;
pub mod process_launcher;
pub mod spotlight_render;
pub mod spotlight_view;
pub mod spotlight_window;
pub mod stage_render;
pub mod stage_tracker;
pub mod stage_view;
pub mod stage_window;
pub mod system_status;
pub mod taskbar_guard;
pub mod window;

pub use app_tracker::{AppInfo, AppTracker};
pub use directx::{Direct2DRenderer, DirectCompositionPipeline, MockRenderContext, RenderError};
#[cfg(windows)]
pub use directx::CachedBrushes;
pub use dock_view::{DockItem, DockRenderItem, DockView, DockViewConfig};
pub use dwm_thumbnail::{
    DwmThumbnailManager, MockThumbnailController, MockThumbnailRecord, ThumbnailController,
    ThumbnailRegistration,
};
#[cfg(windows)]
pub use dwm_thumbnail::Win32ThumbnailController;
pub use indexer::AppIndexer;
pub use menu_bar_render::{MenuBarRenderContext, MenuBarRenderer, MockMenuBarRenderContext};
#[cfg(windows)]
pub use menu_bar_render::{CachedMenuBarBrushes, Direct2DMenuBarRenderer};
pub use menu_bar_view::{MenuBarAction, MenuBarView, MenuDropdown, MenuDropdownItem};
pub use menu_bar_window::{MenuBarWindow, MenuBarWindowConfig, MenuBarWindowEvent};
pub use process_launcher::{format_arguments, LaunchResult, LaunchTarget, ProcessLauncher};
pub use spotlight_render::{MockSpotlightRenderContext, SpotlightRenderContext, SpotlightRenderer};
#[cfg(windows)]
pub use spotlight_render::{CachedSpotlightBrushes, Direct2DSpotlightRenderer};
pub use spotlight_view::SpotlightView;
pub use spotlight_window::{SpotlightEvent, SpotlightWindow, SpotlightWindowConfig};
pub use stage_render::{MockStageRenderer, StageRenderContext, StageRenderer};
pub use stage_tracker::{
    fallback_app_name_from_title, is_stage_candidate, MockWindowEnumerator, StageTracker,
    WindowEnumerator,
};
#[cfg(windows)]
pub use stage_tracker::Win32WindowEnumerator;
pub use stage_view::{StageView, StageViewAction};
pub use stage_window::{MockStageWindow, StageWindow, StageWindowConfig, StageWindowEvent};
pub use mac_physics_core::{StageGroup, StageWindowInfo};
pub use system_status::SystemStatusTracker;
pub use taskbar_guard::TaskbarGuard;
pub use window::{DockWindow, DockWindowConfig, WindowEvent};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
