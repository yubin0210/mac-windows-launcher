//! stage_window
//! Win32 layered topmost window and cross-platform message pump for Stage Manager shelf.

use std::collections::VecDeque;

/// Window events received and processed by the Stage Manager shelf.
#[derive(Debug, Clone, PartialEq)]
pub enum StageWindowEvent {
    MouseMove { x: f32, y: f32 },
    MouseDown { x: f32, y: f32 },
    MouseUp { x: f32, y: f32 },
    MouseLeave,
    KillFocus,
    Paint,
    Destroy,
}

/// Configuration settings for creating a `StageWindow`.
#[derive(Debug, Clone, PartialEq)]
pub struct StageWindowConfig {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for StageWindowConfig {
    fn default() -> Self {
        Self {
            x: 0,
            y: 28,
            width: 220,
            height: 1000,
        }
    }
}

// ---------------------------------------------------------------------------
// Windows Implementation (DirectComposition Layered Topmost Shelf Window)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win32 {
    use super::*;
    use std::sync::{Arc, Mutex};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Gdi::ValidateRect;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Controls::WM_MOUSELEAVE;
    use windows::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetWindowLongPtrW,
        PeekMessageW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW, ShowWindow,
        TranslateMessage, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, MSG, PM_REMOVE, SW_SHOW,
        WNDCLASSEXW, WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        WM_DESTROY, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT,
    };

    pub const STAGE_CLASS_NAME: &str = "MacWinLauncherStageClass";

    struct WindowSharedState {
        events: Mutex<VecDeque<StageWindowEvent>>,
        is_tracking_leave: Mutex<bool>,
        _config: StageWindowConfig,
    }

    pub struct Win32StageWindow {
        hwnd: HWND,
        config: StageWindowConfig,
        shared_state: Arc<WindowSharedState>,
        is_open: bool,
    }

    unsafe extern "system" fn stage_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
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
                        let _ = TrackMouseEvent(&mut tme);
                        *is_tracking = true;
                    }
                }

                if let Ok(mut events) = state.events.lock() {
                    events.push_back(StageWindowEvent::MouseMove { x, y });
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let x = (lparam.0 & 0xFFFF) as i16 as f32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(StageWindowEvent::MouseDown { x, y });
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let x = (lparam.0 & 0xFFFF) as i16 as f32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(StageWindowEvent::MouseUp { x, y });
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                if let Ok(mut is_tracking) = state.is_tracking_leave.lock() {
                    *is_tracking = false;
                }
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(StageWindowEvent::MouseLeave);
                }
                LRESULT(0)
            }
            WM_KILLFOCUS => {
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(StageWindowEvent::KillFocus);
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let _ = ValidateRect(hwnd, None);
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(StageWindowEvent::Paint);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Ok(mut events) = state.events.lock() {
                    events.push_back(StageWindowEvent::Destroy);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    impl Win32StageWindow {
        pub fn new(config: StageWindowConfig) -> Result<Self, String> {
            let instance = unsafe {
                GetModuleHandleW(PCWSTR::null())
                    .map_err(|e| format!("Failed to get module handle: {e}"))?
            };

            let class_name_wide: Vec<u16> = STAGE_CLASS_NAME
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let title_wide: Vec<u16> = "macOS Stage Manager Shelf"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let wcex = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(stage_wndproc),
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

            let shared_state = Arc::new(WindowSharedState {
                events: Mutex::new(VecDeque::new()),
                is_tracking_leave: Mutex::new(false),
                _config: config.clone(),
            });

            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                    PCWSTR(class_name_wide.as_ptr()),
                    PCWSTR(title_wide.as_ptr()),
                    WS_POPUP,
                    config.x,
                    config.y,
                    config.width as i32,
                    config.height as i32,
                    None,
                    None,
                    instance,
                    None,
                )
                .map_err(|e| format!("Failed to create Stage window: {e}"))?
            };

            unsafe {
                SetWindowLongPtrW(
                    hwnd,
                    GWLP_USERDATA,
                    Arc::as_ptr(&shared_state) as *const _ as isize,
                );
                let _ = ShowWindow(hwnd, SW_SHOW);
            }

            Ok(Self {
                hwnd,
                config,
                shared_state,
                is_open: true,
            })
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

        pub fn poll_events(&mut self) -> Vec<StageWindowEvent> {
            let mut msg = MSG::default();
            unsafe {
                while PeekMessageW(&mut msg, self.hwnd, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            let mut queue = self.shared_state.events.lock().unwrap();
            let events: Vec<StageWindowEvent> = queue.drain(..).collect();
            for ev in &events {
                if *ev == StageWindowEvent::Destroy {
                    self.is_open = false;
                }
            }
            events
        }

        pub fn push_event(&mut self, event: StageWindowEvent) {
            self.shared_state.events.lock().unwrap().push_back(event);
        }

        pub fn is_open(&self) -> bool {
            self.is_open
        }

        pub fn config(&self) -> &StageWindowConfig {
            &self.config
        }
    }

    impl Drop for Win32StageWindow {
        fn drop(&mut self) {
            unsafe {
                SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Cross-Platform Mock Implementation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MockStageWindow {
    config: StageWindowConfig,
    events: VecDeque<StageWindowEvent>,
    is_open: bool,
}

impl MockStageWindow {
    pub fn new(config: StageWindowConfig) -> Self {
        Self {
            config,
            events: VecDeque::new(),
            is_open: true,
        }
    }

    pub fn push_mock_event(&mut self, event: StageWindowEvent) {
        self.events.push_back(event);
    }

    pub fn poll_events(&mut self) -> Vec<StageWindowEvent> {
        let drained: Vec<StageWindowEvent> = self.events.drain(..).collect();
        for ev in &drained {
            if *ev == StageWindowEvent::Destroy {
                self.is_open = false;
            }
        }
        drained
    }

    pub fn hwnd(&self) -> isize {
        0
    }

    pub fn width(&self) -> u32 {
        self.config.width
    }

    pub fn height(&self) -> u32 {
        self.config.height
    }

    pub fn config(&self) -> &StageWindowConfig {
        &self.config
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }
}

// ---------------------------------------------------------------------------
// Unified Cross-Platform Public Wrapper
// ---------------------------------------------------------------------------

enum StageWindowInner {
    #[cfg(windows)]
    Real(win32::Win32StageWindow),
    Mock(MockStageWindow),
}

/// Layered window controller for Stage Manager shelf.
pub struct StageWindow {
    inner: StageWindowInner,
}

impl StageWindow {
    /// Creates a new StageWindow with the given configuration.
    pub fn new(config: StageWindowConfig) -> Result<Self, String> {
        #[cfg(windows)]
        {
            let win = win32::Win32StageWindow::new(config)?;
            Ok(Self {
                inner: StageWindowInner::Real(win),
            })
        }
        #[cfg(not(windows))]
        {
            Ok(Self {
                inner: StageWindowInner::Mock(MockStageWindow::new(config)),
            })
        }
    }

    /// Creates a mock-backed `StageWindow` with default configuration.
    pub fn new_mock() -> Self {
        Self {
            inner: StageWindowInner::Mock(MockStageWindow::new(StageWindowConfig::default())),
        }
    }

    /// Creates a mock-backed `StageWindow` with specified configuration.
    pub fn new_mock_with_config(config: StageWindowConfig) -> Self {
        Self {
            inner: StageWindowInner::Mock(MockStageWindow::new(config)),
        }
    }

    /// Alias for `new` to match established window creation pattern.
    pub fn create(config: StageWindowConfig) -> Result<Self, String> {
        Self::new(config)
    }

    /// Returns the native window handle as `isize` (0 in mock mode).
    pub fn hwnd(&self) -> isize {
        match &self.inner {
            #[cfg(windows)]
            StageWindowInner::Real(w) => w.hwnd(),
            StageWindowInner::Mock(w) => w.hwnd(),
        }
    }

    /// Returns window width in pixels.
    pub fn width(&self) -> u32 {
        match &self.inner {
            #[cfg(windows)]
            StageWindowInner::Real(w) => w.width(),
            StageWindowInner::Mock(w) => w.width(),
        }
    }

    /// Returns window height in pixels.
    pub fn height(&self) -> u32 {
        match &self.inner {
            #[cfg(windows)]
            StageWindowInner::Real(w) => w.height(),
            StageWindowInner::Mock(w) => w.height(),
        }
    }

    /// Polls and returns all pending window messages converted into `StageWindowEvent`s.
    pub fn poll_events(&mut self) -> Vec<StageWindowEvent> {
        match &mut self.inner {
            #[cfg(windows)]
            StageWindowInner::Real(w) => w.poll_events(),
            StageWindowInner::Mock(w) => w.poll_events(),
        }
    }

    /// Pushes an event into the window's event queue (useful for testing and simulation).
    pub fn push_mock_event(&mut self, event: StageWindowEvent) {
        match &mut self.inner {
            #[cfg(windows)]
            StageWindowInner::Real(w) => w.push_event(event),
            StageWindowInner::Mock(w) => w.push_mock_event(event),
        }
    }

    /// Returns the window configuration.
    pub fn config(&self) -> &StageWindowConfig {
        match &self.inner {
            #[cfg(windows)]
            StageWindowInner::Real(w) => w.config(),
            StageWindowInner::Mock(w) => w.config(),
        }
    }

    /// Returns `true` while the window remains open.
    pub fn is_open(&self) -> bool {
        match &self.inner {
            #[cfg(windows)]
            StageWindowInner::Real(w) => w.is_open(),
            StageWindowInner::Mock(w) => w.is_open(),
        }
    }
}
