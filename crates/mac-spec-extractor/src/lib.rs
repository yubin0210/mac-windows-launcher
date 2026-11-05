//! mac-spec-extractor
//! macOS REA spec extraction library and CLI.

pub mod extractor;
pub mod models;

pub use extractor::DockMetricsExtractor;
pub use models::DockMetrics;

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
