//! Physical knockdown overlay, built from the RPG rig's measured segment lengths.
//! Avian owns the capsule bodies; the deterministic sim still owns gameplay.
//! Skin poses are reconstructed in parent space, then eased into authored get-up.
use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{anim::FighterRig, sim_bridge::Sim};
use crate::motion_math::world_transform;

#[derive(Component)]
struct RagdollBody;

pub(crate) struct Link {
    pub bone: Entity,
    pub body: Entity,
    pub origin: Vec3,
    pub rotation: Quat,
    pub parent: Option<usize>,
}

pub(crate) struct Doll {
    fighter: usize,
    pub links: Vec<Link>,
    pub joints: Vec<Entity>,
    last_pose: Vec<(Entity, Transform)>,
    recovering: Option<f32>,
    apex: bool,
    apex_weight: f32,
}

#[derive(Resource, Default)]
pub struct Ragdolls {
    dolls: HashMap<Entity, Doll>,
    last_tick: u64,
}

// start bone, end bone, parent segment, radius, mass, swing cone half-angle.
const SEGMENTS: &[(&str, &str, Option<usize>, f32, f32, f32)] = &[
    ("B_Pelvis", "B_Spine2", None, 0.16, 16.0, 0.0),
    ("B_Spine2", "B_Head", Some(0), 0.18, 20.0, 0.45),
    ("B_Head", "", Some(1), 0.12, 5.0, 0.5),
    ("B_L_UpperArm", "B_L_Forearm", Some(1), 0.065, 3.0, 1.4),
    ("B_L_Forearm", "B_L_Forearm.001", Some(3), 0.055, 2.0, 1.0),
    ("B_R_UpperArm", "B_R_Forearm", Some(1), 0.065, 3.0, 1.4),
    ("B_R_Forearm", "B_R_Hand", Some(5), 0.055, 2.0, 1.0),
    ("B_L_Thigh", "B_L_Calf", Some(0), 0.085, 8.0, 1.0),
    ("B_L_Calf", "B_L_Foot", Some(7), 0.07, 5.0, 0.85),
    ("B_R_Thigh", "B_R_Calf", Some(0), 0.085, 8.0, 1.0),
    ("B_R_Calf", "B_R_Foot", Some(9), 0.07, 5.0, 0.85),
    // feet: a body stands on its soles, not on the ends of its shins
    ("B_L_Foot", "B_L_Toe0", Some(8), 0.045, 1.2, 0.6),
    ("B_R_Foot", "B_R_Toe0", Some(10), 0.045, 1.2, 0.6),
];

/// Reconstruct current transforms, rather than using last frame's propagated
/// globals. This also handles unanimated parents and the centimetre-scale rig.
fn build(world: &mut World, bones: &[Entity], fighter: usize) -> Option<Doll> {
    let f = &world.resource::<Sim>().world.fighters[fighter];
    let velocity = Vec3::new(f.velocity.x, 0.0, f.velocity.z).clamp_length_max(12.0);
    let facing = Vec3::new(f.facing.x, 0.0, f.facing.z);
    build_with(world, bones, fighter, velocity, facing)
}

