//! Physics: foot-grab joint and spin maths (PHYSICS_MATH.md §16). Pure functions on sim::math types.
//! The grab is a ball joint hand↔ankle realised as a stiff spring-damper tuned from the reduced
//! mass, so it stays stable at the held-torque 1/120 tick, and breaks above `break_force`.
use crate::math::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct GrabParams {
    pub freq_hz: f64,
    pub zeta: f64,
    pub break_force: f64,
}
impl Default for GrabParams {
    /// 8 Hz, critically damped (ω·dt ≈ 0.42 at 1/120), breaks at 2500 N (~3× a 79 kg body weight).
    fn default() -> Self {
        GrabParams {
            freq_hz: 8.0,
            zeta: 1.0,
            break_force: 2500.0,
        }
    }
}

/// Reduced mass of the two bodies the joint couples (whole-body masses).
pub fn reduced_mass(ma: f64, mb: f64) -> f64 {
    ma * mb / (ma + mb)
}

/// Force on the grabbed ankle (apply −F to the grabbing hand). `pa/va`: hand point, `pb/vb`: ankle point.
/// Returns (force, broke). On broke, the api calls release_grab and applies nothing.
pub fn grab_force(
    pa: Vec3,
    va: Vec3,
    pb: Vec3,
    vb: Vec3,
    m_red: f64,
    p: GrabParams,
) -> (Vec3, bool) {
    let w = 2.0 * std::f64::consts::PI * p.freq_hz;
    let f = (pa - pb) * (m_red * w * w) + (va - vb) * (2.0 * p.zeta * m_red * w);
    if !f.is_finite() || f.length() > p.break_force {
        (Vec3::default(), true)
    } else {
        (f, false)
    }
}

/// Spin centre: the pair's combined COM (the point the spinner's feet must step around).
pub fn spin_center(com_a: Vec3, ma: f64, com_b: Vec3, mb: f64) -> Vec3 {
    (com_a * ma + com_b * mb) * (1.0 / (ma + mb))
}

/// Rope tension needed to keep the victim's COM on a circle: m·ω²·r.
pub fn spin_tension(mb: f64, omega: f64, r: f64) -> f64 {
    mb * omega * omega * r
}

/// Lean (rad, from vertical, away from the victim) for the spinner to balance the pull:
/// tan φ = T / (m_a·g). Animation uses this for the planted-turn lean.
pub fn spin_lean(tension: f64, ma: f64) -> f64 {
    (tension / (ma * 9.81)).atan()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rest_no_force() {
        let (f, b) = grab_force(
            Vec3::default(),
            Vec3::default(),
            Vec3::default(),
            Vec3::default(),
            40.0,
            GrabParams::default(),
        );
        assert_eq!(f.length(), 0.0);
        assert!(!b);
    }
    #[test]
    fn pulls_toward_hand() {
        let (f, _) = grab_force(
            Vec3::new(0.01, 0.0, 0.0),
            Vec3::default(),
            Vec3::default(),
            Vec3::default(),
            40.0,
            GrabParams::default(),
        );
        assert!(f.x > 0.0);
    }
    #[test]
    fn breaks() {
        let (_, b) = grab_force(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::default(),
            Vec3::default(),
            Vec3::default(),
            40.0,
            GrabParams::default(),
        );
        assert!(b);
    }
    #[test]
    fn lean_45() {
        assert!((spin_lean(79.4 * 9.81, 79.4) - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
    }
}
