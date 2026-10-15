//! Donor clip → canonical skeleton.
//! - Same-skeleton libraries (`ual1`, `ual2`): copy local rotations by bone name.
//! - Mixamo (`sekiro`): name map, then per mapped bone in model space
//!   `W_dst(t) = W_src(t) · W_src_bind⁻¹ · W_dst_bind`, converted back to local through the
//!   destination parent chain. Both binds are T-pose, +Z forward, so this removes axis-convention
//!   differences. Unmapped destination bones keep rest. Pelvis translation is the source hips
//!   model-space offset from bind, scaled by the leg-length ratio, expressed in pelvis-parent space.
use super::donor::DonorLib;
use super::{Bones, Pose, SAMPLE_HZ, Skeleton};
use bevy::prelude::*;

/// A retargeted dense clip on the canonical skeleton.
#[derive(Clone, Debug)]
pub struct Clip {
    /// `"{lib}/{clip}"`.
    pub track: String,
    pub duration: f32,
    /// Heuristic: name contains `Loop`, or is a known cyclic locomotion clip
    /// (Walk/Run/Jog/Sprint/Swim/Crouch_Fwd/Strafe/`Magic Walk`), or first≈last frame.
    pub looping: bool,
    /// Frames at `SAMPLE_HZ`, frame i at time `i / SAMPLE_HZ` (last frame clamped to duration).
    pub frames: Vec<Pose>,
    /// Model-space horizontal pelvis travel relative to frame 0, per frame (root motion source).
    /// The `frames[i].pelvis` have this horizontal travel REMOVED (in-place poses).
    pub travel: Vec<Vec3>,
}

/// (mixamo bone without the `mixamorig:` prefix, canonical UAL name).
pub const MIXAMO_MAP: &[(&str, &str)] = &[
    ("Hips", "pelvis"),
    ("Spine", "spine_01"),
    ("Spine1", "spine_02"),
    ("Spine2", "spine_03"),
    ("Neck", "neck_01"),
    ("Head", "Head"),
    ("LeftShoulder", "clavicle_l"),
    ("LeftArm", "upperarm_l"),
    ("LeftForeArm", "lowerarm_l"),
    ("LeftHand", "hand_l"),
    ("RightShoulder", "clavicle_r"),
    ("RightArm", "upperarm_r"),
    ("RightForeArm", "lowerarm_r"),
    ("RightHand", "hand_r"),
    ("LeftUpLeg", "thigh_l"),
    ("LeftLeg", "calf_l"),
    ("LeftFoot", "foot_l"),
    ("LeftToeBase", "ball_l"),
    ("RightUpLeg", "thigh_r"),
    ("RightLeg", "calf_r"),
    ("RightFoot", "foot_r"),
    ("RightToeBase", "ball_r"),
    ("LeftHandThumb1", "thumb_01_l"),
    ("LeftHandThumb2", "thumb_02_l"),
    ("LeftHandThumb3", "thumb_03_l"),
    ("LeftHandIndex1", "index_01_l"),
    ("LeftHandIndex2", "index_02_l"),
    ("LeftHandIndex3", "index_03_l"),
    ("LeftHandMiddle1", "middle_01_l"),
    ("LeftHandMiddle2", "middle_02_l"),
    ("LeftHandMiddle3", "middle_03_l"),
    ("LeftHandRing1", "ring_01_l"),
    ("LeftHandRing2", "ring_02_l"),
    ("LeftHandRing3", "ring_03_l"),
    ("LeftHandPinky1", "pinky_01_l"),
    ("LeftHandPinky2", "pinky_02_l"),
    ("LeftHandPinky3", "pinky_03_l"),
    ("RightHandThumb1", "thumb_01_r"),
    ("RightHandThumb2", "thumb_02_r"),
    ("RightHandThumb3", "thumb_03_r"),
    ("RightHandIndex1", "index_01_r"),
    ("RightHandIndex2", "index_02_r"),
    ("RightHandIndex3", "index_03_r"),
    ("RightHandMiddle1", "middle_01_r"),
    ("RightHandMiddle2", "middle_02_r"),
    ("RightHandMiddle3", "middle_03_r"),
    ("RightHandRing1", "ring_01_r"),
    ("RightHandRing2", "ring_02_r"),
    ("RightHandRing3", "ring_03_r"),
    ("RightHandPinky1", "pinky_01_r"),
    ("RightHandPinky2", "pinky_02_r"),
    ("RightHandPinky3", "pinky_03_r"),
];

/// Retarget every clip of `lib` onto `target`. Clips named `A_TPose` / `T-Pose` are skipped.
pub fn retarget(lib: &DonorLib, target: &Skeleton, bones: &Bones) -> Vec<Clip> {
    todo!("retarget::retarget {} {} {}", lib.lib, target.len(), bones.pelvis)
}

/// Sample a dense clip at `time` (wrap when looping, clamp otherwise) with slerp between frames.
pub fn sample(clip: &Clip, time: f32) -> Pose {
    todo!("retarget::sample {} {time} {SAMPLE_HZ}", clip.track)
}

/// Horizontal travel between two times (handles wrap for looping clips).
pub fn travel(clip: &Clip, t0: f32, t1: f32) -> Vec3 {
    todo!("retarget::travel {} {t0} {t1}", clip.track)
}