/// Capsule bodies and anatomical joints for a rig (fighters on a knockdown; the Studio stage when Possession
/// makes it physical).
pub(crate) fn build_with(world: &mut World, bones: &[Entity], fighter: usize, velocity: Vec3, facing: Vec3) -> Option<Doll> {
    let names: HashMap<_, _> = bones.iter().filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str().to_owned(), e))).collect();
    // Unknown rigs retain their authored knockdown; never guess bone paths.
    if !SEGMENTS.iter().all(|(a, b, ..)| names.contains_key(*a) && (b.is_empty() || names.contains_key(*b))) { return None; }
    let angular = Vec3::Y.cross(velocity) * 0.7;
    let hinge_fallback = facing.cross(Vec3::Y).normalize_or(Vec3::X);
    let mut links = Vec::<Link>::new();
    let mut joints = Vec::new();
    for &(start_name, end_name, parent, radius, mass, swing) in SEGMENTS {
        let bone = names[start_name];
        let start_pose = world_transform(world, bone)?;
        let start = start_pose.translation();
        let end = if end_name.is_empty() { start + Vec3::Y * 0.12 } else { world_transform(world, names[end_name])?.translation() };
        let delta = end - start;
        let length = delta.length().max(0.06);
        let rotation = Quat::from_rotation_arc(Vec3::Y, delta.normalize_or(Vec3::Y));
        let center = (start + end) * 0.5;
        let (_, bone_rotation, _) = start_pose.to_scale_rotation_translation();
        let body = world.spawn((
            RagdollBody,
            Name::new(format!("ragdoll {fighter} {start_name}")),
            RigidBody::Dynamic,
            Collider::capsule(radius, (length - 2.0 * radius).max(0.025)),
            Transform::from_translation(center).with_rotation(rotation),
            LinearVelocity(velocity),
            AngularVelocity(angular.clamp_length_max(8.0)),
            LinearDamping(0.35), AngularDamping(2.5),
            Mass(mass), Friction::new(0.75), Restitution::new(0.0),
            // Environment/props only: no self-collision or own fighter proxy.
            CollisionLayers::new(4u32, 1u32),
        )).id();
        if let Some(index) = parent {
            let parent_body = links[index].body;
            let parent_pose = world.get::<Transform>(parent_body).copied()?;
            let anchor1 = parent_pose.rotation.inverse() * (start - parent_pose.translation);
            let anchor2 = rotation.inverse() * (start - center);
            if matches!(start_name, "B_L_Forearm" | "B_R_Forearm" | "B_L_Calf" | "B_R_Calf") {
                let parent_direction = parent_pose.rotation * Vec3::Y;
                let direction = delta.normalize_or(Vec3::Y);
                let knee = start_name.ends_with("Calf");
                // Knees are anatomical hinges (boss rule, knee.rs): the axis is the body's side axis, so
                // flexion only swings the shin back; never an axis taken from a pose that may be straight
                // or bent backwards. Elbows keep the axis of their current bend.
                let axis = if knee { -hinge_fallback } else { parent_direction.cross(direction).normalize_or(hinge_fallback) };
                let bend = if knee {
                    // signed flexion about the side axis; negative = hyperextended
                    axis.dot(parent_direction.cross(direction)).atan2(parent_direction.dot(direction))
                } else {
                    parent_direction.dot(direction).clamp(-1.0, 1.0).acos()
                };
                let flex = if knee { crate::knee::MAX_FLEX_DEG.to_radians() } else { 2.6 };
                let joint = RevoluteJoint::new(parent_body, body)
                    .with_local_anchor1(anchor1)
                    .with_local_anchor2(anchor2)
                    .with_local_basis1(parent_pose.rotation.inverse())
                    .with_local_basis2(rotation.inverse())
                    .with_hinge_axis(axis)
                    .with_angle_limits(-bend, (flex - bend).max(0.0));
                joints.push(world.spawn((joint, JointCollisionDisabled)).id());
            } else {
            let joint = SphericalJoint::new(parent_body, body)
                .with_local_anchor1(anchor1)
                .with_local_anchor2(anchor2)
                .with_local_basis1(parent_pose.rotation.inverse())
                .with_local_basis2(rotation.inverse())
                .with_swing_limits(0.0, swing)
                .with_twist_limits(-0.4, 0.4);
            joints.push(world.spawn((joint, JointCollisionDisabled)).id());
            }
        }
        links.push(Link { bone, body, origin: rotation.inverse() * (start - center), rotation: rotation.inverse() * bone_rotation, parent });
    }
    Some(Doll { fighter, links, joints, last_pose: Vec::new(), recovering: None, apex: false, apex_weight: 0.0 })
}

pub(crate) fn remove_bodies(world: &mut World, doll: &mut Doll) {
    for entity in doll.joints.drain(..) { let _ = world.despawn(entity); }
    for link in &doll.links { let _ = world.despawn(link.body); }
}

