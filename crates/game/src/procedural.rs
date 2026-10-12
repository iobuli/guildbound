//! Life layers on top of clips, rebuilt from Overgrowth's character script (Wolfire Games, Apache-2.0; ideas and
//! numbers in our own code, notes in Documentation/reference/STUDY_NOTES.md). They run on fighters (driven by
//! the sim) and on the Studio stage model (driven by Possession):
//! - hands: every finger returns to its rest and curls toward the palm by a grip (0 open, 1 fist), eased, so the
//!   Quaternius hands (whose fingers no clip animates) make fists in a fight and relax out of one
//! - look: neck and head turn toward a target through a spring, half on each joint, limited to +-52 deg of yaw
//!   and +-46 deg of pitch; a target that swings behind keeps the side it was on instead of snapping across
//! - lean: the body tilts into velocity and acceleration (Possession only; fighters lean in anim.rs)
//! - breath: the chest lifts with a rhythm that quickens with speed
use std::collections::HashMap;

use bevy::prelude::*;

use crate::ik::rotate_world;
use crate::motion_math::{spring, world_transform};

const YAW_RANGE: f32 = 0.91;
const PITCH_RANGE: f32 = 0.8;
/// Phalanx curl at a full fist (proximal, middle, distal), radians.
const CURL: [f32; 3] = [1.4, 1.65, 1.2];
const THUMB_CURL: [f32; 3] = [0.35, 0.6, 0.5];

/// Local rotations and offsets of a rig's bones as the scene spawned them (the bind pose), captured when ready.
#[derive(Component, Default)]
pub struct RestPose(pub HashMap<Entity, Quat>, pub HashMap<Entity, Vec3>);

/// Bones keep their bind length: every bone but the root and the hips sits at its bind offset from its parent.
/// A source clip's joint translations stretched the Ranger's neck (boss, 2026-10-12: "head stretched"); a bone
/// that changes length is never anatomical. Runs after every pose layer that only rotates.
pub fn bone_lengths(world: &mut World) {
    let mut q = world.query::<&RestPose>();
    let jobs: Vec<(Entity, Vec3)> = q
        .iter(world)
        .flat_map(|r| r.1.iter().filter(|(e, _)| world.get::<ChildOf>(**e).is_some_and(|p| r.1.contains_key(&p.parent()))).map(|(e, p)| (*e, *p)).collect::<Vec<_>>())
        .collect();
    for (e, offset) in jobs {
        let free = world.get::<Name>(e).is_some_and(|n| {
            let n = n.as_str().to_lowercase();
            matches!(n.as_str(), "motion" | "root" | "root_m" | "hip" | "hips") || n.ends_with("pelvis")
        });
        if !free && let Some(mut t) = world.get_mut::<Transform>(e) {
            t.translation = offset;
        }
    }
}

/// Per-rig state of the life layers.
#[derive(Component, Default, Clone, Debug, PartialEq)]
pub struct Life {
    pub grip: [f32; 2],
    /// Smoothed look (yaw, pitch) and its velocity, radians.
    pub look: Vec2,
    look_vel: Vec2,
    pub breath_phase: f32,
    breath_rate: f32,
    pub tilt: Vec3,
    tilt_vel: Vec3,
}

/// What the layers should do this frame.
pub struct Drive {
    /// Target grip per hand (left, right); None leaves the fingers as animated.
    pub grip: Option<[f32; 2]>,
    /// World point to look at and how much (0..1).
    pub look: Option<(Vec3, f32)>,
    /// Body forward (horizontal) the look is measured from.
    pub forward: Vec3,
    /// Yaw already applied to the chest this frame (radians, about +Y), so the head does not turn twice.
    pub chest_yaw: f32,
    /// Chest breathing amount (0 off, 1 full) and speed ratio (0 still, 1 sprinting).
    pub breath: f32,
    pub exertion: f32,
    /// Lean into this world velocity/acceleration (deg per m/s, deg per m/s^2), pelvis-pivoted; None = off.
    pub lean: Option<(Vec3, Vec3)>,
    pub dt: f32,
}

