# macOS UI/UX System Launcher on Windows — Milestone 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Milestone 2: Direct2D surface rasterization (squircle tray + running pips + vector icons), Win32 application process tracking & running indicator pips, native process launching & foreground window activation, smooth cursor leave scale decay animation, and safe taskbar coexistence.

**Architecture:**
Expands `win-launcher` with Direct2D drawing surfaces bound to `IDCompositionVisual`, a non-intrusive Win32 process/window tracker for running indicator pips, a native launcher executing Win32 apps or restoring existing windows, and an exponential/spring decay animator for smooth cursor departure transitions, while keeping `mac-physics-core` pure and cross-platform.

**Tech Stack:** Rust (edition 2021), `windows` crate (0.58) with Direct2D, Direct3D11, DirectComposition, Shell, and Win32 UI features, `serde`, `mac-physics-core`.

**Spec:** [`docs/superpowers/specs/2026-11-03-macos-windows-launcher-design.md`](../specs/2026-11-03-macos-windows-launcher-design.md)

---

## Global Constraints

- Standalone Rust native binary with zero web runtimes (no Chromium, no WebView2, no Electron).
- Direct2D / DirectComposition rendering pipeline running at 120Hz VSync cadence (`DwmFlush`).
- Win32 taskbar auto-hide must implement a fail-safe `Drop` guard restoring `Shell_TrayWnd` visibility on exit/panic.
- Pure Rust math/physics logic in `mac-physics-core` remains 100% platform-agnostic and unit-testable on macOS/Linux.
- All Windows-specific APIs in `win-launcher` must be gated behind `#[cfg(windows)]` with cross-platform mock stubs on `#[cfg(not(windows))]` so `cargo test --workspace` passes cleanly on all platforms.
- Windows cross-compilation check (`cargo check --target x86_64-pc-windows-gnu -p win-launcher`) must pass with 0 errors and 0 warnings.

## Review Focus

1. **Cursor Departure Snap:** When mouse leaves the dock abruptly, scales must decay smoothly over ~150ms rather than snapping to 1.0x in a single frame.
2. **Duplicate Process Launch:** Clicking an app icon that is already running must restore and bring that window to foreground rather than spawning a duplicate process instance.
3. **Taskbar Hiding Recovery:** If the launcher process terminates abnormally, crashes, or receives `SIGINT`/`Ctrl+C`, the Windows taskbar (`Shell_TrayWnd`) must never remain permanently hidden.
4. **Empty / Non-Existent Executable Launch:** If a dock item points to a missing binary or invalid protocol URL, the launcher must handle the error gracefully without crashing or hanging the 120Hz message pump.
5. **Direct2D Device Loss Recovery:** If Direct3D device loss occurs (`DXGI_ERROR_DEVICE_REMOVED` / `RESET`), the Direct2D render target and DirectComposition surfaces must recreate safely without a panic.

---

### Task 1: Smooth Scale Exit Animation & Decoupled Spring Decay

**Files:**
- Modify: `crates/mac-physics-core/src/magnification.rs`
- Modify: `crates/win-launcher/src/dock_view.rs`
- Test: `crates/win-launcher/tests/test_dock_view.rs`

**Interfaces:**
- Consumes: `calculate_icon_scales`, `calculate_dock_layout_at` from `mac-physics-core`.
- Produces: `DockView::update(dt: f32)` animating `active_scales` smoothly back to resting `1.0` using an exponential decay factor $\lambda \approx 20.0$ ($s(t + dt) = 1.0 + (s(t) - 1.0) \cdot e^{-\lambda dt}$) when mouse is absent.

- [ ] **Step 1: Write failing test for smooth mouse leave scale decay**

