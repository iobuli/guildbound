//! Secondary motion: one angular spring per bone chasing the composed target pose, giving
//! overlap / follow-through, plus impulse injection for hits and spine squash/stretch.
//! Springs act on LOCAL rotations in parent space; impulses arrive in world space and are
//! converted using the model rotation and FK.
use super::{Bones, Pose, Skeleton};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct SpringParams {
    /// Natural frequency in Hz for the core (spine/neck/head).
    pub core_hz: f32,
    /// Natural frequency in Hz for limbs (arms; legs use 3× this so feet stay planted).
    pub limb_hz: f32,
    /// Damping ratio: 1 = critically damped, <1 overshoot.
    pub damping: f32,
}

impl Default for SpringParams {
    fn default() -> Self {
        Self {
            core_hz: 7.0,
            limb_hz: 9.0,
            damping: 0.6,
        }
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct Springs {
    /// Current local rotation per bone (empty until the first step).
    pub current: Vec<Quat>,
    /// Angular velocity per bone (parent space, rad/s).
    pub velocity: Vec<Vec3>,
    pub squash: f32,
    pub squash_velocity: f32,
}

impl Springs {
    /// Advance toward `target` by `dt` (sub-stepped for stability, dt clamped to 1/20 s),
    /// apply `impulses` (bone, world axis*angle rad/s), return the sprung pose.
    /// Bones without a spring (pelvis translation, root, fingers) pass through.
    pub fn step(
        &mut self,
        target: &Pose,
        skeleton: &Skeleton,
        bones: &Bones,
        model_rotation: Quat,
        impulses: &[(usize, Vec3)],
        squash_target: f32,
        params: &SpringParams,
        dt: f32,
    ) -> Pose {
        todo!(
            "secondary::step {} {} {model_rotation} {} {squash_target} {params:?} {dt}",
            target.rot.len(),
            skeleton.len(),
            bones.pelvis + impulses.len()
        )
    }
}
