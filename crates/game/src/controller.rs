//! Presentation contract for the RPG hero controller. The sim owns translation,
//! turning, jump, dash, roll and combat permissions; this module maps actual
//! planar travel onto eight authored directions and a shared gait phase.
use bevy::prelude::*;


use crate::anim::AnimLibrary;

#[derive(Resource)]
pub struct LocomotionMap {
    walk: [String; 8],
    run: [String; 8],
}

impl Default for LocomotionMap {
    fn default() -> Self {
        let map: serde_json::Value = serde_json::from_str(include_str!("../../../Tools/blender/hero_mapping.json")).expect("valid hero mapping");
        let read = |kind: &str| std::array::from_fn(|i| map["locomotion"][kind][i].as_str().expect("eight locomotion clips").to_owned());
        Self { walk: read("walk"), run: read("run") }
    }
}

/// Forward, forward-right, right, backward-right, backward, backward-left,
/// left, forward-left. Derived from velocity, so braking keeps its real gait.
#[cfg(test)]
fn direction(velocity: guildbound_sim::V2, facing: guildbound_sim::V2) -> usize {
    let right = guildbound_sim::V2::new(facing.z, -facing.x);
    let angle = velocity.dot(right).atan2(velocity.dot(facing));
    (angle / std::f32::consts::FRAC_PI_4).round().rem_euclid(8.0) as usize
}

impl LocomotionMap {
    /// Weighted gait clips for a travel angle (radians from facing, + = right) and gait 0 (walk) .. 1 (run):
    /// the two nearest of eight directions, each at walk and run. Missing clips drop out and the rest
    /// renormalize.
    pub fn blend<'a>(&'a self, lib: &AnimLibrary, angle: f32, gait: f32) -> Vec<(&'a str, f32)> {
        let (a, b, t) = pair(angle);
        let mut out: Vec<(&str, f32)> = [
            (&self.walk[a], (1.0 - t) * (1.0 - gait)),
            (&self.walk[b], t * (1.0 - gait)),
            (&self.run[a], (1.0 - t) * gait),
            (&self.run[b], t * gait),
        ]
        .into_iter()
        .filter(|(c, w)| *w > 1e-3 && lib.node(c).is_some())
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
    use guildbound_sim::V2;

    #[test]
    fn eight_directions_follow_the_actor_when_it_turns() {
        for facing in [V2::new(0.0, 1.0), V2::new(1.0, 0.0)] {
            let right = V2::new(facing.z, -facing.x);
            for (i, (x, z)) in [(0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (1.0, -1.0), (0.0, -1.0), (-1.0, -1.0), (-1.0, 0.0), (-1.0, 1.0)].into_iter().enumerate() {
                assert_eq!(direction(right * x + facing * z, facing), i);
            }
        }
    }

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
