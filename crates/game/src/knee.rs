//! Knee hinge rule (boss, 2026-10-12): a knee is a hinge. It bends one way only (the knee points forward,
//! the shin swings back), locks straight (no hyperextension, no backward bend) and flexes at most 135 deg.
//! Runs last on every pose (after animation, inertia, support IK and guard IK), so no clip, blend or IK
//! layer can leave a leg bent the wrong way. Corrections keep the hip and the foot's world rotation; only
//! the thigh and shin rotate. Presentation only.
use bevy::prelude::*;

use crate::anim::FighterRig;
use crate::motion_math::world_transform;
use crate::sim_bridge::Sim;

pub const MAX_FLEX_DEG: f32 = 135.0;

/// Counts for the live probe (gb.presentation): legs checked, backward bends and over-flexions fixed.
#[derive(Resource, Clone, Copy)]
pub struct KneeStats {
    pub enabled: bool,
    pub checked: u64,
    pub backward: u64,
    pub overflex: u64,
}

impl Default for KneeStats {
    fn default() -> Self {
        Self { enabled: true, checked: 0, backward: 0, overflex: 0 }
    }
}

/// A correction for one leg: the new ankle target and the forward pole, or None if the leg is legal.
/// `forward` is the direction the knee must face (the pelvis's forward). Knee interior angle 180 = straight.
pub fn fix(hip: Vec3, knee: Vec3, ankle: Vec3, lengths: [f32; 2], forward: Vec3) -> Option<(Vec3, Vec3, bool)> {
    let [a, b] = lengths;
    let line = ankle - hip;
    let u = line.normalize_or(Vec3::NEG_Y);
    let off = (knee - hip) - u * (knee - hip).dot(u);
    let fwd = (forward - u * forward.dot(u)).normalize_or(Vec3::ZERO);
    let backward = fwd != Vec3::ZERO && off.length() > 0.02 && off.dot(fwd) < -0.03;
    // interior angle at the knee from the current hip-ankle distance
    let d = line.length().clamp((a - b).abs() + 1e-4, a + b - 1e-4);
    let interior = ((a * a + b * b - d * d) / (2.0 * a * b)).clamp(-1.0, 1.0).acos();
    let min_interior = (180.0 - MAX_FLEX_DEG).to_radians();
    let overflex = interior < min_interior;
    if !backward && !overflex {
        return None;
    }
    let reach = if overflex { (a * a + b * b - 2.0 * a * b * min_interior.cos()).sqrt() } else { d };
    let pole = if fwd == Vec3::ZERO { forward } else { fwd };
    Some((hip + u * reach, pole, backward))
}

pub fn apply(world: &mut World) {
    let sim = world.resource::<Sim>();
    let facings: Vec<Vec3> = sim.world.fighters.iter().map(|f| Vec3::new(f.facing.x, 0.0, f.facing.z)).collect();
    let mut query = world.query::<&FighterRig>();
    let rigs: Vec<(usize, Vec<Entity>)> = query.iter(world).map(|r| (r.fighter, r.bones.clone())).collect();
    let mut stats = world.get_resource::<KneeStats>().copied().unwrap_or_default();
    if !stats.enabled {
        return;
    }
    for (fighter, bones) in rigs {
        let Some(&facing) = facings.get(fighter) else { continue };
        let find = |world: &World, n: &str| bones.iter().copied().find(|&e| world.get::<Name>(e).is_some_and(|x| x.as_str() == n));
        for side in ["L", "R"] {
            let (Some(hip), Some(knee), Some(foot)) = (
                find(world, &format!("B_{side}_Thigh")),
                find(world, &format!("B_{side}_Calf")),
                find(world, &format!("B_{side}_Foot")),
            ) else { continue };
            let (Some(h), Some(k), Some(f)) = (world_transform(world, hip), world_transform(world, knee), world_transform(world, foot)) else { continue };
            let (h, k, a) = (h.translation(), k.translation(), f.translation());
            let lengths = [h.distance(k), k.distance(a)];
            if !lengths.iter().all(|l| l.is_finite() && (0.1..0.8).contains(l)) {
                continue;
            }
            // the knee faces the pelvis's forward (hip line x up); a foot's toe direction flips when the
            // foot lifts behind, so it is not a safe reference
            let heading = match (find(world, "B_L_Thigh"), find(world, "B_R_Thigh")) {
                (Some(l), Some(r)) => match (world_transform(world, l), world_transform(world, r)) {
                    (Some(l), Some(r)) => {
                        let left = l.translation() - r.translation();
                        Vec3::new(left.x, 0.0, left.z).normalize_or_zero().cross(Vec3::Y).normalize_or(facing)
                    }
                    _ => facing,
                },
                _ => facing,
            };
            stats.checked += 1;
            let Some((target, pole, backward)) = fix(h, k, a, lengths, heading) else { continue };
            if backward { stats.backward += 1 } else { stats.overflex += 1 }
            let (_, foot_rotation, _) = f.to_scale_rotation_translation();
            crate::ik::apply(world, [hip, knee, foot], target, pole, heading, lengths);
            crate::ik::rotate_world(world, foot, foot_rotation);
        }
    }
    world.insert_resource(stats);
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: [f32; 2] = [0.45, 0.42];

    #[test]
    fn a_forward_bent_knee_within_range_is_left_alone() {
        // knee ahead of the hip-ankle line (+Z forward), moderate bend
        let (hip, ankle) = (Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.0, 0.1, 0.0));
        let s = crate::ik::solve(hip, ankle, Vec3::Z, Vec3::Z, L);
        assert!(fix(hip, s.knee, s.tip, L, Vec3::Z).is_none());
    }

    #[test]
    fn a_backward_knee_is_flipped_forward() {
        let (hip, ankle) = (Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.0, 0.1, 0.0));
        let s = crate::ik::solve(hip, ankle, Vec3::NEG_Z, Vec3::NEG_Z, L);
        let (target, pole, backward) = fix(hip, s.knee, s.tip, L, Vec3::Z).expect("backward bend must be fixed");
        assert!(backward);
        let solved = crate::ik::solve(hip, target, pole, pole, L);
        assert!((solved.knee - hip).dot(Vec3::Z) > 0.0, "knee now in front");
        assert!(solved.tip.distance(s.tip) < 1e-3, "foot stays where it was");
    }

    #[test]
    fn over_flexion_is_capped_at_135_degrees() {
        let hip = Vec3::new(0.0, 0.9, 0.0);
        // ankle tucked right under the hip: interior angle about 20 deg
        let s = crate::ik::solve(hip, hip + Vec3::new(0.0, -0.06, -0.08), Vec3::Z, Vec3::Z, L);
        let (target, pole, backward) = fix(hip, s.knee, s.tip, L, Vec3::Z).expect("over-flexion must be fixed");
        assert!(!backward);
        let solved = crate::ik::solve(hip, target, pole, pole, L);
        let interior = (hip - solved.knee).angle_between(solved.tip - solved.knee).to_degrees();
        assert!((interior - (180.0 - MAX_FLEX_DEG)).abs() < 0.5, "{interior}");
    }
}
