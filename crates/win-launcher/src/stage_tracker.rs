//! stage_tracker
//! macOS-fidelity Stage Manager window tracker and application grouping engine.
//!
//! Enumerates top-level visible application windows, filters out utility/tool/cloaked surfaces,
//! groups windows into logical `StageGroup` instances by application, and manages group
//! activation, restoration, and minimization.

use std::sync::{Arc, Mutex};
use mac_physics_core::{StageGroup, StageWindowInfo};

/// Interface for window enumeration, activation, and state control.
pub trait WindowEnumerator: Send + Sync {
    /// Enumerates top-level candidate application windows.
    fn enumerate_windows(&self) -> Vec<StageWindowInfo>;

    /// Brings the specified window handle to the foreground and restores it if needed.
    fn activate_window(&self, id: u64) -> Result<(), String>;

    /// Minimizes the specified window handle.
    fn minimize_window(&self, id: u64) -> Result<(), String>;

    /// Restores the specified window handle from minimization.
    fn restore_window(&self, id: u64) -> Result<(), String>;

    /// Optional mock hook for injecting test windows dynamically.
    fn add_mock_window(&self, _window: StageWindowInfo) {}

    /// Optional mock hook for removing test windows dynamically.
    fn remove_mock_window(&self, _id: u64) {}
}

/// Evaluates whether a window candidate meets Stage Manager eligibility criteria.
///
/// Filters out:
/// - Invisible windows
/// - Dimensions <= 100 on either axis
/// - Windows with `WS_EX_TOOLWINDOW` style
/// - Cloaked windows (DWM cloaked)
/// - Empty, whitespace, or "Program Manager" titles
/// - Launcher's own class names or titles ("MacDockWindow", "MacMenuBarWindow", "MacSpotlightWindow", "MacStageWindow")
pub fn is_stage_candidate(
    title: &str,
    class_name: &str,
    width: u32,
    height: u32,
    is_tool_window: bool,
    is_cloaked: bool,
    is_visible: bool,
) -> bool {
    if !is_visible {
        return false;
    }
    if width <= 100 || height <= 100 {
        return false;
    }
    if is_tool_window || is_cloaked {
        return false;
    }
    let trimmed_title = title.trim();
    if trimmed_title.is_empty() || trimmed_title == "Program Manager" {
        return false;
    }

    const IGNORED_NAMES: &[&str] = &[
        "MacDockWindow",
        "MacMenuBarWindow",
        "MacSpotlightWindow",
        "MacStageWindow",
        "MacWinLauncherDockClass",
        "MacWinLauncherMenuBarClass",
        "MacWinLauncherSpotlightClass",
        "MacWinLauncherStageClass",
    ];

    let trimmed_class = class_name.trim();
    for &ignored in IGNORED_NAMES {
        if trimmed_title.eq_ignore_ascii_case(ignored) || trimmed_class.eq_ignore_ascii_case(ignored) {
            return false;
        }
    }

    true
}

/// Fallback extraction of application name from window title (e.g. "Document - Word" -> "Document").
pub fn fallback_app_name_from_title(title: &str) -> String {
    let trimmed = title.trim();
    if let Some((prefix, _)) = trimmed.split_once('-') {
        let clean = prefix.trim();
        if !clean.is_empty() {
            return clean.to_string();
        }
    }
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    "App".to_string()
}

// ---------------------------------------------------------------------------
// Mock Window Enumerator
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct MockInner {
    windows: Vec<StageWindowInfo>,
    activated_ids: Vec<u64>,
    minimized_ids: Vec<u64>,
    restored_ids: Vec<u64>,
}

/// Thread-safe in-memory window enumerator for headless testing and non-Windows targets.
#[derive(Debug, Clone, Default)]
pub struct MockWindowEnumerator {
    inner: Arc<Mutex<MockInner>>,
}

