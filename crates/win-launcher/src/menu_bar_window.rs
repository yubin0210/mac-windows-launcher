//! menu_bar_window
//! Win32 AppBar reserved top edge window and cross-platform message pump.
//!
//! Windows: Registers as an AppBar with `SHAppBarMessage(ABM_NEW)` and `ABM_SETPOS`
//! with `ABE_TOP`, reserving the top 28px work area so maximized windows do not overlap
//! the macOS Top Menu Bar. Fail-safe RAII `Drop` automatically sends `ABM_REMOVE`
//! to restore the desktop work area.
//! Non-Windows: High-fidelity mock tracking window state, dimensions, and event queues.

use std::collections::VecDeque;

/// Window events received and processed by the Menu Bar subsystem.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuBarWindowEvent {
    MouseMove { x: f32, y: f32 },
    MouseDown { x: f32, y: f32 },
    MouseUp { x: f32, y: f32 },
    MouseLeave,
    KillFocus,
    AppBarPosChanged,
    Paint,
    Destroy,
}

/// Configuration settings for creating a `MenuBarWindow`.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuBarWindowConfig {
    pub width: u32,
    pub height: u32,
    pub title: String,
    pub class_name: String,
    pub reserve_appbar: bool,
}

impl Default for MenuBarWindowConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 28,
            title: "macOS Menu Bar".to_string(),
            class_name: "MacWinLauncherMenuBarClass".to_string(),
            reserve_appbar: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Windows Implementation (DirectComposition Topmost AppBar Window)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32 {
    use super::*;
    use std::sync::{Arc, Mutex};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{UpdateWindow, ValidateRect};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Controls::WM_MOUSELEAVE;
    use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
    use windows::Win32::UI::Shell::{
        SHAppBarMessage, ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS, ABN_POSCHANGED,
        APPBARDATA,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetSystemMetrics,
        GetWindowLongPtrW, MoveWindow, PeekMessageW, PostQuitMessage, RegisterClassExW,
        SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
        GWLP_USERDATA, HWND_TOPMOST, MSG, PM_REMOVE, SM_CXSCREEN, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOZORDER, SW_SHOW, WNDCLASSEXW, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP,
    };

    pub const WM_APPBAR_CALLBACK: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 201;

    struct WindowSharedState {
        events: Mutex<VecDeque<MenuBarWindowEvent>>,
        is_tracking_leave: Mutex<bool>,
        config: MenuBarWindowConfig,
    }

    pub struct Win32MenuBarWindow {
        hwnd: HWND,
        config: MenuBarWindowConfig,
        shared_state: Arc<WindowSharedState>,
        is_appbar_registered: bool,
        is_open: bool,
    }

    unsafe extern "system" fn menu_bar_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        use windows::Win32::UI::WindowsAndMessaging::{
            WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT,
        };

        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowSharedState;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        let state = &*ptr;

        match msg {
            WM_MOUSEMOVE => {
                let x = (lparam.0 & 0xFFFF) as i16 as f32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;

                if let Ok(mut is_tracking) = state.is_tracking_leave.lock() {
                    if !*is_tracking {
                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        if TrackMouseEvent(&mut tme).is_ok() {
                            *is_tracking = true;
                        }
                    }
                }

                if let Ok(mut events) = state.events.lock() {
                    events.push_back(MenuBarWindowEvent::MouseMove { x, y });
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                if let Ok(mut is_tracking) = state.is_tracking_leave.lock() {
                    *is_tracking = false;
                }
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(MenuBarWindowEvent::MouseLeave);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let x = (lparam.0 & 0xFFFF) as i16 as f32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(MenuBarWindowEvent::MouseDown { x, y });
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let x = (lparam.0 & 0xFFFF) as i16 as f32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(MenuBarWindowEvent::MouseUp { x, y });
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let _ = ValidateRect(hwnd, None);
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(MenuBarWindowEvent::Paint);
                }
                LRESULT(0)
            }
            windows::Win32::UI::WindowsAndMessaging::WM_KILLFOCUS => {
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(MenuBarWindowEvent::KillFocus);
                }
                LRESULT(0)
            }
            WM_APPBAR_CALLBACK => {
                if wparam.0 == ABN_POSCHANGED as usize {
                    // System or taskbar work area shifted, recalculate AppBar position
                    let mut abd = APPBARDATA {
                        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
                        hWnd: hwnd,
                        uCallbackMessage: WM_APPBAR_CALLBACK,
                        uEdge: ABE_TOP,
                        rc: RECT {
                            left: 0,
                            top: 0,
                            right: GetSystemMetrics(SM_CXSCREEN),
                            bottom: state.config.height as i32,
                        },
                        lParam: LPARAM(0),
                    };
                    let _ = SHAppBarMessage(ABM_QUERYPOS, &mut abd);
                    abd.rc.top = 0;
                    abd.rc.bottom = state.config.height as i32;
                    let _ = SHAppBarMessage(ABM_SETPOS, &mut abd);
                    let _ = MoveWindow(
                        hwnd,
                        abd.rc.left,
                        abd.rc.top,
                        abd.rc.right - abd.rc.left,
                        abd.rc.bottom - abd.rc.top,
                        true,
                    );
                    if let Ok(mut events) = state.events.lock() {
                        events.push_back(MenuBarWindowEvent::AppBarPosChanged);
                    }
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(MenuBarWindowEvent::Destroy);
                }
                let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    impl Win32MenuBarWindow {
        pub fn create(config: MenuBarWindowConfig) -> Result<Self, String> {
            let instance = unsafe {
                GetModuleHandleW(None).map_err(|e| format!("Failed to get module handle: {e}"))?
            };

            let class_name_wide: Vec<u16> = config
                .class_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let title_wide: Vec<u16> = config
                .title
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let wcex = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(menu_bar_wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance.into(),
                hIcon: Default::default(),
                hCursor: Default::default(),
                hbrBackground: Default::default(),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: PCWSTR(class_name_wide.as_ptr()),
                hIconSm: Default::default(),
            };

            unsafe {
                let _ = RegisterClassExW(&wcex);
            }

            let screen_width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let width = if config.width > 0 {
                config.width as i32
            } else {
                screen_width
            };
            let height = config.height as i32;

            let shared_state = Arc::new(WindowSharedState {
                events: Mutex::new(VecDeque::new()),
                is_tracking_leave: Mutex::new(false),
                config: config.clone(),
            });

            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP,
                    PCWSTR(class_name_wide.as_ptr()),
                    PCWSTR(title_wide.as_ptr()),
                    WS_POPUP,
                    0,
                    0,
                    width,
                    height,
                    None,
                    None,
                    instance,
                    None,
                )
                .map_err(|e| format!("Failed to create menu bar window: {e}"))?
            };

            let state_ptr = Arc::as_ptr(&shared_state) as isize;
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr);
            }

            // Register Win32 AppBar to reserve top 28px work area if requested
            let mut is_appbar_registered = false;
            if config.reserve_appbar {
                let mut abd = APPBARDATA {
                    cbSize: std::mem::size_of::<APPBARDATA>() as u32,
                    hWnd: hwnd,
                    uCallbackMessage: WM_APPBAR_CALLBACK,
                    uEdge: ABE_TOP,
                    rc: RECT {
                        left: 0,
                        top: 0,
                        right: width,
                        bottom: height,
                    },
                    lParam: LPARAM(0),
                };

                let res = unsafe { SHAppBarMessage(ABM_NEW, &mut abd) };
                if res != 0 {
                    is_appbar_registered = true;
                    unsafe {
                        let _ = SHAppBarMessage(ABM_QUERYPOS, &mut abd);
                        abd.rc.top = 0;
                        abd.rc.bottom = height;
                        let _ = SHAppBarMessage(ABM_SETPOS, &mut abd);
                        let _ = MoveWindow(
                            hwnd,
                            abd.rc.left,
                            abd.rc.top,
                            abd.rc.right - abd.rc.left,
                            abd.rc.bottom - abd.rc.top,
                            true,
                        );
                    }
                }
            }

            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = UpdateWindow(hwnd);
            }

            Ok(Self {
                hwnd,
                config,
                shared_state,
                is_appbar_registered,
                is_open: true,
            })
        }

        pub fn poll_events(&mut self) -> Vec<MenuBarWindowEvent> {
            let mut msg = MSG::default();
            unsafe {
                while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            let mut out = Vec::new();
            if let Ok(mut events) = self.shared_state.events.lock() {
                while let Some(e) = events.pop_front() {
                    if e == MenuBarWindowEvent::Destroy {
                        self.is_open = false;
                    }
                    out.push(e);
                }
            }
            out
        }

        pub fn is_open(&self) -> bool {
            self.is_open
        }

        pub fn hwnd(&self) -> isize {
            self.hwnd.0 as isize
        }

        pub fn width(&self) -> u32 {
            self.config.width
        }

        pub fn height(&self) -> u32 {
            self.config.height
        }

        pub fn set_height(&mut self, height: u32) {
            self.config.height = height;
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    self.config.width as i32,
                    height as i32,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }

        pub fn reset_height(&mut self) {
            self.set_height(28);
        }

        pub fn is_appbar_registered(&self) -> bool {
            self.is_appbar_registered
        }
    }

    impl Drop for Win32MenuBarWindow {
        fn drop(&mut self) {
            // Restore desktop work area before destroying window
            if self.is_appbar_registered {
                let mut abd = APPBARDATA {
                    cbSize: std::mem::size_of::<APPBARDATA>() as u32,
                    hWnd: self.hwnd,
                    ..Default::default()
                };
                unsafe {
                    let _ = SHAppBarMessage(ABM_REMOVE, &mut abd);
                }
                self.is_appbar_registered = false;
            }

            if !self.hwnd.is_invalid() {
                unsafe {
                    let _ = DestroyWindow(self.hwnd);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Mock Implementation (Cross-Platform Dev & Tests)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct MockMenuBarWindow {
    config: MenuBarWindowConfig,
    events: VecDeque<MenuBarWindowEvent>,
    is_appbar_registered: bool,
    is_open: bool,
}

impl MockMenuBarWindow {
    pub fn new(config: MenuBarWindowConfig) -> Self {
        let is_appbar = config.reserve_appbar;
        Self {
            config,
            events: VecDeque::new(),
            is_appbar_registered: is_appbar,
            is_open: true,
        }
    }

    pub fn poll_events(&mut self) -> Vec<MenuBarWindowEvent> {
        let mut out = Vec::new();
        while let Some(e) = self.events.pop_front() {
            if e == MenuBarWindowEvent::Destroy {
                self.is_open = false;
            }
            out.push(e);
        }
        out
    }

    pub fn push_event(&mut self, event: MenuBarWindowEvent) {
        self.events.push_back(event);
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn hwnd(&self) -> isize {
        0x1001
    }

    pub fn width(&self) -> u32 {
        self.config.width
    }

    pub fn height(&self) -> u32 {
        self.config.height
    }

    pub fn set_height(&mut self, height: u32) {
        self.config.height = height;
    }

    pub fn reset_height(&mut self) {
        self.set_height(28);
    }

    pub fn is_appbar_registered(&self) -> bool {
        self.is_appbar_registered
    }
}

impl Drop for MockMenuBarWindow {
    fn drop(&mut self) {
        self.is_appbar_registered = false;
        self.is_open = false;
    }
}

// ---------------------------------------------------------------------------
// Unified MenuBarWindow Interface
// ---------------------------------------------------------------------------

enum MenuBarWindowInner {
    #[cfg(windows)]
    Real(win32::Win32MenuBarWindow),
    Mock(MockMenuBarWindow),
}

pub struct MenuBarWindow {
    inner: MenuBarWindowInner,
}

impl MenuBarWindow {
    /// Creates a mock MenuBarWindow for cross-platform development and headless tests.
    pub fn new_mock() -> Self {
        Self::new_mock_with_config(MenuBarWindowConfig::default())
    }

    /// Creates a mock MenuBarWindow with custom configuration.
    pub fn new_mock_with_config(config: MenuBarWindowConfig) -> Self {
        Self {
            inner: MenuBarWindowInner::Mock(MockMenuBarWindow::new(config)),
        }
    }

    /// Creates a MenuBarWindow with specified configuration (alias for `new`).
    pub fn create(config: MenuBarWindowConfig) -> Result<Self, String> {
        Self::new(config)
    }

    /// Creates a MenuBarWindow with specified configuration.
    pub fn new(config: MenuBarWindowConfig) -> Result<Self, String> {
        #[cfg(windows)]
        {
            let win = win32::Win32MenuBarWindow::create(config)?;
            Ok(Self {
                inner: MenuBarWindowInner::Real(win),
            })
        }
        #[cfg(not(windows))]
        {
            Ok(Self {
                inner: MenuBarWindowInner::Mock(MockMenuBarWindow::new(config)),
            })
        }
    }

    /// Polls pending window and AppBar events.
    pub fn poll_events(&mut self) -> Vec<MenuBarWindowEvent> {
        match &mut self.inner {
            #[cfg(windows)]
            MenuBarWindowInner::Real(win) => win.poll_events(),
            MenuBarWindowInner::Mock(mock) => mock.poll_events(),
        }
    }

    /// Pushes a mock event (available in tests and development).
    pub fn push_mock_event(&mut self, event: MenuBarWindowEvent) {
        match &mut self.inner {
            #[cfg(windows)]
            MenuBarWindowInner::Real(_) => {}
            MenuBarWindowInner::Mock(mock) => mock.push_event(event),
        }
    }

    /// Returns whether the window is currently open.
    pub fn is_open(&self) -> bool {
        match &self.inner {
            #[cfg(windows)]
            MenuBarWindowInner::Real(win) => win.is_open(),
            MenuBarWindowInner::Mock(mock) => mock.is_open(),
        }
    }

    /// Returns the window HWND handle as an isize.
    pub fn hwnd(&self) -> isize {
        match &self.inner {
            #[cfg(windows)]
            MenuBarWindowInner::Real(win) => win.hwnd(),
            MenuBarWindowInner::Mock(mock) => mock.hwnd(),
        }
    }

    /// Returns window width in pixels.
    pub fn width(&self) -> u32 {
        match &self.inner {
            #[cfg(windows)]
            MenuBarWindowInner::Real(win) => win.width(),
            MenuBarWindowInner::Mock(mock) => mock.width(),
        }
    }

    /// Returns window height in pixels (standard macOS 28px).
    pub fn height(&self) -> u32 {
        match &self.inner {
            #[cfg(windows)]
            MenuBarWindowInner::Real(win) => win.height(),
            MenuBarWindowInner::Mock(mock) => mock.height(),
        }
    }

    /// Dynamically resizes the window height (e.g. to expand mouse hit testing when a dropdown opens).
    pub fn set_height(&mut self, height: u32) {
        match &mut self.inner {
            #[cfg(windows)]
            MenuBarWindowInner::Real(win) => win.set_height(height),
            MenuBarWindowInner::Mock(mock) => mock.set_height(height),
        }
    }

    /// Resets the window height back to default 28px.
    pub fn reset_height(&mut self) {
        match &mut self.inner {
            #[cfg(windows)]
            MenuBarWindowInner::Real(win) => win.reset_height(),
            MenuBarWindowInner::Mock(mock) => mock.reset_height(),
        }
    }

    /// Returns whether the window successfully registered as an AppBar with top work area reservation.
    pub fn is_appbar_registered(&self) -> bool {
        match &self.inner {
            #[cfg(windows)]
            MenuBarWindowInner::Real(win) => win.is_appbar_registered(),
            MenuBarWindowInner::Mock(mock) => mock.is_appbar_registered(),
        }
    }
}
