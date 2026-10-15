//! Target (ghost) trail samples. The game calls `trail_targets` each fixed tick and draws the result;
//! `is_key` marks clip key ticks.

use super::rig::Rig;
use crate::math::Quat;

/// Default trail set by joint name: hands, feet, head. GUESSES until confirmed by the `[anim_rig]` log.
pub const DEFAULT_BONES: [&str; 5] = ["B_L_Hand", "B_R_Hand", "B_L_Foot", "B_R_Foot", "B_Head"];

/// Names -> skin-joint indices; missing names are skipped.
pub fn resolve(rig: &Rig, names: &[&str]) -> Vec<u16> {
    names
        .iter()
        .filter_map(|n| rig.joint_by_name(n))
        .map(|j| j as u16)
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrailSample {
    pub bone: u16,
    pub tick: u64,
    pub pos: [f32; 3],
    pub is_key: bool,
}

/// Model-space positions of `bones` at `tick`, given each skin joint's local rotation
/// (`pose(joint)`, None = rest). `key_ticks` must be sorted.
pub fn trail_targets(
    rig: &Rig,
    tick: u64,
    pose: &dyn Fn(usize) -> Option<Quat>,
    bones: &[u16],
    key_ticks: &[u64],
) -> Vec<TrailSample> {
    let node_joint = rig.node_joint_table();
    let local = |n: usize| match node_joint[n] {
        usize::MAX => None,
        j => pose(j),
    };
    let is_key = key_ticks.binary_search(&tick).is_ok();
    bones
        .iter()
        .filter_map(|&b| {
            let n = rig.node_of(b as usize)?;
            Some(TrailSample {
                bone: b,
                tick,
                pos: rig.compute_world(n, &local).1.to_f32(),
                is_key,
            })
        })
        .collect()
}
