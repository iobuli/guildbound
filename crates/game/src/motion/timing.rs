//! Stylized timing between extremes: hold near each key, snap through the in-between,
//! optional stepped "on twos" pacing, and exaggeration away from rest.

use super::Pose;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    /// Playback rate multiplier.
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

/// Map linear segment progress `u` (0..1) to eased progress (0..1). Exact at 0 and 1,
/// monotonic, `hold == 0` → identity.
pub fn ease(u: f32, style: &Style) -> f32 {
    todo!("timing::ease {u} {style:?}")
}

/// Quantize a time to `style.stepped_fps` when set.
pub fn quantize(t: f32, style: &Style) -> f32 {
    todo!("timing::quantize {t} {style:?}")
}

/// Push `pose` away from `rest` by `k` (k = 1 → unchanged). Rotations extrapolate along the
/// rest→pose arc (clamped to 170° per bone), pelvis offset scales linearly.
pub fn exaggerate(pose: &Pose, rest: &Pose, k: f32) -> Pose {
    todo!("timing::exaggerate {k} {} {}", pose.rot.len(), rest.rot.len())
}