impl MockWindowEnumerator {
    /// Creates a new empty `MockWindowEnumerator`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or updates a window in the mock enumerator.
    pub fn add_window(&self, win: StageWindowInfo) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(pos) = inner.windows.iter().position(|w| w.id == win.id) {
            inner.windows[pos] = win;
        } else {
            inner.windows.push(win);
        }
    }

    /// Removes a window by ID.
    pub fn remove_window(&self, id: u64) {
        let mut inner = self.inner.lock().unwrap();
        inner.windows.retain(|w| w.id != id);
    }

    /// Returns recorded activation IDs.
    pub fn activated_ids(&self) -> Vec<u64> {
        self.inner.lock().unwrap().activated_ids.clone()
    }

    /// Returns recorded minimization IDs.
    pub fn minimized_ids(&self) -> Vec<u64> {
        self.inner.lock().unwrap().minimized_ids.clone()
    }

    /// Returns recorded restoration IDs.
    pub fn restored_ids(&self) -> Vec<u64> {
        self.inner.lock().unwrap().restored_ids.clone()
    }

    /// Clears recorded action history.
    pub fn clear_records(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.activated_ids.clear();
        inner.minimized_ids.clear();
        inner.restored_ids.clear();
    }

    /// Returns all currently tracked mock windows.
    pub fn windows(&self) -> Vec<StageWindowInfo> {
        self.inner.lock().unwrap().windows.clone()
    }
}

impl WindowEnumerator for MockWindowEnumerator {
    fn enumerate_windows(&self) -> Vec<StageWindowInfo> {
        self.inner.lock().unwrap().windows.clone()
    }

    fn activate_window(&self, id: u64) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        inner.activated_ids.push(id);
        if let Some(w) = inner.windows.iter_mut().find(|w| w.id == id) {
            w.is_minimized = false;
        }
        Ok(())
    }

    fn minimize_window(&self, id: u64) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        inner.minimized_ids.push(id);
        if let Some(w) = inner.windows.iter_mut().find(|w| w.id == id) {
            w.is_minimized = true;
        }
        Ok(())
    }

    fn restore_window(&self, id: u64) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        inner.restored_ids.push(id);
        if let Some(w) = inner.windows.iter_mut().find(|w| w.id == id) {
            w.is_minimized = false;
        }
        Ok(())
    }

    fn add_mock_window(&self, window: StageWindowInfo) {
        self.add_window(window);
    }

    fn remove_mock_window(&self, id: u64) {
        self.remove_window(id);
    }
}

// ---------------------------------------------------------------------------
// Win32 Window Enumerator
// ---------------------------------------------------------------------------

#[cfg(windows)]
#[derive(Debug, Default, Clone)]
pub struct Win32WindowEnumerator;

#[cfg(windows)]
impl Win32WindowEnumerator {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
impl WindowEnumerator for Win32WindowEnumerator {
    fn enumerate_windows(&self) -> Vec<StageWindowInfo> {
        win32_impl::enumerate_win32_windows()
    }

    fn activate_window(&self, id: u64) -> Result<(), String> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
        };

