//! Composes pinned applications and transient running apps without losing saved identities.
use crate::{app_catalog::{AppCatalog, AppRecord}, settings::Settings, AppTracker};
pub struct DockSession {
    base: AppCatalog,
    pub catalog: AppCatalog,
    pub settings: Settings,
    running: Vec<AppRecord>,
    page: usize,
}
impl DockSession {
    pub fn new(base: AppCatalog, mut settings: Settings) -> Self {
        if settings.pinned.is_none() {
            let eligible: Vec<_> = base.apps().iter().filter(|app| {
                let name = app.title.to_lowercase();
                !["uninstall", "remove", "제거", "삭제"].iter().any(|word| name.contains(word))
            }).collect();
            settings.pinned = Some(eligible.iter().filter(|app| app.executable.is_some()).chain(eligible.iter().filter(|app| app.executable.is_none())).take(6).map(|app| app.id.clone()).collect());
        }
        let catalog = AppCatalog::new(base.apps().iter().cloned().chain(settings.custom_apps.iter().cloned()).collect());
        Self { base, catalog, settings, running: Vec::new(), page: 0 }
    }
    pub fn refresh(&mut self, tracker: &AppTracker) {
        let mut seen = std::collections::HashSet::new();
        self.running = tracker.all_apps().into_iter().filter(|app| app.pid != std::process::id()).filter_map(|app| {
            let target = app.process_name;
            let identity = app.app_id.as_ref().map(|id| format!("aumid:{id}")).unwrap_or_else(|| target.to_lowercase());
            if !target.contains(['\\', '/']) || !seen.insert(identity) { return None; }
            let name = target.rsplit(['\\', '/']).next().unwrap_or(&target).trim_end_matches(".exe").to_string();
            Some(if let Some(app_id) = app.app_id {
                AppRecord::new(&target, &name, Some(target.clone())).with_app_id(app_id)
            } else { AppRecord::new(&target, &name, Some(target.clone())) })
        }).collect();
        self.catalog = AppCatalog::new(self.base.apps().iter().cloned().chain(self.settings.custom_apps.iter().cloned()).chain(self.running.iter().cloned()).collect());
    }
    fn ordered(&self) -> Vec<&AppRecord> {
        let mut apps = self.catalog.pinned(self.settings.pinned.as_deref().unwrap_or(&[]));
        for running in &self.running {
            if apps.iter().any(|app| app.id == running.id || app.matches_identity(running.executable.as_deref(), running.app_id.as_deref())) { continue; }
            if let Some(app) = self.catalog.get(&running.id) { apps.push(app); }
        }
        apps
    }
    pub fn visible(&self, capacity: usize) -> Vec<AppRecord> {
        let apps = self.ordered();
        let capacity = capacity.max(1);
        let pages = apps.len().div_ceil(capacity).max(1);
        apps.into_iter().skip((self.page % pages) * capacity).take(capacity).cloned().collect()
    }
    pub fn page_by(&mut self, delta: i32, capacity: usize) {
        let pages = self.ordered().len().div_ceil(capacity.max(1)).max(1);
        self.page = (self.page as i64 + delta as i64).rem_euclid(pages as i64) as usize;
    }
    pub fn pin_settings(&self, id: &str) -> Option<Settings> {
        let app = self.catalog.get(id)?;
        let mut settings = self.settings.clone();
        settings.pin(id);
        if !self.base.apps().iter().any(|app| app.id == id) && !settings.custom_apps.iter().any(|app| app.id == id) {
            settings.custom_apps.push(app.clone());
        }
        Some(settings)
    }
}
