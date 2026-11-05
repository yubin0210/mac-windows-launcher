# macOS Dock Metrics & Physics Specifications

This directory contains the ground-truth geometric specifications, physical constants, and mathematical models reverse-engineered from macOS for the macOS Launcher Windows suite.

---

## 1. Provenance of `dock-metrics.json`

The file [`dock-metrics.json`](dock-metrics.json) captures the authentic layout and geometry parameters of the macOS Dock:

```json
{
  "base_icon_size": 48.0,
  "max_icon_size": 128.0,
  "magnification_spread": 150.0,
  "squircle_exponent": 4.5,
  "border_radius": 16.0,
  "icon_padding": 10.0
}
```

### Extraction Pipeline

1. **Target Inspection**:
   - Primary target: `/System/Library/CoreServices/Dock.app` on macOS.
   - REA (Reverse Engineer Anything) CLI command used:
     ```bash
     rea inspect-plist /System/Library/CoreServices/Dock.app/Contents/Info.plist --format json
     ```
2. **Key Normalization (`mac-spec-extractor`)**:
   - The extractor crate (`crates/mac-spec-extractor`) analyzes the normalized JSON output returned by REA.
   - If executed on non-macOS platforms or in environments without local system Dock access, the pipeline deterministically falls back to calibrated macOS constants verified against macOS Sonoma (14.x) and Sequoia (15.x).
3. **Spec Field Definitions**:
   - **`base_icon_size` (48.0 pt / px)**: The standard resting width and height of an unmagnified Dock icon.
   - **`max_icon_size` (128.0 pt / px)**: The maximum magnified dimension of the icon directly centered under the mouse pointer ($2.67\times$ scaling).
   - **`magnification_spread` (150.0 pt / px)**: The standard deviation ($\sigma$) of the Gaussian magnification bell curve, defining the spatial radius of icon expansion along the horizontal axis.
   - **`squircle_exponent` (4.5)**: The degree $n$ of the Lamé superellipse curve used for Apple's continuous-curvature rounded rectangles.
   - **`border_radius` (16.0 pt / px)**: The reference corner radius of the Dock background tray.
   - **`icon_padding` (10.0 pt / px)**: The unmagnified horizontal gap between adjacent Dock icons.

---

## 2. Mathematical Formats & Formulas

The physics and geometry engines implemented in `mac-physics-core` follow exact analytical formulations:

### 2.1 Continuous Gaussian Dock Magnification Curve

Icon scale factors are calculated continuously as a function of the horizontal distance between each icon's center $x_i$ and the cursor position $x_{\text{cursor}}$:

$$s(x) = 1.0 + \left(\frac{s_{\max}}{s_{\text{base}}} - 1.0\right) \cdot \exp\left(-\frac{(x - x_{\text{cursor}})^2}{2\sigma^2}\right) \cdot w(d)$$

Where:
* $s_{\text{base}} = 48.0$ (base size)
* $s_{\max} = 128.0$ (max magnified size)
* $\sigma = 150.0$ (magnification spread)
* $d = |x - x_{\text{cursor}}|$ (Euclidean distance to cursor)

#### Smooth Boundary Taper ($w(d)$)

To prevent visual popping when an icon enters or leaves the influence radius, a cosine-based window function $w(d)$ attenuates the tail between $2.5\sigma$ and $3.0\sigma$:

$$w(d) = \begin{cases} 
1.0 & \text{if } d < 2.5\sigma \\
\frac{1}{2} \left[1 + \cos\left(\pi \frac{d - 2.5\sigma}{0.5\sigma}\right)\right] & \text{if } 2.5\sigma \le d \le 3.0\sigma \\
0.0 & \text{if } d > 3.0\sigma 
\end{cases}$$

#### Symmetric Inter-Icon Layout Expansion

As icons scale up, their widths expand outward symmetrically from the cursor position. Given scales $s_0, s_1, \dots, s_{n-1}$ and resting sizes $w_i = s_{\text{base}} \cdot s_i$:
* Total tray width: $W = \sum_{i=0}^{n-1} w_i + (n - 1) \cdot \text{padding}$
* Each icon's center position $x_i$ preserves horizontal continuity so adjacent icons glide smoothly as the cursor sweeps across the Dock.

