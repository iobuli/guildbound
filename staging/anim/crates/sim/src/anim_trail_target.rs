//! Target (ghost) trail samples for the Studio trails. Pure sim: no Bevy.
//! The game calls `trail_targets` each fixed 120 Hz tick and pushes the results
//! with `trails.push_target(bone, tick, pos)`; `is_key` marks clip key ticks (diamonds).
use crate::anim_rig::{Q, Rig};

/// Default trail set by joint name: hands, feet, head (COM is drawn by the game).
pub const DEFAULT_BONES: [&str; 5] = ["B_L_Hand", "B_R_Hand", "B_L_Foot", "B_R_Foot", "B_Head"];

/// Resolve names to skin-joint indices (BoneIdx order). Missing names are skipped.
pub fn resolve(rig: &Rig, names: &[&str]) -> Vec<u16> {
    names.iter().filter_map(|n| rig.joint_by_name(n)).map(|j| j as u16).collect()
}

/// Model-space target positions of `bones` at `tick`, given the clip's local
/// rotation for each skin joint (`pose(joint) -> Some(local rot)`; None = rest).
/// `key_ticks` must be sorted (binary search).
pub fn trail_targets(rig: &Rig, tick: u64, pose: &dyn Fn(usize) -> Option<Q>,
                     bones: &[u16], key_ticks: &[u64]) -> Vec<(u16, u64, [f32; 3], bool)> {
    // node-index -> joint-index lookup so the override works on nodes.
    let mut node_joint = vec![usize::MAX; rig.nodes.len()];
    for (j, &n) in rig.joints.iter().enumerate() { if n != usize::MAX { node_joint[n] = j; } }
    let local = |n: usize| { let j = node_joint[n]; if j == usize::MAX { None } else { pose(j) } };
    let is_key = key_ticks.binary_search(&tick).is_ok();
    bones.iter().filter_map(|&b| {
        let n = *rig.joints.get(b as usize)?;
        if n == usize::MAX { return None; }
        let (_, t) = rig.compute_world(n, &local);
        Some((b, tick, [t[0] as f32, t[1] as f32, t[2] as f32], is_key))
    }).collect()
}
