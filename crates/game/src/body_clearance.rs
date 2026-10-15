//! Arm clearance against the model's measured torso. Uses geometry, never the balance estimate.
use std::collections::HashMap;
use bevy::prelude::*;
use crate::{motion_math::world_transform, rig_profile::RigProfile};

struct Volume { origin: Vec3, up: Vec3, right: Vec3, forward: Vec3, length: f32, radius: Vec2, offset: Vec2 }

impl Volume {
    fn correction(&self, p: Vec3, padding: f32, fallback: Vec3) -> Vec3 {
        let d = p - self.origin;
        let h = d.dot(self.up);
        if h < 0.0 || h > self.length { return Vec3::ZERO; }
        let radius = self.radius + Vec2::splat(padding);
        let radial = Vec2::new(d.dot(self.right), d.dot(self.forward)) - self.offset;
        let rho = (radial / radius).length();
        if rho >= 1.0 { return Vec3::ZERO; }
        let radial = if rho > 1e-5 { radial / rho } else {
            let dir = Vec2::new(fallback.dot(self.right), fallback.dot(self.forward)).normalize_or(Vec2::X);
            dir / (dir / radius).length().max(1e-5)
        };
        let shift = radial - (Vec2::new(d.dot(self.right), d.dot(self.forward)) - self.offset);
        self.right * shift.x + self.forward * shift.y
    }
}

fn volumes(world: &World, map: &HashMap<String, Entity>, profile: &RigProfile) -> Vec<Volume> {
    let pos = |n: &str| map.get(n).and_then(|&e| world_transform(world, e)).map(|g| g.translation());
    profile.torso.iter().filter_map(|s| {
        let (a, b, l, r) = (pos(&s.from)?, pos(&s.to)?, pos(&s.lateral[0])?, pos(&s.lateral[1])?);
        let length = a.distance(b);
        if length < 1e-5 { return None; }
        let up = (b - a) / length;
        let right = (l - r - up * (l - r).dot(up)).normalize_or_zero();
        if right == Vec3::ZERO { return None; }
        Some(Volume { origin: a, up, right, forward: up.cross(right), length, radius: s.radius, offset: s.offset })
    }).collect()
}

pub fn apply(world: &mut World) {
    let mut q = world.query::<(Entity, &crate::anim::FighterRig)>();
    let mut jobs: Vec<_> = q.iter(world).map(|(e, r)| (e, r.bones.clone())).collect();
    let mut q = world.query::<(Entity, &crate::studio::StudioRig)>();
    jobs.extend(q.iter(world).filter(|(_, r)| r.rules).map(|(e, r)| (e, r.bones.clone())));
    for (root, bones) in jobs {
        let Some(profile) = world.get::<RigProfile>(root).cloned().filter(|p| !p.torso.is_empty()) else { continue };
        let map: HashMap<_, _> = bones.iter().filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str().to_owned(), e))).collect();
        let volumes = volumes(world, &map, &profile);
        for (s, hand, side) in [("L", "B_L_Forearm.001", 1.0), ("R", "B_R_Hand", -1.0)] {
            let names = [format!("B_{s}_UpperArm"), format!("B_{s}_Forearm"), hand.into()];
            let Some(joints) = names.iter().map(|n| map.get(n).copied()).collect::<Option<Vec<_>>>() else { continue };
            let joints = [joints[0], joints[1], joints[2]];
            let radius = profile.limb_radii.get(&names[1]).copied().unwrap_or(0.04) + 0.008;
            for _ in 0..3 {
                let Some(poses) = joints.iter().map(|&e| world_transform(world, e)).collect::<Option<Vec<_>>>() else { break };
                let points = [poses[0].translation(), poses[1].translation(), poses[2].translation()];
                let lengths = [points[0].distance(points[1]), points[1].distance(points[2])];
                if lengths.iter().any(|l| !(0.05..0.8).contains(l)) { break; }
                let fallback = world.get::<Transform>(root).map_or(Vec3::X * side, |t| t.rotation * Vec3::X * side);
                let push = |p: Vec3| volumes.iter().map(|v| v.correction(p, radius, fallback)).max_by(|a, b| a.length_squared().total_cmp(&b.length_squared())).unwrap_or(Vec3::ZERO);
                let elbow_shift = push(points[1]);
                let wrist_shift = (0..=4).map(|i| push(points[1].lerp(points[2], i as f32 / 4.0)))
                    .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared())).unwrap_or(Vec3::ZERO);
                if elbow_shift.length_squared().max(wrist_shift.length_squared()) < 1e-8 { break; }
                let wrist_rotation = poses[2].to_scale_rotation_translation().1;
                let pole = points[1] + elbow_shift + wrist_shift * 0.5 - points[0];
                crate::ik::apply(world, joints, points[2] + wrist_shift, pole, fallback, lengths);
                crate::ik::rotate_world(world, joints[2], wrist_rotation);
            }
        }
    }
}

pub fn snapshot(world: &World, root: Entity, bones: &[Entity]) -> serde_json::Value {
    let Some(profile) = world.get::<RigProfile>(root) else { return serde_json::json!({"measured":false}) };
    let map: HashMap<_, _> = bones.iter().filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str().to_owned(), e))).collect();
    let volumes = volumes(world, &map, profile);
    let mut worst = 0.0f32;
    for (s, hand) in [("L", "B_L_Forearm.001"), ("R", "B_R_Hand")] {
        let names = [format!("B_{s}_Forearm"), hand.into()];
        let positions: Option<Vec<_>> = names.iter().map(|n| map.get(n).and_then(|&e| world_transform(world, e)).map(|g| g.translation())).collect();
        let Some(positions) = positions else { continue };
        let radius = profile.limb_radii.get(&names[0]).copied().unwrap_or(0.04) + 0.008;
        for i in 0..=4 {
            let p = positions[0].lerp(positions[1], i as f32 / 4.0);
            for volume in &volumes { worst = worst.max(volume.correction(p, radius, Vec3::X).length()); }
        }
    }
    serde_json::json!({"measured":!volumes.is_empty(), "torso_sections":volumes.len(), "forearm_penetration_m":worst})
}
