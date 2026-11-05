//! taskbar_guard
//! Windows taskbar auto-hide and fail-safe Drop restoration guard.
//!
//! # Safety Invariant
//! The Windows taskbar must NEVER remain hidden permanently if the launcher
//! exits, is dropped, or encounters a panic. `TaskbarGuard` implements `Drop`
//! to guarantee `ShowWindow(SW_SHOW)` and desktop work area refresh on cleanup.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, FindWindowW, SetWindowPos, ShowWindow, SWP_FRAMECHANGED, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOW,
};

/// Fail-safe guard that manages hiding the Windows taskbar while the launcher
/// is running, and guarantees its restoration on drop or manual restoration.
pub struct TaskbarGuard {
    is_hidden: bool,
    #[cfg(windows)]
    hidden_hwnds: Vec<HWND>,
    restoration_tracker: Option<Arc<AtomicBool>>,
    restore_callback: Option<Box<dyn FnOnce() + Send + 'static>>,
    is_mock: bool,
}

impl fmt::Debug for TaskbarGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaskbarGuard")
            .field("is_hidden", &self.is_hidden)
            .field("is_mock", &self.is_mock)
            .finish()
    }
}

impl TaskbarGuard {
    /// Creates a new `TaskbarGuard`.
    ///
    /// If `hide_taskbar` is true:
    /// - On Windows: finds primary (`Shell_TrayWnd`) and secondary (`Shell_SecondaryTrayWnd`)
    ///   taskbars and hides them via `ShowWindow(SW_HIDE)`.
    /// - On non-Windows: simulates hiding the taskbar cleanly.
    ///
    /// When this guard is dropped or `restore()` is called, all hidden windows
    /// are restored via `ShowWindow(SW_SHOW)` and their work area is refreshed.
    pub fn new(hide_taskbar: bool) -> Self {
        #[cfg(windows)]
        {
            if hide_taskbar {
                let hwnds = find_taskbar_windows();
                for &hwnd in &hwnds {
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    }
                }
                Self {
                    is_hidden: true,
                    hidden_hwnds: hwnds,
                    restoration_tracker: Some(Arc::new(AtomicBool::new(false))),
                    restore_callback: None,
                    is_mock: false,
                }
            } else {
                Self {
                    is_hidden: false,
                    hidden_hwnds: Vec::new(),
                    restoration_tracker: Some(Arc::new(AtomicBool::new(false))),
                    restore_callback: None,
                    is_mock: false,
                }
            }
        }

        #[cfg(not(windows))]
        {
            Self {
                is_hidden: hide_taskbar,
                restoration_tracker: Some(Arc::new(AtomicBool::new(false))),
                restore_callback: None,
                is_mock: true,
            }
        }
    }

    /// Creates a mock `TaskbarGuard` for testing without interacting with real OS windows.
    pub fn new_mock(hide_taskbar: bool) -> Self {
        Self {
            is_hidden: hide_taskbar,
            #[cfg(windows)]
            hidden_hwnds: Vec::new(),
            restoration_tracker: Some(Arc::new(AtomicBool::new(false))),
            restore_callback: None,
            is_mock: true,
        }
    }

    /// Creates a mock `TaskbarGuard` with an associated one-shot restoration callback.
    pub fn new_mock_with_callback<F>(hide_taskbar: bool, callback: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Self {
            is_hidden: hide_taskbar,
            #[cfg(windows)]
            hidden_hwnds: Vec::new(),
            restoration_tracker: Some(Arc::new(AtomicBool::new(false))),
            restore_callback: Some(Box::new(callback)),
            is_mock: true,
        }
    }

    /// Returns true if the taskbar is currently marked as hidden by this guard.
    pub fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    /// Registers or replaces an optional one-shot callback to be invoked upon restoration.
    pub fn set_restore_callback<F>(&mut self, callback: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.restore_callback = Some(Box::new(callback));
    }

    /// Returns a shared atomic boolean tracker that indicates whether restoration has occurred.
    pub fn restoration_tracker(&self) -> Option<Arc<AtomicBool>> {
        self.restoration_tracker.clone()
    }

    /// Returns the number of hidden taskbar windows managed by this guard.
    pub fn hidden_window_count(&self) -> usize {
        #[cfg(windows)]
        {
            if !self.is_mock {
                return self.hidden_hwnds.len();
            }
        }
        if self.is_hidden {
            1
        } else {
            0
        }
    }

    /// Restores any hidden taskbar windows immediately and idempotently.
    pub fn restore(&mut self) {
        if !self.is_hidden {
            return;
        }

        #[cfg(windows)]
        {
            if !self.is_mock {
                for &hwnd in &self.hidden_hwnds {
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_SHOW);
                        let _ = SetWindowPos(
                            hwnd,
                            HWND(std::ptr::null_mut()),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                        );
                    }
                }
                self.hidden_hwnds.clear();
            }
        }

        if let Some(ref tracker) = self.restoration_tracker {
            tracker.store(true, Ordering::SeqCst);
        }

        if let Some(callback) = self.restore_callback.take() {
            callback();
        }

        self.is_hidden = false;
    }
}

impl Drop for TaskbarGuard {
    fn drop(&mut self) {
        self.restore();
    }
}

// ---------------------------------------------------------------------------
// Windows Window Enumeration Helpers
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn find_taskbar_windows() -> Vec<HWND> {
    let mut hwnds = Vec::new();

    unsafe {
        // 1. Primary taskbar
        if let Ok(primary) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
            if !primary.0.is_null() {
                hwnds.push(primary);
            }
        }

        // 2. Secondary taskbars across all active monitors
        let mut prev = HWND(std::ptr::null_mut());
        loop {
            match FindWindowExW(
                HWND(std::ptr::null_mut()),
                prev,
                w!("Shell_SecondaryTrayWnd"),
                PCWSTR::null(),
            ) {
                Ok(secondary) if !secondary.0.is_null() => {
                    hwnds.push(secondary);
                    prev = secondary;
                }
                _ => break,
            }
        }
    }

    hwnds
}
