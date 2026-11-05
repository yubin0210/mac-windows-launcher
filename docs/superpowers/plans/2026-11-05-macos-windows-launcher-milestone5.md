# macOS UI/UX System Launcher (Milestone 5: Stage Manager Window Grouping & Spaces Subsystem) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the macOS Stage Manager subsystem on Windows with window grouping by application/workspace, left-screen flyout shelf layout geometry, live interactive DWM thumbnail streaming (`DwmRegisterThumbnail`), smooth spring slide animations, and single-click focus/group switching.

**Architecture:** A modular Cargo workspace extension: `mac-physics-core::stage_manager` computes pure platform-independent shelf layout, stacked card offsets, and spring positions; `win-launcher::stage_tracker` enumerates top-level windows, filters cloaked/tool/utility windows, and groups windows by process/app; `win-launcher::dwm_thumbnail` manages native DWM thumbnail streams and destination rect mapping; `win-launcher::stage_window` hosts the left-edge topmost layered window; `win-launcher::stage_render` renders frosted glass squircle cards and app badges; and `win-launcher::main` integrates the subsystem into the 120Hz event loop.

**Tech Stack:** Rust 2021, Win32 (`SetWinEventHook`, `EnumWindows`, `GetWindowTextW`, `ShowWindow`, `SetForegroundWindow`), DWM (`DwmRegisterThumbnail`, `DwmUpdateThumbnailProperties`, `DwmUnregisterThumbnail`, `DwmGetWindowAttribute`), Direct2D / DirectWrite, `serde`, `serde_json`.

