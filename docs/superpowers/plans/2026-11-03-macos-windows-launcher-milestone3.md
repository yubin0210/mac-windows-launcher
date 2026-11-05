# macOS UI/UX System Launcher on Windows — Milestone 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Milestone 3: Spotlight Launcher Module (global hotkey `Alt + Space`, zero-latency fuzzy application/settings search indexer, instant expression/math calculator, and Direct2D frosted glass floating search bar).

**Architecture:**
Expands the system with a pure Rust fuzzy scoring and math evaluation core in `mac-physics-core::spotlight`, a Win32 Start Menu and System Settings indexer in `win-launcher::indexer`, a global hotkey listener (`RegisterHotKey`) with a centered floating focus-managed window in `win-launcher::spotlight_window`, and a Direct2D search bar and dropdown results rasterizer.

**Tech Stack:** Rust (edition 2021), `windows` crate (0.58), `mac-physics-core`, `serde`, Win32 Shell & Direct2D.

**Spec:** [`docs/superpowers/specs/2026-11-03-macos-windows-launcher-design.md`](../specs/2026-11-03-macos-windows-launcher-design.md)

---

## Global Constraints

- Standalone native binary with zero web engines (no Electron, no Chromium, no WebView2).
- Zero input lag: fuzzy search across hundreds of indexed applications must complete in < 2ms in-memory.
- Global hotkey defaults to `Alt + Space` (to avoid Windows Input Language conflict with `Win + Space`, while supporting configurable keys).
- Focus safety: Spotlight window must instantly hide and release focus when user clicks outside (`WM_KILLFOCUS`) or presses `Esc`.
- All Windows-specific APIs gated behind `#[cfg(windows)]` with cross-platform mock stubs on `#[cfg(not(windows))]` so `cargo test --workspace` passes cleanly on all platforms.
- Windows cross-compilation check (`cargo check --target x86_64-pc-windows-gnu -p win-launcher`) must pass with 0 errors and 0 warnings.

## Review Focus

1. **Hotkey Conflict Resilience:** If `Alt + Space` is already registered by another application, the launcher must gracefully try secondary fallbacks (e.g. `Ctrl + Space` or `Win + Shift + Space`) without crashing.
2. **Focus Stealing & Dismissal:** When Spotlight loses focus (`WM_KILLFOCUS`), it must immediately hide and return focus to the previous active window.
3. **Fuzzy Search Scoring Quality:** Acronyms ("gc" -> "Google Chrome", "cmd" -> "Command Prompt") and exact prefix matches must rank strictly higher than arbitrary substring matches.
4. **Instant Math Evaluation Safety:** Math expression evaluator must guard against division by zero, overflow, infinite loops, and NaN results.
5. **Start Menu Shortcut Extraction Failure:** If a `.lnk` file is corrupted or points to a non-existent path, the indexer must skip it cleanly without aborting the index scan.

---

### Task 1: Spotlight Pure Fuzzy Matcher & Math Expression Evaluator (`mac-physics-core`)

**Files:**
- Create: `crates/mac-physics-core/src/spotlight.rs`
- Modify: `crates/mac-physics-core/src/lib.rs`
- Test: `crates/mac-physics-core/tests/test_spotlight_core.rs`

**Interfaces:**
- Produces:
  - `fuzzy_match(query: &str, target: &str) -> Option<i32>` (returns score with prefix, word-boundary, and acronym bonuses)
  - `evaluate_math(expr: &str) -> Option<f64>` (evaluates arithmetic `+`, `-`, `*`, `/`, `^`, parentheses, sqrt)
  - `SpotlightItem`: `id: String`, `title: String`, `subtitle: String`, `category: SpotlightCategory` (`Application`, `SystemSetting`, `Calculation`, `File`)

- [ ] **Step 1: Write failing unit tests for fuzzy matching and math evaluation**

```rust
// crates/mac-physics-core/tests/test_spotlight_core.rs
use mac_physics_core::spotlight::{evaluate_math, fuzzy_match, SpotlightCategory, SpotlightItem};

#[test]
fn test_fuzzy_match_ranking() {
    let score_prefix = fuzzy_match("term", "Terminal").expect("Should match");
    let score_sub = fuzzy_match("term", "Apple Intermediate Term").expect("Should match");
    assert!(score_prefix > score_sub);

    // Acronym bonus
    let score_acronym = fuzzy_match("gc", "Google Chrome").expect("Should match acronym");
    assert!(score_acronym > 0);

    // No match
    assert!(fuzzy_match("xyz", "Terminal").is_none());
}

#[test]
fn test_evaluate_math_expressions() {
    assert_eq!(evaluate_math("120 * 45"), Some(5400.0));
    assert_eq!(evaluate_math("(100 + 25) / 5"), Some(25.0));
    assert_eq!(evaluate_math("2 ^ 10"), Some(1024.0));
    assert_eq!(evaluate_math("10 / 0"), None); // Div by zero guard
    assert_eq!(evaluate_math("not an expression"), None);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p mac-physics-core --test test_spotlight_core`  
Expected: FAIL (module not found)

- [ ] **Step 3: Implement `fuzzy_match` and `evaluate_math`**

