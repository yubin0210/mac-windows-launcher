//! Continuous Gaussian magnification curve and dock icon layout math.
//!
//! Provides the mathematical core for replicating macOS Dock magnification behavior,
//! including smooth Gaussian bell-curve scaling and symmetric expansion layout calculations.

use serde::{Deserialize, Serialize};

/// Configuration parameters for macOS Dock magnification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MagnificationConfig {
    /// Standard unmagnified icon dimension in points/pixels.
    #[serde(alias = "base_icon_size")]
    pub base_size: f32,
    /// Maximum icon dimension when fully magnified under cursor.
    #[serde(alias = "max_icon_size")]
    pub max_size: f32,
    /// Gaussian spread / standard deviation (sigma) in points/pixels.
    #[serde(alias = "magnification_spread")]
    pub spread: f32,
}

impl MagnificationConfig {
    /// Creates a new `MagnificationConfig` with explicit parameters.
    pub fn new(base_size: f32, max_size: f32, spread: f32) -> Self {
        Self {
            base_size,
            max_size,
            spread,
        }
    }

    /// Attempts to parse a `MagnificationConfig` from JSON, with fallback to default specs.
    pub fn from_json_str_or_default(json_str: &str) -> Self {
        serde_json::from_str::<Self>(json_str).unwrap_or_default()
    }
}

impl Default for MagnificationConfig {
    /// Returns default macOS Dock magnification specifications:
    /// 48.0pt base size, 128.0pt max size, 150.0pt Gaussian spread.
    fn default() -> Self {
        Self {
            base_size: 48.0,
            max_size: 128.0,
            spread: 150.0,
        }
    }
}

/// Calculates continuous scale factors for dock icons using a Gaussian bell curve.
///
/// For each icon center $x$, the scale is computed as:
/// $$s(x) = 1.0 + \left(\frac{max\_size}{base\_size} - 1.0\right) \cdot \exp\left(-\frac{(x - cursor\_x)^2}{2\sigma^2}\right)$$
///
/// Icons beyond $3\sigma$ are clamped to $1.0$ using a smooth Hermite cubic taper between
/// $2\sigma$ and $3\sigma$ to guarantee $C^1$ continuity with no sudden snapping at 120Hz.
///
/// # Edge Case Safety
/// Returns 1.0 for all icons if `base_size <= 0`, `max_size <= base_size`, `spread <= 0`,
/// or if non-finite floating point values are supplied.
pub fn calculate_icon_scales(
    cursor_x: f32,
    icon_centers: &[f32],
    config: &MagnificationConfig,
) -> Vec<f32> {
    if icon_centers.is_empty() {
        return Vec::new();
    }

    // Guard against zero-division, inversion, or non-finite inputs
    if !config.base_size.is_finite()
        || config.base_size <= 0.0
        || !config.max_size.is_finite()
        || config.max_size <= config.base_size
        || !config.spread.is_finite()
        || config.spread <= 0.0
        || !cursor_x.is_finite()
    {
        return vec![1.0; icon_centers.len()];
    }

    let sigma = config.spread;
    let sigma_sq_2 = 2.0 * sigma * sigma;
    let max_ratio = config.max_size / config.base_size;
    let amplitude = max_ratio - 1.0;
    let cutoff = 3.0 * sigma;
    let taper_start = 2.0 * sigma;
    let taper_range = cutoff - taper_start; // = sigma

    icon_centers
        .iter()
        .map(|&center_x| {
            if !center_x.is_finite() {
                return 1.0;
            }

            let dist = (center_x - cursor_x).abs();
            if dist >= cutoff {
                1.0
            } else {
                let gaussian = (-((dist * dist) / sigma_sq_2)).exp();
                let taper = if dist > taper_start {
                    let t = (dist - taper_start) / taper_range;
                    // Smoothstep cubic Hermite curve: 1 - (3t^2 - 2t^3)
                    1.0 - t * t * (3.0 - 2.0 * t)
                } else {
                    1.0
                };
                1.0 + amplitude * gaussian * taper
            }
        })
        .collect()
}

/// Calculates positioned centers and rendered dimensions `(center_x, rendered_size)`
/// for each icon, preserving symmetric horizontal expansion around the dock's resting midpoint.
///
/// When all `scales` equal 1.0, the first icon starts at `center_x = base_size / 2.0`
/// and subsequent icons are spaced by `base_size + padding`.
pub fn calculate_dock_layout(
    scales: &[f32],
    base_size: f32,
    padding: f32,
) -> Vec<(f32, f32)> {
    if scales.is_empty() {
        return Vec::new();
    }

    let safe_base_size = if base_size.is_finite() && base_size > 0.0 {
        base_size
    } else {
        0.0
    };
    let safe_padding = if padding.is_finite() && padding > 0.0 {
        padding
    } else {
        0.0
    };

    let n = scales.len() as f32;
    let resting_total_width = n * safe_base_size + (n - 1.0).max(0.0) * safe_padding;
    let resting_midpoint = resting_total_width / 2.0;

    calculate_dock_layout_at(scales, safe_base_size, safe_padding, resting_midpoint)
}

/// Calculates positioned centers and rendered dimensions `(center_x, rendered_size)`
/// for each icon, centered symmetrically around an explicit horizontal anchor `dock_center_x`.
pub fn calculate_dock_layout_at(
    scales: &[f32],
    base_size: f32,
    padding: f32,
    dock_center_x: f32,
) -> Vec<(f32, f32)> {
    if scales.is_empty() {
        return Vec::new();
    }

    let safe_base_size = if base_size.is_finite() && base_size > 0.0 {
        base_size
    } else {
        0.0
    };
    let safe_padding = if padding.is_finite() && padding > 0.0 {
        padding
    } else {
        0.0
    };

    let n = scales.len() as f32;
    let rendered_sizes: Vec<f32> = scales
        .iter()
        .map(|&s| {
            let scale = if s.is_finite() && s >= 1.0 { s } else { 1.0 };
            safe_base_size * scale
        })
        .collect();

    let magnified_icons_sum: f32 = rendered_sizes.iter().sum();
    let magnified_total_width = magnified_icons_sum + (n - 1.0).max(0.0) * safe_padding;

    // Start left coordinate such that the center of the total layout aligns with dock_center_x
    let mut current_left = dock_center_x - magnified_total_width / 2.0;

    let mut layout = Vec::with_capacity(scales.len());
    for &size in &rendered_sizes {
        let center_x = current_left + size / 2.0;
        layout.push((center_x, size));
        current_left += size + safe_padding;
    }

    layout
}