/// Runs after animation, before physics preparation. Reset/library changes clean
/// up all transient physics entities; a dead actor stays physical until reset.
pub fn prepare(world: &mut World) {
    let mut dolls = world.remove_resource::<Ragdolls>().unwrap_or_default();
    let sim = world.resource::<Sim>();
    let tick = sim.world.tick;
    let reset = tick < dolls.last_tick;
    let dt = sim.frame_step_s;
    let states: Vec<_> = sim.world.fighters.iter().map(|f| (f.action.kind, f.hp, f.hit_stop, f.height, f.vertical, f.velocity, f.facing)).collect();
    dolls.last_tick = tick;
    let mut query = world.query::<(Entity, &FighterRig)>();
    let rigs: Vec<_> = query.iter(world).map(|(e, r)| (e, r.fighter, r.bones.clone())).collect();
    let stale: Vec<_> = dolls.dolls.keys().copied().filter(|e| reset || !rigs.iter().any(|(rig, ..)| rig == e)).collect();
    for e in stale {
        if let Some(mut doll) = dolls.dolls.remove(&e) { remove_bodies(world, &mut doll); }
    }
    for (rig, fighter, bones) in rigs {
        let Some(&(kind, hp, stop, height, vertical, velocity, facing)) = states.get(fighter) else { continue };
        let down = kind == guildbound_sim::ActionKind::Knockdown || hp <= 0.0;
        let apex = !down && kind == guildbound_sim::ActionKind::None && height > 0.35 && vertical.abs() < 2.0;
        if down && dolls.dolls.get(&rig).is_some_and(|d| d.apex || d.recovering.is_some()) {
            let mut old = dolls.dolls.remove(&rig).unwrap();
            remove_bodies(world, &mut old);
        }
        if (down || apex) && stop <= 0.0 && !dolls.dolls.contains_key(&rig) {
            let doll = if apex {
                build_with(world, &bones, fighter, Vec3::new(velocity.x, vertical, velocity.z), Vec3::new(facing.x, 0.0, facing.z))
            } else { build(world, &bones, fighter) };
            if let Some(mut doll) = doll {
                doll.apex = apex;
                if apex {
                    for (i, link) in doll.links.iter().enumerate() {
                        if !(3..=6).contains(&i) { world.entity_mut(link.body).insert(RigidBody::Kinematic); }
                        if let Some(mut angular) = world.get_mut::<AngularVelocity>(link.body) { angular.0 = Vec3::ZERO; }
                    }
                }
                dolls.dolls.insert(rig, doll);
            }
        }
        if apex && let Some(doll) = dolls.dolls.get_mut(&rig) && doll.apex && doll.recovering.is_none() {
            let goal = 0.65 * (1.0 - vertical.abs() / 2.0);
            doll.apex_weight += (goal - doll.apex_weight) * (1.0 - (-dt * 18.0).exp());
            // Torso and legs follow the controlled jump. Only the arms are released to Avian.
            for (i, link) in doll.links.iter().enumerate() {
                if (3..=6).contains(&i) { continue; }
                if let Some(bone) = world_transform(world, link.bone) {
                    let (_, rotation, origin) = bone.to_scale_rotation_translation();
                    let rotation = rotation * link.rotation.inverse();
                    if let Some(mut body) = world.get_mut::<Transform>(link.body) {
                        body.translation = origin - rotation * link.origin;
                        body.rotation = rotation;
                    }
                    if let Some(mut v) = world.get_mut::<LinearVelocity>(link.body) { v.0 = Vec3::ZERO; }
                }
            }
        } else if !down && let Some(doll) = dolls.dolls.get_mut(&rig) {
            if doll.recovering.is_none() {
                remove_bodies(world, doll);
                doll.recovering = Some(0.0);
            }
        }
    }
    world.insert_resource(dolls);
}

/// Physical skin override is the last pose layer. Translate in parent space;
/// preserve bone scale, and blend quaternion rotations by the shortest arc.
pub fn apply(world: &mut World) {
    let dt = world.resource::<Sim>().frame_step_s;
    let mut dolls = world.remove_resource::<Ragdolls>().unwrap_or_default();
    let mut done = Vec::new();
    for (&rig, doll) in &mut dolls.dolls {
        if let Some(elapsed) = doll.recovering.as_mut() {
            let s = (*elapsed / 0.28).clamp(0.0, 1.0);
            let weight = 1.0 - s * s * (3.0 - 2.0 * s);
            for &(bone, old) in &doll.last_pose {
                if let Some(mut pose) = world.get_mut::<Transform>(bone) {
                    pose.translation = pose.translation.lerp(old.translation, weight);
                    pose.rotation = pose.rotation.slerp(old.rotation, weight).normalize();
                }
            }
            *elapsed += dt;
            if *elapsed >= 0.28 { done.push(rig); }
            continue;
        }
        if doll.apex {
            doll.last_pose.clear();
            for link in &doll.links[3..=6] {
                let Some(body) = world.get::<Transform>(link.body).copied() else { continue };
                let Some(parent) = world.get::<ChildOf>(link.bone).map(|p| p.parent()) else { continue };
                let Some(parent_pose) = world_transform(world, parent) else { continue };
                let (_, parent_rotation, _) = parent_pose.to_scale_rotation_translation();
                if let Some(mut pose) = world.get_mut::<Transform>(link.bone) {
                    let rotation = (parent_rotation.inverse() * body.rotation * link.rotation).normalize();
                    let angle = pose.rotation.angle_between(rotation);
                    let limit = (25f32.to_radians() / angle.max(0.001)).min(1.0);
                    pose.rotation = pose.rotation.slerp(rotation, doll.apex_weight * limit).normalize();
                    doll.last_pose.push((link.bone, *pose));
                }
            }
        } else { skin(world, doll); }
    }
    for e in done { dolls.dolls.remove(&e); }
    world.insert_resource(dolls);
}

