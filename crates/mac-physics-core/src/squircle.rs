//! Apple superellipse squircle curvature generator.
//!
//! Provides the mathematical core for generating macOS-style continuous curvature
//! ("squircle" or superellipse) icon shapes, tray borders, and window clipping masks.
//!
//! Apple design uses Lamé curves with exponent $n \approx 4.5$ rather than standard rounded
//! rectangles ($C^0$/$C^1$ tangent arcs) to achieve smooth $C^2$ continuous curvature that
//! eliminates visual tangent discontinuities ("creases") at the corner transitions.

/// Default curvature exponent used across macOS for app icons and dock tiles ($n \approx 4.5$).
pub const APPLE_SQUIRCLE_EXPONENT: f32 = 4.5;

/// Generates a closed polygonal contour of a centered superellipse (squircle).
///
/// Implements the parametric Lamé curve equations:
/// $$x(t) = a \cdot \text{sgn}(\cos t) \cdot |\cos t|^{2/n}$$
/// $$y(t) = b \cdot \text{sgn}(\sin t) \cdot |\sin t|^{2/n}$$
/// where:
/// - $a = \text{width} / 2.0$
/// - $b = \text{height} / 2.0$
/// - $n = \text{exponent}$ (Apple uses $n \approx 4.5$)
///
/// Produces $4 \times \text{points\_per\_quadrant}$ points evenly distributed in angle
/// across the 4 quadrants, forming a closed counter-clockwise loop centered at $(0, 0)$.
///
/// # Bounds
/// All points are strictly bounded: $x \in [-a, a]$ and $y \in [-b, b]$.
///
/// # Defensive Guards
/// Returns an empty `Vec` if:
/// - `points_per_quadrant == 0`
/// - `width <= 0.0` or `height <= 0.0`
/// - `exponent <= 0.0`
/// - Any input parameter is non-finite (NaN or Infinity)
pub fn generate_squircle_contour(
    width: f32,
    height: f32,
    exponent: f32,
    points_per_quadrant: usize,
) -> Vec<(f32, f32)> {
    if points_per_quadrant == 0
        || width <= 0.0
        || height <= 0.0
        || exponent <= 0.0
        || !width.is_finite()
        || !height.is_finite()
        || !exponent.is_finite()
    {
        return Vec::new();
    }

    let total_points = match points_per_quadrant.checked_mul(4) {
        Some(n) if n > 0 => n,
        _ => return Vec::new(),
    };

    let a = width as f64 / 2.0;
    let b = height as f64 / 2.0;
    let p = 2.0 / (exponent as f64);

    let mut points = Vec::with_capacity(total_points);

    for i in 0..total_points {
        let t = (std::f64::consts::TAU * i as f64) / (total_points as f64);
        let mut cos_t = t.cos();
        let mut sin_t = t.sin();

        // Guard against tiny floating-point rounding errors at cardinal axes (0, pi/2, pi, 3pi/2)
        if cos_t.abs() < 1e-12 {
            cos_t = 0.0;
        }
        if sin_t.abs() < 1e-12 {
            sin_t = 0.0;
        }

        let sgn_cos = if cos_t > 0.0 {
            1.0
        } else if cos_t < 0.0 {
            -1.0
        } else {
            0.0
        };

        let sgn_sin = if sin_t > 0.0 {
            1.0
        } else if sin_t < 0.0 {
            -1.0
        } else {
            0.0
        };

        let x = if cos_t == 0.0 {
            0.0
        } else {
            a * sgn_cos * cos_t.abs().powf(p)
        };

        let y = if sin_t == 0.0 {
            0.0
        } else {
            b * sgn_sin * sin_t.abs().powf(p)
        };

        points.push((x as f32, y as f32));
    }

    points
}

/// Returns whether a given `(x, y)` coordinate lies inside or on the boundary of the squircle
/// centered at `(0, 0)`.
///
/// Evaluates the implicit Lamé inequality:
/// $$\left|\frac{x}{a}\right|^n + \left|\frac{y}{b}\right|^n \le 1.0$$
pub fn is_inside_squircle(x: f32, y: f32, width: f32, height: f32, exponent: f32) -> bool {
    if width <= 0.0
        || height <= 0.0
        || exponent <= 0.0
        || !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || !exponent.is_finite()
    {
        return false;
    }

    let a = width / 2.0;
    let b = height / 2.0;

    if x.abs() > a + 1e-4 || y.abs() > b + 1e-4 {
        return false;
    }

    let norm_x = (x.abs() / a).powf(exponent);
    let norm_y = (y.abs() / b).powf(exponent);

    norm_x + norm_y <= 1.0 + 1e-4
}

/// Helper to generate an SVG path data string (`d="..."`) representing a closed squircle loop
/// positioned at `(center_x, center_y)`.
pub fn squircle_to_svg_path(points: &[(f32, f32)], center_x: f32, center_y: f32) -> String {
    if points.is_empty() {
        return String::new();
    }
    use std::fmt::Write;
    let mut path = String::with_capacity(points.len() * 24);
    for (i, &(x, y)) in points.iter().enumerate() {
        let px = center_x + x;
        let py = center_y + y;
        if i == 0 {
            let _ = write!(path, "M {:.3} {:.3}", px, py);
        } else {
            let _ = write!(path, " L {:.3} {:.3}", px, py);
        }
    }
    path.push_str(" Z");
    path
}
