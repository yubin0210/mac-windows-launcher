//! Damped harmonic oscillator bounce and spring physics engine.
//!
//! Provides analytical solution and numerical stability for macOS-style dock icon
//! bouncing, launching, and window animations across underdamped, critically damped,
//! and overdamped regimes.

use serde::{Deserialize, Serialize};

/// Maximum time step allowed per simulation step (clamped for stability).
pub const MAX_DT: f32 = 0.033;

/// Positional threshold below which the spring is considered at rest (0.001 units/px).
pub const RESTING_POS_EPSILON: f32 = 1e-3;

/// Velocity threshold below which the spring is considered at rest (0.001 units/s).
pub const RESTING_VEL_EPSILON: f32 = 1e-3;

/// Calibrated macOS dock bounce default mass.
pub const MACOS_MASS: f32 = 1.0;

/// Calibrated macOS dock bounce default stiffness.
pub const MACOS_STIFFNESS: f32 = 180.0;

/// Calibrated macOS dock bounce default damping.
pub const MACOS_DAMPING: f32 = 14.0;

/// Damped harmonic oscillator simulating spring-mass-damper motion:
///
///   `m * x'' + c * x' + k * (x - target) = 0`
///
/// Supports exact analytical integration for underdamped, critically damped,
/// and overdamped configurations, with internal delta-time clamping for stability
/// during frame lag spikes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DampedSpring {
    mass: f32,
    stiffness: f32,
    damping: f32,
    position: f32,
    velocity: f32,
    target: f32,
}

/// Type alias for `DampedSpring`.
pub type Spring = DampedSpring;

impl DampedSpring {
    /// Creates a new damped spring with specified mass, stiffness, and damping.
    ///
    /// Mass is clamped to a minimum of `1e-4` to prevent division by zero.
    /// Stiffness and damping are clamped to non-negative values.
    pub fn new(mass: f32, stiffness: f32, damping: f32) -> Self {
        Self {
            mass: mass.max(1e-4),
            stiffness: stiffness.max(0.0),
            damping: damping.max(0.0),
            position: 0.0,
            velocity: 0.0,
            target: 0.0,
        }
    }

    /// Creates a spring calibrated to macOS dock launch bouncing
    /// (mass: 1.0, stiffness: 180.0, damping: 14.0).
    pub fn with_calibrated_macos() -> Self {
        Self::new(MACOS_MASS, MACOS_STIFFNESS, MACOS_DAMPING)
    }

    /// Triggers a bounce by imparting an initial velocity impulse.
    pub fn trigger_bounce(&mut self, impulse: f32) {
        self.velocity += impulse;
    }

    /// Advances the spring simulation by `dt` seconds towards `target`.
    ///
    /// If `dt <= 0.0` or `dt.is_nan()`, this is a no-op returning current position.
    /// Internal time clamping limits `dt` to `MAX_DT` (0.033s) to prevent numerical
    /// instability during lag spikes.
    ///
    /// Returns the updated position.
    pub fn step(&mut self, dt: f32, target: f32) -> f32 {
        self.target = target;

        if dt.is_nan() || dt <= 0.0 {
            return self.position;
        }

        let dt = dt.min(MAX_DT);

        let y0 = self.position - target;
        let v0 = self.velocity;

        // If already at rest at target, snap and return
        if y0.abs() < RESTING_POS_EPSILON && v0.abs() < RESTING_VEL_EPSILON {
            self.position = target;
            self.velocity = 0.0;
            return self.position;
        }

        let m = self.mass.max(1e-4);
        let k = self.stiffness.max(0.0);
        let c = self.damping.max(0.0);

        if k <= 1e-6 {
            // No spring force: pure drag or free motion
            if c <= 1e-6 {
                self.position += v0 * dt;
            } else {
                let decay = (-c / m * dt).exp();
                let new_v = v0 * decay;
                let new_pos = self.position + (v0 * m / c) * (1.0 - decay);
                self.position = new_pos;
                self.velocity = new_v;
            }
            return self.position;
        }

        let omega0 = (k / m).sqrt();
        let gamma = c / (2.0 * m);
        let zeta = gamma / omega0;

        let (new_y, new_v) = if (zeta - 1.0).abs() < 1e-5 {
            // Critically damped (zeta == 1.0)
            let decay = (-gamma * dt).exp();
            let b = v0 + gamma * y0;
            let y = decay * (y0 + b * dt);
            let v = decay * (v0 - gamma * b * dt);
            (y, v)
        } else if zeta < 1.0 {
            // Underdamped (zeta < 1.0)
            let omega_d = (omega0 * omega0 - gamma * gamma).max(0.0).sqrt();
            let decay = (-gamma * dt).exp();
            let cos_wt = (omega_d * dt).cos();
            let sin_wt = (omega_d * dt).sin();
            let b = if omega_d > 1e-6 {
                (v0 + gamma * y0) / omega_d
            } else {
                0.0
            };
            let y = decay * (y0 * cos_wt + b * sin_wt);
            let v = decay * (v0 * cos_wt - (gamma * b + omega_d * y0) * sin_wt);
            (y, v)
        } else {
            // Overdamped (zeta > 1.0)
            let omega_d = (gamma * gamma - omega0 * omega0).max(0.0).sqrt();
            let r1 = -gamma + omega_d;
            let r2 = -gamma - omega_d;
            let decay = (-gamma * dt).exp();
            let e_pos = (omega_d * dt).exp();
            let e_neg = (-omega_d * dt).exp();
            let (c1, c2) = if omega_d > 1e-6 {
                let c1 = (v0 + (gamma + omega_d) * y0) / (2.0 * omega_d);
                let c2 = y0 - c1;
                (c1, c2)
            } else {
                (y0, 0.0)
            };
            let y = decay * (c1 * e_pos + c2 * e_neg);
            let v = decay * (c1 * r1 * e_pos + c2 * r2 * e_neg);
            (y, v)
        };

        self.position = target + new_y;
        self.velocity = new_v;

        // Snap to target if within resting thresholds
        if (self.position - target).abs() < RESTING_POS_EPSILON
            && self.velocity.abs() < RESTING_VEL_EPSILON
        {
            self.position = target;
            self.velocity = 0.0;
        }

        self.position
    }

    /// Returns `true` if both position error and velocity are within resting thresholds.
    pub fn is_resting(&self) -> bool {
        (self.position - self.target).abs() < RESTING_POS_EPSILON
            && self.velocity.abs() < RESTING_VEL_EPSILON
    }

    /// Returns the current position.
    pub fn position(&self) -> f32 {
        self.position
    }

    /// Returns the current velocity.
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// Returns the current target position.
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Returns the mass.
    pub fn mass(&self) -> f32 {
        self.mass
    }

    /// Returns the spring stiffness.
    pub fn stiffness(&self) -> f32 {
        self.stiffness
    }

    /// Returns the damping coefficient.
    pub fn damping(&self) -> f32 {
        self.damping
    }

    /// Sets the position directly.
    pub fn set_position(&mut self, position: f32) {
        self.position = position;
    }

    /// Sets the velocity directly.
    pub fn set_velocity(&mut self, velocity: f32) {
        self.velocity = velocity;
    }

    /// Sets the target position.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Resets the spring state to a stationary rest position at `position`.
    pub fn reset(&mut self, position: f32) {
        self.position = position;
        self.target = position;
        self.velocity = 0.0;
    }
}

impl Default for DampedSpring {
    fn default() -> Self {
        Self::with_calibrated_macos()
    }
}
