//! App identity shared by the Dock and search. Display names never identify applications.
use crate::{DockItem, LaunchTarget};
use mac_physics_core::spotlight::{SpotlightCategory, SpotlightItem};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppRecord {
    pub id: String,
    pub title: String,
    /// Shell parsing name, also used for icon extraction and launching.
    pub target: String,
    /// Only set when a shortcut resolves to a plain exe with no extra arguments.
    pub executable: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
}
impl AppRecord {
    pub fn new(target: &str, title: &str, executable: Option<String>) -> Self {
        Self { id: format!("shell:{}", target.replace('/', "\\").to_lowercase()), title: title.into(), target: target.into(), executable, app_id: None }
    }
    pub fn with_app_id(mut self, app_id: String) -> Self {
        if !app_id.is_empty() { self.id = format!("aumid:{app_id}"); self.app_id = Some(app_id); }
        self
    }
    pub(crate) fn matches_identity(&self, executable: Option<&str>, app_id: Option<&str>) -> bool {
        match (self.app_id.as_deref(), app_id) {
            (Some(a), Some(b)) => a == b,
            (None, None) => match (self.executable.as_deref(), executable) {
                (Some(a), Some(b)) => crate::app_tracker::names_match(a, b), _ => false,
            },
            _ => false,
        }
    }
    pub fn matches(&self, window: &crate::AppInfo) -> bool {
        self.matches_identity(Some(&window.process_name), window.app_id.as_deref())
    }
    pub fn dock_item(&self) -> DockItem {
        let mut item = DockItem::new(&self.id, &self.title, false).with_icon_path(&self.target);
        item.launch_target = Some(if let Some(executable) = &self.executable {
            item.process_name = Some(executable.clone());
            // Launch the shortcut itself: preserve its arguments and working directory.
            LaunchTarget::app(executable, &self.target)
        } else { LaunchTarget::shell(&self.target) });
        item
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppCatalog { apps: Vec<AppRecord> }
impl AppCatalog {
    pub fn new(mut apps: Vec<AppRecord>) -> Self {
        let mut seen = HashSet::new();
        apps.retain(|app| seen.insert(app.id.clone()));
        apps.sort_by(|a,b| a.title.to_lowercase().cmp(&b.title.to_lowercase()).then(a.id.cmp(&b.id)));
        Self { apps }
    }
    pub fn apps(&self) -> &[AppRecord] { &self.apps }
    pub fn get(&self, id: &str) -> Option<&AppRecord> { self.apps.iter().find(|app| app.id == id) }
    pub fn pinned(&self, ids: &[String]) -> Vec<&AppRecord> {
        let mut seen = HashSet::new();
        ids.iter().filter(|id| seen.insert(id.as_str())).filter_map(|id| self.get(id)).collect()
    }
    pub fn spotlight_items(&self) -> Vec<SpotlightItem> {
        let mut names = std::collections::HashMap::new();
        for app in &self.apps { *names.entry(app.title.as_str()).or_insert(0usize) += 1; }
        self.apps.iter().map(|app| {
            let subtitle = if names.get(app.title.as_str()).copied().unwrap_or(0) > 1 { app.target.as_str() } else { "Application" };
            SpotlightItem::new(&app.id, &app.title, subtitle, SpotlightCategory::Application, &app.target)
        }).collect()
    }
    #[cfg(windows)]
    pub fn discover() -> Self { Self::new(native::discover()) }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::path::Path;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ};
    use windows::Win32::UI::Shell::*;

    pub fn discover() -> Vec<AppRecord> {
        let mut apps = Vec::new();
        for variable in ["APPDATA", "ProgramData"] {
            if let Some(root) = std::env::var_os(variable) {
                scan(&std::path::PathBuf::from(root).join("Microsoft/Windows/Start Menu/Programs"), &mut apps, 0);
            }
        }
        // Includes packaged applications that do not have a Start Menu .lnk file.
        let result = unsafe { apps_folder(&mut apps) };
        if let Err(error) = result { crate::diagnostics::write(&format!("AppsFolder enumeration unavailable: {error}")); }
        apps
    }
    fn scan(path: &Path, apps: &mut Vec<AppRecord>, depth: usize) {
        if depth > 12 { return; }
        let Ok(entries) = std::fs::read_dir(path) else { return; };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else { continue; };
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 { continue; } // Skip junctions/reparse points.
            if metadata.is_dir() { scan(&path, apps, depth + 1); continue; }
            if !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("lnk")) { continue; }
            let Some(target) = path.to_str() else { continue; };
            let title = path.file_stem().unwrap_or_default().to_string_lossy();
            let (executable, app_id) = unsafe { shortcut_identity(target) };
            let mut record = AppRecord::new(target, &title, executable);
            if let Some(app_id) = app_id { record = record.with_app_id(app_id); }
            apps.push(record);
        }
    }
    unsafe fn shortcut_identity(path: &str) -> (Option<String>, Option<String>) {
        let Ok(link): Result<IShellLinkW, _> = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) else { return (None, None); };
        let Ok(persist): Result<IPersistFile, _> = link.cast() else { return (None, None); };
        let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        if persist.Load(PCWSTR(wide.as_ptr()), STGM_READ).is_err() { return (None, None); }
        let app_id = link.cast::<PropertiesSystem::IPropertyStore>().ok().and_then(|store| crate::shell_identity::store_app_id(&store));
        let executable = (|| {
            let mut arguments = [0u16; 32768];
            link.GetArguments(&mut arguments).ok()?;
            if arguments[0] != 0 { return None; }
            let mut target = [0u16; 32768];
            link.GetPath(&mut target, std::ptr::null_mut(), 0).ok()?;
            let count = target.iter().position(|c| *c == 0)?;
            let target = String::from_utf16(&target[..count]).ok()?;
            if Path::new(&target).is_absolute() && target.to_ascii_lowercase().ends_with(".exe") { Some(target) } else { None }
        })();
        (executable, app_id)
    }
    unsafe fn name(item: &IShellItem, kind: SIGDN) -> windows::core::Result<String> {
        let value = item.GetDisplayName(kind)?;
        let result = value.to_string();
        CoTaskMemFree(Some(value.0 as *const core::ffi::c_void));
        Ok(result?)
    }
    unsafe fn apps_folder(apps: &mut Vec<AppRecord>) -> windows::core::Result<()> {
        let root: IShellItem = SHCreateItemFromParsingName(windows::core::w!("shell:AppsFolder"), None)?;
        let enumerator: IEnumShellItems = root.BindToHandler(None, &BHID_EnumItems)?;
        loop {
            let mut fetched = 0;
            let mut items = [None];
            enumerator.Next(&mut items, Some(&mut fetched))?;
            if fetched == 0 { break; }
            let Some(item) = items[0].take() else { break; };
            let Ok(title) = name(&item, SIGDN_NORMALDISPLAY) else { continue; };
            let Ok(parsing) = name(&item, SIGDN_DESKTOPABSOLUTEPARSING) else { continue; };
            let target = if Path::new(&parsing).is_absolute() || parsing.starts_with("::{") || parsing.to_ascii_lowercase().starts_with("shell:") {
                parsing
            } else { format!("shell:AppsFolder\\{parsing}") };
            // Desktop entries already covered by shortcuts retain their richer identity.
            if apps.iter().any(|app| app.target.eq_ignore_ascii_case(&target)) { continue; }
            let mut record = AppRecord::new(&target, &title, None);
            // A namespace parsing name inside AppsFolder carries the explicit AUMID.
            // Real filesystem paths are not AUMIDs.
            if !Path::new(&target).is_absolute() {
                if let Some((_, id)) = target.split_once('\\') {
                    if !id.is_empty() && !id.contains(['\\', '/', ':']) { record = record.with_app_id(id.to_string()); }
                }
            }
            if let Ok(item2) = item.cast::<IShellItem2>() {
                if let Ok(value) = item2.GetString(&crate::shell_identity::APP_ID_KEY) {
                    let app_id = value.to_string();
                    CoTaskMemFree(Some(value.0 as *const core::ffi::c_void));
                    if let Ok(app_id) = app_id { record = record.with_app_id(app_id); }
                }
            }
            apps.push(record);
        }
        Ok(())
    }
}
