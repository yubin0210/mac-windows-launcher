# win-launcher

macOS-fidelity system launcher for Windows powered by Direct3D 11, DirectComposition, Direct2D surface rasterization, and pure Rust physics.

`win-launcher` implements the native Windows presentation and integration layer for the macOS Launcher suite. It creates a borderless, transparent, topmost overlay window backed by DirectComposition, renders frosted Apple squircles and indicator pips via Direct2D, tracks active Windows processes to display running/focused pips, dispatches native process activation/launching, protects Windows taskbar coexistence with a fail-safe Drop guard, and synchronizes animation frames with the Desktop Window Manager (DWM) at up to 120Hz+.

---

## 1. DirectComposition Architecture

`win-launcher` uses the DirectComposition API to achieve zero-latency, tear-free, hardware-accelerated composition directly with Windows DWM.

```
+-------------------------------------------------------------+
|                        Win32 Window                         |
|  (WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_LAYERED)|
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                Direct3D 11 Device & Context                 |
|             (D3D11_CREATE_DEVICE_BGRA_SUPPORT)              |
|        [Hardware Acceleration -> WARP Fallback]             |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                   IDXGIDevice & DCompDevice                 |
|            DCompositionCreateDevice(dxgi_device)            |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|             DirectComposition Visual Tree Target            |
|       dcomp_device.CreateTargetForHwnd(hwnd, topmost=true)  |
|               target.SetRoot(&root_visual)                  |
|                   dcomp_device.Commit()                     |
+-------------------------------------------------------------+
```

### Key Components

1. **Direct3D 11 Device Creation (`D3D11CreateDevice`)**:
   - Initialized with `D3D11_CREATE_DEVICE_BGRA_SUPPORT` for Direct2D / DirectComposition interoperability.
   - Driver hierarchy: attempts `D3D_DRIVER_TYPE_HARDWARE` first; seamlessly falls back to `D3D_DRIVER_TYPE_WARP` in virtual machines, CI runners, or headless test setups.
2. **DXGI Interoperability (`IDXGIDevice`)**:
   - Queries the underlying DXGI device interface from Direct3D 11 to bridge GPU buffers into the Windows composition engine.
3. **DirectComposition Device & Visual Target (`IDCompositionDevice`, `IDCompositionTarget`)**:
   - Bound to the window handle via `CreateTargetForHwnd(hwnd, true)` to create a composition target that renders on top of the window content.
   - Visual nodes (`IDCompositionVisual`) are arranged hierarchically and submitted to the Desktop Window Manager using atomic `Commit()` calls.
4. **Zero GDI Redirection Bitmaps (`WS_EX_NOREDIRECTIONBITMAP`)**:
   - Completely suppresses the standard Win32 GDI redirection bitmap allocation.
   - Eliminates window flicker, black background flashing, and memory duplication during rapid alpha-blended animation frames.

---

## 2. Direct2D Surface Rasterization (`Direct2DRenderer`)

Milestone 2 introduces hardware-accelerated Direct2D surface rasterization bound directly to the DirectComposition visual tree:

```
+-------------------------------------------------------------+
|                  IDCompositionDeviceSurface                 |
|        dcomp_device.CreateSurface(width, height, ...)       |
|                  root_visual.SetContent(surface)            |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|               ID2D1DeviceContext / BeginDraw()              |
|           Clear(0, 0, 0, 0) -> Per-pixel alpha              |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                   Direct2D Drawing Pass                     |
|  1. Fill & Stroke Apple Squircle Tray (ID2D1PathGeometry)   |
|  2. Draw Dock App Icons (Transforms & Bounds)               |
|  3. Draw Running App Indicator Pips (FillEllipse)           |
|  4. Draw Foreground Focus Pips (Enhanced Glow Ellipse)      |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|            EndDraw() & Guaranteed Surface Release           |
|         (RAII EndDraw guard guarantees surface unmap)       |
+-------------------------------------------------------------+
```

### Surface Engineering & Robustness

* **DirectComposition Surface Binding**:
  - `Direct2DRenderer::with_visual` allocates an `IDCompositionSurface` with DXGI format `B8G8R8A8_UNORM` and alpha mode `Premultiplied`.
  - The surface is bound as content to the root visual via `SetContent(&surface)`.
* **Guaranteed `EndDraw` Cleanup**:
  - DirectComposition requires that every `surface.BeginDraw()` call MUST be paired with `surface.EndDraw()` before DWM composition can commit.
  - Rendering utilizes an RAII closure pattern ensuring `EndDraw()` executes even if geometry sinks or brush creation fail mid-frame.
