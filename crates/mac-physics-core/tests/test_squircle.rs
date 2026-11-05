use mac_physics_core::squircle::{
    generate_squircle_contour, is_inside_squircle, squircle_to_svg_path, APPLE_SQUIRCLE_EXPONENT,
};

#[test]
fn test_squircle_contour_bounds_and_symmetry() {
    let points = generate_squircle_contour(100.0, 100.0, 4.5, 16);
    assert_eq!(points.len(), 64); // 4 quadrants * 16 points
    for (x, y) in points {
        assert!(x.abs() <= 50.0001);
        assert!(y.abs() <= 50.0001);
    }
}

#[test]
fn test_squircle_cardinal_axes_points() {
    let a = 64.0;
    let b = 48.0;
    let k = 16;
    let points = generate_squircle_contour(a * 2.0, b * 2.0, APPLE_SQUIRCLE_EXPONENT, k);
    assert_eq!(points.len(), 64);

    // Cardinal points:
    // i = 0 (t = 0): (a, 0)
    assert!((points[0].0 - a).abs() < 1e-4);
    assert!(points[0].1.abs() < 1e-4);

    // i = k (t = pi/2): (0, b)
    assert!(points[k].0.abs() < 1e-4);
    assert!((points[k].1 - b).abs() < 1e-4);

    // i = 2*k (t = pi): (-a, 0)
    assert!((points[2 * k].0 + a).abs() < 1e-4);
    assert!(points[2 * k].1.abs() < 1e-4);

    // i = 3*k (t = 3*pi/2): (0, -b)
    assert!(points[3 * k].0.abs() < 1e-4);
    assert!((points[3 * k].1 + b).abs() < 1e-4);
}

#[test]
fn test_squircle_superellipse_equation_holds() {
    let w = 120.0;
    let h = 80.0;
    let n = 4.5;
    let a = w / 2.0;
    let b = h / 2.0;
    let points = generate_squircle_contour(w, h, n, 32);

    for (x, y) in points {
        let val = (x.abs() / a).powf(n) + (y.abs() / b).powf(n);
        assert!(
            (val - 1.0).abs() < 1e-3,
            "Point ({x}, {y}) satisfies Lamé equation: expected ~1.0, got {val}"
        );
    }
}

#[test]
fn test_squircle_fourfold_symmetry() {
    let k = 16;
    let w = 96.0;
    let h = 96.0;
    let points = generate_squircle_contour(w, h, 4.5, k);

    for i in 0..k {
        let (x1, y1) = points[i];
        // Quadrant 2 counterpart at 2*k - i (or pi - t)
        // With t_i = 2pi*i/N, at index k, t = pi/2.
        // Quadrant 2 has index k + (k - i) for reflected x?
        // Let's check rotation: index + 2*k is 180 deg opposite: (-x, -y)
        let (x3, y3) = points[i + 2 * k];
        assert!((x1 + x3).abs() < 1e-4);
        assert!((y1 + y3).abs() < 1e-4);

        // 90 deg rotation for square: (x, y) -> (-y, x)
        let (x2, y2) = points[i + k];
        assert!((x2 + y1).abs() < 1e-4);
        assert!((y2 - x1).abs() < 1e-4);
    }
}

#[test]
fn test_squircle_non_square_aspect_ratio() {
    let width = 300.0;
    let height = 150.0;
    let points = generate_squircle_contour(width, height, 4.5, 20);
    assert_eq!(points.len(), 80);

    for (x, y) in points {
        assert!(x.abs() <= 150.0001);
        assert!(y.abs() <= 75.0001);
    }
}

#[test]
fn test_squircle_defensive_guards() {
    // 0 points per quadrant -> empty vec
    assert!(generate_squircle_contour(100.0, 100.0, 4.5, 0).is_empty());

    // Non-positive dimensions -> empty vec
    assert!(generate_squircle_contour(0.0, 100.0, 4.5, 16).is_empty());
    assert!(generate_squircle_contour(-10.0, 100.0, 4.5, 16).is_empty());
    assert!(generate_squircle_contour(100.0, 0.0, 4.5, 16).is_empty());
    assert!(generate_squircle_contour(100.0, -5.0, 4.5, 16).is_empty());

    // Non-positive or invalid exponent -> empty vec
    assert!(generate_squircle_contour(100.0, 100.0, 0.0, 16).is_empty());
    assert!(generate_squircle_contour(100.0, 100.0, -2.0, 16).is_empty());
    assert!(generate_squircle_contour(100.0, 100.0, f32::NAN, 16).is_empty());
    assert!(generate_squircle_contour(100.0, 100.0, f32::INFINITY, 16).is_empty());

    // Non-finite dimensions -> empty vec
    assert!(generate_squircle_contour(f32::NAN, 100.0, 4.5, 16).is_empty());
    assert!(generate_squircle_contour(100.0, f32::INFINITY, 4.5, 16).is_empty());
}

#[test]
fn test_squircle_curvature_fuller_than_circle() {
    let size = 100.0;
    let k = 16;
    let circle_pts = generate_squircle_contour(size, size, 2.0, k);
    let squircle_pts = generate_squircle_contour(size, size, 4.5, k);

    // At 45 degrees (index k/2 = 8):
    let (cx, cy) = circle_pts[k / 2];
    let (sx, sy) = squircle_pts[k / 2];

    // For circle of radius 50, at 45 deg x = y = 50 / sqrt(2) ≈ 35.355
    assert!((cx - 50.0 / std::f32::consts::SQRT_2).abs() < 0.1);
    assert!((cy - 50.0 / std::f32::consts::SQRT_2).abs() < 0.1);

    // Squircle is fuller than circle at the diagonal
    assert!(sx > cx + 5.0, "Squircle corner {sx} should be fuller than circle corner {cx}");
    assert!(sy > cy + 5.0, "Squircle corner {sy} should be fuller than circle corner {cy}");
}

#[test]
fn test_is_inside_squircle_query() {
    let w = 100.0;
    let h = 100.0;
    let exp = 4.5;

    // Center is inside
    assert!(is_inside_squircle(0.0, 0.0, w, h, exp));

    // Cardinal extremities are on the boundary (inside)
    assert!(is_inside_squircle(50.0, 0.0, w, h, exp));
    assert!(is_inside_squircle(-50.0, 0.0, w, h, exp));
    assert!(is_inside_squircle(0.0, 50.0, w, h, exp));
    assert!(is_inside_squircle(0.0, -50.0, w, h, exp));

    // Outer corner (50, 50) of bounding box is outside
    assert!(!is_inside_squircle(50.0, 50.0, w, h, exp));
    assert!(!is_inside_squircle(-50.0, 50.0, w, h, exp));
    assert!(!is_inside_squircle(55.0, 0.0, w, h, exp));

    // Defensive guards for is_inside_squircle
    assert!(!is_inside_squircle(0.0, 0.0, -10.0, 100.0, exp));
    assert!(!is_inside_squircle(0.0, 0.0, 100.0, 100.0, 0.0));
    assert!(!is_inside_squircle(f32::NAN, 0.0, 100.0, 100.0, exp));
}

#[test]
fn test_squircle_to_svg_path() {
    assert_eq!(squircle_to_svg_path(&[], 50.0, 50.0), "");

    let points = generate_squircle_contour(100.0, 100.0, 4.5, 4);
    let path = squircle_to_svg_path(&points, 50.0, 50.0);
    assert!(path.starts_with("M "));
    assert!(path.ends_with(" Z"));
    assert_eq!(path.matches('M').count(), 1);
    assert_eq!(path.matches('L').count(), 15); // 16 points total: 1 M + 15 L
}
