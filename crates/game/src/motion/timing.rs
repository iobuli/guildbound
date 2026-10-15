//! Stylized timing between extremes: hold near each key, snap through the in-between,
//! optional stepped "on twos" pacing, and exaggeration away from rest.

use super::Pose;
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Style {
    /// Push poses away from rest: 1.0 = donor, 1.3 = punchy.
    pub exaggeration: f32,
    /// 0..1 share of every key-to-key segment spent easing out of / into the keys.
    /// 0 = linear, 1 = long holds with a sharp snap in the middle.
    pub hold: f32,
    /// Bias of the snap toward the next key (0.5 = symmetric, >0.5 = late snap / anticipation).
    pub bias: f32,
    /// Step time at `fps` (animate on twos at 12 fps, ones at 24 fps). None = continuous.
    pub stepped_fps: Option<f32>,
    /// Playback rate multiplier (applied by whoever advances the layer time, not by sampling).
    pub speed: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            exaggeration: 1.15,
            hold: 0.45,
            bias: 0.55,
            stepped_fps: None,
            speed: 1.0,
        }
    }
}

/// Steepness gained at `hold == 1`: the snap curve is `(2x)^p / 2` with `p = 1 + HOLD_POWER·hold`.
const HOLD_POWER: f32 = 4.0;
/// Largest rotation (rad) exaggeration may produce for one bone (170°).
const MAX_EXAGGERATED_ANGLE: f32 = 170.0 * std::f32::consts::PI / 180.0;