* **Cached Solid Color Brushes**:
  - Direct2D solid color brushes (`tray_brush`, `border_brush`, `pip_brush`, `focus_pip_brush`) are allocated once and cached across frames, eliminating per-frame heap allocations on the 120Hz render loop.
* **Apple Squircle Tray Geometry**:
  - The 64-vertex Lamé superellipse contour ($n = 4.5$) generated by `mac-physics-core` is converted into an `ID2D1PathGeometry` figure.
  - The tray is filled with translucent frosted glass styling (`rgba(240, 240, 245, 0.40)`) and stroked with a crisp specular highlight (`rgba(255, 255, 255, 0.35)`).

---

## 3. Running Application Process Tracker (`AppTracker`)

The `AppTracker` subsystem queries running Windows desktop processes and maps them directly to Dock items to display running status and foreground window focus.

```
+----------------------------------+       +----------------------------------+
|   EnumWindows() + QueryImage     |       |       GetForegroundWindow()      |
|  (Enumerate all Win32 processes) |       |  (Identify active user window)   |
+----------------------------------+       +----------------------------------+
                 |                                          |
                 +--------------------+---------------------+
                                      |
                                      v
                 +------------------------------------------+
                 |       Process Name Normalization         |
                 | - Lowercase case-insensitivity           |
                 | - Extension tolerance (".exe" optional)  |
                 | - Strip directory paths & normalize      |
                 +------------------------------------------+
                                      |
                                      v
                 +------------------------------------------+
                 |       DockView Status Synchronization    |
                 |  item.is_running = tracker.is_running()  |
                 |  item.is_focused = tracker.is_focused()  |
                 +------------------------------------------+
```

### Capabilities & Guarantees

* **Toolhelp32 Process Enumeration**:
  - Periodically enumerates visible desktop windows via `EnumWindows` and resolves executable names via `QueryFullProcessImageNameW`.
  - Fast, lockless in-memory query (<0.1ms) polled at a 500ms cadence to avoid CPU overhead on the render thread.
* **Process Name Normalization**:
  - Matches processes regardless of case, file extensions, or full paths (e.g. `wt.exe`, `wt`, and `C:\Program Files\WindowsApps\...\wt.exe` normalize to `wt.exe`).
* **Foreground Focus Tracking**:
  - Inspects `GetForegroundWindow()` and resolves its owning PID via `GetWindowThreadProcessId`.
  - Dock items matching the focused process display an enhanced focused indicator pip ($r = 2.75\text{px}$, 95% opacity).

---

## 4. Native Process Launcher & Window Activation (`ProcessLauncher`)

When a user clicks a Dock icon, `ProcessLauncher` coordinates between activating existing window instances and launching new applications.

```
User clicks Dock item at (x, y)
             |
             v
+----------------------------+
| Is process already running?|
+----------------------------+
       /              \
     YES               NO
     /                  \
    v                    v
+--------------------+   +------------------------------------+
| Activate Existing  |   | Spawn Application / Shell Action   |
| 1. Find HWND       |   | - AppExecutable: CreateProcessW    |
| 2. ShowWindow()    |   | - ProtocolUri: ShellExecuteW       |
| 3. SetForeground() |   | - ShellAction: Windows Shell dispatch|
+--------------------+   +------------------------------------+
```

### Key Behaviors

1. **Activation First Invariant**:
   - If an application is already running (e.g. `wt.exe` or `explorer.exe`), clicking its Dock icon locates its existing top-level window handle (`HWND`) via `EnumWindows`.
   - Restores minimized windows using `ShowWindow(hwnd, SW_RESTORE)` and brings them to the foreground via `SetForegroundWindow(hwnd)`.
   - Prevents spawning multiple redundant application windows.
2. **Safe Process Launching**:
   - Supports 3 typed targets via `LaunchTarget`:
     - `AppExecutable { process_name, command, args }`: Spawns via `Command` / `CreateProcessW` with safe whitespace quoting for arguments containing spaces.
     - `ProtocolUri(String)`: Dispatches Windows protocol URIs (e.g. `bingmaps:`, `ms-photos:`).
     - `ShellAction(String)`: Triggers system actions (e.g. Start Menu, Action Center).
   - Validates that command paths and targets are non-empty before dispatching.

---

## 5. Fail-Safe Taskbar Auto-Hide Guard (`TaskbarGuard`)

