# macOS UI/UX System Launcher on Windows — Milestone 4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Milestone 4: macOS Top Menu Bar & System Status Subsystem (persistent topmost Win32 AppBar reserved at screen top, Apple logo menu, active application title and menus, right-aligned system status indicators [clock, battery, Wi-Fi, audio, control center trigger], squircle dropdown menus, and Direct2D surface rasterization).

**Architecture:**
Expands the system with:
1. `mac-physics-core::menu_bar`: Pure Rust domain models, layout math, and dropdown squircle metrics for the 28px top bar.
2. `win-launcher::system_status`: Hardware & OS status queries (battery level/charging via `GetSystemPowerStatus`, local time clock, network status, volume state) with cross-platform mock fallback.
3. `win-launcher::menu_bar_window`: Win32 AppBar registration (`SHAppBarMessage` `ABM_NEW` / `ABM_SETPOS` `ABE_TOP`) auto-reserving screen top with fail-safe RAII `Drop` restoration, and DirectComposition layered window pipeline.
4. `win-launcher::menu_bar_view`: State machine coordinating active app name (via `AppTracker`), dropdown menu open/close transitions, click-outside dismissal, and menu actions.
5. `win-launcher::menu_bar_render`: Direct2D frosted glass surface rasterizer for the 28px menu bar, icons, text labels, and dropdown menus.
6. Integration in `main.rs`: Full unified lifecycle combining Dock, Spotlight, and Top Menu Bar.

**Tech Stack:** Rust (edition 2021), `windows` crate (0.58), `mac-physics-core`, `serde`, Win32 Shell AppBar & Direct2D.

**Spec:** [`docs/superpowers/specs/2026-11-03-macos-windows-launcher-design.md`](../specs/2026-11-03-macos-windows-launcher-design.md)

---

## Global Constraints

- Standalone native binary with zero web runtimes (no Electron, no Chromium, no WebView2).
- Pure Rust physics/layout core in `mac-physics-core` remains 100% platform-independent and unit-testable on macOS/Linux.
- Height: exact macOS standard 28px (`DEFAULT_MENU_BAR_HEIGHT = 28.0`).
- Win32 AppBar integration must reserve desktop work area so maximized windows respect the menu bar, with guaranteed RAII `Drop` restoration (`ABM_REMOVE`) on normal exit, termination, or panic.
- All Windows-specific APIs gated behind `#[cfg(windows)]` with cross-platform mock stubs on `#[cfg(not(windows))]` so `cargo test --workspace` passes cleanly.
- Windows cross-compilation check (`cargo check --target x86_64-pc-windows-gnu -p win-launcher`) must pass with 0 errors and 0 warnings.

## Review Focus

1. **AppBar Desktop Work Area Safety:** If the menu bar crashes or exits, the desktop work area must be cleanly restored via `ABM_REMOVE` so maximized windows are not permanently displaced.
2. **Multi-Monitor Coordinate Alignment:** When primary monitor resolution changes, AppBar position and width must dynamically resize without overlapping taskbars.
3. **Active Application Title Synchronization:** When the user switches foreground apps, the menu bar must update the application title in < 50ms without lagging the UI.
4. **Dropdown Menu Outside Click Dismissal:** Opening a menu and clicking outside must immediately dismiss the dropdown and release mouse capture.
5. **Battery Status Query Safety:** On desktop PCs without a battery, `GetSystemPowerStatus` queries must cleanly report AC power without erroring or displaying phantom percentages.

---

### Task 1: Menu Bar Core Models & Layout Geometry (`mac-physics-core`)

**Files:**
- Create: `crates/mac-physics-core/src/menu_bar.rs`
- Modify: `crates/mac-physics-core/src/lib.rs`
- Test: `crates/mac-physics-core/tests/test_menu_bar_core.rs`

**Interfaces:**
- Produces:
  - `MenuBarMetrics`: `height: f32` (28.0), `padding_h: f32` (12.0), `item_spacing: f32` (16.0), `font_size: f32` (13.0)
  - `MenuBarGroup`: `Left`, `Right`
  - `MenuBarItem`: `id: String`, `label: String`, `group: MenuBarGroup`, `is_bold: bool`, `has_dropdown: bool`
  - `SystemStatusSnapshot`: `battery_percent: Option<u8>`, `is_charging: bool`, `wifi_connected: bool`, `volume_percent: u8`, `time_text: String`
  - `compute_menu_bar_layout(items: &[MenuBarItem], status: &SystemStatusSnapshot, screen_width: f32) -> MenuBarLayout`

- [ ] **Step 1: Write failing unit tests for menu bar core models and layout**

