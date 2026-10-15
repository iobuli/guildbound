//! Presentation contract for the RPG hero controller. The sim owns translation,
//! turning, jump, dash, roll and combat permissions; this module maps actual
//! planar travel onto eight authored directions and a shared gait phase.
use bevy::prelude::*;


use crate::anim::AnimLibrary;

#[derive(Resource)]
pub struct LocomotionMap {
    /// Combat (locked on or fighting): the RPG strafe set.
    walk: [String; 8],
    run: [String; 8],
    /// Out of combat: relaxed walk/run in eight directions, sprint forward (other directions fall back to run).
    relaxed_walk: [String; 8],
    relaxed_run: [String; 8],
    sprint: [String; 8],
}

impl Default for LocomotionMap {
    fn default() -> Self {
        let map: serde_json::Value = serde_json::from_str(include_str!("../../../Tools/blender/hero_mapping.json")).expect("valid hero mapping");
        let read = |set: &str, kind: &str| std::array::from_fn(|i| map[set][kind][i].as_str().unwrap_or_default().to_owned());
        let relaxed_run: [String; 8] = read("locomotion_relaxed", "run");
        let fwd = |i: usize| map["locomotion_relaxed"]["sprint"][i].as_str().map(str::to_owned);
        // sprint clips exist for forward (0), forward-right (1) and forward-left (7)
        let sprint = std::array::from_fn(|d| match d { 0 => fwd(0), 1 => fwd(1), 7 => fwd(2), _ => None }.unwrap_or_else(|| relaxed_run[d].clone()));
        Self { walk: read("locomotion", "walk"), run: read("locomotion", "run"), relaxed_walk: read("locomotion_relaxed", "walk"), relaxed_run, sprint }
    }
}

impl LocomotionMap {
    pub fn apply_mapping(&mut self, map: &serde_json::Value) {
        for (set, group, kind) in [(&mut self.walk, "locomotion", "walk"), (&mut self.run, "locomotion", "run"), (&mut self.relaxed_walk, "locomotion_relaxed", "walk"), (&mut self.relaxed_run, "locomotion_relaxed", "run"), (&mut self.sprint, "locomotion_relaxed", "sprint")] {
            for (i, clip) in set.iter_mut().enumerate() {
                if let Some(name) = map[group][kind][i].as_str().filter(|s| !s.is_empty()) { *clip = name.to_owned(); }
            }
        }
    }

    /// Weighted gait clips for a travel angle (radians from facing, + = right) and gait 0 (walk) .. 1 (run):
    /// the two nearest of eight directions, each at walk and run. Missing clips drop out and the rest
    /// renormalize.
    pub fn blend<'a>(&'a self, lib: &AnimLibrary, angle: f32, gait: f32, relaxed: bool) -> Vec<(&'a str, f32)> {
        let (a, b, t) = pair(angle);
        // gait 0..1 = walk..run; relaxed adds 1..2 = run..sprint
        let levels: [(&[String; 8], f32); 3] = if relaxed {
            [(&self.relaxed_walk, (1.0 - gait).max(0.0)), (&self.relaxed_run, 1.0 - (gait - 1.0).abs()), (&self.sprint, (gait - 1.0).max(0.0))]
        } else {
            [(&self.walk, 1.0 - gait.min(1.0)), (&self.run, gait.min(1.0)), (&self.run, 0.0)]
        };
        let mut out: Vec<(&str, f32)> = levels
            .iter()
            .flat_map(|&(set, w)| [(&set[a], (1.0 - t) * w), (&set[b], t * w)])
        .filter(|(c, w)| *w > 1e-3 && !c.is_empty() && lib.node(c).is_some())
        .map(|(c, w)| (c.as_str(), w))
        .collect();
        let total: f32 = out.iter().map(|x| x.1).sum();
        if total > 1e-4 {
            for x in &mut out {
                x.1 /= total;
            }
        }
        out
    }
}

/// The two nearest of the eight directions for an angle from facing, and the share of the second.
pub fn pair(angle: f32) -> (usize, usize, f32) {
    let x = (angle / std::f32::consts::FRAC_PI_4).rem_euclid(8.0);
    let a = x.floor() as usize % 8;
    (a, (a + 1) % 8, x.fract())
}

/// A gait cycle is measured in metres, not rendered frames or held input.
pub fn advance_phase(phase: f32, distance_m: f32, stride_m: f32) -> f32 {
    (phase + distance_m.max(0.0) / stride_m.max(0.01)).rem_euclid(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_pairs_follow_the_angle() {
        let q = std::f32::consts::FRAC_PI_4;
        let (a, b, t) = pair(q * 0.25);
        assert_eq!((a, b), (0, 1));
        assert!((t - 0.25).abs() < 1e-5);
        let (a, b, t) = pair(-q * 0.5);
        assert_eq!((a, b), (7, 0));
        assert!((t - 0.5).abs() < 1e-5);
        assert_eq!(pair(std::f32::consts::PI).0, 4);
    }

    #[test]
    fn hero_mapping_names_every_relaxed_direction() {
        let m = LocomotionMap::default();
        assert!(m.relaxed_walk.iter().chain(&m.relaxed_run).chain(&m.sprint).all(|c| c.starts_with("nc_")), "{:?}", m.sprint);
        assert_eq!(m.sprint[4], m.relaxed_run[4], "no backward sprint: run instead");
    }

    #[test]
    fn gait_phase_is_identical_for_the_same_distance_at_different_frame_rates() {
        for frames in [30, 60, 120] {
            let mut phase = 0.2;
            for _ in 0..frames {
                phase = advance_phase(phase, 3.0 / frames as f32, 2.0);
            }
            assert!((phase - 0.7).abs() < 1e-4);
            assert_eq!(advance_phase(phase, 0.0, 2.0), phase);
        }
    }
}