When configured with `--hide-taskbar`, `TaskbarGuard` manages hiding the native Windows taskbar while the macOS Dock launcher is running, with guaranteed fail-safe restoration.

### Safety Invariants & Drop Restoration

```
            [TaskbarGuard::new(hide_taskbar)]
                            |
                            v
               (Find Shell_TrayWnd & Secondary)
              ShowWindow(SW_HIDE) on taskbars
                            |
           +----------------+----------------+
           |                                 |
      Normal Exit                    Panic / Crash
           |                                 |
           +----------------+----------------+
                            |
                            v
             [impl Drop for TaskbarGuard]
       1. ShowWindow(SW_SHOW) on all taskbars
       2. SetWindowPos(SWP_FRAMECHANGED)
       3. Desktop work area refreshed cleanly
```

* **Zero-Orphan Invariant**:
  - The Windows taskbar must NEVER remain hidden permanently if the launcher terminates, crashes, or is dropped.
  - `TaskbarGuard` implements the Rust `Drop` trait to guarantee that `ShowWindow(SW_SHOW)` and `SWP_FRAMECHANGED` are invoked during stack unwinding.
* **Multi-Monitor Taskbar Support**:
  - Enumerates both the primary taskbar (`Shell_TrayWnd`) and all secondary multi-monitor taskbars (`Shell_SecondaryTrayWnd`).
* **Desktop Work Area Preservation**:
  - Upon restoration, `SetWindowPos` with `SWP_FRAMECHANGED` signals the shell to recalculate the desktop work area, ensuring maximized applications properly respect taskbar boundaries.

---

## 6. Smooth Exponential Scale Decay on Mouse Leave

Milestone 2 implements calibrated exponential scale decay for cursor leave transitions:

* **Instantaneous Snapping Eliminated**:
  - When the mouse cursor leaves the Dock window or exits the hover sensitivity band, target scales immediately return to `1.0`.
  - Instead of snapping to resting size abruptly, each icon scale decays exponentially:
    $$s(t + \Delta t) = s(t) + (s_{\text{target}} - s(t)) \cdot \left(1 - e^{-\lambda \Delta t}\right)$$
* **Calibrated Decay Rate**:
  - `DEFAULT_SCALE_DECAY_RATE = 42.0\text{ s}^{-1}` provides a natural, smooth recovery over ~150-200ms.
  - When all icon scales settle within $10^{-4}$ of target, values lock deterministically to `1.0`.

---

## 7. 120Hz VSync Pipeline & Event Loop

High-refresh-rate displays (120Hz, 144Hz, 240Hz) require deterministic frame pacing without visual stutter or physics instability.

```
       [Win32 Message Pump / PeekMessageW]
                       |
                       v
         [Cross-Platform Event Queue]
      (MouseMove, MouseLeave, MouseClick)
                       |
                       v
          [mac-physics-core Simulation]
    (Gaussian Magnification + Damped Spring)
         (Clamped dt = dt.min(0.033))
                       |
                       v
       [Direct2D Surface Render Pass]
      (Squircle Tray + Icons + Indicator Pips)
                       |
                       v
       [DirectComposition Visual Commit]
             (pipeline.commit())
                       |
                       v
            [DwmFlush() Synchronization]
      (Blocks until next vertical blank interval)
```

### Frame Pacing & Stability Guarantees

* **DWM Vertical Blank Synchronization (`DwmFlush`)**:
  - `window.wait_vsync()` invokes `DwmFlush()`, synchronizing each render iteration with the display's vertical blank interval.
  - On a 120Hz ProMotion / VRR panel, this delivers smooth 8.33ms per-frame cadence.
* **Delta-Time Clamping (`dt.min(0.033)`)**:
  - Frame delta times are clamped to a maximum of 33ms (equivalent to a 30 FPS floor).
  - If a background thread spike or OS freeze delays the message pump, the numerical spring simulation does not experience a catastrophic velocity explosion or produce NaN/infinite values.
* **Decoupled Architecture**:
  - Win32 window callbacks run asynchronously via `dock_wndproc` and push typed `WindowEvent` instances into a thread-safe synchronized queue.
  - The render loop drains the queue at frame start, ensuring deterministic state updates regardless of Windows message dispatch order.

---

## 8. Win32 Window Flags & Desktop Coexistence

The Dock window is created with specialized extended window styles (`dwExStyle`) and window styles (`dwStyle`):

