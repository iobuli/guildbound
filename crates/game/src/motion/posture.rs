//! Running posture: the jog and sprint clips hunch (measured over the clips, angles forward of
//! upright in the body's plane: walk pelvis→spine_03 10°, spine_03→neck 16°, neck→head 20°;
//! jog 19° / 41° / 35°; sprint 34° / 50° / 48°), so a running character looked bent over with its
//! head thrust out. `apply` (from `compose`, after the carry posture) straightens each segment that
//! leans past its cap, in order from the hips up, so the run keeps a forward lean but the chest
//! opens and the head comes up over the shoulders.
//! - Only the share of the pose that comes from running clips is corrected (`run_share`, from the
//!   layer stack), so crouches, bows, attacks and pick-ups keep every degree they have.
//! - Rotations are in model space about the body's left axis, as in `carry`.
use super::bank::PoseBank;
use super::{Layer, Pose, Source};
use bevy::prelude::*;

/// Largest forward lean kept while running (rad): pelvis→spine_03, spine_03→neck, neck→head.
const LOWER_MAX: f32 = 20.0 * std::f32::consts::PI / 180.0;
const UPPER_MAX: f32 = 26.0 * std::f32::consts::PI / 180.0;
const HEAD_MAX: f32 = 24.0 * std::f32::consts::PI / 180.0;

/// Whether a track is a forward running clip (the ones that hunch).
fn running(track: &str) -> bool {
    track.contains("Jog_Fwd") || track.contains("Sprint") || track.contains("Run-Forwards")
}

/// How much of the evaluated pose comes from running clips (0..1): each layer's weight, faded by
/// every layer blended over it.
pub fn run_share(layers: &[Layer]) -> f32 {
    let mut share = 0.0;
    for (i, layer) in layers.iter().enumerate() {
        let own = if i == 0 { 1.0 } else { layer.weight.clamp(0.0, 1.0) };
        share *= 1.0 - own;
        if let Source::Track { name, .. } = &layer.source
            && running(name)
        {
            share += own;
        }
    }
    share.clamp(0.0, 1.0)
}

/// Forward lean of `v` from upright in the body's plane (rad; model space, +Z forward).
fn lean(v: Vec3) -> f32 {
    v.z.atan2(v.y.max(1e-4))
}

/// Straighten a running pose (see the module docs); `share` from `run_share`.
pub fn apply(bank: &PoseBank, pose: &mut Pose, share: f32) {
    if share <= 1e-3 {
        return;
    }
    let skeleton = &bank.skeleton;
    let bones = &bank.bones;
    if pose.rot.len() != skeleton.len() {
        return;
    }
    // (bones turned, bottom and top of the segment measured, cap), hips up.
    let [s1, s2, s3] = bones.spine;
    let segments: [(&[usize], usize, usize, f32); 3] = [
        (&[s1, s2], bones.pelvis, s3, LOWER_MAX),
        (&[s3], s3, bones.neck, UPPER_MAX),
        (&[bones.neck], bones.neck, bones.head, HEAD_MAX),
    ];
    for (turned, from, to, cap) in segments {
        let model = skeleton.fk(pose, bones.pelvis);
        let excess = (lean(model[to].translation - model[from].translation) - cap).max(0.0) * share;
        if excess <= 1e-4 {
            continue;
        }
        let back = Quat::from_rotation_x(-excess / turned.len() as f32);
        for &bone in turned {
            // Each bone turned in turn: the model-space frames below it are unchanged.
            let model = skeleton.fk(pose, bones.pelvis);
            let parent = skeleton.parent[bone].map_or(skeleton.base.rotation, |p| model[p].rotation);
            let local = (parent.inverse() * back * model[bone].rotation).normalize();
            if local.is_finite() {
                pose.rot[bone] = local;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motion::Mask;

    fn track(name: &str, weight: f32) -> Layer {
        Layer { source: Source::Track { name: name.into(), time: 0.0, looping: true }, weight, mask: Mask::Full, style: None }
    }

    #[test]
    fn only_the_running_share_is_corrected() {
        assert_eq!(run_share(&[track("ual1/Idle_Loop", 1.0)]), 0.0);
        assert_eq!(run_share(&[track("ual1/Jog_Fwd_Loop", 1.0)]), 1.0);
        let blend = run_share(&[track("ual1/Walk_Loop", 1.0), track("ual1/Jog_Fwd_Loop", 0.4)]);
        assert!((blend - 0.4).abs() < 1e-5);
        let attacking = run_share(&[track("ual1/Sprint_Loop", 1.0), track("ual1/Punch_Jab", 1.0)]);
        assert_eq!(attacking, 0.0, "a full layer over the run takes the correction away");
    }

    #[test]
    fn a_jog_pose_comes_up_to_the_caps() {
        let bank = PoseBank::build().unwrap();
        let (sk, b) = (&bank.skeleton, &bank.bones);
        let mut pose = bank.sample_raw("ual1/Jog_Fwd_Loop", 0.3, true);
        let before = sk.fk(&pose, b.pelvis);
        apply(&bank, &mut pose, 1.0);
        let after = sk.fk(&pose, b.pelvis);
        let seg = |m: &[Transform], a: usize, c: usize| lean(m[c].translation - m[a].translation);
        assert!(seg(&before, b.spine[2], b.neck) > UPPER_MAX, "the clip hunches");
        assert!(seg(&after, b.spine[2], b.neck) <= UPPER_MAX + 0.02);
        assert!(seg(&after, b.neck, b.head) <= HEAD_MAX + 0.02);
        assert!(seg(&after, b.pelvis, b.spine[2]) > 0.0, "still leaning into the run");
    }
}
