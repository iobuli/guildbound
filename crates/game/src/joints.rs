//! Joint authority (boss, 2026-10-12): every joint gets an anatomical bend range, applied on top of all
//! animation, blending and IK, so the final pose always obeys the body. Knees are hinges handled by knee.rs
//! (which keeps planted feet in place and runs after this layer).
//!
//! The rule is geometric and roll-invariant: the bend at a joint is the angle between the segment into it
//! and the segment out of it, measured from joint positions. This rig (a 3ds Max biped export) flips bone
//! roll between parents and children, so limits in bone-local frames are meaningless; segment angles are not.
//! Limits are soft: inside 85% of the range nothing changes; past it the bend eases into the limit with a
//! tanh shoulder (continuous slope), so a clip brushing a limit never pops. Presentation only.
use bevy::prelude::*;

use crate::anim::FighterRig;
use crate::motion_math::world_transform;

/// (segment start, joint, segment end, min bend deg, max bend deg). Bend 0 = the segments are in line.
/// Calibrated on the Quaternius skeleton (hero and NPCs): its rest pose already bends the neck 30 deg and the
/// upper chest 14 (the RPG rig: 4 and 5), so those limits sit higher than a straight-rest rig would need.
pub const RULES: &[(&str, &str, &str, f32, f32)] = &[
    ("B_Pelvis", "B_Spine", "B_Spine1", 0.0, 40.0),
    ("B_Spine", "B_Spine1", "B_Spine2", 0.0, 35.0),
    ("B_Spine1", "B_Spine2", "B_Neck", 0.0, 50.0),
    ("B_Spine2", "B_Neck", "B_Head", 0.0, 70.0),
    ("B_L_Clavicle", "B_L_UpperArm", "B_L_Forearm", 0.0, 150.0),
    ("B_R_Clavicle", "B_R_UpperArm", "B_R_Forearm", 0.0, 150.0),
    // elbow: flexes up to 155 deg
    ("B_L_UpperArm", "B_L_Forearm", "B_L_Forearm.001", 0.0, 155.0),
    ("B_R_UpperArm", "B_R_Forearm", "B_R_Hand", 0.0, 155.0),
    // wrist
    ("B_L_Forearm", "B_L_Forearm.001", "B_L_Finger2", 0.0, 80.0),
    ("B_R_Forearm", "B_R_Hand", "B_R_Finger2", 0.0, 80.0),
    // ankle: neutral about 70 deg on this skeleton (measured idle); push-off plantarflexes about 50 more
    // (Kevin mocap reaches 22-31 in every run, 13 landing a jump), dorsiflexion stays well under 145
    ("B_L_Calf", "B_L_Foot", "B_L_Toe0", 15.0, 145.0),
    ("B_R_Calf", "B_R_Foot", "B_R_Toe0", 15.0, 145.0),
];

#[derive(Resource)]
pub struct JointAuthority {
    pub enabled: bool,
    /// Live bend per joint for fighter 0 (deg).
    pub readings: Vec<(String, f32)>,
    pub corrected: u64,
    pub evaluated: u64,
    pub worst_excess_deg: f32,
}

impl Default for JointAuthority {
    fn default() -> Self {
        Self { enabled: true, readings: Vec::new(), corrected: 0, evaluated: 0, worst_excess_deg: 0.0 }
    }
}

/// Soft limit of `x` into [lo, hi] (radians): identity well inside, eased into the bound in the last 15%.
pub fn soft_range(x: f32, lo: f32, hi: f32) -> f32 {
    let margin = 0.15 * (hi - lo).max(1e-4);
    let ease = |over: f32| margin * (over / margin).tanh();
    if x > hi - margin {
        hi - margin + ease(x - (hi - margin))
    } else if lo > 0.0 && x < lo + margin {
        lo + margin - ease(lo + margin - x)
    } else {
        x
    }
}

