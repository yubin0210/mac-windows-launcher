use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::models::DockMetrics;

/// Extractor for macOS Dock metrics using REA CLI, native macOS tools (plutil),
/// or falling back to calibrated defaults.
#[derive(Debug, Clone)]
pub struct DockMetricsExtractor {
    plist_path: Option<PathBuf>,
}

impl Default for DockMetricsExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl DockMetricsExtractor {
    /// Create a new extractor targeting the default macOS Dock app path.
    pub fn new() -> Self {
        Self {
            plist_path: Some(PathBuf::from(
                "/System/Library/CoreServices/Dock.app/Contents/Info.plist",
            )),
        }
    }

    /// Specify a custom plist or app path to extract metrics from.
    pub fn with_plist_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.plist_path = Some(path.into());
        self
    }

    /// Extract dock metrics, inspecting plists if available and falling back to calibrated defaults.
    pub fn extract(&self) -> DockMetrics {
        if let Some(ref path) = self.plist_path {
            if let Some(metrics) = self.extract_from_path(path) {
                return metrics;
            }
        }

        DockMetrics::default_macos_spec()
    }

    /// Attempt extraction from a specific file or directory path.
    pub fn extract_from_path<P: AsRef<Path>>(&self, path: P) -> Option<DockMetrics> {
        let path = path.as_ref();
        let target_file = if path.is_dir() {
            let info = path.join("Contents/Info.plist");
            if info.exists() {
                info
            } else {
                path.to_path_buf()
            }
        } else {
            path.to_path_buf()
        };

        if !target_file.exists() {
            return None;
        }

        // Try REA inspect-plist first
        if let Some(val) = Self::inspect_with_rea(&target_file) {
            return Some(Self::extract_from_json_value(&val));
        }

        // Try plutil conversion
        if let Some(val) = Self::inspect_with_plutil(&target_file) {
            return Some(Self::extract_from_json_value(&val));
        }

        // Fallback to basic file read if JSON or plain text
        if let Ok(content) = fs::read_to_string(&target_file) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                return Some(Self::extract_from_json_value(&val));
            }
        }

        // Target file exists and is confirmed (e.g. Info.plist), return calibrated defaults
        Some(DockMetrics::default_macos_spec())
    }

    /// Parse REA CLI `rea inspect-plist <path> --format json`
    fn inspect_with_rea(path: &Path) -> Option<serde_json::Value> {
        let output = Command::new("rea")
            .args(["inspect-plist", path.to_str()?, "--format", "json"])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        if let Some(val) = parsed.pointer("/normalized_result/value") {
            Some(val.clone())
        } else {
            Some(parsed)
        }
    }

    /// Parse using `plutil -convert json -o - <path>`
    fn inspect_with_plutil(path: &Path) -> Option<serde_json::Value> {
        let output = Command::new("plutil")
            .args(["-convert", "json", "-o", "-", path.to_str()?])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        serde_json::from_slice(&output.stdout).ok()
    }

    /// Extract `DockMetrics` from a parsed JSON value (from REA or plutil),
    /// taking any explicitly specified metrics and falling back to calibrated defaults.
    pub fn extract_from_json_value(value: &serde_json::Value) -> DockMetrics {
        let mut metrics = DockMetrics::default_macos_spec();

        if let Some(obj) = value.as_object() {
            // Check for icon sizes
            if let Some(val) = obj.get("tilesize").or_else(|| obj.get("base_icon_size")) {
                if let Some(size) = val.as_f64() {
                    if size > 0.0 {
                        metrics.base_icon_size = size as f32;
                    }
                }
            }

            if let Some(val) = obj.get("largesize").or_else(|| obj.get("max_icon_size")) {
                if let Some(size) = val.as_f64() {
                    if size > 0.0 {
                        metrics.max_icon_size = size as f32;
                    }
                }
            }

            if let Some(val) = obj.get("magnification_spread").or_else(|| obj.get("spread")) {
                if let Some(spread) = val.as_f64() {
                    if spread > 0.0 {
                        metrics.magnification_spread = spread as f32;
                    }
                }
            }

            if let Some(val) = obj.get("squircle_exponent").or_else(|| obj.get("exponent")) {
                if let Some(exp) = val.as_f64() {
                    if exp >= 1.0 {
                        metrics.squircle_exponent = exp as f32;
                    }
                }
            }

            if let Some(val) = obj.get("border_radius") {
                if let Some(r) = val.as_f64() {
                    if r > 0.0 {
                        metrics.border_radius = r as f32;
                    }
                }
            }

            if let Some(val) = obj.get("icon_padding").or_else(|| obj.get("padding")) {
                if let Some(p) = val.as_f64() {
                    if p >= 0.0 {
                        metrics.icon_padding = p as f32;
                    }
                }
            }
        }

        metrics
    }
}