        if id == 0 {
            return Err("Invalid window handle: 0".to_string());
        }
        let hwnd = HWND(id as usize as *mut _);
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
            let _ = SetForegroundWindow(hwnd);
        }
        Ok(())
    }

    fn minimize_window(&self, id: u64) -> Result<(), String> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MINIMIZE};

        if id == 0 {
            return Err("Invalid window handle: 0".to_string());
        }
        let hwnd = HWND(id as usize as *mut _);
        unsafe {
            let _ = ShowWindow(hwnd, SW_MINIMIZE);
        }
        Ok(())
    }

    fn restore_window(&self, id: u64) -> Result<(), String> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_RESTORE};

        if id == 0 {
            return Err("Invalid window handle: 0".to_string());
        }
        let hwnd = HWND(id as usize as *mut _);
        unsafe {
            if windows::Win32::UI::WindowsAndMessaging::IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
mod win32_impl {
    use super::{fallback_app_name_from_title, is_stage_candidate};
    use mac_physics_core::StageWindowInfo;
    use std::path::Path;
    use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowLongPtrW, GetWindowPlacement, GetWindowRect,
        GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE,
        WINDOWPLACEMENT, WS_EX_TOOLWINDOW,
    };

    pub fn enumerate_win32_windows() -> Vec<StageWindowInfo> {
        struct Context {
            windows: Vec<StageWindowInfo>,
        }

        let mut context = Context {
            windows: Vec::new(),
        };

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let ctx = unsafe { &mut *(lparam.0 as *mut Context) };

            let is_visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
            if !is_visible {
                return BOOL(1);
            }

            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            if pid != 0 && pid == std::process::id() {
                return BOOL(1);
            }

            let is_minimized = unsafe { IsIconic(hwnd) }.as_bool();

            let (width, height) = if is_minimized {
                let mut placement = WINDOWPLACEMENT {
                    length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                    ..Default::default()
                };
                if unsafe { GetWindowPlacement(hwnd, &mut placement) }.is_ok() {
                    let w = (placement.rcNormalPosition.right - placement.rcNormalPosition.left).max(0) as u32;
                    let h = (placement.rcNormalPosition.bottom - placement.rcNormalPosition.top).max(0) as u32;
                    (w, h)
                } else {
                    (0, 0)
                }
            } else {
                let mut rect = RECT::default();
                if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
                    let w = (rect.right - rect.left).max(0) as u32;
                    let h = (rect.bottom - rect.top).max(0) as u32;
                    (w, h)
                } else {
                    (0, 0)
                }
            };

            let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
            let is_tool_window = (ex_style & (WS_EX_TOOLWINDOW.0 as isize)) != 0;

            let mut cloaked: u32 = 0;
            let hr = unsafe {
                DwmGetWindowAttribute(
                    hwnd,
                    DWMWA_CLOAKED,
                    &mut cloaked as *mut u32 as *mut std::ffi::c_void,
                    std::mem::size_of::<u32>() as u32,
                )
            };
            let is_cloaked = hr.is_ok() && cloaked != 0;

            let mut text_buf = [0u16; 512];
            let text_len = unsafe { GetWindowTextW(hwnd, &mut text_buf) };
            let title = if text_len > 0 {
                String::from_utf16_lossy(&text_buf[..text_len as usize])
            } else {
                String::new()
            };

            let mut class_buf = [0u16; 256];
            let class_len = unsafe { GetClassNameW(hwnd, &mut class_buf) };
            let class_name = if class_len > 0 {
                String::from_utf16_lossy(&class_buf[..class_len as usize])
            } else {
                String::new()
            };

            if !is_stage_candidate(
                &title,
                &class_name,
                width,
                height,
                is_tool_window,
                is_cloaked,
                is_visible,
            ) {
                return BOOL(1);
            }

            let mut app_name = String::new();
            if pid != 0 {
                if let Ok(handle) = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
                    let mut buf = [0u16; 1024];
                    let mut size = buf.len() as u32;
                    let query_res = unsafe {
                        QueryFullProcessImageNameW(
                            handle,
                            PROCESS_NAME_FORMAT(0),
                            windows::core::PWSTR(buf.as_mut_ptr()),
                            &mut size,
                        )
                    };
                    if query_res.is_ok() && size > 0 {
                        let path_str = String::from_utf16_lossy(&buf[..size as usize]);
                        let process_file = Path::new(&path_str)
                            .file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or(&path_str);
                        let clean = if process_file.to_lowercase().ends_with(".exe") {
                            &process_file[..process_file.len() - 4]
                        } else {
                            process_file
                        };
                        app_name = clean.to_string();
                    }
                    let _ = unsafe { CloseHandle(handle) };
                }
            }

            if app_name.is_empty() {
                app_name = fallback_app_name_from_title(&title);
            }

            ctx.windows.push(StageWindowInfo::new(
                hwnd.0 as usize as u64,
                title.trim(),
                app_name,
                width,
                height,
                is_minimized,
            ));

            BOOL(1)
        }

        unsafe {
            let _ = EnumWindows(
                Some(enum_proc),
                LPARAM(&mut context as *mut Context as isize),
            );
        }

        context.windows
    }
}

// ---------------------------------------------------------------------------
// Stage Tracker
// ---------------------------------------------------------------------------

/// Manages window enumeration, application grouping, and stage activation.
pub struct StageTracker {
    enumerator: Box<dyn WindowEnumerator>,
    groups: Vec<StageGroup>,
    active_group_id: Option<String>,
    previous_group_id: Option<String>,
}

