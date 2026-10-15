//! Contact layer, applied after springs: foot IK onto the ground, pelvis drop to the lower
//! foot, COP lean (Kwon 2020: torso over COP + h·a/g), squash, look-at, hand IK.
//! Pure pose maths; the ground is queried through a callback so this stays testable.
use super::{Bones, ContactRequest, Pose, Skeleton};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Ground {
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct ContactParams {
    /// Lean gain (1 = physical h·a/g).
    pub lean: f32,
    /// Max lean angle (rad).
    pub max_lean: f32,
    /// Foot IK reach above / below the animated foot (m).
    pub reach_up: f32,
    pub reach_down: f32,
    /// Smoothing rate for pelvis drop (1/s).
    pub pelvis_rate: f32,
}

impl Default for ContactParams {
    fn default() -> Self {
        Self {
            lean: 1.0,
            max_lean: 0.35,
            reach_up: 0.45,
            reach_down: 0.35,
            pelvis_rate: 14.0,
        }
    }
}

/// Smoothed per-character contact state.
#[derive(Component, Clone, Debug, Default)]
pub struct ContactState {
    pub pelvis_drop: f32,
    pub lean: Vec3,
    pub look: Quat,
}

/// Analytic two-bone IK (from `skeleton_ik::solve`): returns the middle joint position.
pub fn two_bone(root: Vec3, target: Vec3, pole: Vec3, lengths: [f32; 2]) -> Vec3 {
    todo!("contact::two_bone {root} {target} {pole} {lengths:?}")
}

/// Rotate `pose` in place so its feet sit on the ground, hands reach their targets, the
/// torso leans with acceleration and the head looks toward `look_at`.
/// `model` = world transform of the model (skeleton base is applied on top of it).
pub fn apply(
    pose: &mut Pose,
    skeleton: &Skeleton,
    bones: &Bones,
    model: &Transform,
    request: &ContactRequest,
    ground: &mut dyn FnMut(Vec3) -> Option<Ground>,
    params: &ContactParams,
    state: &mut ContactState,
    dt: f32,
) {
    todo!(
        "contact::apply {} {} {} {model:?} {request:?} {} {params:?} {state:?} {dt}",
        pose.rot.len(),
        skeleton.len(),
        bones.pelvis,
        ground(Vec3::ZERO).is_some()
    )
}