fn names(world: &World, bones: &[Entity]) -> HashMap<String, Entity> {
    bones.iter().filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str().to_owned(), e))).collect()
}

fn pos(world: &World, e: Entity) -> Option<Vec3> {
    world_transform(world, e).map(|t| t.translation())
}

fn rot(world: &World, e: Entity) -> Option<Quat> {
    world_transform(world, e).map(|t| t.to_scale_rotation_translation().1)
}

pub fn apply(world: &mut World, root: Entity, bones: &[Entity], d: &Drive) {
    let mut life = world.entity_mut(root).take::<Life>().unwrap_or_default();
    let by_name = names(world, bones);
    let dt = d.dt.max(0.0);
    if let Some(target) = d.grip {
        let k = 1.0 - (-dt * 12.0).exp();
        for side in 0..2 {
            life.grip[side] += (target[side] - life.grip[side]) * k;
            curl_hand(world, root, &by_name, side == 0, life.grip[side]);
        }
    }
    let (want, amount) = match d.look {
        Some((target, amount)) => (look_angles(world, &by_name, target, d.forward, d.chest_yaw, life.look.x, look_limits(world, root)), amount),
        None => (Vec2::ZERO, 0.0),
    };
    let goal = want * amount.clamp(0.0, 1.0);
    (life.look.x, life.look_vel.x) = spring(life.look.x, life.look_vel.x, goal.x, 11.0, dt);
    (life.look.y, life.look_vel.y) = spring(life.look.y, life.look_vel.y, goal.y, 11.0, dt);
    if life.look.length_squared() > 1e-6 {
        turn_head(world, &by_name, d.forward, life.look);
    }
    if d.breath > 0.0 {
        // Overgrowth: the breathing rate eases between 0.9 rad/s at rest and 4.9 at full exertion
        let target_rate = 0.9 + 4.0 * d.exertion.clamp(0.0, 1.0);
        life.breath_rate += (target_rate - life.breath_rate) * (1.0 - (-dt * 0.8).exp());
        life.breath_phase = (life.breath_phase + dt * life.breath_rate).rem_euclid(std::f32::consts::TAU);
        let lift = (life.breath_phase.sin() * 0.5 + 0.5) * (0.5 + 0.5 * life.breath_rate / 4.9) * d.breath;
        breathe(world, &by_name, d.forward, lift);
    }
    if let Some((vel, acc)) = d.lean {
        // Overgrowth: 0.5 deg per m/s plus 1 deg per m/s^2, horizontal, eased (about 0.1 s)
        let target = Vec3::new(vel.x, 0.0, vel.z) * 0.5 + Vec3::new(acc.x, 0.0, acc.z);
        for i in 0..3 {
            (life.tilt[i], life.tilt_vel[i]) = spring(life.tilt[i], life.tilt_vel[i], target[i], 10.0, dt);
        }
        lean(world, &by_name, life.tilt);
    }
    world.entity_mut(root).insert(life);
}

fn look_limits(world: &World, root: Entity) -> Vec2 {
    world.get::<crate::rig_profile::RigProfile>(root).map_or(Vec2::new(YAW_RANGE, PITCH_RANGE), |p| p.look)
}

