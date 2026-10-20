//! Rubber-band fists: a fist strike stretches the striking forearm like a rubber band and lets it
//! snap back with a wobble.
//!
//! - Each fighter carries a damped spring per arm (`Goop`). While a fist strike's blow is landing
//!   (`Melee::fist`), the spring of that arm is driven toward `STRETCH`; afterwards toward 0. It is
//!   underdamped (`STIFFNESS`, `DAMPING`: damping ratio about 0.3), so it overshoots and wobbles back.
//! - After `compose` writes the pose and before transforms propagate, the forearm is scaled along its
//!   own length by `1 + x` and by `1/sqrt(1 + x)` across (volume kept), which carries the fist out.
//!   The length axis is the hand's rest offset in the forearm (the dominant local axis).
//! - The fist undoes the arm's stretch and adds its own squash along the punch while stretched
//!   (`SQUASH`), so it flattens on contact and bulges sideways. This assumes the wrist is near its
//!   rest bend, so the forearm and hand share a length axis; a sharply bent wrist shears slightly.
//! - Scales are written absolutely every frame, so a missed frame never accumulates.
use super::combat::Melee;
use crate::motion::bank::PoseBank;
use crate::motion::{MotionSet, Rig, RigBone};
use bevy::prelude::*;

/// Forearm length gain at full stretch (0.35 = 35 % longer).
pub const STRETCH: f32 = 0.35;
/// Spring stiffness (1/s²) and damping (1/s): natural frequency ~15 rad/s, damping ratio ~0.3.
pub const STIFFNESS: f32 = 220.0;
pub const DAMPING: f32 = 9.0;
/// Fist flattening along the punch per unit of stretch.
pub const SQUASH: f32 = 0.6;
/// Largest sub-step (s) of the spring integration.
const SUBSTEP: f32 = 0.004;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    pub x: f32,
    pub v: f32,
}

impl Spring {
    /// Advance by `dt` toward `target` (semi-implicit Euler, sub-stepped for stability).
    pub fn step(&mut self, target: f32, dt: f32) {
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.1) } else { 0.0 };
        let n = ((dt / SUBSTEP).ceil() as usize).clamp(1, 32);
        let h = dt / n as f32;
        for _ in 0..n {
            self.v += (STIFFNESS * (target - self.x) - DAMPING * self.v) * h;
            self.x += self.v * h;
        }
    }
}

/// One spring per arm: [left, right].
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Goop {
    pub arms: [Spring; 2],
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, attach)
        .add_systems(PostUpdate, stretch.after(MotionSet::Compose).before(TransformSystems::Propagate));
}

fn attach(mut commands: Commands, fighters: Query<Entity, (With<Melee>, With<Rig>, Without<Goop>)>) {
    for e in &fighters {
        commands.entity(e).insert(Goop::default());
    }
}

/// Scale `s` along local axis `axis` (0 = X, 1 = Y, 2 = Z) and `1/sqrt(s)` along the other two.
pub fn along(axis: usize, s: f32) -> Vec3 {
    let s = s.max(0.05);
    let across = 1.0 / s.sqrt();
    let mut v = Vec3::splat(across);
    v[axis] = s;
    v
}

/// The local axis a bone's child offset mostly points along.
pub fn length_axis(child_offset: Vec3) -> usize {
    let a = child_offset.abs();
    if a.x >= a.y && a.x >= a.z {
        0
    } else if a.y >= a.z {
        1
    } else {
        2
    }
}

/// (forearm scale, hand scale) for spring extension `x` along `axis`.
pub fn arm_scales(axis: usize, x: f32) -> (Vec3, Vec3) {
    let arm = along(axis, 1.0 + x);
    // Undo the arm's stretch on the fist, then squash it along the punch while stretched.
    let squash = along(axis, 1.0 - SQUASH * x.max(0.0));
    (arm, squash / arm)
}

fn stretch(
    time: Res<Time>,
    bank: Option<Res<PoseBank>>,
    mut fighters: Query<(&Melee, &Rig, &mut Goop)>,
    mut bones: Query<&mut Transform, With<RigBone>>,
) {
    let Some(bank) = bank else { return };
    let dt = time.delta_secs();
    for (melee, rig, mut goop) in &mut fighters {
        let fist = melee.fist();
        for side in 0..2 {
            let target = match fist {
                Some((s, true)) if s == side => STRETCH,
                _ => 0.0,
            };
            goop.arms[side].step(target, dt);
            let (lower, hand) = (bank.bones.lowerarm[side], bank.bones.hand[side]);
            let axis = length_axis(bank.skeleton.rest[hand].translation);
            let (arm_scale, hand_scale) = arm_scales(axis, goop.arms[side].x);
            if let Some(mut tf) = rig.bones.get(lower).and_then(|&e| bones.get_mut(e).ok()) {
                tf.scale = arm_scale;
            }
            if let Some(mut tf) = rig.bones.get(hand).and_then(|&e| bones.get_mut(e).ok()) {
                tf.scale = hand_scale;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_overshoots_then_settles() {
        let mut s = Spring::default();
        let mut peak = 0.0f32;
        for _ in 0..60 {
            s.step(STRETCH, 1.0 / 120.0);
            peak = peak.max(s.x);
        }
        assert!(peak > STRETCH * 1.2, "underdamped: overshoot {peak} past {STRETCH}");
        for _ in 0..600 {
            s.step(0.0, 1.0 / 60.0);
        }
        assert!(s.x.abs() < 1e-3 && s.v.abs() < 1e-2, "settles back: x {} v {}", s.x, s.v);
    }

    #[test]
    fn spring_is_stable_at_low_frame_rates() {
        let mut s = Spring::default();
        for _ in 0..200 {
            s.step(STRETCH, 0.1);
            assert!(s.x.abs() < 2.0, "blew up at x {}", s.x);
        }
        assert!((s.x - STRETCH).abs() < 1e-2);
    }

    #[test]
    fn stretch_keeps_volume_and_returns_to_one() {
        let v = along(0, 1.35);
        assert!((v.x * v.y * v.z - 1.0).abs() < 1e-5, "volume {}", v.x * v.y * v.z);
        assert_eq!(along(1, 1.0), Vec3::ONE);
        let (arm, hand) = arm_scales(0, 0.0);
        assert!((arm - Vec3::ONE).abs().max_element() < 1e-6 && (hand - Vec3::ONE).abs().max_element() < 1e-6);
    }

    #[test]
    fn the_fist_flattens_along_the_punch_while_the_arm_stretches() {
        let (arm, hand) = arm_scales(0, STRETCH);
        assert!(arm.x > 1.3 && arm.y < 1.0, "arm {arm}");
        // The fist's net scale (arm × hand) is a squash along X, a bulge across.
        let net = arm * hand;
        assert!(net.x < 1.0 && net.y > 1.0, "net fist {net}");
    }

    #[test]
    fn length_axis_follows_the_child_offset() {
        assert_eq!(length_axis(Vec3::new(0.27, 0.01, -0.02)), 0);
        assert_eq!(length_axis(Vec3::new(0.0, -0.3, 0.05)), 1);
        assert_eq!(length_axis(Vec3::new(0.02, 0.0, 0.25)), 2);
    }
}
