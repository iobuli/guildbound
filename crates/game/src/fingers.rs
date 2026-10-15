use crate::{
    combat, ranger, skeleton_ik as ik,
    tuning::{Knob as K, Tuning},
};
use bevy::prelude::*;
use std::collections::HashMap;

struct FingerJoint {
    bone: Entity,
    rest: Quat,
    axis: Vec3,
    limit: f32,
}
struct Digit {
    joints: Vec<FingerJoint>,
    tip: Entity,
    curl: f32,
}
struct Hand {
    digits: Vec<Digit>,
    forward: Vec3,
    normal: Vec3,
}
struct Scratch {
    time: f32,
    hand: usize,
}
#[derive(Resource, Default)]
struct Hands {
    hands: Vec<Hand>,
    head: Option<Entity>,
    scratch: Option<Scratch>,
    requested: bool,
    generation: u32,
}
pub fn plugin(app: &mut App) {
    app.init_resource::<Hands>()
        .add_systems(Update, input)
        .add_systems(
            PostUpdate,
            pose.in_set(Stage::Fingers)
                .after(combat::Stage::Hands)
                .before(bevy::transform::TransformSystems::Propagate),
        );
}
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Stage {
    Fingers,
}
pub(crate) fn calibrate(
    world: &mut World,
    names: &HashMap<String, Entity>,
    arms: [combat::Arm; 2],
    head: Entity,
    pelvis: Entity,
) {
    let mut hands = Vec::new();
    for i in 0..2 {
        let suffix = if i == 0 { "l" } else { "r" };
        let side = if i == 0 { "L" } else { "R" };
        let wrist = ranger::world_transform(world, arms[i].joints[2]).unwrap();
        let wrist_pos = wrist.translation();
        let (_, wrist_rotation, _) = wrist.to_scale_rotation_translation();
        let index = ranger::world_transform(world, names[&format!("index_01_{suffix}")])
            .unwrap()
            .translation();
        let pinky = ranger::world_transform(world, names[&format!("pinky_01_{suffix}")])
            .unwrap()
            .translation();
        let middle = ranger::world_transform(world, names[&format!("B_{side}_Finger2")])
            .unwrap()
            .translation();
        let forward = (middle - wrist_pos).normalize_or(Vec3::Y);
        let mut normal = (index - pinky).cross(forward).normalize_or(Vec3::Z);
        let inward = (ranger::world_transform(world, pelvis)
            .unwrap()
            .translation()
            - wrist_pos)
            .normalize_or(Vec3::NEG_Y);
        if normal.dot(Vec3::NEG_Y * 0.7 + inward * 0.3) < 0.0 {
            normal = -normal;
        }
        let mut digits = Vec::new();
        for (d, name) in ["thumb", "index", "middle", "ring", "pinky"]
            .iter()
            .enumerate()
        {
            let mut joints = Vec::new();
            for j in 1..=3 {
                let bone_name = if d == 2 && j == 1 {
                    format!("B_{side}_Finger2")
                } else {
                    format!("{name}_{j:02}_{suffix}")
                };
                let bone = names[&bone_name];
                let pose = ranger::world_transform(world, bone).unwrap();
                let next_name = if j < 3 {
                    format!("{name}_{:02}_{suffix}", j + 1)
                } else {
                    format!("{name}_04_leaf_{suffix}")
                };
                let next = ranger::world_transform(world, names[&next_name])
                    .unwrap()
                    .translation();
                let direction = (next - pose.translation()).normalize_or(forward);
                let axis = pose.to_scale_rotation_translation().1.inverse()
                    * direction.cross(normal).normalize_or(Vec3::X);
                joints.push(FingerJoint {
                    bone,
                    rest: world.get::<Transform>(bone).unwrap().rotation,
                    axis,
                    limit: if d == 0 { 0.85 } else { [1.3, 1.5, 1.0][j - 1] },
                });
            }
            digits.push(Digit {
                joints,
                tip: names[&format!("{name}_04_leaf_{suffix}")],
                curl: 0.0,
            });
        }
        hands.push(Hand {
            digits,
            forward: wrist_rotation.inverse() * forward,
            normal: wrist_rotation.inverse() * normal,
        });
    }
    world.insert_resource(Hands {
        hands,
        head: Some(head),
        ..default()
    });
    println!("[fingers] calibrated 30 phalange joints from selected model bind frames");
}
fn input(keys: Res<ButtonInput<KeyCode>>, mut hands: ResMut<Hands>) {
    hands.requested |= keys.just_pressed(KeyCode::KeyH);
}
fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}
fn pose(world: &mut World) {
    let Some(actor) = ranger::combat_actor(world) else {
        return;
    };
    let mut hands = world.remove_resource::<Hands>().unwrap();
    if hands.hands.len() != 2 {
        world.insert_resource(hands);
        return;
    }
    if actor.generation != hands.generation {
        hands.generation = actor.generation;
        hands.scratch = None;
    }
    let feel = world.resource::<Tuning>().clone();
    let dt = world.resource::<Time>().delta_secs().min(0.05);
    let occupied = world.resource::<combat::Combat>().finger_curl();
    if std::mem::take(&mut hands.requested) && !actor.rolling && actor.grounded {
        let hand = if occupied[1] < 0.1 {
            Some(1)
        } else if occupied[0] < 0.1 {
            Some(0)
        } else {
            None
        };
        if let Some(hand) = hand {
            hands.scratch = Some(Scratch { time: 0.0, hand });
        }
    }
    if let Some(scratch) = hands.scratch.as_mut() {
        scratch.time += dt * feel.get(K::ScratchRate);
    }
    let scratch = hands.scratch.as_ref().map(|s| (s.hand, s.time));
    if scratch
        .is_some_and(|(i, t)| t > 3.2 || occupied[i] > 0.1 || actor.rolling || !actor.grounded)
    {
        hands.scratch = None;
    }
    let scratch = hands.scratch.as_ref().map(|s| (s.hand, s.time));
    let head = ranger::world_transform(world, hands.head.unwrap())
        .unwrap()
        .translation();
    let frame = ranger::combat_frame(world).unwrap();
    let orientation = frame.to_scale_rotation_translation().1;
    let up = orientation * Vec3::Y;
    let forward = orientation * Vec3::Z;
    let side = up.cross(forward);
    for i in 0..2 {
        let manual = if i == 0 {
            [K::LThumb, K::LIndex, K::LMiddle, K::LRing, K::LPinky]
        } else {
            [K::RThumb, K::RIndex, K::RMiddle, K::RRing, K::RPinky]
        };
        let active = scratch.filter(|(j, _)| *j == i);
        let weight = active
            .map(|(_, t)| ease(t / 0.5) * ease((3.2 - t) / 0.5))
            .unwrap_or(0.0);
        for d in 0..5 {
            let automatic = occupied[i] * feel.get(K::FingerGrip);
            let gesture = if let Some((_, t)) = active {
                let rub = (t * std::f32::consts::TAU * 3.5 + d as f32 * 0.3).sin();
                (if d == 1 || d == 2 {
                    0.28 + 0.16 * rub
                } else {
                    0.6
                }) * feel.get(K::ScratchCurl)
                    * weight
            } else {
                0.0
            };
            let desired = feel.get(manual[d]).max(automatic).max(gesture);
            let digit = &mut hands.hands[i].digits[d];
            digit.curl += (desired - digit.curl) * (1.0 - (-22.0 * dt).exp());
            for joint in &digit.joints {
                world.get_mut::<Transform>(joint.bone).unwrap().rotation = (joint.rest
                    * Quat::from_axis_angle(joint.axis, joint.limit * digit.curl))
                .normalize();
            }
        }
        if let Some((_, time)) = active {
            let sign = if i == 0 { 1.0 } else { -1.0 };
            let outward = side * sign;
            let touch = head + up * 0.13 + outward * 0.105 + forward * 0.025;
            let direction = (-up * 0.8 - outward * 0.6).normalize();
            let arm = actor.arms[i];
            let hand = &hands.hands[i];
            let wrist_pose = ranger::world_transform(world, arm.joints[2]).unwrap();
            let wrist = wrist_pose.translation();
            let old = wrist_pose.to_scale_rotation_translation().1;
            let tip = ranger::world_transform(world, hand.digits[1].tip)
                .unwrap()
                .translation();
            let reach = tip.distance(wrist).clamp(0.04, 0.14);
            let target = touch - direction * reach
                + forward * (time * std::f32::consts::TAU * 3.5).sin() * 0.008;
            let arc = outward * 0.12 * (std::f32::consts::PI * weight).sin();
            ik::apply(
                world,
                arm.joints,
                wrist.lerp(target, weight) + arc,
                outward + forward * 0.2,
                up,
                arm.lengths,
            );
            let rotation =
                (Quat::from_rotation_arc(old * hand.forward, direction) * old).normalize();
            let normal = (rotation * hand.normal).normalize_or(outward);
            let wanted = (-outward - direction * (-outward).dot(direction)).normalize_or(forward);
            let current = (normal - direction * normal.dot(direction)).normalize_or(forward);
            let angle = direction
                .dot(current.cross(wanted))
                .atan2(current.dot(wanted));
            let rotation = Quat::from_axis_angle(direction, angle) * rotation;
            let current = ranger::world_transform(world, arm.joints[2])
                .unwrap()
                .to_scale_rotation_translation()
                .1;
            ik::rotate_world(world, arm.joints[2], current.slerp(rotation, weight));
            // Correct the whole hand by a few millimetres using the actual fingertip, preserving every finger segment.
            let tip = ranger::world_transform(world, hand.digits[1].tip)
                .unwrap()
                .translation();
            let correction = (touch - tip).clamp_length_max(0.035) * weight;
            let wrist = ranger::world_transform(world, arm.joints[2])
                .unwrap()
                .translation();
            ik::apply(
                world,
                arm.joints,
                wrist + correction,
                outward + forward * 0.2,
                up,
                arm.lengths,
            );
        }
    }
    world.insert_resource(hands);
}