---

### 2.2 Damped Harmonic Oscillator Launch Bounce

Icon launch animations follow an underdamped second-order linear differential equation:

$$m \ddot{y}(t) + c \dot{y}(t) + k (y(t) - y_{\text{target}}) = 0$$

Dividing by mass $m$:

$$\ddot{y}(t) + 2\zeta\omega_n \dot{y}(t) + \omega_n^2 (y(t) - y_{\text{target}}) = 0$$

Where:
* **Mass ($m$)**: $1.0\text{ kg}$
* **Spring Stiffness ($k$)**: $180.0\text{ N/m}$
* **Damping Coefficient ($c$)**: $14.0\text{ N}\cdot\text{s/m}$
* **Natural Angular Frequency ($\omega_n$)**: $\sqrt{\frac{k}{m}} = \sqrt{180} \approx 13.4164\text{ rad/s}$
* **Damping Ratio ($\zeta$)**: $\frac{c}{2\sqrt{km}} = \frac{14}{2\sqrt{180}} \approx 0.5217$
* **Damped Angular Frequency ($\omega_d$)**: $\omega_n \sqrt{1 - \zeta^2} = 13.4164 \cdot \sqrt{1 - 0.5217^2} \approx 11.444\text{ rad/s}$

Because $\zeta < 1$, the system operates strictly in the **underdamped regime**, producing the characteristic macOS launch bounce (overshoot, oscillation, and rapid settling).

#### Analytical Closed-Form Solution

For any time step $\Delta t$, the position $y(t)$ and velocity $v(t) = \dot{y}(t)$ relative to target $y_{\text{target}}$ are calculated analytically:

$$y(t) = y_{\text{target}} + e^{-\zeta \omega_n t} \left[ A \cos(\omega_d t) + B \sin(\omega_d t) \right]$$

$$v(t) = e^{-\zeta \omega_n t} \left[ (B \omega_d - A \zeta \omega_n) \cos(\omega_d t) - (A \omega_d + B \zeta \omega_n) \sin(\omega_d t) \right]$$

Where integration constants from initial conditions $y(0)$ and $v(0)$ are:
* $A = y(0) - y_{\text{target}}$
* $B = \frac{v(0) + \zeta \omega_n A}{\omega_d}$

#### Numerical Safeguards
* **Delta-Time Clamping**: If a frame stall occurs, $\Delta t$ is clamped internally ($\Delta t \le 0.0333\text{ s}$) to prevent numerical divergence.
* **Convergence Threshold**: The oscillator transitions to resting state when $|y - y_{\text{target}}| < 10^{-4}$ and $|v| < 10^{-3}$.

---

### 2.3 Apple Superellipse Squircle Curvature ($n = 4.5$)

Standard UI frameworks use simple circular rounded corners ($r$-radius circles connected to linear edges), creating a $C^1$ tangent discontinuity where the straight edge meets the circular arc (often referred to as visual "corner pinching").

Apple designs use a **Lamé superellipse curve** (squircle) with continuous $C^2$ curvature variation:

$$\left|\frac{x}{a}\right|^n + \left|\frac{y}{b}\right|^n = 1$$

Where:
* $a = \frac{\text{width}}{2}$ (semi-major axis)
* $b = \frac{\text{height}}{2}$ (semi-minor axis)
* $n = 4.5$ (Apple continuous curvature exponent)

#### Parametric Generation Equation

For parameter $\theta \in [0, 2\pi]$, contour vertices are generated symmetrically across all four quadrants:

$$x(\theta) = a \cdot \operatorname{sgn}(\cos \theta) \cdot |\cos \theta|^{2/n}$$

$$y(\theta) = b \cdot \operatorname{sgn}(\sin \theta) \cdot |\sin \theta|^{2/n}$$

#### Curvature Comparison

