# macOS UI/UX System Launcher on Windows (Rust + DirectComposition Native) Design Document

**Date:** 2026-11-03  
**Status:** In Review  
**Author:** Pair Programming Agent & User  

---

## 1. Intent & The Why

Many users who switch between macOS and Windows, or who appreciate the refined productivity and visual elegance of macOS, miss core macOS desktop interactions:
* The responsive, spring-physics **Dock** with smooth magnification and running-indicator pips.
* The keyboard-centric **Spotlight** launcher with zero-latency fuzzy search.
* The intuitive window grouping of **Stage Manager**.
* The clean, unified **Top Menu Bar**.

Existing Windows skin packs and docks often suffer from sluggish rendering (GDI/WPF lag), high RAM/CPU usage, blurred/misaligned icons, or fragile window hooking that crashes under load. This project aims to build a **production-grade, ultra-lightweight, 120Hz-native Windows system launcher suite** that faithfully reconstructs macOS interactions down to the exact pixel metrics and animation physics, reverse-engineered directly from real macOS system binaries using **REA**.

---

## 2. Goals, Non-Goals, & Anti-Goals

### Goals
* **True 1:1 macOS Fidelity:** Extract exact layout metrics (padding, squircle radii, shadow diffusion, blur radius) and physics constants (spring mass, stiffness, damping ratio, magnification curve) from actual macOS binaries (`Dock.app`, `Spotlight.app`).
* **Ultra-low Resource Footprint:** Standalone native binary (Rust + `windows-rs`) consuming < 40MB idle RAM and < 0.1% CPU when idle.
* **120Hz+ Butter-Smooth Rendering:** Rendered directly via Direct2D and DirectComposition with hardware acceleration and proper alpha-channel blending.
* **Full Shell Integration:** Proper Windows AppBar registration (`SHAppBarMessage`), multi-monitor DPI awareness, multi-window DWM thumbnail streaming (`DwmRegisterThumbnail`), and non-intrusive global hooks.
* **Modular Architecture:** Dock, Spotlight, Stage Manager, and Menu Bar can be enabled/disabled independently in a lightweight configuration.

### Non-Goals
* Full Windows Shell replacement (killing or disabling `explorer.exe` is not required; the app coexists or hides taskbar gracefully).
* Emulating macOS APIs (this is not an Obj-C/Cocoa runtime emulator like Darling; it is a native Windows app replicating the UI/UX).
* Replacing complex third-party app window frames with Mac traffic-light buttons (out of scope for launcher utilities).

### Anti-Goals
* **No Web Views or Heavy Runtimes:** No Electron, no WebView2, no Chromium processes running in the background.
* **No High-Poll System Hooks:** Avoid polling loops or aggressive polling hooks that cause frame drops in full-screen games or video playback.
* **No Destructive Registry/System File Modifications:** Must run entirely as a standalone user-space utility without patching system DLLs.

---

## 3. Constraints

* **Host Platform:** Windows 10 (1809+) and Windows 11 (64-bit / ARM64 compatible).
* **Display Support:** Dynamic per-monitor DPI scaling (v2 DPI awareness), mixed-refresh-rate multi-monitor setups.
* **Development & Reverse Engineering Platform:** macOS (current local machine) with REA (6.8.0) used for asset and binary analysis, cross-compiling or building with Rust (`x86_64-pc-windows-msvc` / `cargo`).
* **Graphics API:** DirectX 11 / Direct2D 1.1 / DirectComposition for layered transparent window composition and DWM thumbnail management.

---

## 4. Approaches Considered & Tradeoffs

| Approach | Technology | Pros | Cons / Tradeoffs | Decision |
|---|---|---|---|---|
| **Approach 1** | C# / .NET 9 + WPF / WinUI 3 | Fast UI authoring, easy P/Invoke, mature sample ecosystems (PowerToys). | 60~100MB+ RAM footprint, GC pauses during 120Hz cursor hover, large runtime bundle. | Rejected |
| **Approach 2 (Chosen)** | **Rust + Direct2D / DirectComposition + Win32** | **< 30MB RAM, 0% idle CPU, zero GC spikes, microsecond response, single static binary, perfect hardware composition.** | **Higher implementation complexity (custom layout, text rendering, math).** | **Selected** |
| **Approach 3** | Tauri v2 (Rust + React/CSS) | Fast CSS/Glass styling, easy layout. | WebView2 transparent window border clipping, inability to stream 60fps DWM thumbnails into DOM. | Rejected |

