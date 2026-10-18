//! Carry posture: what a character holds bends its body a little (`compose` applies it after
//! `body::resolve`, before contact, so foot IK keeps the feet planted).
//! - the holding shoulder drops with the weight in that hand;
//! - the torso leans away from the heavier hand, to balance it;
//! - a load on the back, or a heavy weapon held at all, pitches the chest forward.
//!
//! `Load` (kilograms) is written by `play::carry`, smoothed there; `angles` turns it into the
//! offsets, all clamped small so no load ever folds the body.
use super::bank::PoseBank;
use super::Pose;
use bevy::prelude::*;

/// Shoulder drop per kilogram in the hand (rad), and its cap.
const DROP_PER_KG: f32 = 0.03;
const DROP_MAX: f32 = 0.14;
/// Side lean per kilogram of difference between the hands (rad), and its cap.
const LEAN_PER_KG: f32 = 0.018;
const LEAN_MAX: f32 = 0.08;
/// Forward pitch per kilogram on the back, and per kilogram held beyond `HEAVY` (rad), capped.
const PITCH_BACK_PER_KG: f32 = 0.012;
const HEAVY: f32 = 2.0;
const PITCH_HELD_PER_KG: f32 = 0.025;
const PITCH_MAX: f32 = 0.12;

/// Kilograms carried: in each hand (`[left, right]`) and on the back / hips.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Load {
    pub hand: [f32; 2],
    pub back: f32,
}

/// The offsets a load asks for (rad): `drop` per shoulder, `lean` of the torso toward the body's
/// right (+) or left (−), `pitch` of the chest forward.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Angles {
    pub drop: [f32; 2],
    pub lean: f32,
    pub pitch: f32,
}

fn kg(x: f32) -> f32 {
    if x.is_finite() { x.max(0.0) } else { 0.0 }
}

pub fn angles(load: &Load) -> Angles {
    let hand = [kg(load.hand[0]), kg(load.hand[1])];
    let held = hand[0] + hand[1];
    Angles {
        drop: hand.map(|m| (m * DROP_PER_KG).min(DROP_MAX)),
        // Heavier left hand → lean right (+), and the other way round.
        lean: ((hand[0] - hand[1]) * LEAN_PER_KG).clamp(-LEAN_MAX, LEAN_MAX),
        pitch: (kg(load.back) * PITCH_BACK_PER_KG + (held - HEAVY).max(0.0) * PITCH_HELD_PER_KG).min(PITCH_MAX),
    }
}

/// Bend `pose` by `load`. Rotations are taken in model space about each bone (+X the body's
/// left, +Y up, +Z forward), as `select::straddle` does.
pub fn apply(bank: &PoseBank, pose: &mut Pose, load: &Load) {
    let a = angles(load);
    if a == Angles::default() {
        return;
    }
    let skeleton = &bank.skeleton;
    let bones = &bank.bones;
    if pose.rot.len() != skeleton.len() {
        return;
    }
    let model = skeleton.fk(pose, bones.pelvis);
    let mut turn = |bone: usize, delta: Quat| {
        let parent = skeleton.parent[bone].map_or(skeleton.base.rotation, |p| model[p].rotation);
        let local = (parent.inverse() * delta * model[bone].rotation).normalize();
        if local.is_finite() {
            pose.rot[bone] = local;
        }
    };
    // Shoulders: the clavicle's outer end goes down (left side about −Z, right about +Z).
    for side in 0..2 {
        if a.drop[side] > 0.0 {
            let s = if side == 0 { -1.0 } else { 1.0 };
            turn(bones.clavicle[side], Quat::from_rotation_z(s * a.drop[side]));
        }
    }
    // Torso: lean and pitch shared by the two upper spine bones (+Z turns the top toward −X,
    // the body's right; +X turns it forward).
    let delta = Quat::from_rotation_z(a.lean * 0.5) * Quat::from_rotation_x(a.pitch * 0.5);
    for bone in [bones.spine[1], bones.spine[2]] {
        turn(bone, delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_hands_change_nothing() {
        assert_eq!(angles(&Load::default()), Angles::default());
        assert_eq!(angles(&Load { hand: [f32::NAN, -3.0], back: f32::INFINITY }), Angles::default());
    }

    #[test]
    fn the_torso_leans_away_from_the_heavy_hand() {
        let right = angles(&Load { hand: [0.0, 3.0], back: 0.0 });
        let left = angles(&Load { hand: [3.0, 0.0], back: 0.0 });
        assert!(right.lean < 0.0 && left.lean > 0.0);
        assert!(right.drop[1] > 0.0 && right.drop[0] == 0.0);
        let even = angles(&Load { hand: [2.0, 2.0], back: 0.0 });
        assert_eq!(even.lean, 0.0);
    }

    #[test]
    fn more_weight_bends_more_but_never_far() {
        let light = angles(&Load { hand: [0.0, 0.4], back: 0.0 });
        let heavy = angles(&Load { hand: [0.0, 3.5], back: 0.0 });
        assert!(heavy.drop[1] > light.drop[1]);
        assert!(heavy.pitch > light.pitch);
        let huge = angles(&Load { hand: [80.0, 0.0], back: 80.0 });
        assert!(huge.drop[0] <= DROP_MAX && huge.lean <= LEAN_MAX && huge.pitch <= PITCH_MAX);
        assert!(angles(&Load { hand: [0.0, 0.0], back: 2.0 }).pitch > 0.0);
    }
}