```rust
// crates/mac-physics-core/tests/test_menu_bar_core.rs
use mac_physics_core::menu_bar::{
    compute_menu_bar_layout, MenuBarGroup, MenuBarItem, MenuBarMetrics, SystemStatusSnapshot,
};

#[test]
fn test_menu_bar_layout_left_and_right_distribution() {
    let items = vec![
        MenuBarItem::apple_logo(),
        MenuBarItem::new("app", "Finder", MenuBarGroup::Left, true, true),
        MenuBarItem::new("file", "File", MenuBarGroup::Left, false, true),
        MenuBarItem::new("edit", "Edit", MenuBarGroup::Left, false, true),
    ];
    let status = SystemStatusSnapshot::new(Some(85), false, true, 60, "12:00 PM");
    let layout = compute_menu_bar_layout(&items, &status, 1920.0);

    assert_eq!(layout.height, MenuBarMetrics::default().height);
    assert!(!layout.left_items.is_empty());
    assert!(!layout.right_items.is_empty());
    assert!(layout.right_items.last().unwrap().bounds.x > 1500.0);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p mac-physics-core --test test_menu_bar_core`  
Expected: FAIL (unresolved import `menu_bar`)

- [ ] **Step 3: Implement `mac-physics-core::menu_bar`**

In `crates/mac-physics-core/src/menu_bar.rs`:
- Implements `MenuBarMetrics`, `MenuBarItem`, `MenuBarGroup`, `SystemStatusSnapshot`.
- Implements `compute_menu_bar_layout` calculating bounding boxes for left-aligned app menus and right-aligned status items (clock, battery, Wi-Fi, audio).
- Re-export in `crates/mac-physics-core/src/lib.rs`.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p mac-physics-core --test test_menu_bar_core`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/mac-physics-core/
git commit -m "feat(physics): implement menu bar models layout geometry and status snapshot"
```

---

### Task 2: System Status Monitor (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/system_status.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_system_status.rs`

**Interfaces:**
- Produces `SystemStatusTracker`:
  - `new() -> Self`
  - `refresh(&mut self) -> SystemStatusSnapshot`
  - `snapshot(&self) -> &SystemStatusSnapshot`
  - `format_clock() -> String` (e.g. "Thu Nov 5 12:23 PM")

- [ ] **Step 1: Write failing unit test for system status tracker**

```rust
// crates/win-launcher/tests/test_system_status.rs
use win_launcher::system_status::SystemStatusTracker;

#[test]
fn test_system_status_tracker_clock_and_battery() {
    let mut tracker = SystemStatusTracker::new_mock();
    let snapshot = tracker.refresh();
    assert!(!snapshot.time_text.is_empty());
    assert!(snapshot.volume_percent <= 100);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_system_status`  
Expected: FAIL

- [ ] **Step 3: Implement `SystemStatusTracker`**

In `crates/win-launcher/src/system_status.rs`:
- On Windows: Queries `GetSystemPowerStatus` for battery and charging status. Formats local time via `GetLocalTime`.
- On non-Windows: Mock status provider returning simulated battery, Wi-Fi, volume, and clock.
- Throttles hardware queries to 1-second intervals to minimize CPU overhead.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_system_status`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement win32 system status monitor and clock formatter"
```

---

### Task 3: Win32 AppBar Window & Work Area Reservation (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/menu_bar_window.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_menu_bar_window.rs`

**Interfaces:**
- Produces `MenuBarWindow`:
  - `create(config: MenuBarWindowConfig) -> Result<Self, String>`
  - `new_mock() -> Self`
  - `is_open(&self) -> bool`
  - `poll_events(&mut self) -> Vec<MenuBarWindowEvent>`
  - `hwnd(&self) -> isize`
  - `width(&self) -> u32`, `height(&self) -> u32`
- RAII `Drop` automatically sends `SHAppBarMessage(ABM_REMOVE)` to restore top screen edge.

- [ ] **Step 1: Write failing unit test for MenuBarWindow**

```rust
// crates/win-launcher/tests/test_menu_bar_window.rs
use win_launcher::menu_bar_window::{MenuBarWindow, MenuBarWindowConfig};

#[test]
fn test_menu_bar_window_lifecycle_and_mock() {
    let win = MenuBarWindow::new_mock();
    assert!(win.is_open());
    assert_eq!(win.height(), 28);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_menu_bar_window`  
Expected: FAIL

- [ ] **Step 3: Implement `MenuBarWindow`**

In `crates/win-launcher/src/menu_bar_window.rs`:
- On Windows:
  - Registers window class `MacWinLauncherMenuBarClass`.
  - Creates window with `WS_POPUP`, `WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW`.
  - Sends `SHAppBarMessage(ABM_NEW)` and `SHAppBarMessage(ABM_SETPOS)` with `ABE_TOP`, reserving the top 28px work area so maximized apps do not obscure the menu bar.
  - Implements `Drop` sending `SHAppBarMessage(ABM_REMOVE)` to guarantee desktop work area restoration.
- On non-Windows: High-fidelity mock tracking window state and event queues.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_menu_bar_window`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement win32 appbar top edge window and work area reservation"
```

---

