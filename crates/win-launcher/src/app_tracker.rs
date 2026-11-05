//! app_tracker
//! Win32 running application and foreground process tracker.
//!
//! Enumerates top-level visible application windows via `EnumWindows`, maps window handles
//! to process IDs and executable image paths, and detects active foreground focus.

use serde::{Deserialize, Serialize};

/// Information regarding a detected or mock running application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    pub process_name: String,
    pub pid: u32,
    pub is_focused: bool,
    pub hwnd: usize,
}

/// Tracks active processes and windows, identifying running apps and foreground focus.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppTracker {
    running_apps: Vec<AppInfo>,
    mock_apps: Vec<AppInfo>,
}

impl AppTracker {
    /// Creates a new `AppTracker`.
    pub fn new() -> Self {
        Self {
            running_apps: Vec::new(),
            mock_apps: Vec::new(),
        }
    }

    /// Registers a mock running application for headless testing or non-Windows environments.
    pub fn register_mock_running_app(&mut self, process_name: &str, pid: u32, is_focused: bool) {
        self.register_mock_running_app_with_hwnd(process_name, pid, is_focused, 0);
    }

    /// Registers a mock running application with an explicit window handle.
    pub fn register_mock_running_app_with_hwnd(
        &mut self,
        process_name: &str,
        pid: u32,
        is_focused: bool,
        hwnd: usize,
    ) {
        self.mock_apps.push(AppInfo {
            process_name: process_name.to_string(),
            pid,
            is_focused,
            hwnd,
        });
    }

    /// Clears any registered mock applications.
    pub fn clear_mock_apps(&mut self) {
        self.mock_apps.clear();
    }

    /// Refreshes running applications by querying the system window manager (on Windows)
    /// or maintaining mock state (on non-Windows).
    pub fn refresh_running_apps(&mut self) {
        #[cfg(windows)]
        {
            self.running_apps = win32_tracker::enumerate_running_windows();
        }
        #[cfg(not(windows))]
        {
            // Non-Windows: mock apps serve as running apps
        }
    }

    /// Returns `true` if an application with the specified process name or alias is running.
    pub fn is_app_running(&self, process_name: &str) -> bool {
        self.running_apps
            .iter()
            .chain(self.mock_apps.iter())
            .any(|app| names_match(&app.process_name, process_name))
    }

    /// Returns `true` if an application with the specified process name currently owns foreground focus.
    pub fn is_app_focused(&self, process_name: &str) -> bool {
        self.running_apps
            .iter()
            .chain(self.mock_apps.iter())
            .any(|app| app.is_focused && names_match(&app.process_name, process_name))
    }

    /// Returns the currently focused foreground application, if any.
    pub fn focused_app(&self) -> Option<&AppInfo> {
        self.running_apps
            .iter()
            .chain(self.mock_apps.iter())
            .find(|app| app.is_focused)
    }

    /// Returns all process IDs associated with the specified application name.
    pub fn get_running_pids(&self, process_name: &str) -> Vec<u32> {
        let mut pids: Vec<u32> = self
            .running_apps
            .iter()
            .chain(self.mock_apps.iter())
            .filter(|app| names_match(&app.process_name, process_name))
            .map(|app| app.pid)
            .collect();
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    /// Returns a slice of currently tracked running applications (including live and mock).
    pub fn all_apps(&self) -> Vec<AppInfo> {
        let mut apps = self.running_apps.clone();
        apps.extend(self.mock_apps.clone());
        apps
    }
}

/// Matches executable names case-insensitively, tolerating presence or absence of `.exe`
/// and full directory path prefixes (e.g. `C:\Windows\notepad.exe` matches `notepad.exe`).
pub(crate) fn names_match(registered_or_running: &str, query: &str) -> bool {
    fn extract_basename(s: &str) -> &str {
        let trimmed = s.trim();
        // Handle both Windows backslash and Unix forward slash path separators
        match trimmed.rfind(|c| c == '/' || c == '\\') {
            Some(idx) => &trimmed[idx + 1..],
            None => trimmed,
        }
    }

    let r_base = extract_basename(registered_or_running).to_ascii_lowercase();
    let q_base = extract_basename(query).to_ascii_lowercase();

    if r_base == q_base {
        return true;
    }
    let r_strip = r_base.strip_suffix(".exe").unwrap_or(&r_base);
    let q_strip = q_base.strip_suffix(".exe").unwrap_or(&q_base);
    r_strip == q_strip
}

// ---------------------------------------------------------------------------
// Win32 Process and Window Enumeration
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32_tracker {
    use super::AppInfo;
    use std::path::Path;
    use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, RECT};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowLongW, GetWindowRect,
        GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    };

    pub fn enumerate_running_windows() -> Vec<AppInfo> {
        let apps = Vec::new();
        let fg_hwnd = unsafe { GetForegroundWindow() };
        let mut fg_pid = 0u32;
        if fg_hwnd.0 != std::ptr::null_mut() {
            unsafe {
                GetWindowThreadProcessId(fg_hwnd, Some(&mut fg_pid));
            }
        }

        struct Context {
            apps: Vec<AppInfo>,
            fg_pid: u32,
        }

        let mut context = Context { apps, fg_pid };

        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let ctx = unsafe { &mut *(lparam.0 as *mut Context) };

            // Filter invisible windows
            if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
                return BOOL(1);
            }

            // Filter zero-sized windows
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;
                if width <= 0 || height <= 0 {
                    return BOOL(1);
                }
            }

            // Filter tool windows (e.g. system trays, floating toolbars)
            let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) };
            if (ex_style & (WS_EX_TOOLWINDOW.0 as i32)) != 0 {
                return BOOL(1);
            }

            let mut pid = 0u32;
            unsafe {
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
            }
            if pid == 0 {
                return BOOL(1);
            }

            let is_focused = pid == ctx.fg_pid;

            // Query process executable image name
            if let Ok(handle) = unsafe {
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            } {
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
                    let process_name = Path::new(&path_str)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or(&path_str)
                        .to_string();

                    ctx.apps.push(AppInfo {
                        process_name,
                        pid,
                        is_focused,
                        hwnd: hwnd.0 as usize,
                    });
                }

                let _ = unsafe { CloseHandle(handle) };
            }

            BOOL(1)
        }

        unsafe {
            let _ = EnumWindows(
                Some(enum_proc),
                LPARAM(&mut context as *mut Context as isize),
            );
        }

        context.apps
    }
}
