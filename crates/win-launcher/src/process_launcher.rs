//! process_launcher
//! Win32 native application launcher and foreground window activator.
//!
//! If a requested target application is already running, its existing top-level window
//! is brought to the foreground and restored via `ShowWindow(SW_RESTORE)` and `SetForegroundWindow`.
//! If the application is not running, it is spawned cleanly and detached via `ShellExecuteExW`.

use crate::app_tracker::AppTracker;
use serde::{Deserialize, Serialize};

/// Formats a list of command-line arguments into a single string for Windows ShellExecute,
/// wrapping any argument containing spaces or tabs in quotes.
pub fn format_arguments(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if (arg.contains(' ') || arg.contains('\t'))
                && !arg.starts_with('"')
                && !arg.ends_with('"')
            {
                format!("\"{}\"", arg)
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Target specification for launching or activating an application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaunchTarget {
    /// Native executable with process name for tracking, command, and arguments.
    AppExecutable {
        process_name: String,
        command: String,
        args: Vec<String>,
    },
    /// Protocol URI handler (e.g. `ms-settings:`, `bingmaps:`, `https://...`).
    ProtocolUri(String),
    /// Shell action or command string.
    ShellAction(String),
}

impl LaunchTarget {
    /// Creates an `AppExecutable` target without arguments.
    pub fn app(process_name: impl Into<String>, command: impl Into<String>) -> Self {
        Self::AppExecutable {
            process_name: process_name.into(),
            command: command.into(),
            args: Vec::new(),
        }
    }

    /// Creates an `AppExecutable` target with arguments.
    pub fn app_with_args(
        process_name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
    ) -> Self {
        Self::AppExecutable {
            process_name: process_name.into(),
            command: command.into(),
            args,
        }
    }

    /// Creates a `ProtocolUri` target.
    pub fn protocol(uri: impl Into<String>) -> Self {
        Self::ProtocolUri(uri.into())
    }

    /// Creates a `ShellAction` target.
    pub fn shell(action: impl Into<String>) -> Self {
        Self::ShellAction(action.into())
    }

    /// Returns the descriptive target identifier or executable name.
    pub fn name(&self) -> &str {
        match self {
            Self::AppExecutable { process_name, .. } => process_name,
            Self::ProtocolUri(uri) => uri,
            Self::ShellAction(action) => action,
        }
    }
}

/// Outcome of a launch or activation attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaunchResult {
    /// Process was newly launched.
    Launched {
        process_name: String,
        pid: Option<u32>,
    },
    /// Existing window of a running process was activated and focused.
    Activated {
        process_name: String,
        hwnd: usize,
    },
    /// Launch or activation failed with an error message.
    Failed {
        target: String,
        error: String,
    },
}

/// Executes application launch or window focus activation.
pub struct ProcessLauncher;

impl ProcessLauncher {
    #[cfg(windows)]
    pub fn activate_window(hwnd: usize, expected_pid: u32) -> Result<(), String> {
        win32_launcher::activate_window(hwnd, expected_pid)
    }

    /// Launches or brings an existing instance of `target` to the foreground.
    pub fn launch_or_activate(target: &LaunchTarget, tracker: &AppTracker) -> LaunchResult {
        // Validate target
        match target {
            LaunchTarget::AppExecutable {
                process_name,
                command,
                ..
            } => {
                if command.trim().is_empty() {
                    return LaunchResult::Failed {
                        target: if !process_name.is_empty() {
                            process_name.clone()
                        } else {
                            String::new()
                        },
                        error: "Executable command path cannot be empty".to_string(),
                    };
                }
                if process_name.trim().is_empty() {
                    return LaunchResult::Failed {
                        target: command.clone(),
                        error: "Target executable name cannot be empty".to_string(),
                    };
                }
            }
            LaunchTarget::ProtocolUri(uri) => {
                if uri.trim().is_empty() {
                    return LaunchResult::Failed {
                        target: String::new(),
                        error: "Protocol URI is empty".to_string(),
                    };
                }
            }
            LaunchTarget::ShellAction(action) => {
                if action.trim().is_empty() {
                    return LaunchResult::Failed {
                        target: String::new(),
                        error: "Shell action is empty".to_string(),
                    };
                }
            }
        }

        // Determine if target is already running
        let running_app = match target {
            LaunchTarget::AppExecutable { process_name, .. } => {
                tracker.all_apps().into_iter().find(|app| {
                    crate::app_tracker::names_match(&app.process_name, process_name)
                })
            }
            LaunchTarget::ProtocolUri(uri) => {
                tracker.all_apps().into_iter().find(|app| {
                    crate::app_tracker::names_match(&app.process_name, uri)
                })
            }
            LaunchTarget::ShellAction(action) => {
                tracker.all_apps().into_iter().find(|app| {
                    crate::app_tracker::names_match(&app.process_name, action)
                })
            }
        };

        if let Some(app) = running_app {
            let hwnd = if app.hwnd != 0 {
                app.hwnd
            } else {
                app.pid as usize
            };

            #[cfg(windows)]
            {
                if let Err(error) = Self::activate_window(app.hwnd, app.pid) {
                    return LaunchResult::Failed { target: target.name().into(), error };
                }
            }

            return LaunchResult::Activated {
                process_name: match target {
                    LaunchTarget::AppExecutable { process_name, .. } => process_name.clone(),
                    _ => app.process_name,
                },
                hwnd,
            };
        }

        // If not running, spawn process
        #[cfg(windows)]
        {
            win32_launcher::spawn_target(target)
        }
        #[cfg(not(windows))]
        {
            non_windows_launcher::spawn_target(target)
        }
    }
}

// ---------------------------------------------------------------------------
// Non-Windows Mock / Simulated Launcher
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod non_windows_launcher {
    use super::{LaunchResult, LaunchTarget};

    pub fn spawn_target(target: &LaunchTarget) -> LaunchResult {
        match target {
            LaunchTarget::AppExecutable {
                process_name,
                command,
                ..
            } => {
                let name = if !process_name.is_empty() {
                    process_name.clone()
                } else {
                    command.clone()
                };
                LaunchResult::Launched {
                    process_name: name,
                    pid: None,
                }
            }
            LaunchTarget::ProtocolUri(uri) => LaunchResult::Launched {
                process_name: uri.clone(),
                pid: None,
            },
            LaunchTarget::ShellAction(action) => LaunchResult::Launched {
                process_name: action.clone(),
                pid: None,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Win32 Native Launcher Implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32_launcher {
    use super::{LaunchResult, LaunchTarget};
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::Threading::GetProcessId;
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOWNORMAL,
    };

    pub fn activate_window(hwnd_val: usize, expected_pid: u32) -> Result<(), String> {
        use windows::Win32::UI::WindowsAndMessaging::{IsWindow, GetWindowThreadProcessId, GetForegroundWindow};
        let hwnd = HWND(hwnd_val as *mut _);
        unsafe {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if !IsWindow(hwnd).as_bool() || pid == 0 || pid != expected_pid {
                return Err("The selected window has closed or changed. Try again.".into());
            }
            if windows::Win32::UI::WindowsAndMessaging::IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            if !SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() != hwnd {
                return Err("Windows did not allow this window to become active.".into());
            }
        }
        Ok(())
    }

    pub fn spawn_target(target: &LaunchTarget) -> LaunchResult {
        let (file, params) = match target {
            LaunchTarget::AppExecutable { command, args, .. } => {
                let joined = super::format_arguments(args);
                (
                    command.clone(),
                    if joined.is_empty() { None } else { Some(joined) },
                )
            }
            LaunchTarget::ProtocolUri(uri) => (uri.clone(), None),
            LaunchTarget::ShellAction(action) => (action.clone(), None),
        };

        let wide_file: Vec<u16> = std::ffi::OsStr::new(&file)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let wide_params: Option<Vec<u16>> = params.map(|p| {
            std::ffi::OsStr::new(&p)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        });

        let mut info = SHELLEXECUTEINFOW::default();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.hwnd = HWND::default();
        info.lpFile = PCWSTR(wide_file.as_ptr());
        if let Some(ref p) = wide_params {
            info.lpParameters = PCWSTR(p.as_ptr());
        }
        info.nShow = SW_SHOWNORMAL.0 as i32;

        match unsafe { ShellExecuteExW(&mut info) } {
            Ok(()) => {
                let pid = if !info.hProcess.is_invalid() {
                    let p = unsafe { GetProcessId(info.hProcess) };
                    let _ = unsafe { CloseHandle(info.hProcess) };
                    if p != 0 {
                        Some(p)
                    } else {
                        None
                    }
                } else {
                    None
                };

                LaunchResult::Launched {
                    process_name: target.name().to_string(),
                    pid,
                }
            }
            Err(e) => LaunchResult::Failed {
                target: target.name().to_string(),
                error: e.to_string(),
            },
        }
    }
}