/// Pose the skin from the bodies (parent first, so each link sees its just-written parent).
pub(crate) fn skin(world: &mut World, doll: &mut Doll) {
    let all = vec![true; doll.links.len()];
    skin_some(world, doll, &all, 1.0);
}

/// Skin only the links marked true (the rest keep their animated pose).
/// `weight` blends the animated pose (0) toward the bodies (1).
pub(crate) fn skin_some(world: &mut World, doll: &mut Doll, keep: &[bool], weight: f32) {
        let w = weight.clamp(0.0, 1.0);
        doll.last_pose.clear();
        for (i, link) in doll.links.iter().enumerate() {
            if !keep.get(i).copied().unwrap_or(true) {
                continue;
            }
            let Some(body) = world.get::<Transform>(link.body).copied() else { continue };
            let Some(parent) = world.get::<ChildOf>(link.bone).map(|p| p.parent()) else { continue };
            let Some(parent_world) = world_transform(world, parent) else { continue };
            let (_, parent_rotation, _) = parent_world.to_scale_rotation_translation();
            let origin = body.translation + body.rotation * link.origin;
            let rotation = body.rotation * link.rotation;
            if let Some(mut pose) = world.get_mut::<Transform>(link.bone) {
                let t = parent_world.affine().inverse().transform_point3(origin);
                let r = (parent_rotation.inverse() * rotation).normalize();
                pose.translation = pose.translation.lerp(t, w);
                pose.rotation = pose.rotation.slerp(r, w).normalize();
                doll.last_pose.push((link.bone, *pose));
            }
        }
}

pub fn snapshot(world: &mut World) -> serde_json::Value {
    let dolls = world.resource::<Ragdolls>();
    let rows: Vec<_> = dolls.dolls.values().map(|doll| {
        let mut finite = true;
        let mut min_y = f32::INFINITY;
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut bodies = 0;
        let mut max_joint_error = 0.0f32;
        let mut max_speed = 0.0f32;
        let mut hinges = 0;
        let mut max_hinge_axis_error = 0.0f32;
        for link in &doll.links {
            if let Some(p) = world.get::<Transform>(link.body) {
                bodies += 1;
                if let Some(v) = world.get::<LinearVelocity>(link.body) { max_speed = max_speed.max(v.length()); }
                min_y = min_y.min(p.translation.y);
                for value in p.translation.to_array().into_iter().chain(p.rotation.to_array()) {
                    finite &= value.is_finite();
                    hash = (hash ^ u64::from(value.to_bits())).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        for &entity in &doll.joints {
            if let Some(joint) = world.get::<RevoluteJoint>(entity)
                && let Some(a) = world.get::<Transform>(joint.body1)
                && let Some(b) = world.get::<Transform>(joint.body2)
                && let Some(anchor_a) = joint.local_anchor1()
                && let Some(anchor_b) = joint.local_anchor2()
                && let Some(hinge_a) = joint.local_hinge_axis1()
                && let Some(hinge_b) = joint.local_hinge_axis2()
            {
                hinges += 1;
                max_joint_error = max_joint_error.max((a.translation + a.rotation * anchor_a).distance(b.translation + b.rotation * anchor_b));
                let axis_a = a.rotation * hinge_a;
                let axis_b = b.rotation * hinge_b;
                max_hinge_axis_error = max_hinge_axis_error.max(axis_a.dot(axis_b).clamp(-1.0, 1.0).acos());
            }
            if let Some(joint) = world.get::<SphericalJoint>(entity)
                && let Some(a) = world.get::<Transform>(joint.body1)
                && let Some(b) = world.get::<Transform>(joint.body2)
                && let Some(anchor_a) = joint.local_anchor1()
                && let Some(anchor_b) = joint.local_anchor2()
            {
                let pa = a.translation + a.rotation * anchor_a;
                let pb = b.translation + b.rotation * anchor_b;
                max_joint_error = max_joint_error.max(pa.distance(pb));
            }
        }
        serde_json::json!({"fighter": doll.fighter, "apex": doll.apex, "apex_weight": doll.apex_weight, "bodies": bodies, "joints": doll.joints.len(), "hinges": hinges, "max_hinge_axis_error_rad": max_hinge_axis_error, "recovering": doll.recovering.is_some(), "finite": finite, "max_joint_error_m": max_joint_error, "max_speed_mps": max_speed, "min_center_y": min_y.is_finite().then_some(min_y), "hash": format!("{hash:016x}")})
    }).collect();
    serde_json::json!({"ragdolls": rows, "physics_delta_s": world.resource::<Time<Physics>>().delta_secs()})
}