In `crates/mac-physics-core/src/spotlight.rs`:
- Implement prefix-biased fuzzy matcher with word boundary (+50), acronym (+100), and case match bonuses.
- Implement Pratt / Shunting-yard recursive descent arithmetic parser with zero-division guards.
- Export `pub mod spotlight;` in `crates/mac-physics-core/src/lib.rs`.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p mac-physics-core --test test_spotlight_core`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/mac-physics-core/
git commit -m "feat(physics): implement spotlight fuzzy matching and math evaluation engine"
```

---

### Task 2: Win32 Application & Settings Indexer (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/indexer.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_indexer.rs`

**Interfaces:**
- Produces: `AppIndexer` struct:
  - `new() -> Self`
  - `build_index(&mut self)`
  - `search(&self, query: &str, limit: usize) -> Vec<SpotlightItem>`
  - `register_mock_item(&mut self, item: SpotlightItem)`

- [ ] **Step 1: Write failing test for AppIndexer**

```rust
// crates/win-launcher/tests/test_indexer.rs
use win_launcher::indexer::AppIndexer;
use mac_physics_core::spotlight::SpotlightCategory;

#[test]
fn test_indexer_search_and_ranking() {
    let mut indexer = AppIndexer::new();
    indexer.register_mock_item("Terminal", "Command Line", SpotlightCategory::Application, "wt.exe");
    indexer.register_mock_item("Google Chrome", "Web Browser", SpotlightCategory::Application, "chrome.exe");

    let results = indexer.search("gc", 5);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Google Chrome");

    let results_term = indexer.search("term", 5);
    assert_eq!(results_term[0].title, "Terminal");
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_indexer`  
Expected: FAIL

- [ ] **Step 3: Implement `AppIndexer`**

In `crates/win-launcher/src/indexer.rs`:
- On Windows: Scans Start Menu folders (`CSIDL_COMMON_PROGRAMS`, `CSIDL_PROGRAMS`) for `.lnk` shortcuts, extracts names, and adds standard Control Panel / Settings URI targets (`ms-settings:display`, `ms-settings:network`, etc.).
- On non-Windows: Populates standard mock system items (Safari, Terminal, Finder, Settings, Chrome, VS Code).
- `search(&self, query: &str, limit: usize) -> Vec<SpotlightItem>`:
  - Checks if query is a math expression (`evaluate_math`); if so, prepends calculation result item.
  - Scores all indexed items using `fuzzy_match`.
  - Sorts descending by score and truncates to `limit`.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_indexer`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement spotlight application and settings indexer"
```

---

### Task 3: Global Hotkey Hook & Spotlight Window Pipeline (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/spotlight_window.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_spotlight_window.rs`

**Interfaces:**
- Produces: `SpotlightWindow` struct:
  - `new() -> Result<Self>`
  - `show(&mut self)`
  - `hide(&mut self)`
  - `is_visible(&self) -> bool`
  - `poll_events(&mut self) -> Vec<SpotlightEvent>`
  - `SpotlightEvent`: `Opened`, `Closed`, `QueryChanged(String)`, `Execute(usize)`, `Navigate(i32)`

- [ ] **Step 1: Write failing test for SpotlightWindow lifecycle**

```rust
// crates/win-launcher/tests/test_spotlight_window.rs
use win_launcher::spotlight_window::{SpotlightEvent, SpotlightWindow};

#[test]
fn test_spotlight_window_open_hide_and_events() {
    let mut win = SpotlightWindow::new_mock();
    assert!(!win.is_visible());
    win.show();
    assert!(win.is_visible());
    win.hide();
    assert!(!win.is_visible());
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_spotlight_window`  
Expected: FAIL

- [ ] **Step 3: Implement `SpotlightWindow`**

In `crates/win-launcher/src/spotlight_window.rs`:
- On Windows:
  - Window class `Spotlight_OverlayWnd` (`WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_POPUP`).
  - Centered horizontally, positioned at 25% from top of primary screen (640px width, 56px resting height, expands to 380px with results).
  - Registers hotkey `MOD_ALT | VK_SPACE` with fallback to `MOD_CONTROL | VK_SPACE`.
  - On `WM_KILLFOCUS` or `VK_ESCAPE`, invokes `hide()`.
  - On `WM_CHAR` / `WM_KEYDOWN`, handles text input, Up/Down arrow selection, and Enter key execution.
- On non-Windows:
  - Cross-platform mock window with synthetic event queue for testing.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_spotlight_window`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement spotlight global hotkey and focus-managed floating window"
```

---

### Task 4: Spotlight State Machine & Keyboard Navigation (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/spotlight_view.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_spotlight_view.rs`

**Interfaces:**
- Produces: `SpotlightView` struct:
  - `new(indexer: AppIndexer) -> Self`
  - `input_text(&mut self, text: &str)`
  - `delete_char(&mut self)`
  - `navigate(&mut self, delta: i32)` (Up/Down arrow key)
  - `get_query(&self) -> &str`
  - `get_results(&self) -> &[SpotlightItem]`
  - `selected_index(&self) -> usize`
  - `selected_item(&self) -> Option<&SpotlightItem>`