/// Map linear segment progress `u` (0..1) to eased progress (0..1). Exact at 0 and 1,
/// monotonic, `hold == 0` → identity.
///
/// Two monotonic stages: a power bias that moves the snap point to `bias` (scaled in by
/// `hold`), then a symmetric hold/snap gain curve whose steepness grows with `hold`.
pub fn ease(u: f32, style: &Style) -> f32 {
    if u.is_nan() {
        return 0.0;
    }
    let u = u.clamp(0.0, 1.0);
    let hold = if style.hold.is_finite() {
        style.hold.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if hold <= 0.0 || u <= 0.0 || u >= 1.0 {
        return u;
    }
    let bias = if style.bias.is_finite() {
        style.bias.clamp(0.05, 0.95)
    } else {
        0.5
    };
    // u^e maps `bias` to 0.5; blend the exponent toward 1 as hold → 0.
    let exponent = 1.0 + (0.5f32.ln() / bias.ln() - 1.0) * hold;
    let x = u.powf(exponent);
    let p = 1.0 + HOLD_POWER * hold;
    let y = if x < 0.5 {
        0.5 * (2.0 * x).powf(p)
    } else {
        1.0 - 0.5 * (2.0 - 2.0 * x).powf(p)
    };
    y.clamp(0.0, 1.0)
}

/// Quantize a time to `style.stepped_fps` when set.
pub fn quantize(t: f32, style: &Style) -> f32 {
    match style.stepped_fps {
        Some(fps) if fps.is_finite() && fps > 0.0 && t.is_finite() => {
            // Small epsilon so exact frame times do not fall into the previous step.
            (t * fps + 1e-4).floor() / fps
        }
        _ => t,
    }
}

/// Push `pose` away from `rest` by `k` (k = 1 → unchanged). Rotations extrapolate along the
/// rest→pose arc (clamped to 170° per bone), pelvis offset scales linearly.
pub fn exaggerate(pose: &Pose, rest: &Pose, k: f32) -> Pose {
    if !k.is_finite() || (k - 1.0).abs() < 1e-6 {
        return pose.clone();
    }
    let rot = pose
        .rot
        .iter()
        .enumerate()
        .map(|(i, &q)| {
            let Some(&r) = rest.rot.get(i) else {
                return q;
            };
            // Delta in the bone's rest frame, shortest arc.
            let mut delta = r.inverse() * q;
            if delta.w < 0.0 {
                delta = -delta;
            }
            let (axis, angle) = delta.to_axis_angle();
            if !(angle > 1e-5) || !axis.is_finite() {
                return q;
            }
            let limit = MAX_EXAGGERATED_ANGLE.max(angle);
            let scaled = (angle * k).clamp(0.0, limit);
            let out = r * Quat::from_axis_angle(axis.normalize(), scaled);
            let out = out.normalize();
            if out.is_finite() { out } else { q }
        })
        .collect();
    let pelvis = rest.pelvis + (pose.pelvis - rest.pelvis) * k;
    Pose {
        rot,
        pelvis: if pelvis.is_finite() { pelvis } else { pose.pelvis },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styles() -> Vec<Style> {
        let mut v = vec![Style::default()];
        for hold in [0.0, 0.2, 0.45, 0.8, 1.0] {
            for bias in [0.2, 0.5, 0.55, 0.8] {
                v.push(Style {
                    hold,
                    bias,
                    ..default()
                });
            }
        }
        v
    }

    #[test]
    fn ease_endpoints_and_monotonic() {
        for s in styles() {
            assert_eq!(ease(0.0, &s), 0.0, "{s:?}");
            assert_eq!(ease(1.0, &s), 1.0, "{s:?}");
            assert_eq!(ease(-1.0, &s), 0.0);
            assert_eq!(ease(2.0, &s), 1.0);
            let mut prev = 0.0;
            for i in 0..=1000 {
                let y = ease(i as f32 / 1000.0, &s);
                assert!(y.is_finite() && y >= prev - 1e-6, "{s:?} not monotonic at {i}");
                prev = y;
            }
        }
        assert_eq!(ease(f32::NAN, &Style::default()), 0.0);
    }

    #[test]
    fn ease_hold_zero_is_identity_and_bias_shifts_snap() {
        let linear = Style {
            hold: 0.0,
            bias: 0.8,
            ..default()
        };
        for i in 0..=100 {
            let u = i as f32 / 100.0;
            assert!((ease(u, &linear) - u).abs() < 1e-6);
        }
        let early = Style {
            hold: 1.0,
            bias: 0.3,
            ..default()
        };
        let late = Style {
            hold: 1.0,
            bias: 0.7,
            ..default()
        };
        assert!(ease(0.5, &early) > 0.5 && ease(0.5, &late) < 0.5);
        // Holds: little progress early, almost done late.
        let s = Style {
            hold: 0.8,
            bias: 0.5,
            ..default()
        };
        assert!(ease(0.15, &s) < 0.05 && ease(0.85, &s) > 0.95);
    }

    #[test]
    fn quantize_steps() {
        let s = Style {
            stepped_fps: Some(12.0),
            ..default()
        };
        assert!((quantize(0.1, &s) - 1.0 / 12.0).abs() < 1e-6);
        assert!((quantize(2.0 / 12.0, &s) - 2.0 / 12.0).abs() < 1e-6);
        assert_eq!(quantize(0.37, &Style::default()), 0.37);
    }

    #[test]
    fn exaggerate_identity_and_scaling() {
        let rest = Pose {
            rot: vec![Quat::from_rotation_x(-1.0), Quat::IDENTITY, Quat::IDENTITY],
            pelvis: Vec3::ZERO,
        };
        let pose = Pose {
            rot: vec![
                Quat::from_rotation_x(-1.0) * Quat::from_rotation_y(0.4),
                Quat::from_rotation_z(3.0),
                -Quat::from_rotation_x(0.5),
            ],
            pelvis: Vec3::new(0.0, -0.1, 0.02),
        };
        assert_eq!(exaggerate(&pose, &rest, 1.0), pose);
        let e = exaggerate(&pose, &rest, 1.5);
        let expect0 = Quat::from_rotation_x(-1.0) * Quat::from_rotation_y(0.6);
        assert!(e.rot[0].angle_between(expect0) < 2e-3);
        // 3.0 rad already beyond the clamp: never shrunk, never grown.
        assert!((e.rot[1].angle_between(Quat::IDENTITY) - 3.0).abs() < 1e-3);
        // Sign-flipped quaternion still extrapolates along the short arc.
        assert!(e.rot[2].angle_between(Quat::from_rotation_x(0.75)) < 2e-3);
        assert!((e.pelvis - Vec3::new(0.0, -0.15, 0.03)).length() < 1e-6);
        let z = exaggerate(&pose, &rest, 0.0);
        assert!(z.rot[0].angle_between(rest.rot[0]) < 2e-3);
        let big = exaggerate(&pose, &rest, 10.0);
        assert!(big.rot[0].angle_between(rest.rot[0]) <= MAX_EXAGGERATED_ANGLE + 1e-3);
    }
}