impl StageTracker {
    /// Creates a new `StageTracker` backed by the provided `WindowEnumerator`.
    pub fn new(enumerator: Box<dyn WindowEnumerator>) -> Self {
        Self {
            enumerator,
            groups: Vec::new(),
            active_group_id: None,
            previous_group_id: None,
        }
    }

    /// Creates a mock-backed `StageTracker` for headless testing or non-Windows platforms.
    pub fn new_mock() -> Self {
        Self::new(Box::new(MockWindowEnumerator::new()))
    }

    /// Refreshes window enumeration and updates application groups.
    ///
    /// Groups windows by `app_name` into `StageGroup`s with `group_id = format!("group_{}", app_name.to_lowercase())`.
    /// Preserves previous active group if it still exists. If active group was None or disappeared,
    /// defaults to the first non-empty group.
    pub fn refresh(&mut self) {
        let windows = self.enumerator.enumerate_windows();
        let mut new_groups: Vec<StageGroup> = Vec::new();

        for win in windows {
            let group_id = format!("group_{}", win.app_name.to_lowercase());
            if let Some(existing) = new_groups.iter_mut().find(|g| g.group_id == group_id) {
                existing.windows.push(win);
            } else {
                new_groups.push(StageGroup::new(
                    group_id,
                    win.app_name.clone(),
                    vec![win],
                    0,
                ));
            }
        }

        let still_exists = if let Some(ref current_active) = self.active_group_id {
            new_groups.iter().any(|g| &g.group_id == current_active && !g.windows.is_empty())
        } else {
            false
        };

        if !still_exists {
            self.active_group_id = new_groups.iter().find(|g| !g.windows.is_empty()).map(|g| g.group_id.clone());
        }

        self.groups = new_groups;
    }

    /// Returns the currently active groups.
    pub fn groups(&self) -> &[StageGroup] {
        &self.groups
    }

    /// Returns a reference to the active `StageGroup`, if any.
    pub fn active_group(&self) -> Option<&StageGroup> {
        let id = self.active_group_id.as_deref()?;
        self.groups.iter().find(|g| g.group_id == id)
    }

    /// Returns the identifier of the currently active group.
    pub fn active_group_id(&self) -> Option<&str> {
        self.active_group_id.as_deref()
    }

    /// Returns the identifier of the previously active group, if any.
    pub fn previous_group_id(&self) -> Option<&str> {
        self.previous_group_id.as_deref()
    }

    /// Switches the active stage group to the specified `group_id`.
    ///
    /// Minimizes windows belonging to the previous active group, and restores/activates
    /// the target group's active window.
    pub fn set_active_group(&mut self, group_id: &str) -> Result<(), String> {
        let target_idx = self
            .groups
            .iter()
            .position(|g| g.group_id == group_id)
            .ok_or_else(|| format!("Stage group '{}' not found", group_id))?;

        if let Some(ref current_active) = self.active_group_id {
            if current_active != group_id {
                // Minimize windows belonging to the previous active group
                if let Some(prev_group) = self.groups.iter().find(|g| &g.group_id == current_active) {
                    for win in &prev_group.windows {
                        let _ = self.enumerator.minimize_window(win.id);
                    }
                }
                self.previous_group_id = Some(current_active.clone());
            }
        }

        let target_group = &self.groups[target_idx];
        for win in &target_group.windows {
            let _ = self.enumerator.restore_window(win.id);
        }
        if let Some(active_win) = target_group.active_window() {
            self.enumerator.activate_window(active_win.id)?;
        }

        self.active_group_id = Some(group_id.to_string());
        Ok(())
    }

    /// Adds a mock window for testing and immediately refreshes groups.
    pub fn add_mock_window(&mut self, window: StageWindowInfo) {
        self.enumerator.add_mock_window(window);
        self.refresh();
    }

    /// Sets the active window index within a specific group.
    pub fn set_group_active_window(&mut self, group_id: &str, window_id: u64) -> Result<(), String> {
        if let Some(group) = self.groups.iter_mut().find(|g| g.group_id == group_id) {
            if let Some(idx) = group.windows.iter().position(|w| w.id == window_id) {
                group.active_window_index = idx;
                return Ok(());
            }
            return Err(format!("Window {} not in group {}", window_id, group_id));
        }
        Err(format!("Group '{}' not found", group_id))
    }
}
