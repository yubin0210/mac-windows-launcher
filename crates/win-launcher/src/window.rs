//! window
//! Win32 layered topmost window and cross-platform message pump.

use std::collections::VecDeque;

/// Window events received and processed by the dock launcher.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowEvent {
    MouseMove { x: f32, y: f32 },
    MouseLeave,
    MouseClick { x: f32, y: f32 },
    Paint,
    Destroy,
}

/// Configuration settings for creating a `DockWindow`.
#[derive(Debug, Clone, PartialEq)]
pub struct DockWindowConfig {
    pub width: i32,
    pub height: i32,
    pub title: String,
    pub class_name: String,
}

impl Default for DockWindowConfig {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 180,
            title: "macOS Dock".to_string(),
            class_name: "MacWinLauncherDockClass".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Windows Implementation (DirectComposition Layered Topmost Window)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32 {
    use super::*;
    use std::sync::{Arc, Mutex};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Dwm::DwmFlush;
    use windows::Win32::Graphics::Gdi::{UpdateWindow, ValidateRect};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetSystemMetrics,
        GetWindowLongPtrW, PeekMessageW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW,
        ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, MSG, PM_REMOVE,
        SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, WNDCLASSEXW, WS_EX_LAYERED,
        WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    };

    struct WindowSharedState {
        events: Mutex<VecDeque<WindowEvent>>,
        is_tracking_leave: Mutex<bool>,
    }

    pub struct Win32DockWindow {
        hwnd: HWND,
        config: DockWindowConfig,
        shared_state: Arc<WindowSharedState>,
        is_open: bool,
    }

    unsafe extern "system" fn dock_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        use windows::Win32::UI::Controls::WM_MOUSELEAVE;
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
                    events.push_back(WindowEvent::MouseMove { x, y });
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                if let Ok(mut is_tracking) = state.is_tracking_leave.lock() {
                    *is_tracking = false;
                }
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(WindowEvent::MouseLeave);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let x = (lparam.0 & 0xFFFF) as i16 as f32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(WindowEvent::MouseClick { x, y });
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => LRESULT(0),
            WM_PAINT => {
                let _ = ValidateRect(hwnd, None);
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(WindowEvent::Paint);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(WindowEvent::Destroy);
                }
                unsafe {
                    let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    impl Win32DockWindow {
        pub fn create(config: DockWindowConfig) -> windows::core::Result<Self> {
            let instance = unsafe { GetModuleHandleW(None)? };

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
                lpfnWndProc: Some(dock_wndproc),
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
                RegisterClassExW(&wcex);
            }

            // Calculate bottom-centered positioning on primary monitor
            let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };

            let pos_x = (screen_w - config.width) / 2;
            let pos_y = screen_h - config.height;

            // DirectComposition layered window extended styles
            let ex_style = WS_EX_NOREDIRECTIONBITMAP
                | WS_EX_TOPMOST
                | WS_EX_TOOLWINDOW
                | WS_EX_LAYERED;
            let style = WS_POPUP;

            let shared_state = Arc::new(WindowSharedState {
                events: Mutex::new(VecDeque::new()),
                is_tracking_leave: Mutex::new(false),
            });

            let hwnd = unsafe {
                CreateWindowExW(
                    ex_style,
                    PCWSTR(class_name_wide.as_ptr()),
                    PCWSTR(title_wide.as_ptr()),
                    style,
                    pos_x,
                    pos_y,
                    config.width,
                    config.height,
                    None,
                    None,
                    instance,
                    None,
                )?
            };

            // Associate shared state with window handle for wndproc callbacks
            let raw_state = Arc::as_ptr(&shared_state) as isize;
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, raw_state);
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = UpdateWindow(hwnd);
            }

            Ok(Self {
                hwnd,
                config,
                shared_state,
                is_open: true,
            })
        }

        pub fn poll_events(&mut self) -> Vec<WindowEvent> {
            let mut msg = MSG::default();
            unsafe {
                while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            if let Ok(mut events) = self.shared_state.events.lock() {
                let drained: Vec<WindowEvent> = events.drain(..).collect();
                for ev in &drained {
                    if *ev == WindowEvent::Destroy {
                        self.is_open = false;
                    }
                }
                drained
            } else {
                Vec::new()
            }
        }

        pub fn wait_vsync(&self) {
            unsafe {
                let _ = DwmFlush();
            }
        }

        pub fn hwnd(&self) -> HWND {
            self.hwnd
        }

        pub fn is_open(&self) -> bool {
            self.is_open
        }

        fn cleanup(&mut self) {
            self.is_open = false;
            if !self.hwnd.0.is_null() {
                unsafe {
                    let _ = SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                    let _ = DestroyWindow(self.hwnd);
                }
                self.hwnd = HWND(std::ptr::null_mut());
            }
        }

        pub fn close(&mut self) {
            self.cleanup();
        }

        pub fn config(&self) -> &DockWindowConfig {
            &self.config
        }
    }

    impl Drop for Win32DockWindow {
        fn drop(&mut self) {
            self.cleanup();
        }
    }
}