pub fn apply(world: &mut World) {
    let mut auth = world.remove_resource::<JointAuthority>().unwrap_or_default();
    let mut query = world.query::<(Entity, &FighterRig)>();
    let mut rigs: Vec<_> = query.iter(world).map(|(e, r)| (e, r.fighter, r.bones.clone())).collect();
    let mut studio = world.query::<(Entity, &crate::studio::StudioRig)>();
    rigs.extend(studio.iter(world).filter(|(_, r)| r.rules).map(|(e, r)| (e, usize::MAX, r.bones.clone())));
    auth.readings.clear();
    for (root, fighter, bones) in rigs {
        let find = |world: &World, n: &str| bones.iter().copied().find(|&e| world.get::<Name>(e).is_some_and(|x| x.as_str() == n));
        let profile = world.get::<crate::rig_profile::RigProfile>(root).cloned().unwrap_or_default();
        for rule in &profile.rules {
            let (j, lo, hi) = (rule.joint.as_str(), rule.min, rule.max);
            let (Some(ea), Some(ej), Some(eb)) = (find(world, &rule.from), find(world, j), find(world, &rule.to)) else { continue };
            let (Some(pa), Some(pj), Some(pb)) = (world_transform(world, ea), world_transform(world, ej), world_transform(world, eb)) else { continue };
            let u = (pj.translation() - pa.translation()).normalize_or_zero();
            let v = (pb.translation() - pj.translation()).normalize_or_zero();
            if u == Vec3::ZERO || v == Vec3::ZERO {
                continue;
            }
            let bend = u.dot(v).clamp(-1.0, 1.0).acos();
            if fighter == 0 {
                auth.readings.push((j.to_owned(), bend.to_degrees()));
            }
            if !auth.enabled {
                continue;
            }
            auth.evaluated += 1;
            let (lo, hi) = (lo.to_radians(), hi.to_radians());
            let target = soft_range(bend, lo, hi);
            let axis = u.cross(v).normalize_or_zero();
            if (target - bend).abs() < 1e-4 || axis == Vec3::ZERO {
                continue;
            }
            auth.corrected += 1;
            let excess = (bend - hi).max(lo - bend).max(0.0);
            auth.worst_excess_deg = auth.worst_excess_deg.max(excess.to_degrees());
            // turn the joint's bone about the bend axis so the outgoing segment sits at the target bend
            let (_, rot, _) = pj.to_scale_rotation_translation();
            crate::ik::rotate_world(world, ej, Quat::from_axis_angle(axis, target - bend) * rot);
        }
    }
    world.insert_resource(auth);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_inside_the_range_are_untouched() {
        for x in [0.0f32, 0.3, 1.0, 2.0] {
            assert_eq!(soft_range(x, 0.0, 2.7), x);
        }
        assert_eq!(soft_range(1.5, 0.7, 2.5), 1.5);
    }

    #[test]
    fn the_range_is_never_exceeded() {
        for k in 0..400 {
            let x = k as f32 * 0.01;
            let y = soft_range(x, 0.7, 2.5);
            assert!((0.7 - 1e-5..=2.5 + 1e-5).contains(&y), "{x} -> {y}");
        }
    }

    #[test]
    fn the_limit_is_continuous_no_pops() {
        let mut prev = soft_range(0.0, 0.7, 2.5);
        for k in 1..400 {
            let y = soft_range(k as f32 * 0.01, 0.7, 2.5);
            assert!((y - prev).abs() <= 0.0101, "jump at {k}");
            prev = y;
        }
    }

    #[test]
    fn a_rotated_bone_reaches_the_target_bend() {
        // upper arm along +X, forearm folded back past the elbow limit; rotating about u x v by (target - bend)
        let u = Vec3::X;
        let v = Vec3::new(-0.95, 0.31, 0.0).normalize();
        let bend = u.dot(v).acos();
        let target = soft_range(bend, 0.0, 155f32.to_radians());
        let v2 = Quat::from_axis_angle(u.cross(v).normalize(), target - bend) * v;
        assert!((u.dot(v2).acos() - target).abs() < 1e-4);
    }
}
