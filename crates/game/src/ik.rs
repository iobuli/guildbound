//! Analytic two-bone IK. Rest lengths are immutable; only rotations change.
use bevy::prelude::*;
use crate::motion_math::world_transform;

pub struct Solution {
    pub knee: Vec3,
    pub tip: Vec3,
}

pub fn solve(root: Vec3, target: Vec3, previous_pole: Vec3, fallback: Vec3, lengths: [f32; 2]) -> Solution {
    let [a, b] = lengths;
    let offset = target - root;
    let axis = offset.normalize_or(Vec3::NEG_Y);
    let distance = offset.length().clamp((a - b).abs() + 0.0001, a + b - 0.0001);
    let project = |v: Vec3| v - axis * v.dot(axis);
    let mut pole = project(previous_pole);
    if pole.length_squared() < 0.0001 { pole = project(fallback); }
    if pole.length_squared() < 0.0001 {
        pole = project(if axis.x.abs() < 0.8 { Vec3::X } else { Vec3::Z });
    }
    pole = pole.normalize_or(Vec3::Z);
    let along = (a * a - b * b + distance * distance) / (2.0 * distance);
    let height = (a * a - along * along).max(0.0).sqrt();
    Solution { knee: root + axis * along + pole * height, tip: root + axis * distance }
}

pub fn rotate_world(world: &mut World, bone: Entity, rotation: Quat) -> Option<()> {
    let parent = world.get::<ChildOf>(bone)?.parent();
    let (_, parent_rotation, _) = world_transform(world, parent)?.to_scale_rotation_translation();
    world.get_mut::<Transform>(bone)?.rotation = (parent_rotation.inverse() * rotation).normalize();
    Some(())
}

pub fn apply(world: &mut World, joints: [Entity; 3], target: Vec3, pole: Vec3, fallback: Vec3, lengths: [f32; 2]) -> Option<Solution> {
    let [hip, knee, foot] = joints;
    let hip_pose = world_transform(world, hip)?;
    let knee_pose = world_transform(world, knee)?;
    let solution = solve(hip_pose.translation(), target, pole, fallback, lengths);
    let (_, hip_rotation, _) = hip_pose.to_scale_rotation_translation();
    let before = (knee_pose.translation() - hip_pose.translation()).normalize_or(Vec3::NEG_Y);
    let after = (solution.knee - hip_pose.translation()).normalize_or(Vec3::NEG_Y);
    rotate_world(world, hip, Quat::from_rotation_arc(before, after) * hip_rotation)?;
    // The first rotation moves the knee and foot. Use that current hierarchy,
    // rather than stale propagated transforms, for the second rotation.
    let knee_pose = world_transform(world, knee)?;
    let foot_pose = world_transform(world, foot)?;
    let (_, knee_rotation, _) = knee_pose.to_scale_rotation_translation();
    let before = (foot_pose.translation() - knee_pose.translation()).normalize_or(Vec3::NEG_Y);
    let after = (solution.tip - knee_pose.translation()).normalize_or(Vec3::NEG_Y);
    rotate_world(world, knee, Quat::from_rotation_arc(before, after) * knee_rotation)?;
    Some(solution)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreachable_and_near_straight_targets_keep_lengths_and_pole_side() {
        let root = Vec3::Y;
        for target in [Vec3::ZERO, Vec3::new(10.0, -10.0, 2.0), root, Vec3::new(0.00001, 0.1, 0.0)] {
            let s = solve(root, target, Vec3::Z, Vec3::Z, [0.45, 0.42]);
            assert!((s.knee.distance(root) - 0.45).abs() < 1e-4);
            assert!((s.tip.distance(s.knee) - 0.42).abs() < 1e-4);
            assert!(s.knee.is_finite() && s.tip.is_finite());
            assert!((s.knee - root).dot(Vec3::Z) >= -1e-4, "knee stays on the pole side");
        }
    }

    #[test]
    fn scaled_hierarchy_hits_target_without_changing_bone_translation_or_scale() {
        let mut world = World::new();
        let rig = world.spawn(Transform::from_scale(Vec3::splat(0.01))).id();
        let hip = world.spawn(Transform::from_xyz(0.0, 90.0, 0.0)).id();
        let knee = world.spawn(Transform::from_xyz(0.0, -45.0, 0.0)).id();
        let foot = world.spawn(Transform::from_xyz(0.0, -42.0, 0.0)).id();
        world.entity_mut(rig).add_child(hip);
        world.entity_mut(hip).add_child(knee);
        world.entity_mut(knee).add_child(foot);
        let before: Vec<_> = [hip, knee, foot].iter().map(|&e| *world.get::<Transform>(e).unwrap()).collect();
        let target = Vec3::new(0.2, 0.1, 0.1);
        apply(&mut world, [hip, knee, foot], target, Vec3::Z, Vec3::Z, [0.45, 0.42]).unwrap();
        assert!(world_transform(&world, foot).unwrap().translation().distance(target) < 1e-4);
        for (e, old) in [hip, knee, foot].iter().zip(before) {
            let t = world.get::<Transform>(*e).unwrap();
            assert_eq!(t.translation, old.translation);
            assert_eq!(t.scale, old.scale);
        }
    }
}
