//! dwm_thumbnail
//! macOS-fidelity DWM live thumbnail stream manager with RAII lifecycle cleanup.
//!
//! Registers, updates, and synchronizes live DWM thumbnail streams from source window
//! handles (`HWND`) onto the launcher / Stage Manager shelf destination window surface.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use mac_physics_core::Rect;

/// Active thumbnail stream registration metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ThumbnailRegistration {
    /// Source window handle (HWND) as integer ID.
    pub window_id: u64,
    /// DWM thumbnail registration handle returned by DWM.
    pub thumb_id: u64,
    /// Destination bounding box on the launcher window surface.
    pub target_rect: Rect,
    /// Opacity level (0 = transparent, 255 = fully opaque).
    pub opacity: u8,
    /// Visibility state of the thumbnail stream.
    pub is_visible: bool,
}

/// Abstract interface for DWM thumbnail stream registration, update, and teardown.
pub trait ThumbnailController: Send + Sync {
    /// Registers a thumbnail stream from `src_hwnd` into `dest_hwnd`.
    fn register(&mut self, dest_hwnd: isize, src_hwnd: isize) -> Result<u64, String>;

    /// Updates destination bounding rect, opacity, and visibility for an active thumbnail.
    fn update(
        &mut self,
        thumb_id: u64,
        dest_rect: Rect,
        opacity: u8,
        visible: bool,
    ) -> Result<(), String>;

    /// Unregisters an active thumbnail stream handle.
    fn unregister(&mut self, thumb_id: u64) -> Result<(), String>;

    /// Checks if a thumbnail handle is currently registered.
    fn is_registered(&self, thumb_id: u64) -> bool;

    /// Clears all active registrations in the controller.
    fn clear(&mut self);
}

// ---------------------------------------------------------------------------
// Mock Thumbnail Controller (Headless & Cross-Platform Testing)
// ---------------------------------------------------------------------------

/// In-memory mock thumbnail record.
#[derive(Debug, Clone, PartialEq)]
pub struct MockThumbnailRecord {
    pub dest_hwnd: isize,
    pub src_hwnd: isize,
    pub thumb_id: u64,
    pub dest_rect: Rect,
    pub opacity: u8,
    pub visible: bool,
}

#[derive(Debug, Default)]
struct MockThumbnailInner {
    next_id: u64,
    active_thumbnails: HashMap<u64, MockThumbnailRecord>,
    registrations_history: Vec<(isize, isize, u64)>,
    updates_history: Vec<(u64, Rect, u8, bool)>,
    unregistrations_history: Vec<u64>,
}

/// Thread-safe in-memory `ThumbnailController` for headless unit tests.
#[derive(Debug, Clone)]
pub struct MockThumbnailController {
    inner: Arc<Mutex<MockThumbnailInner>>,
}

impl Default for MockThumbnailController {
    fn default() -> Self {
        Self::new()
    }
}

impl MockThumbnailController {
    /// Creates a new `MockThumbnailController` with monotonic ID counter starting at 1.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MockThumbnailInner {
                next_id: 1,
                active_thumbnails: HashMap::new(),
                registrations_history: Vec::new(),
                updates_history: Vec::new(),
                unregistrations_history: Vec::new(),
            })),
        }
    }

    /// Number of currently active registered thumbnails.
    pub fn registered_count(&self) -> usize {
        self.inner.lock().unwrap().active_thumbnails.len()
    }

    /// Snapshot of all currently active thumbnail records.
    pub fn active_records(&self) -> HashMap<u64, MockThumbnailRecord> {
        self.inner.lock().unwrap().active_thumbnails.clone()
    }

    /// History of all registrations: `(dest_hwnd, src_hwnd, thumb_id)`.
    pub fn registrations_history(&self) -> Vec<(isize, isize, u64)> {
        self.inner.lock().unwrap().registrations_history.clone()
    }

    /// History of all property updates: `(thumb_id, dest_rect, opacity, visible)`.
    pub fn updates_history(&self) -> Vec<(u64, Rect, u8, bool)> {
        self.inner.lock().unwrap().updates_history.clone()
    }

    /// History of all unregistrations: `thumb_id`.
    pub fn unregistrations_history(&self) -> Vec<u64> {
        self.inner.lock().unwrap().unregistrations_history.clone()
    }
}

impl ThumbnailController for MockThumbnailController {
    fn register(&mut self, dest_hwnd: isize, src_hwnd: isize) -> Result<u64, String> {
        if dest_hwnd == 0 {
            return Err("Invalid destination handle: HWND cannot be zero".to_string());
        }
        if src_hwnd == 0 {
            return Err("Invalid source handle: HWND cannot be zero".to_string());
        }

        let mut inner = self.inner.lock().unwrap();
        let thumb_id = inner.next_id;
        inner.next_id += 1;

        let record = MockThumbnailRecord {
            dest_hwnd,
            src_hwnd,
            thumb_id,
            dest_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            opacity: 255,
            visible: true,
        };
        inner.active_thumbnails.insert(thumb_id, record);
        inner.registrations_history.push((dest_hwnd, src_hwnd, thumb_id));
        Ok(thumb_id)
    }

