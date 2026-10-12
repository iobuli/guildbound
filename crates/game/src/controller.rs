//! Presentation contract for the RPG hero controller. The sim owns translation,
//! turning, jump, dash, roll and combat permissions; this module maps actual
//! planar travel onto eight authored directions and a shared gait phase.
use bevy::prelude::*;
use guildbound_sim::V2;

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
pub fn direction(velocity: V2, facing: V2) -> usize {
    let right = V2::new(facing.z, -facing.x);
    let angle = velocity.dot(right).atan2(velocity.dot(facing));
    (angle / std::f32::consts::FRAC_PI_4).round().rem_euclid(8.0) as usize
}

impl LocomotionMap {
    pub fn clip<'a>(&'a self, lib: &'a AnimLibrary, velocity: V2, facing: V2, running: bool) -> Option<&'a str> {
        let name = &(if running { &self.run } else { &self.walk })[direction(velocity, facing)];
        if lib.node(name).is_some() {
            return Some(name);
        }
        if velocity.dot(facing) < 0.0 {
            lib.roles.back.as_deref().or(lib.roles.walk.as_deref())
        } else if running {
            lib.roles.run.as_deref().or(lib.roles.walk.as_deref())
        } else {
            lib.roles.walk.as_deref().or(lib.roles.run.as_deref())
        }
    }
}

/// A gait cycle is measured in metres, not rendered frames or held input.
pub fn advance_phase(phase: f32, distance_m: f32, stride_m: f32) -> f32 {
    (phase + distance_m.max(0.0) / stride_m.max(0.01)).rem_euclid(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