**Why Approach 2 was chosen:** The user explicitly prioritized authentic, 100% smooth, latency-free desktop execution without the memory bloat and input latency typical of web-based or heavy runtime skin utilities.

---

## 5. Architectural Specification & Phases

```
+-------------------------------------------------------------------------+
|                        REA Extraction Pipeline                          |
|  /System/Library/CoreServices/Dock.app & Spotlight.app (macOS Host)     |
|  --> UI Specs (JSON), Animation Math, Extracted Asset Vectors           |
+-------------------------------------------------------------------------+
                                    |
                                    v
+-------------------------------------------------------------------------+
|                Core Runtime Architecture (Rust / Win32)                 |
|                                                                         |
|  +------------------------+  +---------------------------------------+  |
|  |     Win32 Shell Layer  |  |      DirectComposition Render Engine  |  |
|  | - SHAppBarMessage      |  | - Direct2D DirectContext & DirectWrite|  |
|  | - SetWinEventHook      |  | - Acrylic / Mica / Gaussian Blur      |  |
|  | - DwmRegisterThumbnail |  | - 120Hz VSync Spring Physics Loop    |  |
|  +------------------------+  +---------------------------------------+  |
|                                    |                                    |
|          +-------------------------+-------------------------+          |
|          |                         |                         |          |
|          v                         v                         v          |
|  +---------------+         +---------------+         +---------------+  |
|  |  Dock Module  |         |Spotlight Module|        |Stage Manager  |  |
|  | Magnification |         | Global Hotkey |         | Window Groups |  |
|  | Bounce Physics|         | Fuzzy Indexer |         | DWM Live Thumb|  |
|  | App Indicators|         | Quick Actions |         | Focus Handler |  |
|  +---------------+         +---------------+         +---------------+  |
+-------------------------------------------------------------------------+
```

### Phase 0: REA Reverse-Engineering & Spec Extraction
* **Target Binaries:**
  * `/System/Library/CoreServices/Dock.app/Contents/MacOS/Dock`
  * `/System/Library/CoreServices/Dock.app/Contents/Resources/Assets.car`
  * `/System/Library/CoreServices/Spotlight.app`
* **Deliverables:**
  * `specs/dock-metrics.json`: Base icon size (typically 48/64px), max magnification size (128px), spread Gaussian distribution width ($\sigma$), icon padding, border radius (squircle curvature exponent $n \approx 4.5$ or 5.0), shadow blur/offset.
  * `specs/dock-physics.json`: Spring parameters (mass $m$, stiffness $k$, damping $c$) for icon bounce on launch and item insertion.
  * Extracted SVG/PNG assets: Trash empty/full, separator cartouches, indicator dots.

### Phase 1: Rust Core & DirectComposition Rendering Pipeline
* **Crates:** `windows` (Win32, Direct2D, Direct3D11, DirectComposition, DWM), `raw-window-handle`, `serde`, `crossbeam-channel`.
* **Window Architecture:**
  * Frameless layered window (`WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW`).
  * DirectComposition visual tree mapping directly to hardware overlays to prevent black borders or flicker.
  * Real-time blur effect using `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)` or custom D2D Gaussian blur filter.
* **Multi-Monitor & DPI:**
  * `WM_DPICHANGED` handler dynamically re-rasterizing vector icons at crisp physical resolutions.
  * Shell AppBar registration reserving screen edge or floating above taskbar based on user preference.

### Phase 2: macOS Dock Module
* **Icon Magnification Engine:**
  * Math: Gaussian bell curve modulation $s(x) = s_{base} + (s_{max} - s_{base}) \cdot \exp\left(-\frac{(x - x_{cursor})^2}{2\sigma^2}\right)$.
  * Continuous translation interpolation so adjacent icons glide smoothly as the cursor moves across the Dock.