    fn update(
        &mut self,
        thumb_id: u64,
        dest_rect: Rect,
        opacity: u8,
        visible: bool,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(record) = inner.active_thumbnails.get_mut(&thumb_id) {
            record.dest_rect = dest_rect;
            record.opacity = opacity;
            record.visible = visible;
            inner.updates_history.push((thumb_id, dest_rect, opacity, visible));
            Ok(())
        } else {
            Err(format!("Thumbnail not found: {thumb_id}"))
        }
    }

    fn unregister(&mut self, thumb_id: u64) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap();
        if inner.active_thumbnails.remove(&thumb_id).is_some() {
            inner.unregistrations_history.push(thumb_id);
            Ok(())
        } else {
            Err(format!("Thumbnail not found: {thumb_id}"))
        }
    }

    fn is_registered(&self, thumb_id: u64) -> bool {
        self.inner.lock().unwrap().active_thumbnails.contains_key(&thumb_id)
    }

    fn clear(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        inner.active_thumbnails.clear();
    }
}

// ---------------------------------------------------------------------------
// Win32 Production Thumbnail Controller (Windows Target)
// ---------------------------------------------------------------------------

/// Production Win32 controller backed by `dwmapi.dll`.
#[cfg(windows)]
#[derive(Debug, Default, Clone)]
pub struct Win32ThumbnailController;

#[cfg(windows)]
impl Win32ThumbnailController {
    /// Creates a new `Win32ThumbnailController`.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
impl ThumbnailController for Win32ThumbnailController {
    fn register(&mut self, dest_hwnd: isize, src_hwnd: isize) -> Result<u64, String> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Dwm::DwmRegisterThumbnail;

        if dest_hwnd == 0 {
            return Err("Invalid destination handle: HWND cannot be zero".to_string());
        }
        if src_hwnd == 0 {
            return Err("Invalid source handle: HWND cannot be zero".to_string());
        }

        let dest = HWND(dest_hwnd as *mut _);
        let src = HWND(src_hwnd as *mut _);

        unsafe {
            DwmRegisterThumbnail(dest, src)
                .map(|thumb_id| thumb_id as u64)
                .map_err(|e| format!("DwmRegisterThumbnail failed: {e}"))
        }
    }

    fn update(
        &mut self,
        thumb_id: u64,
        dest_rect: Rect,
        opacity: u8,
        visible: bool,
    ) -> Result<(), String> {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::Graphics::Dwm::{
            DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY,
            DWM_TNP_RECTDESTINATION, DWM_TNP_VISIBLE,
        };

        let left = dest_rect.x.round() as i32;
        let top = dest_rect.y.round() as i32;
        let right = ((dest_rect.x + dest_rect.width.max(0.0)).round() as i32).max(left);
        let bottom = ((dest_rect.y + dest_rect.height.max(0.0)).round() as i32).max(top);

        let props = DWM_THUMBNAIL_PROPERTIES {
            dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_OPACITY | DWM_TNP_VISIBLE,
            rcDestination: RECT {
                left,
                top,
                right,
                bottom,
            },
            rcSource: RECT::default(),
            opacity,
            fVisible: visible.into(),
            fSourceClientAreaOnly: false.into(),
        };

        unsafe {
            DwmUpdateThumbnailProperties(thumb_id as isize, &props)
                .map_err(|e| format!("DwmUpdateThumbnailProperties failed: {e}"))
        }
    }

    fn unregister(&mut self, thumb_id: u64) -> Result<(), String> {
        use windows::Win32::Graphics::Dwm::DwmUnregisterThumbnail;

        unsafe {
            DwmUnregisterThumbnail(thumb_id as isize)
                .map_err(|e| format!("DwmUnregisterThumbnail failed: {e}"))
        }
    }

    fn is_registered(&self, _thumb_id: u64) -> bool {
        // DWM API does not support querying handle validity directly;
        // DwmThumbnailManager tracks active registrations.
        true
    }

    fn clear(&mut self) {
        // Win32 DWM requires individual handle teardown, coordinated by DwmThumbnailManager.
    }
}

// ---------------------------------------------------------------------------
// DWM Thumbnail Manager
// ---------------------------------------------------------------------------

/// High-level manager coordinating live DWM thumbnail streams and automatic RAII cleanup.
pub struct DwmThumbnailManager {
    dest_hwnd: isize,
    controller: Box<dyn ThumbnailController>,
    registrations: HashMap<u64, ThumbnailRegistration>,
}

impl DwmThumbnailManager {
    /// Creates a new `DwmThumbnailManager` with a given destination window and thumbnail controller.
    pub fn new(dest_hwnd: isize, controller: Box<dyn ThumbnailController>) -> Self {
        Self {
            dest_hwnd,
            controller,
            registrations: HashMap::new(),
        }
    }

    /// Creates a mock `DwmThumbnailManager` for testing.
    pub fn new_mock(dest_hwnd: isize) -> Self {
        Self::new(dest_hwnd, Box::new(MockThumbnailController::new()))
    }

