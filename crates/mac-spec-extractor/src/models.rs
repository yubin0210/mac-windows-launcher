use serde::{Deserialize, Serialize};

/// Metrics and styling parameters extracted or calibrated from macOS Dock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DockMetrics {
    /// Default icon dimension in points/pixels when unmagnified.
    pub base_icon_size: f32,
    /// Maximum icon dimension in points/pixels when fully magnified.
    pub max_icon_size: f32,
    /// Gaussian spread / standard deviation in points/pixels for magnification influence.
    pub magnification_spread: f32,
    /// Curvature exponent for Apple superellipse squircle contour (typically ~4.5, >= 4.0).
    pub squircle_exponent: f32,
    /// Background dock tray border/corner radius in points.
    pub border_radius: f32,
    /// Horizontal padding between icons in points.
    pub icon_padding: f32,
}

impl DockMetrics {
    /// Returns the standard calibrated macOS Dock specification.
    pub fn default_macos_spec() -> Self {
        Self {
            base_icon_size: 48.0,
            max_icon_size: 128.0,
            magnification_spread: 150.0,
            squircle_exponent: 4.5,
            border_radius: 16.0,
            icon_padding: 10.0,
        }
    }
}

impl Default for DockMetrics {
    fn default() -> Self {
        Self::default_macos_spec()
    }
}
