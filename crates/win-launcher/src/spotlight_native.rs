//! Native EDIT owns IME, UTF-16 input, caret, selection and paste. The parent owns search events.
use crate::{settings::SpotlightHotkey, spotlight_layout::SpotlightGeometry, spotlight_window::{SpotlightEvent, SpotlightWindowConfig}, display::WorkArea};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{SetWindowSubclass, RemoveWindowSubclass, DefSubclassProc};
use windows::Win32::UI::WindowsAndMessaging::*;

const EDIT_ID: usize = 100;
struct State {
    events: RefCell<VecDeque<SpotlightEvent>>,
    edit: Cell<HWND>, visible: Cell<bool>, composing: Cell<bool>, previous: Cell<HWND>,
    geometry: Cell<SpotlightGeometry>, count: Cell<usize>, has_query: Cell<bool>,
    selected: Cell<usize>, hotkey_id: Cell<i32>, row_ids: RefCell<Vec<String>>,
    brush: HBRUSH, font: Cell<HFONT>,
}
impl Drop for State {
    fn drop(&mut self) { unsafe {
        if !self.font.get().0.is_null() { let _ = DeleteObject(self.font.get()); }
        let _ = DeleteObject(self.brush);
    } }
}
impl State {
    fn emit(&self, event: SpotlightEvent) { self.events.borrow_mut().push_back(event); }
    unsafe fn query(&self) -> String {
        let length = GetWindowTextLengthW(self.edit.get()).max(0) as usize;
        let mut buffer = vec![0u16; length + 1];
        let read = GetWindowTextW(self.edit.get(), &mut buffer).max(0) as usize;
        String::from_utf16_lossy(&buffer[..read])
    }
}
unsafe fn hide(hwnd: HWND, state: &State, restore: bool) {
    if !state.visible.replace(false) { return; }
    let previous = state.previous.get();
    let had_focus = GetForegroundWindow() == hwnd;
    let _ = ShowWindow(hwnd, SW_HIDE);
    let _ = SetWindowTextW(state.edit.get(), w!(""));
    state.composing.set(false);
    state.selected.set(0);
    state.count.set(0);
    state.has_query.set(false);
    state.emit(SpotlightEvent::Closed);
    if restore && had_focus && IsWindow(previous).as_bool() { let _ = SetForegroundWindow(previous); }
}
unsafe fn geometry(anchor: HWND, count: usize, query: bool) -> Result<SpotlightGeometry, String> {
    let monitor = MonitorFromWindow(anchor, MONITOR_DEFAULTTOPRIMARY);
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !GetMonitorInfoW(monitor, &mut info).as_bool() { return Err("Cannot locate Spotlight monitor".into()); }
    let mut x = 0; let mut y = 0;
    windows::Win32::UI::HiDpi::GetDpiForMonitor(monitor, windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI, &mut x, &mut y).map_err(|e| e.to_string())?;
    let rect = info.rcWork;
    SpotlightGeometry::fit(WorkArea { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom }, x, count, query)
}
unsafe fn reflow(hwnd: HWND, state: &State, anchor: HWND) -> Result<(), String> {
    let old = state.geometry.get();
    let next = geometry(anchor, state.count.get(), state.has_query.get())?;
    if old == next && !state.font.get().0.is_null() { return Ok(()); }
    if old != next {
        SetWindowPos(hwnd, None, next.x, next.y, next.width, next.height, SWP_NOACTIVATE | SWP_NOZORDER).map_err(|e| e.to_string())?;
        state.geometry.set(next);
    }
    let edit = state.edit.get();
    if !edit.0.is_null() {
        let scale = next.scale;
        if state.font.get().0.is_null() || old.scale != next.scale {
            let mut font = LOGFONTW { lfHeight: -(20.0 * scale).round() as i32, lfWeight: FW_NORMAL.0 as i32, ..Default::default() };
            let face: Vec<u16> = "Segoe UI".encode_utf16().collect();
            font.lfFaceName[..face.len()].copy_from_slice(&face);
            let font = CreateFontIndirectW(&font);
            if font.0.is_null() { return Err("Cannot create Spotlight input font".into()); }
            SendMessageW(edit, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
            let old_font = state.font.replace(font);
            if !old_font.0.is_null() { let _ = DeleteObject(old_font); }
        }
        SetWindowPos(edit, None, (48.0*scale) as i32, (14.0*scale) as i32,
            (next.width as f32 - 64.0*scale).max(1.0) as i32, (28.0*scale).ceil() as i32,
            SWP_NOACTIVATE | SWP_NOZORDER).map_err(|e| e.to_string())?;
    }
    Ok(())
}
unsafe fn show(hwnd: HWND, state: &State) {
    if state.visible.get() { return; }
    let previous = GetForegroundWindow();
    // Every new session starts clean, including text messages queued just before a close.
    let _ = SetWindowTextW(state.edit.get(), w!(""));
    state.count.set(0); state.has_query.set(false); state.selected.set(0);
    state.row_ids.borrow_mut().clear();
    state.previous.set(previous);
    if let Err(error) = reflow(hwnd, state, previous) { state.emit(SpotlightEvent::Error(error)); return; }
    state.visible.set(true);
    let _ = ShowWindow(hwnd, SW_SHOW);
    if !SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() != hwnd {
        hide(hwnd, state, false);
        state.emit(SpotlightEvent::Error("Windows did not allow Spotlight to take focus".into()));
        return;
    }
    let _ = SetFocus(state.edit.get());
    state.emit(SpotlightEvent::Opened);
}
unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _: usize, data: usize) -> LRESULT {
    let state = &*(data as *const State);
    match msg {
        WM_IME_STARTCOMPOSITION => state.composing.set(true),
        WM_IME_ENDCOMPOSITION => {
            state.composing.set(false);
            let result = DefSubclassProc(hwnd, msg, wp, lp);
            let parent = GetParent(hwnd).unwrap_or_default();
            let foreground = GetForegroundWindow();
            if foreground != parent && GetAncestor(foreground, GA_ROOTOWNER) != parent {
                hide(parent, state, false);
            }
            return result;
        }
        WM_GETDLGCODE => return LRESULT(DLGC_WANTALLKEYS as isize),
        WM_KEYDOWN if !state.composing.get() => {
            match wp.0 as u16 {
                key if key == VK_RETURN.0 => { state.emit(SpotlightEvent::Execute); return LRESULT(0); }
                key if key == VK_UP.0 => { state.emit(SpotlightEvent::Navigate(-1)); return LRESULT(0); }
                key if key == VK_DOWN.0 => { state.emit(SpotlightEvent::Navigate(1)); return LRESULT(0); }
                key if key == VK_ESCAPE.0 => {
                    state.emit(SpotlightEvent::Escaped);
                    hide(GetParent(hwnd).unwrap_or_default(), state, true);
                    return LRESULT(0);
                }
                0x41 if GetKeyState(VK_CONTROL.0 as i32) < 0 => {
                    SendMessageW(hwnd, windows::Win32::UI::Controls::EM_SETSEL, WPARAM(0), LPARAM(-1));
                    return LRESULT(0);
                }
                _ => {}
            }
        }
        WM_CHAR if !state.composing.get() && matches!(wp.0, 1 | 13 | 27) => return LRESULT(0),
        WM_NCDESTROY => { let _ = RemoveWindowSubclass(hwnd, Some(edit_proc), 1); }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}
unsafe extern "system" fn parent_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
    if pointer.is_null() { return DefWindowProcW(hwnd, msg, wp, lp); }
    let state = &*pointer;
    match msg {
        WM_HOTKEY if wp.0 as i32 == state.hotkey_id.get() => {
            if state.visible.get() { hide(hwnd, state, true); } else { show(hwnd, state); }
            LRESULT(0)
        }
        WM_COMMAND if (wp.0 & 0xFFFF) == EDIT_ID && ((wp.0 >> 16) & 0xFFFF) == EN_CHANGE as usize => {
            if state.visible.get() { state.emit(SpotlightEvent::QueryChanged(state.query())); }
            LRESULT(0)
        }
        WM_CTLCOLOREDIT => {
            let dc = HDC(wp.0 as *mut _);
            SetBkColor(dc, COLORREF(0x00292420));
            SetTextColor(dc, COLORREF(0x00f4f4f4));
            LRESULT(state.brush.0 as isize)
        }
        WM_ACTIVATE if (wp.0 & 0xFFFF) == WA_INACTIVE as usize => {
            let next = HWND(lp.0 as *mut _);
            // Owned IME/error UI must not dismiss its input owner.
            if !state.composing.get() && GetAncestor(next, GA_ROOTOWNER) != hwnd { hide(hwnd, state, false); }
            LRESULT(0)
        }
        WM_SETFOCUS => { let _ = SetFocus(state.edit.get()); LRESULT(0) }
        WM_LBUTTONUP => {
            let x = (lp.0 & 0xFFFF) as i16 as f32;
            let y = ((lp.0 >> 16) & 0xFFFF) as i16 as f32;
            let layout = state.geometry.get();
            if x >= 0.0 && x < layout.width as f32 {
                if let Some(row) = layout.row_at(y) {
                    let id = state.row_ids.borrow().get(row).cloned();
                    if let Some(id) = id { state.emit(SpotlightEvent::ClickResult(id)); }
                }
            }
            LRESULT(0)
        }
        WM_PAINT => { let _ = ValidateRect(hwnd, None); state.emit(SpotlightEvent::Paint); LRESULT(0) }
        WM_DPICHANGED | WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            if state.visible.get() {
                if let Err(error) = reflow(hwnd, state, hwnd) { state.emit(SpotlightEvent::Error(error)); }
                state.emit(SpotlightEvent::DisplayChanged);
            }
            LRESULT(0)
        }
        WM_CLOSE => { hide(hwnd, state, true); LRESULT(0) }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub struct NativeSpotlightWindow {
    hwnd: HWND, state: Box<State>, hotkey: Option<SpotlightHotkey>,
}
impl NativeSpotlightWindow {
    pub fn new(config: SpotlightWindowConfig) -> Result<Self, String> {
        unsafe {
            let brush = CreateSolidBrush(COLORREF(0x00292420));
            if brush.0.is_null() { return Err("Cannot create Spotlight input background".into()); }
            let state = Box::new(State {
                events: RefCell::new(VecDeque::new()), edit: Cell::new(HWND::default()), visible: Cell::new(false),
                composing: Cell::new(false), previous: Cell::new(HWND::default()), selected: Cell::new(0), hotkey_id: Cell::new(0), row_ids: RefCell::new(Vec::new()),
                count: Cell::new(0), has_query: Cell::new(false), brush, font: Cell::new(HFONT::default()),
                geometry: Cell::new(SpotlightGeometry::fit(WorkArea { left: 0, top: 0, right: 640, bottom: 480 }, 96, 0, false)?),
            });
            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class: Vec<u16> = config.class_name.encode_utf16().chain(Some(0)).collect();
            let title: Vec<u16> = config.title.encode_utf16().chain(Some(0)).collect();
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32, lpfnWndProc: Some(parent_proc), hInstance: instance.into(),
                lpszClassName: PCWSTR(class.as_ptr()), hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(), ..Default::default()
            };
            let _ = RegisterClassExW(&wc);
            let hwnd = CreateWindowExW(WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                PCWSTR(class.as_ptr()), PCWSTR(title.as_ptr()), WS_POPUP, 0, 0, 640, 80, None, None, instance, None).map_err(|e| e.to_string())?;
            let native = Self { hwnd, state, hotkey: None };
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, native.state.as_ref() as *const State as isize);
            let edit = CreateWindowExW(WINDOW_EX_STYLE(0), w!("EDIT"), w!(""), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                48, 14, 576, 28, hwnd, HMENU(EDIT_ID as *mut _), instance, None).map_err(|e| format!("Cannot create Spotlight input: {e}"))?;
            native.state.edit.set(edit);
            SendMessageW(edit, windows::Win32::UI::Controls::EM_SETLIMITTEXT, WPARAM(4096), LPARAM(0));
            let cue: Vec<u16> = "Search apps or calculate".encode_utf16().chain(Some(0)).collect();
            SendMessageW(edit, windows::Win32::UI::Controls::EM_SETCUEBANNER, WPARAM(1), LPARAM(cue.as_ptr() as isize));
            if !SetWindowSubclass(edit, Some(edit_proc), 1, native.state.as_ref() as *const State as usize).as_bool() {
                return Err("Cannot attach Spotlight input handler".into());
            }
            Ok(native)
        }
    }
    pub fn hotkey(&self) -> Option<SpotlightHotkey> { self.hotkey }
    pub fn clear_hotkey(&mut self) {
        let old = self.state.hotkey_id.replace(0);
        self.hotkey = None;
        if old != 0 { unsafe { let _ = UnregisterHotKey(self.hwnd, old); } }
    }
    pub fn set_row_ids(&mut self, ids: Vec<String>) { *self.state.row_ids.borrow_mut() = ids; }
    pub fn register_hotkey(&mut self, key: SpotlightHotkey) -> Result<(), String> {
        if self.hotkey == Some(key) { return Ok(()); }
        let old = self.state.hotkey_id.get();
        let next = if old == 1001 { 1002 } else { 1001 };
        let modifiers = match key {
            SpotlightHotkey::AltSpace => MOD_ALT,
            SpotlightHotkey::CtrlAltSpace => MOD_CONTROL | MOD_ALT,
            SpotlightHotkey::CtrlShiftSpace => MOD_CONTROL | MOD_SHIFT,
        };
        unsafe { RegisterHotKey(self.hwnd, next, modifiers | MOD_NOREPEAT, VK_SPACE.0 as u32) }
            .map_err(|e| format!("{} is already in use or unavailable: {e}. Choose another Spotlight shortcut in the Dock menu; Open Spotlight remains available in the tray.", key.label()))?;
        self.state.hotkey_id.set(next);
        self.hotkey = Some(key);
        if old != 0 { unsafe { let _ = UnregisterHotKey(self.hwnd, old); } }
        Ok(())
    }
    pub fn show(&mut self) { unsafe { show(self.hwnd, &self.state); } }
    pub fn hide(&mut self) { unsafe { hide(self.hwnd, &self.state, true); } }
    pub fn dismiss_after_execution(&mut self) { unsafe { hide(self.hwnd, &self.state, false); } }
    pub fn toggle(&mut self) { if self.is_visible() { self.hide(); } else { self.show(); } }
    pub fn is_visible(&self) -> bool { self.state.visible.get() }
    pub fn poll_events(&mut self) -> Vec<SpotlightEvent> { self.state.events.borrow_mut().drain(..).collect() }
    pub fn width(&self) -> u32 { self.state.geometry.get().width as u32 }
    pub fn height(&self) -> u32 { self.state.geometry.get().height as u32 }
    pub fn geometry(&self) -> SpotlightGeometry { self.state.geometry.get() }
    pub fn set_results(&mut self, count: usize, has_query: bool) -> Result<(), String> {
        self.state.count.set(count); self.state.has_query.set(has_query);
        unsafe { reflow(self.hwnd, &self.state, self.hwnd) }
    }
    pub fn set_height(&mut self, height: u32) {
        let mut layout = self.state.geometry.get(); layout.height = height.clamp(56, 420) as i32; self.state.geometry.set(layout);
        unsafe { let _ = SetWindowPos(self.hwnd, None, layout.x, layout.y, layout.width, layout.height, SWP_NOACTIVATE | SWP_NOZORDER); }
    }
    pub fn reset_height(&mut self) { let _ = self.set_results(0, false); }
    pub fn reset_query(&mut self) { unsafe { let _ = SetWindowTextW(self.state.edit.get(), w!("")); } self.state.selected.set(0); }
    pub fn query(&self) -> String { unsafe { self.state.query() } }
    pub fn selected_index(&self) -> usize { self.state.selected.get() }
    pub fn set_selected_index(&mut self, index: usize) { self.state.selected.set(index); }
    pub fn push_event(&mut self, event: SpotlightEvent) { self.state.emit(event); }
    pub fn hwnd(&self) -> HWND { self.hwnd }
    pub fn edit_hwnd(&self) -> HWND { self.state.edit.get() }
}
impl Drop for NativeSpotlightWindow {
    fn drop(&mut self) { unsafe {
        let key = self.state.hotkey_id.get();
        if key != 0 { let _ = UnregisterHotKey(self.hwnd, key); }
        SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
        let _ = DestroyWindow(self.hwnd);
    } }
}