    /// Destination HWND where thumbnails are rendered.
    pub fn dest_hwnd(&self) -> isize {
        self.dest_hwnd
    }

    /// Returns a reference to all active thumbnail registrations keyed by source window ID.
    pub fn active_registrations(&self) -> &HashMap<u64, ThumbnailRegistration> {
        &self.registrations
    }

    /// Returns the number of currently active thumbnail registrations.
    pub fn count(&self) -> usize {
        self.registrations.len()
    }

    /// Checks if a source window ID currently has an active thumbnail stream.
    pub fn is_window_streamed(&self, window_id: u64) -> bool {
        self.registrations.contains_key(&window_id)
    }

    /// Registers a single window thumbnail stream directly.
    pub fn register_window(
        &mut self,
        window_id: u64,
        target_rect: Rect,
        opacity: u8,
    ) -> Result<u64, String> {
        if window_id == 0 {
            return Err("Invalid source handle: HWND cannot be zero".to_string());
        }
        if self.dest_hwnd == 0 {
            return Err("Invalid destination handle: HWND cannot be zero".to_string());
        }

        // Unregister any existing registration for this window handle first
        if let Some(existing) = self.registrations.remove(&window_id) {
            let _ = self.controller.unregister(existing.thumb_id);
        }

        let thumb_id = self.controller.register(self.dest_hwnd, window_id as isize)?;
        if let Err(e) = self.controller.update(thumb_id, target_rect, opacity, true) {
            let _ = self.controller.unregister(thumb_id);
            return Err(e);
        }

        self.registrations.insert(
            window_id,
            ThumbnailRegistration {
                window_id,
                thumb_id,
                target_rect,
                opacity,
                is_visible: true,
            },
        );

        Ok(thumb_id)
    }

    /// Updates destination bounding box, opacity, and visibility for an existing streamed window.
    pub fn update_window(
        &mut self,
        window_id: u64,
        target_rect: Rect,
        opacity: u8,
        visible: bool,
    ) -> Result<(), String> {
        if let Some(reg) = self.registrations.get_mut(&window_id) {
            self.controller
                .update(reg.thumb_id, target_rect, opacity, visible)?;
            reg.target_rect = target_rect;
            reg.opacity = opacity;
            reg.is_visible = visible;
            Ok(())
        } else {
            Err(format!(
                "Window {window_id} is not registered for thumbnail streaming"
            ))
        }
    }

    /// Unregisters and purges the thumbnail stream for a single window.
    pub fn unregister_window(&mut self, window_id: u64) -> Result<(), String> {
        if let Some(reg) = self.registrations.remove(&window_id) {
            self.controller.unregister(reg.thumb_id)
        } else {
            Err(format!(
                "Window {window_id} is not registered for thumbnail streaming"
            ))
        }
    }

    /// Synchronizes active thumbnail streams to match the desired configuration.
    ///
    /// - Existing streamed windows are updated with new `dest_rect` and `opacity`.
    /// - Newly requested windows are registered with DWM and configured.
    /// - Windows no longer present in `desired` are unregistered and removed.
    pub fn sync_thumbnails(&mut self, desired: &[(u64, Rect, u8)]) -> Result<(), String> {
        use std::collections::HashSet;

        let desired_ids: HashSet<u64> = desired.iter().map(|(id, _, _)| *id).collect();

        // 1. Unregister and purge windows that are no longer in desired set
        let stale_ids: Vec<u64> = self
            .registrations
            .keys()
            .copied()
            .filter(|id| !desired_ids.contains(id))
            .collect();

        for stale_id in stale_ids {
            if let Some(reg) = self.registrations.remove(&stale_id) {
                let _ = self.controller.unregister(reg.thumb_id);
            }
        }

        // 2. Register new windows or update existing streams
        for &(window_id, target_rect, opacity) in desired {
            if let Some(reg) = self.registrations.get_mut(&window_id) {
                // Window already streamed: update properties
                self.controller
                    .update(reg.thumb_id, target_rect, opacity, true)?;
                reg.target_rect = target_rect;
                reg.opacity = opacity;
                reg.is_visible = true;
            } else {
                // New window: register stream and apply properties
                let thumb_id = self.controller.register(self.dest_hwnd, window_id as isize)?;
                if let Err(e) = self.controller.update(thumb_id, target_rect, opacity, true) {
                    let _ = self.controller.unregister(thumb_id);
                    return Err(e);
                }
                self.registrations.insert(
                    window_id,
                    ThumbnailRegistration {
                        window_id,
                        thumb_id,
                        target_rect,
                        opacity,
                        is_visible: true,
                    },
                );
            }
        }

        Ok(())
    }

    /// Unregisters all currently tracked thumbnail streams with the controller.
    pub fn unregister_all(&mut self) {
        for (_id, reg) in self.registrations.drain() {
            let _ = self.controller.unregister(reg.thumb_id);
        }
        self.controller.clear();
    }
}

impl Drop for DwmThumbnailManager {
    fn drop(&mut self) {
        self.unregister_all();
    }
}
