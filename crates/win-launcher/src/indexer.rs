//! Spotlight Application and Settings Indexer.
//!
//! Indexes Windows Start Menu applications, Control Panel & Settings URIs,
//! and runs fuzzy matching and instant arithmetic expression evaluation.

use mac_physics_core::spotlight::{evaluate_math, fuzzy_match, SpotlightCategory, SpotlightItem};

/// High-performance application and settings search indexer.
#[derive(Debug, Clone, Default)]
pub struct AppIndexer {
    items: Vec<SpotlightItem>,
}

impl AppIndexer {
    /// Creates a new empty indexer.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Number of indexed items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Registers a custom or mock item.
    pub fn register_item(&mut self, item: SpotlightItem) {
        self.items.push(item);
    }

    /// Convenience method to register a mock item with string parameters.
    pub fn register_mock_item(
        &mut self,
        title: impl Into<String>,
        subtitle: impl Into<String>,
        category: SpotlightCategory,
        action_target: impl Into<String>,
    ) {
        let title_str = title.into();
        let id = format!("{}-{}", category_prefix(category), title_str.to_ascii_lowercase().replace(' ', "-"));
        self.items.push(SpotlightItem::new(
            id,
            title_str,
            subtitle,
            category,
            action_target,
        ));
    }

    /// Builds or rebuilds the index.
    ///
    /// On Windows, scans the Start Menu directories for `.lnk` shortcuts
    /// and indexes Windows Settings URIs (`ms-settings:`).
    /// On non-Windows platforms, populates standard mock desktop applications
    /// and settings for development and testing.
    pub fn build_index(&mut self) {
        self.items.clear();

        // 1. Register system settings (standard across Windows 10/11)
        self.register_system_settings();

        // 2. Platform-specific application indexing
        #[cfg(windows)]
        {
            self.scan_windows_start_menu();
        }

        #[cfg(not(windows))]
        {
            self.populate_mock_applications();
        }
    }