* **App Process Tracking:**
  * Win32 `RegisterShellHookWindow` listening for `HSHELL_WINDOWCREATED`, `HSHELL_WINDOWDESTROYED`, and `HSHELL_RUDEAPPACTIVATED` to show running pips under active application icons.
* **Launch & Bounce:**
  * Harmonic damped oscillator step function updating vertical offset: $\ddot{y} + 2\zeta\omega_n \dot{y} + \omega_n^2 y = 0$.

### Phase 3: Spotlight Launcher Module
* **Trigger:** Global hotkey `Win + Space` (or user-configurable `Alt + Space`).
* **Window Behavior:** Centered floating window with instant keyboard focus, escaping on loss of focus or Esc key.
* **Search Engine:**
  * Low-overhead in-memory SQLite / Tantivy or Windows Search Indexer (`ISearchCatalogManager`) integration.
  * Instant calculation parser (e.g. `evalexpr`) and unit conversion.
  * Rich preview pane on the right side of the results list.

### Phase 4: Stage Manager & Menu Bar
* **Stage Manager:**
  * Hook window z-order and minimize/restore events via `SetWinEventHook`.
  * Left-screen flyout container holding active window groups.
  * Uses `DwmRegisterThumbnail` to render live interactive thumbnail previews without consuming extra GPU readback memory.
* **Top Menu Bar (Optional / Toggleable):**
  * Persistent top AppBar showing system status items (clock, battery, Wi-Fi, audio) and global search launcher.

---

## 6. Development, Toolchain & Coexistence Decisions

### Build & Toolchain Architecture (Builder Decision)
To ensure reliable development and testing both on macOS and Windows:
* **Cargo Workspace Structure:**
  * `crates/mac-spec-extractor`: Runs directly on macOS using REA to parse `/System/Library/CoreServices/Dock.app` and output `dock-metrics.json`.
  * `crates/mac-physics-core`: Pure Rust, platform-agnostic math and physics engine (Gaussian magnification, damped harmonic oscillator bounce, squircle curvature algorithms). Fully unit-testable directly on macOS (`cargo test`).
  * `crates/win-launcher`: Native Windows application implementing DirectComposition rendering, DWM thumbnail management, and Win32 shell hooks. Cross-compilable from macOS via `cargo build --target x86_64-pc-windows-gnu` or compiled natively on a Windows host/VM.

### Taskbar Coexistence Model (Builder Decision)
* **Default Behavior:** The Dock renders as a floating topmost layered window (`WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`) resting right above the bottom screen margin.
* **Taskbar Auto-Hide Integration:** If the user sets `hide_windows_taskbar = true` in `config.toml`, the application safely calls `ShowWindow(FindWindow("Shell_TrayWnd", NULL), SW_HIDE)`.
* **Crash & Exit Safety:** An exit handler / `Drop` guard guarantees `SW_SHOW` is restored upon normal exit or panic, preventing the user from ever losing their Windows taskbar.

### Implementation Phasing Strategy (Builder Decision)
* The execution plan followed progressive milestones with complete TDD test coverage and cross-compilation validation:
  * **Milestone 1:** REA extraction + Physics math core + Minimal Dock prototype with magnification. (COMPLETED)
  * **Milestone 2:** Running app indicators, process launching, taskbar auto-hide guard, and Direct2D surface rasterizer. (COMPLETED)
  * **Milestone 3:** Spotlight launcher (global hotkey Alt+Space, instant calculation, and fuzzy search). (COMPLETED)
  * **Milestone 4:** Top Menu Bar & System Status (Win32 AppBar 28px work area reservation, hardware status, and squircle dropdowns). (COMPLETED)
  * **Milestone 5:** Stage Manager & Window Grouping (left flyout shelf, multi-window stack fan-out, live DWM thumbnail streaming via `DwmRegisterThumbnail`, and focus switching). (COMPLETED)

---

## 7. What's Left to the Builder

* Selection of specific icon extraction format (DirectWrite font glyphs vs. SVG vs. PNG texture atlas).
* Exact default hotkey resolution on Windows (`Win + Space` conflicts with Windows Input Language switcher, so builder will provide an automatic fallback to `Alt + Space` with option to remap).
* Configuration file format (`config.toml`) located at `%APPDATA%/MacLauncher/config.toml`.

