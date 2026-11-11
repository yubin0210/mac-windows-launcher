//! Versioned user preferences. A missing pin list differs from an intentionally empty Dock.
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpotlightHotkey {
    #[default]
    AltSpace,
    CtrlAltSpace,
    CtrlShiftSpace,
}
impl SpotlightHotkey {
    pub fn label(self) -> &'static str { match self { Self::AltSpace => "Alt + Space", Self::CtrlAltSpace => "Ctrl + Alt + Space", Self::CtrlShiftSpace => "Ctrl + Shift + Space" } }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    #[serde(default)]
    pub pinned: Option<Vec<String>>,
    #[serde(default)]
    pub custom_apps: Vec<crate::app_catalog::AppRecord>,
    #[serde(default)]
    pub monitor: Option<String>,
    #[serde(default = "default_icon_size")]
    pub icon_size: u32,
    #[serde(default)]
    pub spotlight_hotkey: SpotlightHotkey,
    #[serde(default)]
    pub menu_bar_enabled: bool,
    #[serde(default)]
    pub stage_enabled: bool,
}
fn default_icon_size() -> u32 { 48 }
impl Default for Settings {
    fn default() -> Self { Self { version: 1, pinned: None, custom_apps: Vec::new(), monitor: None, icon_size: 48, spotlight_hotkey: SpotlightHotkey::default(), menu_bar_enabled: false, stage_enabled: false } }
}
impl Settings {
    pub fn pin(&mut self, id: &str) {
        let pins = self.pinned.get_or_insert_with(Vec::new);
        if !pins.iter().any(|pin| pin == id) { pins.push(id.into()); }
    }
    pub fn unpin(&mut self, id: &str) {
        self.pinned.get_or_insert_with(Vec::new).retain(|pin| pin != id);
    }
    pub fn move_pin(&mut self, id: &str, delta: i32) {
        let Some(pins) = &mut self.pinned else { return; };
        if let Some(index) = pins.iter().position(|pin| pin == id) {
            let destination = (index as i64 + delta as i64).clamp(0, pins.len() as i64 - 1) as usize;
            let pin = pins.remove(index);
            pins.insert(destination, pin);
        }
    }
}

pub struct LoadedSettings {
    pub settings: Settings,
    pub backup: Option<PathBuf>,
}
pub struct SettingsStore { path: PathBuf }
impl SettingsStore {
    pub fn new(path: PathBuf) -> Self { Self { path } }
    pub fn load(&self) -> io::Result<LoadedSettings> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(LoadedSettings { settings: Settings::default(), backup: None }),
            Err(e) => return Err(e),
        };
        // Version negotiation precedes schema parsing: never reset a newer schema.
        let parsed = serde_json::from_slice::<serde_json::Value>(&bytes);
        if let Ok(value) = &parsed {
            if let Some(version) = value.get("version").and_then(|v| v.as_u64()) {
                if version != 1 { return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Unsupported settings version {version}"))); }
            }
        }
        if let Ok(settings) = serde_json::from_slice::<Settings>(&bytes) {
            if settings.version == 1 && (24..=96).contains(&settings.icon_size) {
                return Ok(LoadedSettings { settings, backup: None });
            }
        }
        let backup = self.path.with_extension(format!("invalid-{}.json", unique_stamp()));
        let mut file = OpenOptions::new().write(true).create_new(true).open(&backup)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(LoadedSettings { settings: Settings::default(), backup: Some(backup) })
    }
    pub fn save(&self, settings: &Settings) -> io::Result<()> {
        if settings.version != 1 || !(24..=96).contains(&settings.icon_size) { return Err(io::Error::new(io::ErrorKind::InvalidData, "Unsupported settings version")); }
        let parent = self.path.parent().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Settings path needs a parent directory"))?;
        fs::create_dir_all(parent)?;
        let temporary = self.path.with_extension(format!("tmp-{}", unique_stamp()));
        let result = (|| {
            let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
            file.write_all(&serde_json::to_vec_pretty(settings)?)?;
            file.sync_all()?;
            drop(file);
            #[cfg(not(windows))]
            fs::rename(&temporary, &self.path)?;
            #[cfg(windows)]
            unsafe {
                use std::os::windows::ffi::OsStrExt;
                use windows::core::PCWSTR;
                use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};
                let source: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
                let target: Vec<u16> = self.path.as_os_str().encode_wide().chain(Some(0)).collect();
                MoveFileExW(PCWSTR(source.as_ptr()), PCWSTR(target.as_ptr()), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH).map_err(io::Error::other)?;
            }
            Ok(())
        })();
        if result.is_err() { let _ = fs::remove_file(temporary); }
        result
    }
}
fn unique_stamp() -> String {
    format!("{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos())
}
