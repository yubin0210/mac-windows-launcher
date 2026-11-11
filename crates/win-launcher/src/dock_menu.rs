//! Native Dock context menus. Commands carry stable IDs, never display-row indexes.
use crate::{app_catalog::AppCatalog, settings::Settings, AppInfo};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, POINT, WPARAM, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone)]
pub enum DockAction { ToggleStage, ToggleMenuBar, Pin(String), Unpin(String), Move(String, i32), Activate(usize, u32), Launch(String), Page(i32), Monitor(String), IconSize(u32), SpotlightHotkey(crate::settings::SpotlightHotkey) }
struct Menu(HMENU);
impl Menu {
    fn new() -> Result<Self, String> { unsafe { CreatePopupMenu().map(Self).map_err(|e| e.to_string()) } }
    fn item(&self, text: &str, action: DockAction, actions: &mut Vec<DockAction>, checked: bool) -> Result<(), String> {
        let id = actions.len() + 100;
        let wide = wide_label(text);
        unsafe { AppendMenuW(self.0, MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED }, id, PCWSTR(wide.as_ptr())).map_err(|e| e.to_string())?; }
        actions.push(action);
        Ok(())
    }
    fn child(&self, title: &str, child: Menu) -> Result<(), String> {
        let wide = wide_label(title);
        unsafe { AppendMenuW(self.0, MF_POPUP, child.0.0 as usize, PCWSTR(wide.as_ptr())).map_err(|e| e.to_string())?; }
        std::mem::forget(child); // Parent menu now owns the child HMENU.
        Ok(())
    }
}
impl Drop for Menu { fn drop(&mut self) { unsafe { let _ = DestroyMenu(self.0); } } }
fn wide_label(label: &str) -> Vec<u16> { label.chars().take(140).collect::<String>().replace('&', "&&").encode_utf16().chain(Some(0)).collect() }
fn select(hwnd: HWND, menu: &Menu, actions: &[DockAction]) -> Option<DockAction> {
    unsafe {
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let previous = GetForegroundWindow();
        let _ = SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(menu.0, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, None).0;
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        if GetForegroundWindow() == hwnd && previous != hwnd { let _ = SetForegroundWindow(previous); }
        command.checked_sub(100).and_then(|index| actions.get(index as usize)).cloned()
    }
}
fn add_windows(menu: &Menu, windows: &[AppInfo], actions: &mut Vec<DockAction>) -> Result<(), String> {
    for window in windows {
        let mut title = [0u16; 512];
        let count = unsafe { GetWindowTextW(HWND(window.hwnd as *mut _), &mut title) };
        let title = if count > 0 { String::from_utf16_lossy(&title[..count as usize]) } else { format!("Window {}", window.hwnd) };
        menu.item(&title, DockAction::Activate(window.hwnd, window.pid), actions, window.is_focused)?;
    }
    Ok(())
}
pub fn windows(hwnd: HWND, windows: &[AppInfo]) -> Result<Option<DockAction>, String> {
    let menu = Menu::new()?;
    let mut actions = Vec::new();
    add_windows(&menu, windows, &mut actions)?;
    Ok(select(hwnd, &menu, &actions))
}
pub fn context(hwnd: HWND, id: Option<&str>, catalog: &AppCatalog, settings: &Settings, windows: &[AppInfo]) -> Result<Option<DockAction>, String> {
    let root = Menu::new()?;
    let mut actions = Vec::new();
    if let Some(id) = id {
        let title = catalog.get(id).map(|app| format!("Open {}", app.title)).unwrap_or_else(|| "Open".into());
        root.item(&title, DockAction::Launch(id.into()), &mut actions, false)?;
        let pinned = settings.pinned.as_ref().is_some_and(|pins| pins.iter().any(|pin| pin == id));
        if pinned {
            root.item("Remove from Dock", DockAction::Unpin(id.into()), &mut actions, false)?;
            root.item("Move left", DockAction::Move(id.into(), -1), &mut actions, false)?;
            root.item("Move right", DockAction::Move(id.into(), 1), &mut actions, false)?;
        } else { root.item("Keep in Dock", DockAction::Pin(id.into()), &mut actions, false)?; }
        add_windows(&root, windows, &mut actions)?;
    }
    let add = Menu::new()?;
    for chunk in catalog.apps().chunks(25) {
        let group = Menu::new()?;
        for app in chunk {
            let checked = settings.pinned.as_ref().is_some_and(|pins| pins.contains(&app.id));
            let title = if catalog.apps().iter().filter(|other| other.title == app.title).count() > 1 { format!("{} — {}", app.title, app.target) } else { app.title.clone() };
            group.item(&title, DockAction::Pin(app.id.clone()), &mut actions, checked)?;
        }
        if let (Some(first), Some(last)) = (chunk.first(), chunk.last()) { add.child(&format!("{} … {}", first.title, last.title), group)?; }
    }
    root.child("Add app to Dock", add)?;
    let displays = Menu::new()?;
    for monitor in crate::display::monitors() {
        let checked = settings.monitor.as_ref().map(|id| id == &monitor.id).unwrap_or_else(|| {
            monitor.handle == unsafe { windows::Win32::Graphics::Gdi::MonitorFromWindow(hwnd, windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTOPRIMARY) }
        });
        displays.item(&monitor.id, DockAction::Monitor(monitor.id.clone()), &mut actions, checked)?;
    }
    root.child("Display", displays)?;
    let sizes = Menu::new()?;
    for (label, size) in [("Small", 32), ("Medium", 48), ("Large", 64)] {
        sizes.item(label, DockAction::IconSize(size), &mut actions, settings.icon_size == size)?;
    }
    root.child("Dock size", sizes)?;
    let shortcuts = Menu::new()?;
    for key in [crate::settings::SpotlightHotkey::AltSpace, crate::settings::SpotlightHotkey::CtrlAltSpace, crate::settings::SpotlightHotkey::CtrlShiftSpace] {
        shortcuts.item(key.label(), DockAction::SpotlightHotkey(key), &mut actions, settings.spotlight_hotkey == key)?;
    }
    root.child("Spotlight shortcut", shortcuts)?;
    root.item(if settings.menu_bar_enabled { "Hide menu bar" } else { "Show menu bar" }, DockAction::ToggleMenuBar, &mut actions, settings.menu_bar_enabled)?;
    root.item(if settings.stage_enabled { "Disable Stage Manager" } else { "Enable Stage Manager" }, DockAction::ToggleStage, &mut actions, settings.stage_enabled)?;
    root.item("Previous page", DockAction::Page(-1), &mut actions, false)?;
    root.item("Next page", DockAction::Page(1), &mut actions, false)?;
    Ok(select(hwnd, &root, &actions))
}