| Curve Type | Exponent $n$ | Curvature Continuity | Visual Characteristic |
|---|---|---|---|
| Circle / Ellipse | $n = 2.0$ | $C^\infty$ (constant $\kappa = 1/r$) | Standard circle |
| Standard Rounded Rect | Hybrid | $C^1$ discontinuous at tangency | Noticeable "pinched" corner joints |
| **Apple Squircle (REA)** | **$n = 4.5$** | **$C^2$ smooth progressive curvature** | **Fuller, organically smoothed corners** |
| Hard Rectangle | $n \to \infty$ | Discontinuous ($C^0$) | Sharp $90^\circ$ corners |

---

### 2.4 Exponential Scale Decay on Mouse Leave

When the cursor exits the Dock window or moves outside the vertical hover sensitivity zone, instantaneous snapping to scale 1.0 is avoided. Instead, icon scales decay exponentially back to resting state:

$$s(t) = s_{\text{target}} + (s_0 - s_{\text{target}}) \cdot e^{-\lambda t}$$

For discrete frame simulation with time step $\Delta t$:

$$s_{k+1} = s_k + (s_{\text{target}} - s_k) \cdot \left(1 - e^{-\lambda \Delta t}\right)$$

Where:
* **Decay Rate ($\lambda$)**: $42.0\text{ s}^{-1}$ (`DEFAULT_SCALE_DECAY_RATE`)
* **Half-life ($t_{1/2}$)**: $\frac{\ln 2}{\lambda} \approx 16.5\text{ ms}$ (smooth decay initiates immediately within 1-2 frames at 120Hz)
* **99% Settling Time ($t_{99\%}$)**: $\frac{\ln 100}{\lambda} \approx 110\text{ ms}$ (~100-150ms settling without visual pop)
* **Convergence Epsilon**: When $|s_k - s_{\text{target}}| < 10^{-4}$ across all items, scales lock directly to $1.0$.

---

### 2.5 Running and Focused Indicator Pip Specifications

Active desktop applications display small indicator pips positioned directly underneath the icon baseline:

| Indicator State | Center Coordinate $(x, y)$ | Radius ($r$) | Color / Fill | Alpha |
|---|---|---|---|---|
| **Running (Background)** | $(x_i, y_{\text{baseline}} + 6.0\text{ pt})$ | $2.25\text{ pt}$ | Pure White `#FFFFFF` | $70\%$ (`0.70`) |
| **Running (Foreground Focus)** | $(x_i, y_{\text{baseline}} + 6.0\text{ pt})$ | $2.75\text{ pt}$ | Pure White `#FFFFFF` | $95\%$ (`0.95`) |

---

### 2.6 Direct2D Apple Squircle Path Mapping

The 64-vertex Lamé superellipse contour ($n = 4.5$, 16 points per quadrant) generated by `mac-physics-core` is converted into a Direct2D path geometry:
* Generated vertices $(x_\theta, y_\theta)$ are shifted relative to the tray top-left origin $(x_{\text{tray}}, y_{\text{tray}})$.
* Sent to an `ID2D1GeometrySink` via `BeginFigure(D2D1_FIGURE_BEGIN_FILLED)` and `AddLines`.
* Closed with `EndFigure(D2D1_FIGURE_END_CLOSED)`.
* Filled with a translucent frosted glass brush (`rgba(240, 240, 245, 0.40)`) and stroked with a crisp specular highlight (`rgba(255, 255, 255, 0.35)`).

---

### 2.7 Spotlight Fuzzy Ranking and Arithmetic Evaluation Engine

Milestone 3 extracts and formalizes the search and calculation behavior of macOS Spotlight:

#### 1. Fuzzy Matching Scoring Algorithm (`fuzzy_match`)
Sub-millisecond fuzzy matching ranks search items through character sequence alignment with contextual bonuses:
* **Sequential Character Matching**: Characters in `query` must appear in sequence within `target` (case-insensitive).
* **Prefix Match Bonus (+150)**: Awarded when the query matches the beginning of the target string.
* **Word Boundary Bonus (+100)**: Awarded when matching characters immediately follow a space, hyphen, underscore, or period.
* **CamelCase / Acronym Bonus (+80)**: Awarded when matching an uppercase letter following a lowercase letter.
* **Exact Consecutive Character Bonus (+25)**: Awarded for contiguous character streaks.
* **Exact Case Match Bonus (+10)**: Awarded when query casing matches target casing.
* **Length Penalty**: Subtracts character distance penalties to prioritize concise, exact titles.