/// The stage is seekable: these layers depend on the sampled time, never render history.
/// Gameplay retains the stateful springs in `apply`; generated motion supplies its own timed targets.
pub fn sample(world: &mut World, root: Entity, bones: &[Entity], d: &Drive, time: f32) {
    let by_name = names(world, bones);
    if let Some(grip) = d.grip {
        for (side, value) in grip.into_iter().enumerate() {
            curl_hand(world, root, &by_name, side == 0, value.clamp(0.0, 1.0));
        }
    }
    if let Some((target, amount)) = d.look {
        let look = look_angles(world, &by_name, target, d.forward, d.chest_yaw, 0.0, look_limits(world, root));
        turn_head(world, &by_name, d.forward, look * amount.clamp(0.0, 1.0));
    }
    if d.breath > 0.0 {
        let rate = 0.9 + 4.0 * d.exertion.clamp(0.0, 1.0);
        let lift = ((time * rate).sin() * 0.5 + 0.5) * (0.5 + 0.5 * rate / 4.9) * d.breath;
        breathe(world, &by_name, d.forward, lift);
    }
    if let Some((vel, acc)) = d.lean {
        lean(world, &by_name, Vec3::new(vel.x, 0.0, vel.z) * 0.5 + Vec3::new(acc.x, 0.0, acc.z));
    }
}

/// Fingers back to rest, then curled toward the palm about the hand's knuckle axis.
fn curl_hand(world: &mut World, root: Entity, by_name: &HashMap<String, Entity>, left: bool, grip: f32) {
    let s = if left { "l" } else { "r" };
    let hand = by_name.get(if left { "B_L_Forearm.001" } else { "B_R_Hand" }).copied();
    let middle = by_name.get(if left { "B_L_Finger2" } else { "B_R_Finger2" }).copied();
    let get = |n: String| by_name.get(&n).copied();
    let chains: Vec<[Option<Entity>; 3]> = ["index", "middle", "ring", "pinky"]
        .iter()
        .map(|f| if *f == "middle" { [middle, get(format!("middle_02_{s}")), get(format!("middle_03_{s}"))] } else { [1, 2, 3].map(|k| get(format!("{f}_0{k}_{s}"))) })
        .collect();
    let thumb = [1, 2, 3].map(|k| get(format!("thumb_0{k}_{s}")));
    let (Some(hand), Some(Some(i1)), Some(Some(p1)), [Some(m1), Some(m2), _], [Some(t1), Some(t2), _]) = (hand, chains.first().map(|c| c[0]), chains.get(3).map(|c| c[0]), chains[1], thumb) else { return };
    if let Some(rest) = world.get::<RestPose>(root).map(|r| r.0.clone()) {
        for &e in chains.iter().flatten().chain(thumb.iter()).flatten() {
            if let (Some(q), Some(mut t)) = (rest.get(&e).copied(), world.get_mut::<Transform>(e)) {
                t.rotation = q;
            }
        }
    }
    let (Some(h), Some(i1p), Some(p1p), Some(m1p), Some(m2p), Some(t1p), Some(t2p)) = (pos(world, hand), pos(world, i1), pos(world, p1), pos(world, m1), pos(world, m2), pos(world, t1), pos(world, t2)) else { return };
    let along = (m2p - m1p).normalize_or_zero();
    let across = (i1p - p1p).normalize_or_zero();
    // the palm faces the side the thumb sits on
    let mut palm = along.cross(across).normalize_or_zero();
    if palm.dot(t2p - h) < 0.0 {
        palm = -palm;
    }
    let axis = along.cross(palm).normalize_or_zero();
    if axis == Vec3::ZERO {
        return;
    }
    for chain in &chains {
        let mut acc = Quat::IDENTITY;
        for (k, e) in chain.iter().enumerate() {
            let (Some(e), Some(r)) = (*e, chain[k].and_then(|e| rot(world, e))) else { break };
            let q = Quat::from_axis_angle(acc * axis, CURL[k] * grip);
            rotate_world(world, e, q * r);
            acc = q * acc;
        }
    }
    // the thumb folds across the front of the curled fingers
    let t_axis = (t2p - t1p).normalize_or_zero().cross(palm).normalize_or_zero();
    if t_axis != Vec3::ZERO {
        let mut acc = Quat::IDENTITY;
        for (k, e) in thumb.iter().enumerate() {
            let (Some(e), Some(r)) = (*e, thumb[k].and_then(|e| rot(world, e))) else { break };
            let q = Quat::from_axis_angle(acc * t_axis, THUMB_CURL[k] * grip);
            rotate_world(world, e, q * r);
            acc = q * acc;
        }
    }
}