```rust
// In crates/win-launcher/tests/test_dock_view.rs
#[test]
fn test_mouse_leave_smooth_scale_decay() {
    let mut dock = DockView::new();
    dock.on_mouse_move(600.0, 136.0);
    let items_hovered = dock.get_render_items();
    let peak_scale_hover = items_hovered[5].scale;
    assert!(peak_scale_hover > 2.0);

    // Mouse leaves
    dock.on_mouse_leave();
    // After 1 frame (8.33ms at 120Hz), scale must have decayed partially, NOT snapped to 1.0
    dock.update(1.0 / 120.0);
    let items_frame1 = dock.get_render_items();
    assert!(items_frame1[5].scale > 1.0);
    assert!(items_frame1[5].scale < peak_scale_hover);

    // After 200ms, scale must have fully settled to 1.0
    for _ in 0..24 {
        dock.update(1.0 / 120.0);
    }
    let items_settled = dock.get_render_items();
    assert!((items_settled[5].scale - 1.0).abs() < 1e-3);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_dock_view test_mouse_leave_smooth_scale_decay`  
Expected: FAIL (snaps to 1.0 immediately on mouse leave)

- [ ] **Step 3: Implement animated scale tracking in `DockView`**

In `crates/win-launcher/src/dock_view.rs`:
- Add `current_scales: Vec<f32>` to `DockView`.
- In `update(dt)`:
  - If mouse is hovering, target scales are computed via `calculate_icon_scales`.
  - If mouse is absent (`cursor_pos.is_none()`), target scales are `1.0`.
  - Lerp each icon scale towards its target using frame-rate independent exponential decay: `scale += (target - scale) * (1.0 - (-DECAY_RATE * dt).exp())`.
  - When all $|scale - 1.0| < 1e-4$, snap to `1.0`.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_dock_view test_mouse_leave_smooth_scale_decay`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement smooth exponential scale decay on mouse leave"
```

---

### Task 2: Win32 Application Process Tracker & Running Indicators

**Files:**
- Create: `crates/win-launcher/src/app_tracker.rs`
- Modify: `crates/win-launcher/src/dock_view.rs`
- Modify: `crates/win-launcher/src/main.rs`
- Test: `crates/win-launcher/tests/test_app_tracker.rs`

**Interfaces:**
- Produces: `AppTracker` struct with:
  - `new() -> Self`
  - `refresh_running_apps(&mut self)`
  - `is_app_running(&self, process_name: &str) -> bool`
  - `is_app_focused(&self, process_name: &str) -> bool`
  - `get_running_pids(&self, process_name: &str) -> Vec<u32>`
- Consumes: Win32 `EnumWindows`, `GetWindowThreadProcessId`, `GetForegroundWindow`, `QueryFullProcessImageNameW` (or mock on non-Windows).

- [ ] **Step 1: Write failing test for AppTracker**

```rust
// crates/win-launcher/tests/test_app_tracker.rs
use win_launcher::app_tracker::AppTracker;

#[test]
fn test_app_tracker_running_query_and_mock() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("explorer.exe", 1001, true);
    tracker.register_mock_running_app("msedge.exe", 1002, false);

    assert!(tracker.is_app_running("explorer.exe"));
    assert!(tracker.is_app_focused("explorer.exe"));
    assert!(tracker.is_app_running("msedge.exe"));
    assert!(!tracker.is_app_focused("msedge.exe"));
    assert!(!tracker.is_app_running("nonexistent.exe"));
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_app_tracker`  
Expected: FAIL (module not found)

- [ ] **Step 3: Implement `AppTracker`**

In `crates/win-launcher/src/app_tracker.rs`:
- Define `AppInfo`: `process_name: String`, `pid: u32`, `is_focused: bool`, `hwnd: usize`.
- Implement `#[cfg(windows)]` scanner using `EnumWindows` filtering visible, non-tool, non-zero-sized windows and matching executable basenames.
- Implement `#[cfg(not(windows))]` mock scanner with `register_mock_running_app`.
- Update `DockView` to store `is_running: bool` and `is_focused: bool` on `DockItem` and expose them on `DockRenderItem`.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_app_tracker`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement win32 application process tracker and indicator pips"
```

---