### Task 4: Menu Bar State Machine & Dropdown Menu Model (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/menu_bar_view.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_menu_bar_view.rs`

**Interfaces:**
- Produces `MenuBarView`:
  - `new() -> Self`
  - `update_active_app(&mut self, app_name: &str)`
  - `on_mouse_move(&mut self, x: f32, y: f32)`
  - `on_mouse_click(&mut self, x: f32, y: f32) -> Option<MenuBarAction>`
  - `open_dropdown(&mut self, item_id: &str)`
  - `close_dropdown(&mut self)`
  - `active_dropdown(&self) -> Option<&MenuDropdown>`

- [ ] **Step 1: Write failing unit test for MenuBarView**

```rust
// crates/win-launcher/tests/test_menu_bar_view.rs
use win_launcher::menu_bar_view::{MenuBarAction, MenuBarView};

#[test]
fn test_menu_bar_view_active_app_and_dropdown() {
    let mut view = MenuBarView::new();
    view.update_active_app("Visual Studio Code");
    assert_eq!(view.active_app_title(), "Visual Studio Code");

    // Open Apple menu
    view.on_mouse_click(18.0, 14.0);
    assert!(view.is_dropdown_open());

    // Click outside closes dropdown
    view.on_mouse_click(500.0, 300.0);
    assert!(!view.is_dropdown_open());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_menu_bar_view`  
Expected: FAIL

- [ ] **Step 3: Implement `MenuBarView`**

In `crates/win-launcher/src/menu_bar_view.rs`:
- Tracks active application name, updating menus dynamically ("Code", "File", "Edit", "View", "Help").
- Built-in Apple Menu items ("About This PC", "System Settings...", "Restart...", "Shut Down...").
- Manages dropdown open/closed states, selected row index, and click outside dismissal.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_menu_bar_view`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement menu bar state machine and dropdown menu controller"
```

---

### Task 5: Direct2D Menu Bar Surface Rasterizer (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/menu_bar_render.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_menu_bar_render.rs`

**Interfaces:**
- Produces `MenuBarRenderer` and `MockMenuBarRenderContext`:
  - `render_menu_bar(&mut self, layout: &MenuBarLayout, view: &MenuBarView, width: f32, height: f32) -> Result<(), RenderError>`
  - Draws frosted glass bar (`rgba(22, 24, 28, 0.85)`), bottom 1px specular line, Apple logo, active app label, status icons, and active dropdown menu with squircle contour.

- [ ] **Step 1: Write failing unit test for MenuBarRenderer**

```rust
// crates/win-launcher/tests/test_menu_bar_render.rs
use win_launcher::menu_bar_render::MockMenuBarRenderContext;
use win_launcher::menu_bar_view::MenuBarView;

#[test]
fn test_menu_bar_renderer_draws_bar_and_items() {
    let mut mock_ctx = MockMenuBarRenderContext::new();
    let view = MenuBarView::new();
    mock_ctx.render_menu_bar(&view.layout(), &view, 1920.0, 28.0).expect("Render succeeds");
    assert!(mock_ctx.bar_drawn);
    assert!(mock_ctx.items_drawn_count > 0);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_menu_bar_render`  
Expected: FAIL

- [ ] **Step 3: Implement `MenuBarRenderer`**

In `crates/win-launcher/src/menu_bar_render.rs`:
- On Windows: Uses Direct2D path geometries, cached brushes, and DirectComposition surface to draw the 28px bar, Apple logo, menu items, status indicators, and dropdown squircle popup. Guaranteed `EndDraw` invocation.
- On non-Windows: `MockMenuBarRenderContext` tracking dimensions and drawn components.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_menu_bar_render`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement direct2d menu bar and dropdown rasterizer"
```

---

### Task 6: End-to-End Milestone 4 Integration & Verification

**Files:**
- Modify: `crates/win-launcher/src/main.rs`
- Modify: `crates/win-launcher/README.md`
- Modify: `specs/README.md`

- [ ] **Step 1: Wire Menu Bar into `main.rs`**

Coordinate the 3 unified subsystems:
- Top: `MenuBarWindow` (28px AppBar reserved top edge).
- Bottom: `DockWindow` (120Hz DirectComposition layered Dock).
- Center: `SpotlightWindow` (floating centered hotkey search overlay).
Update cross-platform simulation to demonstrate menu bar clock, battery query, active app title change, and dropdown interaction.

- [ ] **Step 2: Run all workspace tests**

Run: `cargo test --workspace`  
Expected: All tests PASS.

- [ ] **Step 3: Verify Windows cross-compilation**

Run: `cargo check --target x86_64-pc-windows-gnu -p win-launcher`  
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Update Documentation**

Document Top Menu Bar AppBar architecture, work area reservation, status monitors, and lifecycle safety in `crates/win-launcher/README.md` and `specs/README.md`.

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/ specs/
git commit -m "feat(launcher): integrate top menu bar into runtime and update documentation"
```