/// Wanted (yaw, pitch) toward a target, measured from the body forward minus what the chest already turned.
fn look_angles(world: &World, by_name: &HashMap<String, Entity>, target: Vec3, forward: Vec3, chest_yaw: f32, current_yaw: f32, limits: Vec2) -> Vec2 {
    let Some(head) = by_name.get("B_Head").and_then(|&e| pos(world, e)) else { return Vec2::ZERO };
    let to = target - head;
    let flat = Vec3::new(to.x, 0.0, to.z);
    let f = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::Z);
    if flat.length_squared() < 1e-4 {
        return Vec2::ZERO;
    }
    let flat_n = flat.normalize();
    let mut yaw = f.cross(flat_n).y.atan2(f.dot(flat_n)) - chest_yaw;
    // behind the shoulder: keep the side the head is already on (Overgrowth's hysteresis)
    if yaw.abs() > limits.x && current_yaw.abs() > 0.5 * limits.x && yaw.signum() != current_yaw.signum() {
        yaw = current_yaw.signum() * limits.x;
    }
    let pitch = to.normalize().y.clamp(-1.0, 1.0).asin();
    Vec2::new(yaw.clamp(-limits.x, limits.x), pitch.clamp(-limits.y, limits.y))
}

/// Yaw about world up and pitch about the body's right, half at the neck and half at the head.
fn turn_head(world: &mut World, by_name: &HashMap<String, Entity>, forward: Vec3, look: Vec2) {
    let f = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::Z);
    let pitch_axis = f.cross(Vec3::Y);
    let half = Quat::from_rotation_y(look.x * 0.5) * Quat::from_axis_angle(pitch_axis, look.y * 0.5);
    for n in ["B_Neck", "B_Head"] {
        if let Some(&e) = by_name.get(n)
            && let Some(r) = rot(world, e)
        {
            rotate_world(world, e, half * r);
        }
    }
}

fn breathe(world: &mut World, by_name: &HashMap<String, Entity>, forward: Vec3, lift: f32) {
    let f = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::Z);
    // chest rises (pitches back 1.6 deg at a full breath), shoulders lift 2.5 deg
    if let Some(&e) = by_name.get("B_Spine2")
        && let Some(r) = rot(world, e)
    {
        rotate_world(world, e, Quat::from_axis_angle(f.cross(Vec3::Y), 0.028 * lift) * r);
    }
    for (n, sign) in [("B_L_Clavicle", 1.0), ("B_R_Clavicle", -1.0)] {
        if let Some(&e) = by_name.get(n)
            && let Some(r) = rot(world, e)
        {
            rotate_world(world, e, Quat::from_axis_angle(f, sign * 0.044 * lift) * r);
        }
    }
}

/// Tilt the whole body about the pelvis toward `tilt` (a horizontal vector whose length is degrees).
fn lean(world: &mut World, by_name: &HashMap<String, Entity>, tilt: Vec3) {
    let deg = tilt.length();
    if deg < 1e-3 {
        return;
    }
    let axis = Vec3::Y.cross(tilt / deg);
    if let Some(&e) = by_name.get("B_Pelvis")
        && let Some(r) = rot(world, e)
    {
        rotate_world(world, e, Quat::from_axis_angle(axis, deg.min(15.0).to_radians()) * r);
    }
}