**Spec:** [docs/superpowers/specs/2026-11-03-macos-windows-launcher-design.md](file:///Users/yubin/Projects/REA_for_Mac/docs/superpowers/specs/2026-11-03-macos-windows-launcher-design.md)

## Global Constraints

- **Platform Target:** Windows 10 (1809+) & Windows 11 (64-bit / ARM64).
- **Toolchain:** Rust (stable), all tests must run cleanly on both macOS host and Windows target (`cargo test --workspace`).
- **Resource Limits:** Idle RAM < 40MB, idle CPU < 0.1%. Zero web runtimes (no Chromium, no WebView2).
- **Graphics Pipeline:** Direct2D + DirectComposition with zero GDI flicker (`WS_EX_NOREDIRECTIONBITMAP`).
- **Safety & Coexistence:** Standalone user-space execution without modifying system files or registry; graceful mock fallbacks on non-Windows platforms.
- **Fail-Safe Cleanup:** All registered DWM thumbnails must be unregistered (`DwmUnregisterThumbnail`) upon group switch, window close, or app shutdown.

## Review Focus

1. **Window Cloaking & Filtering:** Windows 10/11 UWP apps, hidden background frames, and tool windows (`WS_EX_TOOLWINDOW`, `DWMWA_CLOAKED`) must be excluded so empty or ghost tiles never appear on the stage shelf.
2. **Multi-Window Stack Geometry:** When an application has multiple open windows (e.g. 3 browser windows or 2 document windows), the shelf must render an overlapping squircle card fan-out (offset up to 8px-16px) with an indicator badge instead of collapsing or hiding them.
3. **Smooth Stage Transition Spring:** When switching between the active "Center Stage" group and a staged shelf group, the incoming and outgoing group cards must animate using damped spring physics without snapping or popping.
4. **Thumbnail Aspect Ratio Fitting:** Windows with arbitrary aspect ratios (e.g. wide 21:9 windows vs tall 9:16 windows) must fit cleanly within the stage card bounding box ($W \le 180$, $H \le 120$) with aspect ratio preservation (`letterboxing` / `pillarboxing`) without stretching or distortion.
5. **DWM Thumbnail Leak Prevention:** If a tracked window closes abruptly or the launcher shuts down, registered DWM thumbnail handles (`HTHUMBNAIL`) must be safely unregistered via RAII `Drop` guards to prevent DWM resource exhaustion.

---

### Task 1: Stage Manager Core Models & Shelf Layout Geometry (`mac-physics-core`)

**Files:**
- Create: `crates/mac-physics-core/src/stage_manager.rs`
- Modify: `crates/mac-physics-core/src/lib.rs`
- Test: `crates/mac-physics-core/tests/test_stage_manager_core.rs`

**Interfaces:**
- `pub struct StageWindowInfo { pub id: u64, pub title: String, pub app_name: String, pub width: u32, pub height: u32, pub is_minimized: bool }`
- `pub struct StageGroup { pub group_id: String, pub app_name: String, pub windows: Vec<StageWindowInfo>, pub active_window_index: usize }`
- `pub struct StageCardLayout { pub group_id: String, pub bounds: Rect, pub thumbnail_bounds: Rect, pub stack_depth: usize, pub opacity: f32, pub is_hovered: bool }`
- `pub struct StageShelfMetrics { pub shelf_x: f32, pub shelf_width: f32, pub card_width: f32, pub card_height: f32, pub card_spacing: f32, pub max_groups: usize, pub corner_radius: f32 }`
- `pub fn compute_stage_shelf_layout(groups: &[StageGroup], active_group_id: Option<&str>, metrics: &StageShelfMetrics, screen_height: f32, shelf_slide_t: f32) -> Vec<StageCardLayout>`
- `pub fn compute_thumbnail_aspect_fit(source_w: u32, source_h: u32, target_w: f32, target_h: f32) -> Rect`

- [ ] **Step 1: Write failing tests for stage manager models and layout geometry**
  - Verify `compute_stage_shelf_layout` positions up to `max_groups` non-active groups along the left screen margin with consistent vertical spacing.
  - Verify active "Center Stage" group is excluded from the left shelf layout.
  - Verify multi-window groups correctly report `stack_depth` (e.g., 1 for single window, 3 for 3 windows).
  - Verify `compute_thumbnail_aspect_fit` preserves aspect ratio within target boundaries without distortion.
  - Verify `shelf_slide_t` smoothly interpolates card $X$ position from $-card\_width$ to `metrics.shelf_x`.

- [ ] **Step 2: Run tests to confirm failure**
  - Command: `cargo test -p mac-physics-core --test test_stage_manager_core`

- [ ] **Step 3: Implement stage manager core models and layout math**
  - Add `stage_manager.rs` in `mac-physics-core`.
  - Export module in `crates/mac-physics-core/src/lib.rs`.

- [ ] **Step 4: Run tests and ensure all pass**
  - Command: `cargo test -p mac-physics-core --test test_stage_manager_core`

- [ ] **Step 5: Commit Task 1**
  - Command: `git add crates/mac-physics-core/ && git commit -m "feat(core): implement stage manager models and shelf layout geometry"`

---

### Task 2: Window Grouping & Stage Tracker (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/stage_tracker.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_stage_tracker.rs`

**Interfaces:**
- `pub trait WindowEnumerator: Send + Sync { fn enumerate_windows(&self) -> Vec<StageWindowInfo>; fn activate_window(&self, id: u64) -> Result<(), String>; fn minimize_window(&self, id: u64) -> Result<(), String>; }`
- `pub struct MockWindowEnumerator { ... }`
- `pub struct Win32WindowEnumerator { ... }` (conditional on `#[cfg(windows)]`)
- `pub struct StageTracker { ... }`
- `impl StageTracker { pub fn new(enumerator: Box<dyn WindowEnumerator>) -> Self; pub fn refresh(&mut self); pub fn groups(&self) -> &[StageGroup]; pub fn active_group_id(&self) -> Option<&str>; pub fn set_active_group(&mut self, group_id: &str) -> Result<(), String>; pub fn add_mock_window(&mut self, window: StageWindowInfo); }`

- [ ] **Step 1: Write failing tests for StageTracker window grouping and activation**
  - Test grouping multiple windows of the same application into a single `StageGroup`.
  - Test switching active group (activating the selected group's active window and minimizing/de-activating previous group windows).
  - Test filtering out tool windows, invisible windows, or dock/menu bar utility windows.
  - Test dynamic addition and removal of windows (window created / destroyed).

- [ ] **Step 2: Run tests to confirm failure**
  - Command: `cargo test -p win-launcher --test test_stage_tracker`

- [ ] **Step 3: Implement StageTracker and WindowEnumerator**
  - Implement grouping logic by `app_name` or executable name.
  - Implement active group determination based on current foreground window.
  - Implement Win32 window enumeration using `EnumWindows`, `IsWindowVisible`, `GetWindowTextW`, and `DWMWA_CLOAKED`.

- [ ] **Step 4: Run tests and ensure all pass**
  - Command: `cargo test -p win-launcher --test test_stage_tracker`

- [ ] **Step 5: Commit Task 2**
  - Command: `git add crates/win-launcher/ && git commit -m "feat(launcher): implement stage tracker and window grouping logic"`

---

### Task 3: DWM Thumbnail Stream Manager (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/dwm_thumbnail.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_dwm_thumbnail.rs`

**Interfaces:**
- `pub struct ThumbnailRegistration { pub window_id: u64, pub target_rect: Rect, pub opacity: u8, pub is_visible: bool }`
- `pub trait ThumbnailController: Send + Sync { fn register(&mut self, dest_hwnd: isize, src_hwnd: isize) -> Result<u64, String>; fn update(&mut self, thumb_id: u64, dest_rect: Rect, opacity: u8, visible: bool) -> Result<(), String>; fn unregister(&mut self, thumb_id: u64) -> Result<(), String>; fn clear(&mut self); }`
- `pub struct DwmThumbnailManager { controller: Box<dyn ThumbnailController>, registrations: HashMap<u64, u64> }`

- [ ] **Step 1: Write failing tests for DWM thumbnail management**
  - Test registering a thumbnail stream between destination window and target window.
  - Test updating thumbnail target destination rect and opacity.
  - Test automatic unregistering on drop (RAII) and `clear()`.
  - Test idempotent unregistering for closed/stale window handles.

- [ ] **Step 2: Run tests to confirm failure**
  - Command: `cargo test -p win-launcher --test test_dwm_thumbnail`

- [ ] **Step 3: Implement DwmThumbnailManager with Mock and Real Win32/DWM backend**
  - Safe wrappers around `DwmRegisterThumbnail`, `DwmUpdateThumbnailProperties`, and `DwmUnregisterThumbnail`.
  - RAII `Drop` implementation guaranteeing all registered handles are freed.

- [ ] **Step 4: Run tests and ensure all pass**
  - Command: `cargo test -p win-launcher --test test_dwm_thumbnail`

- [ ] **Step 5: Commit Task 3**
  - Command: `git add crates/win-launcher/ && git commit -m "feat(launcher): implement DWM thumbnail stream manager with RAII cleanup"`

---

### Task 4: Win32 Layered Window & Stage Shelf State Machine (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/stage_window.rs`
- Create: `crates/win-launcher/src/stage_view.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_stage_view.rs`
- Test: `crates/win-launcher/tests/test_stage_window.rs`

**Interfaces:**
- `pub enum StageViewAction { ActivateGroup(String), CloseWindow(u64), HideShelf }`
- `pub struct StageView { ... }`
- `impl StageView { pub fn new(screen_width: f32, screen_height: f32) -> Self; pub fn update(&mut self, dt: f32, tracker: &StageTracker); pub fn on_mouse_move(&mut self, x: f32, y: f32); pub fn on_mouse_click(&mut self, x: f32, y: f32) -> Option<StageViewAction>; pub fn cards(&self) -> &[StageCardLayout]; pub fn is_visible(&self) -> bool; }`
- `pub struct StageWindow { ... }`

- [ ] **Step 1: Write failing tests for StageView state machine and StageWindow**
  - Test shelf slide-in animation when cursor hovers near left screen edge ($x \le 20$).
  - Test shelf slide-out or auto-hide when cursor moves away ($x > 220$).
  - Test clicking on a stage card generates `StageViewAction::ActivateGroup`.
  - Test hover feedback and card highlighting.
  - Test mock `StageWindow` event processing.

- [ ] **Step 2: Run tests to confirm failure**
  - Command: `cargo test -p win-launcher --test test_stage_view --test test_stage_window`

- [ ] **Step 3: Implement StageView and StageWindow**
  - Implement spring animation for shelf slide progress ($0.0 \le t \le 1.0$).
  - Implement hit testing for cards and multi-window stacked tabs.
  - Implement Win32 borderless topmost layered window for left shelf.

- [ ] **Step 4: Run tests and ensure all pass**
  - Command: `cargo test -p win-launcher --test test_stage_view --test test_stage_window`

- [ ] **Step 5: Commit Task 4**
  - Command: `git add crates/win-launcher/ && git commit -m "feat(launcher): implement stage view state machine and layered window"`

---

### Task 5: Direct2D Stage Shelf Surface Rasterizer (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/stage_render.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_stage_render.rs`

**Interfaces:**
- `pub trait StageRenderContext: Send + Sync { fn draw_stage_card(&mut self, card: &StageCardLayout, app_name: &str, window_count: usize) -> Result<(), String>; fn clear(&mut self); }`
- `pub struct MockStageRenderer { ... }`
- `pub struct StageRenderer { ... }` (conditional on `#[cfg(windows)]`)

- [ ] **Step 1: Write failing tests for StageRenderer**
  - Test rendering single-window card with frosted squircle backdrop and app badge.
  - Test rendering multi-window card with fanned-out stacked backdrops.
  - Test hover highlight border and scale animation rendering.
  - Verify mock renderer records drawing commands and validates card bounds.

- [ ] **Step 2: Run tests to confirm failure**
  - Command: `cargo test -p win-launcher --test test_stage_render`

- [ ] **Step 3: Implement StageRenderer Direct2D surface drawer**
  - Direct2D rounded rectangle / squircle geometry caching.
  - DirectWrite app title text formatting.
  - Acrylic / frosted glass fill with subtle gradient borders.
  - Safe error propagation from Direct2D `EndDraw`.

- [ ] **Step 4: Run tests and ensure all pass**
  - Command: `cargo test -p win-launcher --test test_stage_render`

- [ ] **Step 5: Commit Task 5**
  - Command: `git add crates/win-launcher/ && git commit -m "feat(launcher): implement Direct2D stage shelf card surface renderer"`

---

### Task 6: End-to-End Milestone 5 Integration & Verification

**Files:**
- Modify: `crates/win-launcher/src/main.rs`
- Modify: `crates/win-launcher/src/config.rs`
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-11-03-macos-windows-launcher-design.md`

- [ ] **Step 1: Integrate Stage Manager into main runtime loop**
  - Wire `StageTracker`, `StageView`, `DwmThumbnailManager`, and `StageWindow` into `run_windows_launcher`.
  - Connect `StageViewAction::ActivateGroup` to window activation and thumbnail stream reallocation.
  - Add config option `enable_stage_manager: bool` (default: `true`).
  - Wire hotkey / menu bar toggle for Stage Manager.

- [ ] **Step 2: Run full workspace test suite**
  - Command: `cargo test --workspace`

- [ ] **Step 3: Run Windows GNU cross-compilation check**
  - Command: `cargo check --target x86_64-pc-windows-gnu -p win-launcher`

- [ ] **Step 4: Update README and specifications**
  - Document Stage Manager features, window grouping, DWM live streaming, and hotkeys.

- [ ] **Step 5: Commit Task 6**
  - Command: `git add crates/win-launcher/ README.md docs/ && git commit -m "feat(launcher): integrate Stage Manager into runtime and update documentation"`
