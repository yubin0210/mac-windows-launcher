# macOS System Launcher on Windows (Rust + DirectComposition 120Hz)

<div align="center">

**Ultra-lightweight, 120Hz-native system utility bringing true macOS UI/UX fidelity to Windows 10 & 11.**

[![CI](https://github.com/yubin0210/mac-windows-launcher/actions/workflows/ci.yml/badge.svg)](https://github.com/yubin0210/mac-windows-launcher/actions)
[![Release](https://github.com/yubin0210/mac-windows-launcher/actions/workflows/release.yml/badge.svg)](https://github.com/yubin0210/mac-windows-launcher/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

*Zero web runtimes (No Electron, No WebView2, No Chromium). < 35MB Idle RAM. 0% Idle CPU.*

</div>

---

## Key Features

### 1. macOS Dock with Exact Physics
- **Gaussian Magnification Curve**: Reverse-engineered Gaussian bell-curve magnification with continuous translation interpolation.
- **Damped Harmonic Oscillator Bounce**: True harmonic physics ($\ddot{y} + 2\zeta\omega_n \dot{y} + \omega_n^2 y = 0$) on icon launch and bounce notifications.
- **Squircle Geometry**: Mathematical superellipse ($n=4.5$) frosted glass tray.
- **Process Tracker & Indicator Pips**: Dynamic detection of running apps with running pips and focused pips.
- **Fail-Safe Taskbar Guard**: Optional `--hide-taskbar` flag with RAII `Drop` restoration ensuring the Windows taskbar is never lost.

### 2. Spotlight Search Launcher
- **Instant Hotkey**: `Alt + Space` global shortcut.
- **Real-Time Calculator**: Instant expression evaluation (`evalexpr`) directly inside the search bar.
- **Fuzzy Search Indexer**: Low-overhead in-memory application and settings indexer with acronym and boundary matching.
- **Direct2D Floating Squircle Bar**: Hardware-accelerated floating search bar with keyboard arrow navigation.

### 3. Top Menu Bar & System Status
- **Win32 AppBar Reservation**: Automatically reserves the top 28px desktop work area (`ABE_TOP`), keeping normal windows below the bar.
- **Hardware Status Monitors**: Real-time battery percent & charging status, system clock, Wi-Fi connectivity, and volume.
- **Apple & Application Menus**: Dynamic title of currently focused application with squircle dropdown menus and click-outside dismissal.

### 4. Stage Manager & Window Grouping
- **Left Flyout Shelf**: Smooth spring slide animation revealed on screen edge proximity.
- **Live DWM Streaming**: Hardware-accelerated zero-readback thumbnail streaming via `DwmRegisterThumbnail`.
- **Multi-Window Stack Fan-Out**: Fanned-out card backdrops with window count badges for apps with multiple open windows.
- **One-Click Group Switching**: Minimizes previous group windows and restores/elevates target application windows.

---

## Installation & Download

### Option 1: Download Setup Installer (Recommended)
Download the latest `MacLauncher-Setup.exe` from the **[Releases](https://github.com/yubin0210/mac-windows-launcher/releases)** page and run the installer.

### Option 2: One-Click Script Installation
1. Download or clone this repository.
2. Double-click `installer/install.bat`.
3. The app is automatically installed to `%LOCALAPPDATA%\Programs\MacLauncher` with Desktop and Start Menu shortcuts.

---

## Build from Source

### Prerequisites
- [Rust](https://rustup.rs/) (Stable 1.80+)
- Visual Studio C++ Build Tools (on Windows) or MinGW (for cross-compilation)

### Commands
```bash
# Run tests
cargo test --workspace

# Build optimized release binary
cargo build --release -p win-launcher

# Run directly
cargo run --release -p win-launcher

# Run with Windows taskbar auto-hidden
cargo run --release -p win-launcher -- --hide-taskbar
```

### Compile Installer (.exe)
```powershell
iscc installer\MacLauncher.iss
```

---

## Workspace Structure

```
mac-windows-launcher/
├── Cargo.toml                  # Workspace definition
├── crates/
│   ├── mac-physics-core/       # Pure platform-independent layout math & spring physics
│   ├── mac-spec-extractor/     # REA spec extraction utility
│   └── win-launcher/           # Native DirectComposition, Direct2D & Win32 runtime
├── installer/
│   ├── MacLauncher.iss         # Inno Setup installer script
│   ├── install.bat / .ps1      # Zero-dependency one-click installer
│   └── uninstall.bat / .ps1    # Clean uninstaller
├── specs/                      # Extracted macOS metrics & physics JSON files
└── .github/workflows/          # CI and Release automated build workflows
```

---

## License

MIT License.