/// Fighters: fists in a fight, open hands for magic, relaxed out of combat; eyes on the lock or strike target,
/// otherwise the player looks where the camera looks (and down the road while running), others at the player.
pub fn fighters(world: &mut World) {
    use guildbound_sim::ActionKind as K;
    let cam = world.query_filtered::<&GlobalTransform, With<crate::camera::OrbitCamera>>().iter(world).next().map(|g| (g.translation(), g.forward().as_vec3()));
    let mut q = world.query::<(Entity, &crate::anim::FighterRig)>();
    let rigs: Vec<(Entity, usize, Vec<Entity>, f32)> = q.iter(world).map(|(e, r)| (e, r.fighter, r.bones.clone(), r.chest_aim())).collect();
    let sim = world.resource::<crate::sim_bridge::Sim>();
    let dt = sim.frame_step_s;
    let t = sim.world.tick as f32 / 120.0;
    let mut jobs = Vec::new();
    for (root, i, bones, chest_yaw) in rigs {
        let Some(f) = sim.world.fighters.get(i) else { continue };
        let fight = sim.world.in_combat(i);
        let grip = match f.action.kind {
            K::Strike(_) | K::HeavyCharge(_) | K::Guard | K::GuardBreak | K::BlockStun | K::Staggered => [1.0, 1.0],
            K::Cast(_) | K::Charge(_) => [0.1, 0.1],
            K::Drink(_) => [0.35, 1.0],
            K::Knockdown | K::GetUp | K::Dizzy | K::HitStun => [0.45, 0.45],
            K::Step | K::Roll => [0.7, 0.7],
            _ if fight => [0.9, 0.9],
            _ => [0.3, 0.3],
        };
        let head_of = |j: usize| sim.world.fighters.get(j).map(|o| Vec3::new(o.position.x, o.height + 1.55, o.position.z));
        let me = Vec3::new(f.position.x, f.height + 1.55, f.position.z);
        let forward = Vec3::new(f.facing.x, 0.0, f.facing.z);
        let busy = matches!(f.action.kind, K::Knockdown | K::GetUp | K::Roll | K::Step | K::HitStun | K::Dizzy);
        let look = if busy || f.hp <= 0.0 {
            None
        } else if let Some(p) = f.lock_target.or(f.strike_target).and_then(head_of) {
            Some((p, 1.0))
        } else if i == 0 {
            // player: where the camera looks; while running, glance down the road now and then
            let speed = f.velocity.length();
            let road = speed > 0.5 && (t * 3.0).sin() + (t * 2.3).sin() > 0.0;
            if road {
                Some((me + Vec3::new(f.velocity.x, 0.0, f.velocity.z).normalize_or_zero() * 20.0, 0.8))
            } else {
                cam.map(|(_, dir)| (me + dir * 30.0, 0.8))
            }
        } else {
            head_of(0).filter(|p| (*p - me).length() < 8.0 && (*p - me).normalize_or_zero().dot(forward) > -0.2).map(|p| (p, 0.9))
        };
        let exertion = (f.velocity.length() / 6.0).clamp(0.0, 1.0).max(if fight { 0.5 } else { 0.0 });
        jobs.push((root, bones, Drive { grip: Some(grip), look, forward, chest_yaw, breath: 1.0, exertion, lean: None, dt }));
    }
    for (root, bones, d) in jobs {
        apply(world, root, &bones, &d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bone(world: &mut World, name: &str, parent: Option<Entity>, at: Vec3) -> Entity {
        let mut e = world.spawn((Name::new(name.to_owned()), Transform::from_translation(at)));
        if let Some(p) = parent {
            e.insert(ChildOf(p));
        }
        e.id()
    }

    fn drive() -> Drive {
        Drive { grip: None, look: None, forward: Vec3::Z, chest_yaw: 0.0, breath: 0.0, exertion: 0.0, lean: None, dt: 1.0 / 30.0 }
    }

    #[test]
    fn a_fist_folds_the_fingers_into_the_palm() {
        // left hand, fingers along +X, index toward +Z, the thumb sitting on the palm side (+Y)
        let mut world = World::new();
        let root = world.spawn(Transform::IDENTITY).id();
        let hand = bone(&mut world, "B_L_Forearm.001", Some(root), Vec3::ZERO);
        let mut bones = vec![hand];
        let mut tips = Vec::new();
        for (base, z) in [("index_0{}_l", 0.04), ("B_L_Finger2", 0.015), ("ring_0{}_l", -0.012), ("pinky_0{}_l", -0.035)] {
            let first = if base == "B_L_Finger2" { base.to_owned() } else { base.replace("{}", "1") };
            let a = bone(&mut world, &first, Some(hand), Vec3::new(0.11, 0.0, z));
            let second = if base == "B_L_Finger2" { "middle_02_l".to_owned() } else { base.replace("{}", "2") };
            let b = bone(&mut world, &second, Some(a), Vec3::new(0.035, 0.0, 0.0));
            let third = if base == "B_L_Finger2" { "middle_03_l".to_owned() } else { base.replace("{}", "3") };
            let c = bone(&mut world, &third, Some(b), Vec3::new(0.03, 0.0, 0.0));
            bones.extend([a, b, c]);
            tips.push(c);
        }
        let t1 = bone(&mut world, "thumb_01_l", Some(hand), Vec3::new(0.034, 0.027, 0.041));
        let t2 = bone(&mut world, "thumb_02_l", Some(t1), Vec3::new(0.03, 0.02, 0.015));
        let t3 = bone(&mut world, "thumb_03_l", Some(t2), Vec3::new(0.028, 0.017, 0.014));
        bones.extend([t1, t2, t3]);
        let rest = RestPose(bones.iter().map(|&e| (e, Quat::IDENTITY)).collect(), HashMap::new());
        world.entity_mut(root).insert((rest, Life { grip: [1.0, 1.0], ..default() }));
        let open: Vec<Vec3> = tips.iter().map(|&t| pos(&world, t).unwrap()).collect();
        apply(&mut world, root, &bones, &Drive { grip: Some([1.0, 1.0]), ..drive() });
        for (&t, o) in tips.iter().zip(open) {
            let p = pos(&world, t).unwrap();
            assert!(p.y > 0.03, "fingertip did not curl to the palm side: {p}");
            assert!(p.x < o.x - 0.05, "fingertip did not fold back: {p} from {o}");
        }
        // releasing the grip returns to the bind pose (rest), not to a curled pose
        apply(&mut world, root, &bones, &Drive { grip: Some([0.0, 0.0]), dt: 10.0, ..drive() });
        assert!(pos(&world, tips[0]).unwrap().y.abs() < 1e-3);
    }

    #[test]
    fn the_head_turns_toward_a_target_within_its_range() {
        let mut world = World::new();
        let root = world.spawn(Transform::IDENTITY).id();
        let chest = bone(&mut world, "B_Spine2", Some(root), Vec3::new(0.0, 1.4, 0.0));
        let neck = bone(&mut world, "B_Neck", Some(chest), Vec3::new(0.0, 0.12, 0.0));
        let head = bone(&mut world, "B_Head", Some(neck), Vec3::new(0.0, 0.1, 0.0));
        let bones = vec![chest, neck, head];
        let mut yaw = 0.0;
        for _ in 0..120 {
            // the clip resets the joints each frame
            for e in [neck, head] {
                world.get_mut::<Transform>(e).unwrap().rotation = Quat::IDENTITY;
            }
            apply(&mut world, root, &bones, &Drive { look: Some((Vec3::new(5.0, 1.62, 0.5), 1.0)), ..drive() });
            let f = rot(&world, head).unwrap() * Vec3::Z;
            yaw = f.x.atan2(f.z);
        }
        // the target sits about 84 deg to the side (+X); the head stops at its 52 deg limit, turned that way
        assert!((yaw - YAW_RANGE).abs() < 0.02, "yaw {yaw}");
        let neck_share = (rot(&world, neck).unwrap() * Vec3::Z).x.atan2((rot(&world, neck).unwrap() * Vec3::Z).z);
        assert!((neck_share - YAW_RANGE * 0.5).abs() < 0.02, "neck carries half: {neck_share}");
    }
}
