//! Physics: in-air recovery maths (PHYSICS_MATH.md §15). Pure functions on sim::math types.
//! Flight is ballistic for the COM: angular momentum about the COM is conserved, so recovery
//! re-poses limbs; it cannot change total spin.
use crate::math::Vec3;

pub const G: f64 = 9.81;

/// Seconds until the COM falls to `land_com_y` (rest COM height over the landing surface).
/// Positive root of y + vy·t − ½g t² = y_land. None if it is never reached (shouldn't happen under gravity).
pub fn time_to_land(com: Vec3, com_v: Vec3, land_com_y: f64) -> Option<f64> {
    let dy = com.y - land_com_y;
    let disc = com_v.y * com_v.y + 2.0 * G * dy;
    if disc < 0.0 {
        return None;
    }
    let t = (com_v.y + disc.sqrt()) / G;
    if t.is_finite() && t >= 0.0 {
        Some(t)
    } else {
        None
    }
}

/// Horizontal COM position at touchdown (y set to `land_com_y`).
pub fn landing_com(com: Vec3, com_v: Vec3, t: f64, land_com_y: f64) -> Vec3 {
    Vec3::new(com.x + com_v.x * t, land_com_y, com.z + com_v.z * t)
}

/// Where the feet should land: the touchdown capture point x + ẋ·√(h/g) (horizontal velocity is
/// unchanged in flight), ±`half_stance` along `side` (unit, horizontal). Returns [L, R] at y = floor_y.
pub fn foot_targets(
    land: Vec3,
    com_v: Vec3,
    rest_com_h: f64,
    side: Vec3,
    half_stance: f64,
    floor_y: f64,
) -> [Vec3; 2] {
    let k = (rest_com_h / G).sqrt();
    let cp = Vec3::new(land.x + com_v.x * k, floor_y, land.z + com_v.z * k);
    [cp + side * half_stance, cp - side * half_stance]
}

/// Motor weight during Recovering: 0 until `ramp_s + margin_s` before touchdown, then smoothstep
/// to 1, reaching 1 at `margin_s` before touchdown. ramp_s = 12/120 by default.
pub fn recover_weight(t_land: f64, ramp_s: f64, margin_s: f64) -> f64 {
    let u = ((ramp_s + margin_s - t_land) / ramp_s).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drop_time() {
        let t = time_to_land(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 0.0, 0.0), 1.0).unwrap();
        assert!((t - (2.0 / G).sqrt()).abs() < 1e-12);
    }
    #[test]
    fn up_then_down() {
        let t = time_to_land(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 4.905, 0.0), 1.0).unwrap();
        assert!((t - 1.0).abs() < 1e-9);
    }
    #[test]
    fn weight_ramp() {
        assert_eq!(recover_weight(1.0, 0.1, 0.05), 0.0);
        assert_eq!(recover_weight(0.05, 0.1, 0.05), 1.0);
        assert!((recover_weight(0.1, 0.1, 0.05) - 0.5).abs() < 1e-12);
    }
    #[test]
    fn feet_lead_velocity() {
        let f = foot_targets(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 2.0),
            1.0,
            Vec3::new(1.0, 0.0, 0.0),
            0.1,
            0.0,
        );
        assert!(f[0].z > 0.6 && f[0].x > 0.0 && f[1].x < 0.0);
    }
}
