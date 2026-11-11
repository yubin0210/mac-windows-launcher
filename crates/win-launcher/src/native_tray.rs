//! Notification-area lifetime and explicit exit/log access.
use std::path::PathBuf;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, POINT, WPARAM, LPARAM};
use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW};
use windows::Win32::UI::WindowsAndMessaging::*;

pub const TRAY_MESSAGE: u32 = WM_APP + 77;
pub const OPEN_SPOTLIGHT_MESSAGE: u32 = WM_APP + 78;
pub struct TrayIcon { data: NOTIFYICONDATAW }
impl TrayIcon {
    pub fn new(hwnd: HWND) -> Result<Self, String> {
        let mut data = NOTIFYICONDATAW::default();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = TRAY_MESSAGE;
        data.hIcon = unsafe { LoadIconW(None, IDI_APPLICATION) }.map_err(|e| e.to_string())?;
        let tip: Vec<u16> = "MacLauncher — right-click to exit".encode_utf16().collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        let icon = Self { data };
        icon.readd()?;
        Ok(icon)
    }
    pub fn readd(&self) -> Result<(), String> {
        if !unsafe { Shell_NotifyIconW(NIM_ADD, &self.data) }.as_bool() {
            return Err("Cannot create notification-area icon; Windows Explorer may be unavailable".into());
        }
        Ok(())
    }
}
impl Drop for TrayIcon { fn drop(&mut self) { unsafe { let _ = Shell_NotifyIconW(NIM_DELETE, &self.data); } } }

pub fn show_menu(hwnd: HWND) {
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return; };
        let _ = AppendMenuW(menu, MF_STRING, 3, w!("Open Spotlight"));
        let _ = AppendMenuW(menu, MF_STRING, 1, w!("Open logs"));
        let _ = AppendMenuW(menu, MF_STRING, 2, w!("Exit MacLauncher"));
        let mut point = POINT::default();
        let _ = GetCursorPos(&mut point);
        let previous = GetForegroundWindow();
        let _ = SetForegroundWindow(hwnd);
        let command = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, None).0;
        let _ = DestroyMenu(menu);
        if GetForegroundWindow() == hwnd && previous != hwnd { let _ = SetForegroundWindow(previous); }
        if command == 3 { let _ = PostMessageW(hwnd, OPEN_SPOTLIGHT_MESSAGE, WPARAM(0), LPARAM(0)); }
        if command == 2 { let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)); }
        if command == 1 {
            if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                let path = PathBuf::from(local).join("MacLauncher").join("logs");
                let path: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect();
                windows::Win32::UI::Shell::ShellExecuteW(hwnd, w!("open"), PCWSTR(path.as_ptr()), None, None, SW_SHOWNORMAL);
            }
        }
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
    }
}