    /// Searches indexed items and evaluates math expressions.
    ///
    /// Returns up to `limit` items, ordered with calculations and highest fuzzy scores first.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SpotlightItem> {
        let q = query.trim();
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }

        let mut results = Vec::new();

        // 1. Check for arithmetic calculation (requires at least one digit or operator to prevent single-letter apps like "e" being hijacked)
        let is_math_query = q.chars().any(|c| c.is_ascii_digit())
            || q.contains('+')
            || q.contains('-')
            || q.contains('*')
            || q.contains('/')
            || q.contains('^')
            || q.starts_with("sqrt")
            || q.starts_with("abs");

        if is_math_query {
            if let Some(calc_val) = evaluate_math(q) {
                let formatted_val = format_calc_result(calc_val);
                results.push(SpotlightItem::new(
                    format!("calc-{q}"),
                    formatted_val.clone(),
                    format!("= {q}"),
                    SpotlightCategory::Calculation,
                    formatted_val,
                ));
            }
        }

        // 2. Fuzzy match against all indexed items (storing indices to avoid cloning)
        let mut scored_indices: Vec<(i32, usize)> = Vec::new();
        for (idx, item) in self.items.iter().enumerate() {
            if let Some(score) = fuzzy_match(q, &item.title) {
                scored_indices.push((score, idx));
            } else if let Some(sub_score) = fuzzy_match(q, &item.subtitle) {
                // Secondary match on subtitle with penalty
                scored_indices.push((sub_score - 100, idx));
            }
        }

        // Sort descending by score
        scored_indices.sort_by(|a, b| b.0.cmp(&a.0));

        // Append scored items up to limit (cloning only the returned items)
        for (_, idx) in scored_indices {
            if results.len() >= limit {
                break;
            }
            results.push(self.items[idx].clone());
        }

        results
    }

    /// Registers standard Windows Settings URI items.
    fn register_system_settings(&mut self) {
        let settings = [
            ("Display Settings", "System > Display", "ms-settings:display"),
            ("Sound Settings", "System > Sound", "ms-settings:sound"),
            ("Notifications & Actions", "System > Notifications", "ms-settings:notifications"),
            ("Bluetooth & Devices", "Devices > Bluetooth", "ms-settings:bluetooth"),
            ("Network & Internet", "Network status and setup", "ms-settings:network"),
            ("Installed Apps", "Apps > Installed apps", "ms-settings:appsfeatures"),
            ("Personalization & Background", "Personalization", "ms-settings:personalization"),
            ("Windows Update", "Update & Security", "ms-settings:windowsupdate"),
            ("Taskbar Settings", "Personalization > Taskbar", "ms-settings:taskbar"),
            ("Privacy & Security", "Privacy options", "ms-settings:privacy"),
        ];

        for (title, subtitle, target) in settings {
            self.register_mock_item(title, subtitle, SpotlightCategory::SystemSetting, target);
        }
    }

    #[cfg(windows)]
    fn scan_windows_start_menu(&mut self) {
        use std::collections::HashSet;
        use std::path::PathBuf;

        let mut search_dirs = Vec::new();
        let mut seen_titles = HashSet::new();

        if let Ok(prog_data) = std::env::var("ProgramData") {
            search_dirs.push(
                PathBuf::from(prog_data)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }

        if let Ok(app_data) = std::env::var("APPDATA") {
            search_dirs.push(
                PathBuf::from(app_data)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }

        for dir in search_dirs {
            scan_directory_recursive(&dir, &mut self.items, &mut seen_titles, 0);
        }
    }

    #[cfg(not(windows))]
    fn populate_mock_applications(&mut self) {
        let apps = [
            ("Terminal", "Command Line Interface", "wt.exe"),
            ("Google Chrome", "Fast, secure web browser", "chrome.exe"),
            ("Visual Studio Code", "Code editor", "code.exe"),
            ("File Explorer", "Files and folders", "explorer.exe"),
            ("Settings", "System configuration and options", "ms-settings:"),
            ("Calculator", "Standard calculator", "calc.exe"),
            ("Task Manager", "Monitor system resources", "taskmgr.exe"),
            ("Notepad", "Simple text editor", "notepad.exe"),
        ];

        for (title, subtitle, target) in apps {
            self.register_mock_item(title, subtitle, SpotlightCategory::Application, target);
        }
    }
}

fn category_prefix(category: SpotlightCategory) -> &'static str {
    match category {
        SpotlightCategory::Application => "app",
        SpotlightCategory::SystemSetting => "setting",
        SpotlightCategory::Calculation => "calc",
        SpotlightCategory::File => "file",
        SpotlightCategory::Other => "item",
    }
}

fn format_calc_result(val: f64) -> String {
    if val.is_infinite() {
        return "Infinity".to_string();
    }
    if val.is_nan() {
        return "NaN".to_string();
    }
    if val.fract() == 0.0 && val >= (i64::MIN as f64) && val <= (i64::MAX as f64) {
        format!("{}", val as i64)
    } else {
        let s = format!("{:.6}", val);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(windows)]
fn scan_directory_recursive(
    dir: &std::path::Path,
    items: &mut Vec<SpotlightItem>,
    seen_titles: &mut std::collections::HashSet<String>,
    depth: usize,
) {
    if depth > 5 || !dir.is_dir() {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_directory_recursive(&path, items, seen_titles, depth + 1);
        } else if let Some(ext) = path.extension() {
            if ext.eq_ignore_ascii_case("lnk") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let lower = stem.to_ascii_lowercase();
                    // Ignore uninstaller / help shortcuts
                    if lower.contains("uninstall") || lower.contains("help") || lower.contains("readme") {
                        continue;
                    }
                    if !seen_titles.insert(lower.clone()) {
                        // Already indexed this application name from another location
                        continue;
                    }
                    let id = format!("app-{}", lower.replace(' ', "-"));
                    items.push(SpotlightItem::new(
                        id,
                        stem.to_string(),
                        "Application",
                        SpotlightCategory::Application,
                        path.to_string_lossy().to_string(),
                    ));
                }
            }
        }
    }
}