// ---------------------------------------------------------------------------
// Unified Cross-Platform Public Wrapper
// ---------------------------------------------------------------------------

/// The dock launcher window instance.
pub struct DockWindow {
    #[cfg(windows)]
    inner: win32::Win32DockWindow,

    #[cfg(not(windows))]
    config: DockWindowConfig,
    #[cfg(not(windows))]
    events: VecDeque<WindowEvent>,
    #[cfg(not(windows))]
    is_open: bool,
}

impl DockWindow {
    /// Creates and displays the layered topmost dock window.
    pub fn create(config: DockWindowConfig) -> Result<Self, String> {
        #[cfg(windows)]
        {
            win32::Win32DockWindow::create(config)
                .map(|inner| Self { inner })
                .map_err(|e| format!("Win32 window creation failed: {e}"))
        }

        #[cfg(not(windows))]
        {
            Ok(Self {
                config,
                events: VecDeque::new(),
                is_open: true,
            })
        }
    }

    /// Polls and returns all pending window messages converted into `WindowEvent`s.
    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        #[cfg(windows)]
        {
            self.inner.poll_events()
        }

        #[cfg(not(windows))]
        {
            let drained: Vec<WindowEvent> = self.events.drain(..).collect();
            for ev in &drained {
                if *ev == WindowEvent::Destroy {
                    self.is_open = false;
                }
            }
            drained
        }
    }

    /// Synchronizes execution with the compositor refresh rate (e.g. 120Hz via DwmFlush on Windows).
    pub fn wait_vsync(&self) {
        #[cfg(windows)]
        {
            self.inner.wait_vsync();
        }

        #[cfg(not(windows))]
        {
            // Simulate 120Hz interval (8.33ms) or yield on non-Windows
            std::thread::yield_now();
        }
    }

    /// Returns `true` while the window remains open and active.
    pub fn is_open(&self) -> bool {
        #[cfg(windows)]
        {
            self.inner.is_open()
        }

        #[cfg(not(windows))]
        {
            self.is_open
        }
    }

    /// Requests window closure.
    pub fn close(&mut self) {
        #[cfg(windows)]
        {
            self.inner.close();
        }

        #[cfg(not(windows))]
        {
            self.is_open = false;
        }
    }

    /// Returns the active window configuration.
    pub fn config(&self) -> &DockWindowConfig {
        #[cfg(windows)]
        {
            self.inner.config()
        }

        #[cfg(not(windows))]
        {
            &self.config
        }
    }

    /// On Windows, returns the native `HWND`.
    #[cfg(windows)]
    pub fn hwnd(&self) -> windows::Win32::Foundation::HWND {
        self.inner.hwnd()
    }

    /// Injects an event into the queue (convenient for cross-platform simulation and testing).
    pub fn queue_event(&mut self, event: WindowEvent) {
        #[cfg(windows)]
        {
            // Not needed in production Win32 message pump, but provided for testing consistency
            let _ = event;
        }

        #[cfg(not(windows))]
        {
            self.events.push_back(event);
        }
    }
}