| Flag | Purpose |
|---|---|
| `WS_EX_NOREDIRECTIONBITMAP` | Eliminates GDI backbuffer allocation; routes rendering directly through DirectComposition to DWM. |
| `WS_EX_TOPMOST` | Keeps the Dock floating above standard desktop applications without stealing foreground window focus. |
| `WS_EX_TOOLWINDOW` | Prevents the Dock from appearing as an application in the Alt+Tab task switcher or the Windows taskbar. |
| `WS_EX_LAYERED` | Enables per-pixel alpha transparency composition with DWM. |
| `WS_POPUP` | Removes standard window borders, title bars, sizing frames, and system menus. |

### Mouse Tracking & Edge-Leave Decay

* **`TrackMouseEvent` (`TME_LEAVE`)**:
  - When the cursor enters the Dock window bounds, `TrackMouseEvent` is registered with `TME_LEAVE`.
  - When the cursor exits the window at any speed, Windows posts `WM_MOUSELEAVE`.
  - `DockView::on_mouse_leave()` smoothly transitions all icons back to their resting 1.0x scale using exponential decay.
* **Resource Reclamation (`Drop` Guarantee)**:
  - `Win32DockWindow` implements `Drop`, clearing `GWLP_USERDATA` and calling `DestroyWindow` to prevent window handle leaks upon application exit or panic.

---

## 9. Spotlight Launcher Module (`SpotlightWindow`, `AppIndexer`, `SpotlightView`)

Milestone 3 implements the macOS Spotlight search experience natively on Windows:

```
+-------------------------------------------------------------+
|               Global Hotkey Hook (Alt + Space)              |
|        RegisterHotKey(MOD_ALT | MOD_NOREPEAT, VK_SPACE)     |
|          Fallback: RegisterHotKey(MOD_CONTROL, VK_SPACE)     |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                 Topmost Floating Layered Window             |
|          (WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST)         |
|      Centered at 22% screen top, 640px width, 56px height   |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|         Search Indexer & Arithmetic Engine (< 1ms)          |
|  1. Windows Start Menu Apps (ProgramData & AppData .lnk)    |
|  2. Windows 10/11 Settings URIs (ms-settings:display, etc.)  |
|  3. Instant Math Calculator (+, -, *, /, ^, sqrt, trig, abs)|
|  4. Sub-millisecond Fuzzy Matching (prefix & boundary bonus)|
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|         Spotlight State Machine & Navigation View           |
|  - Incremental text input & backspace buffer                |
|  - Toroidal wrap-around arrow selection (rem_euclid)        |
|  - Selection resets to top result on query changes          |
|  - Auto-hide on WM_KILLFOCUS or Escape                      |
|  - Launch activation via ProcessLauncher                    |
+-------------------------------------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                 Direct2D Surface Rasterizer                 |
|  - Frosted Squircle Container Bar (Lamé exponent n=4.5)     |
|  - Vector search magnifying glass icon                      |
|  - Active row highlight pill (#007AFFD9 / macOS blue)       |
|  - Category badge pills (APP, SETTING, CALC)                |
|  - Guaranteed surface unlock via RAII EndDraw               |
+-------------------------------------------------------------+
```

### Key Subsystems:
* **`SpotlightWindow`**: Floating topmost window positioned at screen center horizontally and 22% from the top (matching macOS Spotlight). Registers `Alt + Space` globally with `Ctrl + Space` fallback. Automatically dismisses on `WM_KILLFOCUS` or `VK_ESCAPE`. Prevents mutex reentrancy deadlocks during synchronous `ShowWindow(SW_HIDE)` dismissals.
* **`AppIndexer`**: Recursively crawls Windows Start Menu programs (`ProgramData` and `APPDATA`), filters uninstaller shortcuts, deduplicates application names, and indexes standard Windows 10/11 Settings URIs (`ms-settings:display`, `ms-settings:network`, etc.).
* **`SpotlightView`**: State machine managing search query input buffer, results caching, and toroidal wrap-around navigation (`(cur + delta).rem_euclid(count)`). Any query modification automatically resets selection to the top hero result.
* **`SpotlightRenderer`**: Generates a 64-vertex Lamé superellipse contour ($n=4.5$) for the container background, draws a vector magnifying glass lens, and renders dropdown rows with selection pills and category badges.

---

## 7. macOS Top Menu Bar & System Status Subsystem (Milestone 4)

Milestone 4 delivers the persistent topmost 28px macOS Menu Bar with native Windows AppBar desktop work area reservation, live hardware indicators, active application tracking, and squircle dropdown menus:

```
+----------------------------------------------------------------------------------------------------+
|   Finder  File  Edit  View  Window  Help                     [Wi-Fi] [Battery: 85%] [Vol]  Thu Nov 5 1:20 PM |
+----------------------------------------------------------------------------------------------------+
| +-----------------------+                                                                          |
| | About This Mac        |                                                                          |
| |-----------------------|                                                                          |
| | System Settings...    |                                                                          |
| | App Store...          |                                                                          |
| |-----------------------|                                                                          |
| | Sleep                 |                                                                          |
| | Restart...            |                                                                          |
| | Shut Down...          |                                                                          |
| |-----------------------|                                                                          |
| | Lock Screen           |                                                                          |
| | Log Out...            |                                                                          |
| +-----------------------+                                                                          |
v                                                                                                    v
(Reserved Desktop Work Area: 28px subtracted from top so maximized apps never obscure or overlap bar)
```

### 7.1 Win32 AppBar Work Area Reservation (`menu_bar_window.rs`)
* **Registration**: Window registers via `SHAppBarMessage(ABM_NEW)` with callback `WM_APPBAR_CALLBACK` and edge `ABE_TOP`.
* **Work Area Allocation**: Uses `ABM_QUERYPOS` followed by `ABM_SETPOS` to reserve exactly 28px at screen top, causing Windows to resize the desktop work area (`SPI_SETWORKAREA`). Maximized windows automatically stop at `y = 28px`.
* **Resolution & Taskbar Shifts**: Listens for `ABN_POSCHANGED` notifications via custom `WM_APPBAR_CALLBACK` to dynamically re-adjust coordinates if display resolutions change or taskbars move.
* **Fail-Safe RAII Restoration**: `Win32MenuBarWindow::drop` sends `SHAppBarMessage(ABM_REMOVE)` before destroying the window, ensuring the work area is immediately restored.

### 7.2 Hardware Status Monitor (`system_status.rs`)
* **Battery & Power**: Queries `GetSystemPowerStatus`. Cleanly maps `BatteryLifePercent == 255` to `None` for AC desktop workstations without batteries. Dynamically detects AC charging state.
* **Lightweight Clock Formatter**: Queries `GetLocalTime` on Windows (and system epoch fallback on mock) to format the clock into authentic macOS 12-hour strings (e.g. `"Thu Nov 5 1:20 PM"`).
* **Rate Limiting**: Hardware status queries are throttled to 1-second intervals, consuming 0% idle CPU overhead.

### 7.3 Menu State Machine & Dropdown Model (`menu_bar_view.rs`)
* **Active Application Menus**: Tracks foreground focused application via `AppTracker::focused_app` and dynamically sets the root application menu name.
* **Interaction Fidelity**:
  - Click-to-open dropdown menu.
  - Smooth hover switching: moving across top-level items while a dropdown is open switches to the newly hovered dropdown menu without requiring an additional click.
  - Hover highlight tracking inside dropdown rows, automatically ignoring separators and disabled items.
  - Click-outside dismissal: clicking anywhere outside the open dropdown immediately dismisses it.

### 7.4 Direct2D Surface Rasterization (`menu_bar_render.rs`)
* **Top Bar Surface**: Frosted translucent background (`rgba(28, 28, 30, 0.82)`) with subtle 1px lower separator line.
* **Status Indicators**: Solid battery outline with level fill rectangle and green charging indicator, volume speaker glyph, Wi-Fi arc, and digital clock.
* **Squircle Dropdowns**: Rendered with $n=4.0$ continuous curvature superellipse contour, subtle border stroke, selection pills, and 1px separator lines.
* **Guaranteed `EndDraw` Cleanup**: Immediate closure pattern ensures `surface.EndDraw()` executes and combines error results on all render passes.

---

## 10. Build and Run Instructions

### Prerequisites

* Rust 2021 toolchain (v1.75+)
* On Windows: Visual Studio C++ Build Tools (MSVC) or MinGW (GNU)
* On macOS / Linux: standard Rust toolchain (unit tests and mock simulation run cross-platform)

### Building on Windows

```powershell
# Debug build
cargo build -p win-launcher

# Optimized release build
cargo build -p win-launcher --release
```

### Running on Windows

```powershell
# Standard run (taskbar visible)
cargo run -p win-launcher --release

# Run with taskbar auto-hide enabled
cargo run -p win-launcher --release -- --hide-taskbar
# Alternatively, via environment variable:
# $env:WIN_LAUNCHER_HIDE_TASKBAR="1"; cargo run -p win-launcher --release
```

### Cross-Compiling / Cross-Checking from macOS