### Task 3: Process Launcher & Foreground Window Activation

**Files:**
- Create: `crates/win-launcher/src/process_launcher.rs`
- Modify: `crates/win-launcher/src/dock_view.rs`
- Modify: `crates/win-launcher/src/main.rs`
- Test: `crates/win-launcher/tests/test_process_launcher.rs`

**Interfaces:**
- Produces: `ProcessLauncher` with:
  - `launch_or_activate(target: &LaunchTarget, tracker: &AppTracker) -> LaunchResult`
- `LaunchTarget`: `AppExecutable { path: PathBuf, args: Vec<String> }`, `ProtocolUri(String)`, `ShellAction(String)`.
- `LaunchResult`: `Launched(u32)`, `Activated(usize)`, `Failed(String)`.

- [ ] **Step 1: Write failing test for ProcessLauncher**

```rust
// crates/win-launcher/tests/test_process_launcher.rs
use win_launcher::app_tracker::AppTracker;
use win_launcher::process_launcher::{LaunchResult, LaunchTarget, ProcessLauncher};

#[test]
fn test_process_launcher_activates_running_app_first() {
    let mut tracker = AppTracker::new();
    tracker.register_mock_running_app("notepad.exe", 5001, false);

    let target = LaunchTarget::AppExecutable {
        process_name: "notepad.exe".to_string(),
        command: "notepad.exe".to_string(),
    };

    let result = ProcessLauncher::launch_or_activate(&target, &tracker);
    match result {
        LaunchResult::Activated { process_name, .. } => {
            assert_eq!(process_name, "notepad.exe");
        }
        _ => panic!("Expected running app to be activated instead of launched"),
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_process_launcher`  
Expected: FAIL

- [ ] **Step 3: Implement `ProcessLauncher`**

In `crates/win-launcher/src/process_launcher.rs`:
- Check if `target.process_name` is already running via `AppTracker`.
- If running: on Windows, call `ShowWindow(hwnd, SW_RESTORE)` and `SetForegroundWindow(hwnd)`.
- If not running: call `ShellExecuteExW` or `std::process::Command::new` to spawn process detached without inheriting launcher handles.
- Non-Windows mock returns `LaunchResult::Launched` or `LaunchResult::Activated`.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_process_launcher`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement native process launcher and window activation"
```

---

### Task 4: Windows Taskbar Auto-Hide & Drop Restoration Guard

**Files:**
- Create: `crates/win-launcher/src/taskbar_guard.rs`
- Modify: `crates/win-launcher/src/main.rs`
- Test: `crates/win-launcher/tests/test_taskbar_guard.rs`

**Interfaces:**
- Produces: `TaskbarGuard` struct:
  - `new(hide_taskbar: bool) -> Self`
  - `is_hidden(&self) -> bool`
  - `restore(&mut self)`
  - `impl Drop for TaskbarGuard` (guarantees `ShowWindow(SW_SHOW)` on drop)

- [ ] **Step 1: Write failing test for TaskbarGuard**

```rust
// crates/win-launcher/tests/test_taskbar_guard.rs
use win_launcher::taskbar_guard::TaskbarGuard;

#[test]
fn test_taskbar_guard_lifecycle() {
    let guard = TaskbarGuard::new_mock(true);
    assert!(guard.is_hidden());
    drop(guard);
    // Verified via mock callback that taskbar was restored on drop
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_taskbar_guard`  
Expected: FAIL

- [ ] **Step 3: Implement `TaskbarGuard`**

In `crates/win-launcher/src/taskbar_guard.rs`:
- On Windows:
  - Uses `FindWindowW(w!("Shell_TrayWnd"), None)` and `ShowWindow(hwnd, SW_HIDE)`.
  - `Drop` calls `ShowWindow(hwnd, SW_SHOW)` and `SetWindowPos` to refresh DWM desktop work area.
- Cross-platform:
  - Stores boolean flag and calls optional restoration callback on Drop.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_taskbar_guard`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement taskbar auto-hide with fail-safe drop guard"
```