- [ ] **Step 1: Write failing test for SpotlightView**

```rust
// crates/win-launcher/tests/test_spotlight_view.rs
use win_launcher::spotlight_view::SpotlightView;
use win_launcher::indexer::AppIndexer;

#[test]
fn test_spotlight_view_typing_and_navigation() {
    let mut indexer = AppIndexer::new();
    indexer.register_mock_item("Terminal", "CLI", Default::default(), "wt.exe");
    indexer.register_mock_item("Task Manager", "System", Default::default(), "taskmgr.exe");

    let mut view = SpotlightView::new(indexer);
    view.input_text("t");
    assert_eq!(view.get_results().len(), 2);
    assert_eq!(view.selected_index(), 0);

    view.navigate(1); // Down arrow
    assert_eq!(view.selected_index(), 1);

    view.navigate(-1); // Up arrow wraps
    assert_eq!(view.selected_index(), 0);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_spotlight_view`  
Expected: FAIL

- [ ] **Step 3: Implement `SpotlightView`**

In `crates/win-launcher/src/spotlight_view.rs`:
- Manages search text buffer, current results list from `indexer.search`, and `selected_index`.
- Wraps arrow navigation seamlessly without out-of-bounds indices.
- Updates search results in < 1ms on every keystroke.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_spotlight_view`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement spotlight state machine and keyboard navigation"
```

---

### Task 5: Direct2D Spotlight Floating Bar & Results List Rasterizer (`win-launcher`)

**Files:**
- Create: `crates/win-launcher/src/spotlight_render.rs`
- Modify: `crates/win-launcher/src/directx.rs`
- Modify: `crates/win-launcher/src/lib.rs`
- Test: `crates/win-launcher/tests/test_spotlight_render.rs`

**Interfaces:**
- Produces: `SpotlightRenderer` struct:
  - `render_spotlight(&mut self, query: &str, results: &[SpotlightItem], selected_idx: usize, width: f32, height: f32) -> Result<()>`
  - Draws frosted glass squircle background bar (640px width, $n=4.5$).
  - Draws search magnifying glass icon and input text.
  - Draws dropdown results with category badge pills, item subtitles, and smooth selection highlight.

- [ ] **Step 1: Write failing test for SpotlightRenderer**

```rust
// crates/win-launcher/tests/test_spotlight_render.rs
use win_launcher::spotlight_render::MockSpotlightRenderContext;
use win_launcher::spotlight_view::SpotlightView;
use win_launcher::indexer::AppIndexer;

#[test]
fn test_spotlight_renderer_draws_bar_and_results() {
    let mut indexer = AppIndexer::new();
    indexer.register_mock_item("Terminal", "CLI", Default::default(), "wt.exe");
    let mut view = SpotlightView::new(indexer);
    view.input_text("term");

    let mut mock_ctx = MockSpotlightRenderContext::new();
    mock_ctx.render_spotlight(view.get_query(), view.get_results(), view.selected_index(), 640.0, 180.0).expect("Render failed");

    assert!(mock_ctx.bar_drawn);
    assert_eq!(mock_ctx.items_drawn_count, 1);
    assert_eq!(mock_ctx.selected_index, 0);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test -p win-launcher --test test_spotlight_render`  
Expected: FAIL

- [ ] **Step 3: Implement `SpotlightRenderer`**

In `crates/win-launcher/src/spotlight_render.rs`:
- On Windows: Uses Direct2D path geometries, squircle contour ($n=4.5$), and cached brushes to draw the floating spotlight container, search icon, query text, selection background (`#FFFFFF25`), and category badges.
- On non-Windows: `MockSpotlightRenderContext` tracking dimensions, query, and items count.

- [ ] **Step 4: Run test to verify pass**

Run: `cargo test -p win-launcher --test test_spotlight_render`  
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/
git commit -m "feat(launcher): implement direct2d spotlight floating bar and results renderer"
```

---

### Task 6: End-to-End Milestone 3 Integration & Verification

**Files:**
- Modify: `crates/win-launcher/src/main.rs`
- Modify: `crates/win-launcher/README.md`
- Modify: `specs/README.md`

- [ ] **Step 1: Wire Spotlight into `main.rs`**

Connect global hotkey hook, `SpotlightWindow`, `SpotlightView`, `SpotlightRenderer`, and `ProcessLauncher` execution. Update cross-platform simulation to demonstrate Spotlight fuzzy search, math evaluation, and hotkey launch.

- [ ] **Step 2: Run all tests across workspace**

Run: `cargo test --workspace`  
Expected: All tests PASS.

- [ ] **Step 3: Verify Windows cross-compilation check**

Run: `cargo check --target x86_64-pc-windows-gnu -p win-launcher`  
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Update Documentation**

Document Spotlight architecture, global hotkey registration, fuzzy matching algorithms, and math evaluation in `crates/win-launcher/README.md` and `specs/README.md`.

- [ ] **Step 5: Commit**

```bash
git add crates/win-launcher/ specs/ README.md
git commit -m "docs: add milestone 3 spotlight integration and architecture documentation"
```