```bash
# Check Windows target using MinGW GNU toolchain
cargo check --target x86_64-pc-windows-gnu -p win-launcher

# Check host target (mock DirectComposition & Direct2D pipeline)
cargo check -p win-launcher

# Run all workspace tests (cross-platform mock test suite)
cargo test --workspace
```

### Running the Demonstration Simulation (Cross-Platform)

Running `win-launcher` on macOS executes the headless mock pipeline, verifying taskbar guard lifecycle, active process tracking, hover magnification, click launching and window activation, 120Hz bounce physics, smooth exponential scale decay, Direct2D squircle surface rasterization, and DirectComposition commit:

```bash
cargo run -p win-launcher
```

Expected output:
```text
=== macOS Dock & Spotlight System Launcher (DirectComposition 120Hz) ===
Platform: Non-Windows host (mock DirectComposition & Direct2D mode)
TaskbarGuard initialized: auto-hide active (is_hidden: true, managed_taskbars: 1)
Dock items loaded: 10 (active processes: 2, focused: 1)
Tray resting geometry: x=301.0, y=104.0, w=598.0, h=64.0
Simulating cursor hover at (600.0, 136.0)...
Hover active: peak icon 'Maps' (index 5) scaled to 2.64x (126.5px)
Simulating click on non-running app 'Maps' to launch...
  ProcessLauncher dispatched target 'bingmaps:': Launched { process_name: "bingmaps:", pid: None }
Simulating click on running app 'Terminal' (index 8) to activate existing window...
  ProcessLauncher activated target 'wt.exe': Activated { process_name: "wt.exe", hwnd: 1002 }
Simulating 5 frames of damped harmonic bounce physics at 120Hz...
  Bounce frame 1: offset = -2.82px, is_bouncing = true
  Bounce frame 2: offset = -5.31px, is_bouncing = true
  Bounce frame 3: offset = -7.45px, is_bouncing = true
  Bounce frame 4: offset = -9.27px, is_bouncing = true
  Bounce frame 5: offset = -10.78px, is_bouncing = true
Simulating cursor leaving dock window (WM_MOUSELEAVE)...
Simulating smooth exponential scale decay across 5 frames...
  Decay frame 1: peak icon 'Maps' scale = 1.835x (88.1px)
  Decay frame 2: peak icon 'Maps' scale = 1.427x (68.5px)
  Decay frame 3: peak icon 'Maps' scale = 1.218x (58.5px)
  Decay frame 4: peak icon 'Maps' scale = 1.111x (53.3px)
  Decay frame 5: peak icon 'Maps' scale = 1.057x (50.7px)
Direct2D surface rasterized: tray_drawn=true, icons=10, running_pips=2, focused_pips=1, squircle_pts=64
DirectComposition mock commit count: 1
Testing TaskbarGuard Drop / manual restoration...
  [TaskbarGuard] Restoration hook fired: taskbars restored to desktop work area.
TaskbarGuard is_hidden after restore: false

--- Milestone 3: Spotlight Launcher Demonstration ---
12. AppIndexer loaded 18 applications & settings items.
13. Simulating global hotkey (Alt + Space) press...
  Spotlight floating window opened (is_visible: true, height: 56px)
14. Simulating math expression input: '120 * 60 + 500'...
  Instant Calculation Result: '7700' (= 120 * 60 + 500)
15. Simulating application fuzzy search: 'term'...
  Found 1 matches for 'term':
    [0] Terminal (Command Line Interface) - target: wt.exe
  Navigated Down: selected_index = 0
  Navigated Up: selected_index = 0
  Simulating Enter key: executing 'Terminal'...
  Execution dispatched: Activated { process_name: "wt.exe", hwnd: 1002 }
16. Spotlight Direct2D rasterized: 640px floating squircle (n=4.5), height=104px, results=1.
  Spotlight window dismissed (is_visible: false)

--- Milestone 4: Top Menu Bar & System Status Demonstration ---
17. MenuBarWindow initialized: height=28px, is_appbar_registered=true
  System status queried: clock='Thu Nov 5 1:20 PM', battery=Some(85)%, is_charging=false, wifi=true
  Active app initially: 'Finder'
  Active app updated on foreground window switch: 'Visual Studio Code'
  Simulating click on Apple logo (x: 18.0, y: 14.0)...
  Dropdown opened: 'apple' with 11 items
  Dropdown action executed: Some(AboutThisMac)
18. MenuBar Direct2D rasterized: 13 elements drawn, frames=1

Demo completed in 2.91ms. Ready for Windows deployment.
```