#### 2. Safe Arithmetic Expression Evaluation (`evaluate_math`)
Spotlight parses mathematical queries using a recursive descent parser supporting:
* Operations: Addition (`+`), subtraction (`-`), multiplication (`*`), division (`/`), exponentiation (`^`).
* Functions: `sqrt(x)`, `abs(x)`, `sin(x)`, `cos(x)`, `tan(x)`, `ln(x)`, `log(x)`, `exp(x)`.
* Precedence: Enforces strict mathematical operator precedence with parentheses grouping.
* Precision: Numbers formatted with integer truncation for whole values or up to 6 decimal places stripped of trailing zeroes.
* Output: Hero calculation result row displayed at index 0 under `SpotlightCategory::Calculation`.

---

## 3. Windows Native Subsystems & Safety Invariants

### 3.1 Process Tracking & Activation Coordination

* **EnumWindows Desktop Window Enumeration**: Enumerates visible top-level windows via `EnumWindows` and resolves executable names via `QueryFullProcessImageNameW` every 500ms to avoid render-thread bottlenecks while capturing window `HWND` for instant activation.
* **Foreground Focus Tracking**: Correlates `GetForegroundWindow()` with process IDs from `GetWindowThreadProcessId` to dynamically flag focused Dock items.
* **Activation First Policy**: When a running app is clicked, `ProcessLauncher` activates the existing top-level window via `ShowWindow(hwnd, SW_RESTORE)` and `SetForegroundWindow(hwnd)` rather than spawning a duplicate process.

### 3.2 Fail-Safe Taskbar Drop Guard

* **Auto-Hide Coexistence**: Primary (`Shell_TrayWnd`) and multi-monitor (`Shell_SecondaryTrayWnd`) taskbars can be hidden with `ShowWindow(SW_HIDE)` when requested via `--hide-taskbar`.
* **Zero-Orphan Invariant**: `TaskbarGuard` implements the Rust `Drop` trait. If `win-launcher` exits normally, is terminated, or encounters an unexpected panic, all managed taskbars are immediately restored via `ShowWindow(SW_SHOW)` and `SWP_FRAMECHANGED` to restore desktop work area dimensions.

### 3.3 Win32 AppBar Top Edge Work Area Reservation

* **Ground-Truth Menu Bar Geometry**:
  - Height: strictly 28.0 pt / px.
  - Horizontal Padding: 12.0 pt / px.
  - Item Spacing: 16.0 pt / px.
  - Font Size: 13.0 pt.
* **AppBar Protocol (`SHAppBarMessage`)**:
  - Registers as an Application Desktop Toolbar via `ABM_NEW` (`ABE_TOP`).
  - Sets edge position via `ABM_QUERYPOS` and `ABM_SETPOS` reserving the top 28px work area so maximized Windows applications never obscure the menu bar.
  - Subscribes to `ABN_POSCHANGED` via custom callback message `WM_APPBAR_CALLBACK` to dynamically re-adjust if resolution or taskbars shift.
  - **Zero-Orphan Invariant**: `Win32MenuBarWindow::drop` sends `SHAppBarMessage(ABM_REMOVE)` before destroying the window, ensuring the system desktop work area is restored instantly.

### 3.4 Hardware Status & Power Queries

* **Desktop Battery Safety**: Queries `GetSystemPowerStatus`. When `BatteryLifePercent == 255`, cleanly maps to `None` to prevent phantom battery icons on AC-powered desktop workstations.
* **Rate-Limiting**: Hardware status queries are throttled to 1-second intervals to guarantee 0% idle CPU utilization.

---

## 4. Workspace Usage

These specifications are consumed across the Cargo workspace:
* `mac-spec-extractor`: Reads local macOS plists or falls back to `dock-metrics.json`.
* `mac-physics-core`: Implements Gaussian magnification, damped harmonic springs, and squircle contours in pure, platform-independent Rust.
* `win-launcher`: Binds the calculated layouts, squircle contours, process tracking, launch coordination, and taskbar protection into the DirectComposition 120Hz and Direct2D rendering pipeline on Windows.
