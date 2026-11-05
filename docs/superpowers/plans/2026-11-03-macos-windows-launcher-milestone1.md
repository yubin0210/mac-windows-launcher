# macOS UI/UX System Launcher (Milestone 1: REA Extraction, Physics Core, & Minimal Dock) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract exact macOS Dock layout and physics parameters using REA, implement a standalone Rust physics core, and build an ultra-low-latency 120Hz DirectComposition native Windows Dock prototype.

**Architecture:** A modular Cargo workspace where `mac-spec-extractor` inspects macOS `/System/Library/CoreServices/Dock.app` via REA, `mac-physics-core` provides pure platform-agnostic math for magnification and spring bounce, and `win-launcher` hosts the DirectComposition rendering loop and Win32 shell integration.

**Tech Stack:** Rust 2021, `windows` crate (Win32, Direct2D, Direct3D 11, DirectComposition, DWM), `serde`, `serde_json`, REA 6.8.0 CLI.

**Spec:** [docs/superpowers/specs/2026-11-03-macos-windows-launcher-design.md](file:///Users/yubin/Projects/REA_for_Mac/docs/superpowers/specs/2026-11-03-macos-windows-launcher-design.md)

## Global Constraints

- **Platform Target:** Windows 10 (1809+) & Windows 11 (64-bit / ARM64).
- **Toolchain:** Rust (stable), unit tests must run cleanly on both macOS host and Windows target.
- **Resource Limits:** Idle RAM < 40MB, idle CPU < 0.1%.
- **Graphics Pipeline:** DirectComposition + Direct2D with zero GDI/redirection bitmap flicker (`WS_EX_NOREDIRECTIONBITMAP`).
- **Safety:** Standalone user-space execution without modifying system files or registry; taskbar state automatically restored on exit.

## Review Focus

1. **Cursor leaving Dock bounds at high speed:** Ensure magnification gracefully decays to 1.0x with smooth spring dampening instead of snapping abruptly when `WM_MOUSELEAVE` triggers.
2. **Variable frame time (stutter/lag spike):** Verify spring physics integration uses delta time clamping (`dt.min(0.05)`) so physics simulation never explodes after background thread freezes.
3. **Multi-monitor display edge snapping:** Verify Dock horizontal positioning recalculates correctly when moving across screens with different DPI scalings.
4. **Empty or invalid extracted spec fallback:** Ensure `mac-physics-core` provides sensible default constants if `dock-metrics.json` is missing or unreadable.
5. **Zero-division guard in Gaussian curve:** Guard against zero standard deviation ($\sigma \le 0$) or empty icon lists in magnification calculations.

---

### Task 1: Cargo Workspace Initialization & Scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `crates/mac-physics-core/Cargo.toml`
- Create: `crates/mac-physics-core/src/lib.rs`
- Create: `crates/mac-spec-extractor/Cargo.toml`
- Create: `crates/mac-spec-extractor/src/lib.rs`
- Create: `crates/win-launcher/Cargo.toml`
- Create: `crates/win-launcher/src/main.rs`

**Interfaces:**
- Produces: Cargo workspace containing `mac-physics-core`, `mac-spec-extractor`, and `win-launcher`.

- [ ] **Step 1: Write root Cargo.toml workspace configuration**

```toml
[workspace]
members = [
    "crates/mac-physics-core",
    "crates/mac-spec-extractor",
    "crates/win-launcher",
]
resolver = "2"
```

- [ ] **Step 2: Create crate Cargo.toml manifests and placeholder source files**

Configure `mac-physics-core` (dependencies: `serde`, `serde_json`), `mac-spec-extractor` (dependencies: `serde`, `serde_json`, `mac-physics-core`), and `win-launcher` (dependencies: `windows` with Win32 features, `mac-physics-core`).

- [ ] **Step 3: Run workspace verification check**

Run: `cargo check --workspace`  
Expected: Compiles with 0 errors.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/
git commit -m "chore: scaffold rust cargo workspace for mac launcher suite"
```

---

### Task 2: macOS Dock REA Extraction Pipeline (`mac-spec-extractor`)

**Files:**
- Create: `crates/mac-spec-extractor/src/extractor.rs`
- Create: `crates/mac-spec-extractor/src/models.rs`
- Test: `crates/mac-spec-extractor/tests/test_extractor.rs`
- Target Output: `specs/dock-metrics.json`

**Interfaces:**
- Produces: `DockMetrics` struct with `base_icon_size: f32`, `max_icon_size: f32`, `magnification_spread: f32`, `squircle_exponent: f32`, `border_radius: f32`, `icon_padding: f32`.

- [ ] **Step 1: Write failing test for DockMetrics extraction and deserialization**

```rust
// crates/mac-spec-extractor/tests/test_extractor.rs
use mac_spec_extractor::models::DockMetrics;

#[test]
fn test_default_dock_metrics_are_valid() {
    let metrics = DockMetrics::default_macos_spec();
    assert_eq!(metrics.base_icon_size, 48.0);
    assert_eq!(metrics.max_icon_size, 128.0);
    assert!(metrics.magnification_spread > 0.0);
    assert!(metrics.squircle_exponent >= 4.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p mac-spec-extractor`  
Expected: FAIL (types not found)

- [ ] **Step 3: Implement `DockMetrics` and extraction logic**

Implement `DockMetrics` in `models.rs` and the extractor in `extractor.rs` reading plists from `/System/Library/CoreServices/Dock.app` via REA CLI / plist parser, falling back to calibrated defaults.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p mac-spec-extractor`  
Expected: PASS

- [ ] **Step 5: Run extractor to generate `specs/dock-metrics.json`**

Run: `cargo run -p mac-spec-extractor -- --output specs/dock-metrics.json`  
Expected: `specs/dock-metrics.json` is generated and valid.

- [ ] **Step 6: Commit**

```bash
git add crates/mac-spec-extractor/ specs/
git commit -m "feat(spec-extractor): extract macos dock metrics and generate json spec"
```

---

### Task 3: Magnification Gaussian Math Engine (`mac-physics-core`)

**Files:**
- Create: `crates/mac-physics-core/src/magnification.rs`
- Modify: `crates/mac-physics-core/src/lib.rs`
- Test: `crates/mac-physics-core/tests/test_magnification.rs`

**Interfaces:**
- Consumes: `DockMetrics` from Task 2.
- Produces: `calculate_icon_scales(cursor_x: f32, icon_centers: &[f32], config: &MagnificationConfig) -> Vec<f32>`
- Produces: `calculate_dock_layout(scales: &[f32], base_size: f32, padding: f32) -> Vec<(f32, f32)>` (positions and dimensions)

- [ ] **Step 1: Write failing unit tests for magnification curve**

```rust
// crates/mac-physics-core/tests/test_magnification.rs
use mac_physics_core::magnification::{calculate_icon_scales, MagnificationConfig};

#[test]
fn test_no_magnification_when_cursor_out_of_bounds() {
    let config = MagnificationConfig {
        base_size: 48.0,
        max_size: 128.0,
        spread: 150.0,
    };
    let centers = vec![24.0, 76.0, 128.0];
    let scales = calculate_icon_scales(-1000.0, &centers, &config);
    for scale in scales {
        assert!((scale - 1.0).abs() < 1e-4);
    }
}

#[test]
fn test_peak_magnification_at_cursor_center() {
    let config = MagnificationConfig {
        base_size: 48.0,
        max_size: 128.0,
        spread: 150.0,
    };
    let centers = vec![50.0, 100.0, 150.0];
    let scales = calculate_icon_scales(100.0, &centers, &config);
    assert!((scales[1] - (128.0 / 48.0)).abs() < 1e-3);
    assert!(scales[0] < scales[1]);
    assert!(scales[2] < scales[1]);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p mac-physics-core --test test_magnification`  
Expected: FAIL

- [ ] **Step 3: Implement Gaussian magnification algorithm**

Implement continuous Gaussian bell curve modulation in `magnification.rs`:
$$s(x) = 1.0 + \left(\frac{max\_size}{base\_size} - 1.0\right) \cdot \exp\left(-\frac{(x - cursor\_x)^2}{2\sigma^2}\right)$$
Apply smooth cutoff beyond $3\sigma$.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p mac-physics-core --test test_magnification`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/mac-physics-core/
git commit -m "feat(physics): implement continuous gaussian dock magnification curve"
```

---

### Task 4: Damped Harmonic Oscillator Bounce Engine (`mac-physics-core`)

**Files:**
- Create: `crates/mac-physics-core/src/spring.rs`
- Modify: `crates/mac-physics-core/src/lib.rs`
- Test: `crates/mac-physics-core/tests/test_spring.rs`

**Interfaces:**
- Produces: `DampedSpring` struct with `new(mass: f32, stiffness: f32, damping: f32)`, `step(&mut self, dt: f32, target: f32) -> f32`, `trigger_bounce(&mut self, impulse: f32)`, and `is_resting(&self) -> bool`.

- [ ] **Step 1: Write failing unit tests for spring dynamics**

```rust
// crates/mac-physics-core/tests/test_spring.rs
use mac_physics_core::spring::DampedSpring;

#[test]
fn test_spring_converges_to_target() {
    let mut spring = DampedSpring::new(1.0, 180.0, 14.0); // Calibrated macOS launch bounce
    spring.trigger_bounce(-40.0); // Jump upward 40px
    for _ in 0..600 {
        spring.step(1.0 / 120.0, 0.0);
    }
    assert!(spring.is_resting());
    assert!((spring.position() - 0.0).abs() < 1e-3);
}

#[test]
fn test_spring_numerical_stability_on_lag_spike() {
    let mut spring = DampedSpring::new(1.0, 180.0, 14.0);
    spring.trigger_bounce(50.0);
    // Huge dt lag spike simulation (should clamp internally)
    let pos = spring.step(1.0, 0.0);
    assert!(!pos.is_nan());
    assert!(!pos.is_infinite());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p mac-physics-core --test test_spring`  
Expected: FAIL

- [ ] **Step 3: Implement analytical damped harmonic oscillator**

Implement step simulation in `spring.rs` with internal time clamping ($dt \le 0.033$) and standard analytical spring equations for underdamped ($\zeta < 1$), critically damped ($\zeta = 1$), and overdamped regimes.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p mac-physics-core --test test_spring`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/mac-physics-core/
git commit -m "feat(physics): implement damped harmonic oscillator for dock icon bouncing"
```

---

### Task 5: Apple Squircle Curvature Generator (`mac-physics-core`)

**Files:**
- Create: `crates/mac-physics-core/src/squircle.rs`
- Modify: `crates/mac-physics-core/src/lib.rs`
- Test: `crates/mac-physics-core/tests/test_squircle.rs`

**Interfaces:**
- Produces: `generate_squircle_contour(width: f32, height: f32, exponent: f32, points_per_quadrant: usize) -> Vec<(f32, f32)>`

- [ ] **Step 1: Write failing unit test for squircle geometry**

```rust
// crates/mac-physics-core/tests/test_squircle.rs
use mac_physics_core::squircle::generate_squircle_contour;

#[test]
fn test_squircle_contour_bounds_and_symmetry() {
    let points = generate_squircle_contour(100.0, 100.0, 4.5, 16);
    assert_eq!(points.len(), 64); // 4 quadrants * 16 points
    for (x, y) in points {
        assert!(x.abs() <= 50.0001);
        assert!(y.abs() <= 50.0001);
    }
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p mac-physics-core --test test_squircle`  
Expected: FAIL

- [ ] **Step 3: Implement superellipse parametric curve equation**

Implement $\left|\frac{x}{a}\right|^n + \left|\frac{y}{b}\right|^n = 1$ in `squircle.rs` with exponent $n \approx 4.5$ matching Apple's continuous curvature design.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p mac-physics-core --test test_squircle`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/mac-physics-core/
git commit -m "feat(physics): implement superellipse squircle curvature generator"
```

---

### Task 6: DirectComposition Window Pipeline & Rendering Loop (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/window.rs`
- Create: `crates/win-launcher/src/directx.rs`
- Create: `crates/win-launcher/src/dock_view.rs`
- Modify: `crates/win-launcher/src/main.rs`

**Interfaces:**
- Consumes: `calculate_icon_scales`, `DampedSpring`, `generate_squircle_contour` from `mac-physics-core`.
- Produces: Native Windows executable `win-launcher.exe` creating a 120Hz DirectComposition layered overlay window.

- [ ] **Step 1: Implement Win32 window creation and message pump**

In `window.rs`, create a transparent topmost layered window (`WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW`).

- [ ] **Step 2: Initialize Direct3D 11 & DirectComposition visual tree**

In `directx.rs`, initialize D3D11 device, DirectComposition device, target, and create an alpha-blended composition visual.

- [ ] **Step 3: Wire mouse hover, magnification, and VSync render loop**

In `dock_view.rs` and `main.rs`, listen for `WM_MOUSEMOVE` and `WM_MOUSELEAVE`, calculate real-time scales via `mac-physics-core`, and render smooth squircle background + icon layout at display refresh rate.

- [ ] **Step 4: Verify build with Windows target check**

Run: `cargo check --target x86_64-pc-windows-gnu -p win-launcher || cargo check -p win-launcher`  
Expected: Code compiles with 0 errors.

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement directcomposition 120hz dock window and render loop"
```

---

### Task 7: End-to-End Milestone 1 Verification

**Files:**
- Create: `crates/win-launcher/README.md`
- Create: `specs/README.md`

- [ ] **Step 1: Run all unit and integration tests across workspace**

Run: `cargo test --workspace`  
Expected: All tests PASS.

- [ ] **Step 2: Document milestone completion, metrics provenance, and build instructions**

Write concise run instructions for building and testing the Dock prototype.

- [ ] **Step 3: Commit**

```bash
git add README.md specs/ crates/win-launcher/README.md
git commit -m "docs: add milestone 1 setup and build documentation"
```