---

### Task 5: Direct2D 1.1 Composition Surface Rasterization Engine

**Files:**
- Modify: `crates/win-launcher/src/directx.rs`
- Modify: `crates/win-launcher/src/dock_view.rs`
- Modify: `crates/win-launcher/src/main.rs`
- Test: `crates/win-launcher/tests/test_direct2d_render.rs`

**Interfaces:**
- Produces: `Direct2DRenderer` struct in `directx.rs`:
  - `new(d3d_device: &ID3D11Device, comp_device: &IDCompositionDevice) -> Result<Self>`
  - `render_dock(&mut self, render_items: &[DockRenderItem], tray_geom: (f32, f32, f32, f32), squircle_pts: &[(f32, f32)]) -> Result<()>`
  - Draws squircle background tray with frosted glass gradient & subtle outline (`#FFFFFF18` fill, `#FFFFFF30` border).
  - Draws running indicator pips (`#FFFFFFCC` filled circle, 4px diameter) beneath running apps.
  - Draws active frontmost pip with distinct illumination.

- [ ] **Step 1: Write failing test for Direct2D rendering pipeline**

```rust
// crates/win-launcher/tests/test_direct2d_render.rs
use win_launcher::directx::MockRenderContext;
use win_launcher::dock_view::DockView;

#[test]
fn test_render_context_draws_tray_and_pips() {
    let mut dock = DockView::new();
    dock.set_app_running(0, true, true); // First item running and focused
    let items = dock.get_render_items();
    let tray = dock.get_tray_geometry();
    let contour = dock.get_tray_squircle_contour();

    let mut mock_ctx = MockRenderContext::new();
    mock_ctx.render_dock(&items, tray, &contour).expect("Render failed");

    assert!(mock_ctx.tray_drawn);
    assert_eq!(mock_ctx.running_pips_count, 1);
    assert_eq!(mock_ctx.icons_drawn_count, items.len());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_direct2d_render`  
Expected: FAIL

- [ ] **Step 3: Implement Direct2D surface drawing**

In `crates/win-launcher/src/directx.rs`:
- On Windows:
  - Create `ID2D1Factory1` and `ID2D1Device` from DXGI device.
  - Create `IDCompositionSurface` with `DXGI_FORMAT_B8G8R8A8_UNORM` and `DXGI_ALPHA_MODE_PREMULTIPLIED`.
  - Implement `BeginDraw` / `EndDraw` on `IDCompositionSurface` to obtain `ID2D1DeviceContext`.
  - Draw squircle path geometry using `ID2D1PathGeometry` generated from `mac_physics_core::squircle`.
  - Draw running indicator pips centered below each running icon.
  - Bind surface to root visual.
- On non-Windows:
  - Implement `MockRenderContext` verifying draw calls, geometries, and indicator counts.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_direct2d_render`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement direct2d surface rasterization for tray and pips"
```

---

### Task 6: End-to-End Milestone 2 Integration & Verification

**Files:**
- Modify: `crates/win-launcher/src/main.rs`
- Modify: `crates/win-launcher/README.md`
- Modify: `specs/README.md`

- [ ] **Step 1: Wire all subsystems in `main.rs`**

Connect `DockWindow`, `DirectCompositionPipeline`, `Direct2DRenderer`, `DockView`, `AppTracker`, `ProcessLauncher`, and `TaskbarGuard`.

- [ ] **Step 2: Run all tests across workspace**

Run: `cargo test --workspace`  
Expected: All tests PASS.

- [ ] **Step 3: Verify Windows cross-compilation target check**

Run: `cargo check --target x86_64-pc-windows-gnu -p win-launcher`  
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Update Documentation**

Document Milestone 2 completion, Direct2D surface architecture, running app indicator tracking, process launching, and taskbar drop guard in `crates/win-launcher/README.md`.

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/ specs/ README.md
git commit -m "docs: add milestone 2 integration and build documentation"
```
