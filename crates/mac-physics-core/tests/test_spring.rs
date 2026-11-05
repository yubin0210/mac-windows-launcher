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

#[test]
fn test_spring_with_calibrated_macos() {
    let spring = DampedSpring::with_calibrated_macos();
    assert_eq!(spring.mass(), 1.0);
    assert_eq!(spring.stiffness(), 180.0);
    assert_eq!(spring.damping(), 14.0);
    assert_eq!(spring.position(), 0.0);
    assert_eq!(spring.velocity(), 0.0);
    assert!(spring.is_resting());
}

#[test]
fn test_spring_underdamped_oscillation() {
    let mut spring = DampedSpring::new(1.0, 180.0, 14.0);
    spring.trigger_bounce(-40.0);

    let mut min_pos = 0.0f32;
    let mut max_pos = 0.0f32;

    for _ in 0..200 {
        let pos = spring.step(1.0 / 120.0, 0.0);
        if pos < min_pos {
            min_pos = pos;
        }
        if pos > max_pos {
            max_pos = pos;
        }
    }

    // Oscillates into negative then rebounds past target 0.0 into positive
    assert!(min_pos < -1.0, "Expected downward/negative bounce");
    assert!(max_pos > 0.1, "Expected overshoot above target in underdamped spring");
}

#[test]
fn test_spring_critically_damped() {
    // m = 1.0, k = 100.0 => omega_0 = 10.0, c_crit = 2 * sqrt(1.0 * 100.0) = 20.0
    let mut spring = DampedSpring::new(1.0, 100.0, 20.0);
    spring.set_position(50.0);

    for _ in 0..600 {
        spring.step(1.0 / 120.0, 0.0);
    }

    assert!(spring.is_resting());
    assert!((spring.position() - 0.0).abs() < 1e-3);
}

#[test]
fn test_spring_overdamped() {
    // m = 1.0, k = 100.0, c = 40.0 => zeta = 40 / 20 = 2.0 > 1.0
    let mut spring = DampedSpring::new(1.0, 100.0, 40.0);
    spring.set_position(50.0);

    for _ in 0..600 {
        spring.step(1.0 / 120.0, 0.0);
    }

    assert!(spring.is_resting());
    assert!((spring.position() - 0.0).abs() < 1e-3);
}

#[test]
fn test_spring_step_to_non_zero_target() {
    let mut spring = DampedSpring::with_calibrated_macos();
    for _ in 0..600 {
        spring.step(1.0 / 120.0, 100.0);
    }

    assert!(spring.is_resting());
    assert!((spring.position() - 100.0).abs() < 1e-3);
}

#[test]
fn test_spring_zero_or_negative_dt() {
    let mut spring = DampedSpring::with_calibrated_macos();
    spring.trigger_bounce(-40.0);
    let initial_pos = spring.position();
    let initial_vel = spring.velocity();

    // Step with 0.0 or negative dt should be a no-op
    let pos_zero = spring.step(0.0, 0.0);
    assert_eq!(pos_zero, initial_pos);
    assert_eq!(spring.velocity(), initial_vel);

    let pos_neg = spring.step(-0.016, 0.0);
    assert_eq!(pos_neg, initial_pos);
    assert_eq!(spring.velocity(), initial_vel);
}

#[test]
fn test_spring_reset_and_setters() {
    let mut spring = DampedSpring::with_calibrated_macos();
    spring.trigger_bounce(-50.0);
    spring.step(1.0 / 60.0, 0.0);
    assert!(!spring.is_resting());

    spring.reset(10.0);
    assert_eq!(spring.position(), 10.0);
    assert_eq!(spring.velocity(), 0.0);
    assert_eq!(spring.target(), 10.0);
    assert!(spring.is_resting());
}

#[test]
fn test_spring_serde_roundtrip() {
    let spring = DampedSpring::new(1.0, 180.0, 14.0);
    let json = serde_json::to_string(&spring).expect("Must serialize spring");
    let deserialized: DampedSpring = serde_json::from_str(&json).expect("Must deserialize spring");
    assert_eq!(spring, deserialized);
}
