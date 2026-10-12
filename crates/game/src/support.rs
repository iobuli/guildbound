//! Contact-driven foot anchors and fixed-length support for the flat courtyard.
//! Anchors change only on PLANT. The rules remain the sole movement authority.
use std::collections::HashMap;
use bevy::prelude::*;
use crate::{anim::{AnimLibrary, FighterRig}, ik, motion_math::{spring, world_transform}, sim_bridge::Sim};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Phase { #[default] Swing, Plant, Planted, Release }

#[derive(Default)]
struct FootLock {
    phase: Phase,
    anchor: Vec3,
    rotation: Quat,
    weight: f32,
    velocity: f32,
}

impl FootLock {
    fn step(&mut self, raw: Vec3, rotation: Quat, ground_y: f32, contact: f32, dt: f32) {
        if dt <= 0.0 { return; }
        match self.phase {
            Phase::Swing if contact > 0.65 => {
                self.anchor = Vec3::new(raw.x, ground_y, raw.z);
                self.rotation = rotation;
                self.phase = Phase::Plant;
            }
            Phase::Plant | Phase::Planted if contact < 0.35 || raw.distance(self.anchor) > 0.22 => self.phase = Phase::Release,
            _ => {}
        }
        let want = if matches!(self.phase, Phase::Plant | Phase::Planted) { 1.0 } else { 0.0 };
        (self.weight, self.velocity) = spring(self.weight, self.velocity, want, if want > 0.0 { 65.0 } else { 40.0 }, dt);
        self.weight = self.weight.clamp(0.0, 1.0);
        if self.phase == Phase::Plant && self.weight > 0.98 { self.phase = Phase::Planted; }
        if self.phase == Phase::Release && self.weight < 0.005 {
            self.phase = Phase::Swing;
            self.weight = 0.0;
            self.velocity = 0.0;
        }
    }
}

struct Leg {
    joints: [Entity; 3],
    lengths: [f32; 2],
    sole_up: Vec3,
    pole: Vec3,
    lock: FootLock,
    raw: Vec3,
    solved: Vec3,
    length_error: f32,
    contact: f32,
}

struct RigSupport {
    fighter: usize,
    pelvis: Entity,
    legs: [Leg; 2],
    drop: f32,
    drop_velocity: f32,
    enabled: bool,
    last_facing: Vec3,
}

#[derive(Resource, Default)]
pub struct Support {
    rigs: HashMap<Entity, RigSupport>,
    last_tick: u64,
}

#[derive(Resource)]
pub struct SupportSettings { pub enabled: bool }
impl Default for SupportSettings { fn default() -> Self { Self { enabled: true } } }

fn build(world: &World, fighter: usize, bones: &[Entity]) -> Option<RigSupport> {
    let names: HashMap<_, _> = bones.iter().filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str(), e))).collect();
    let leg = |hip: &str, knee: &str, foot: &str| -> Option<Leg> {
        let joints = [*names.get(hip)?, *names.get(knee)?, *names.get(foot)?];
        let a = world_transform(world, joints[0])?;
        let b = world_transform(world, joints[1])?;
        let c = world_transform(world, joints[2])?;
        let lengths = [a.translation().distance(b.translation()), b.translation().distance(c.translation())];
        if !lengths.iter().all(|v| v.is_finite() && (0.1..0.7).contains(v)) { return None; }
        let (_, rotation, _) = c.to_scale_rotation_translation();
        Some(Leg { joints, lengths, sole_up: rotation.inverse() * Vec3::Y, pole: b.translation() - a.translation(), lock: FootLock::default(), raw: c.translation(), solved: c.translation(), length_error: 0.0, contact: 0.0 })
    };
    let facing = world.resource::<Sim>().world.fighters.get(fighter)?.facing;
    Some(RigSupport { fighter, pelvis: *names.get("B_Pelvis")?, legs: [leg("B_L_Thigh", "B_L_Calf", "B_L_Foot")?, leg("B_R_Thigh", "B_R_Calf", "B_R_Foot")?], drop: 0.0, drop_velocity: 0.0, enabled: false, last_facing: Vec3::new(facing.x, 0.0, facing.z) })
}

fn ground_rotation(rotation: Quat, sole_up: Vec3) -> Quat {
    let correction = Quat::from_rotation_arc((rotation * sole_up).normalize_or(Vec3::Y), Vec3::Y);
    let (axis, angle) = correction.to_axis_angle();
    (Quat::from_axis_angle(axis, angle.min(30f32.to_radians())) * rotation).normalize()
}

pub fn apply(world: &mut World) {
    use guildbound_sim::ActionKind as K;
    let mut support = world.remove_resource::<Support>().unwrap_or_default();
    let sim = world.resource::<Sim>();
    let tick = sim.world.tick;
    let dt = sim.frame_step_s;
    let fighters = sim.world.fighters.clone();
    let settings = world.resource::<SupportSettings>().enabled;
    if tick < support.last_tick { support.rigs.clear(); }
    support.last_tick = tick;
    let mut query = world.query::<(Entity, &FighterRig)>();
    let lib = world.resource::<AnimLibrary>();
    let rigs: Vec<_> = query.iter(world).map(|(e, r)| {
        let contact = r.sample_info(world, lib).and_then(|(clip, phase)| lib.foot_contact(&clip, phase));
        (e, r.fighter, r.bones.clone(), contact, lib.sole_height_m)
    }).collect();
    support.rigs.retain(|e, _| rigs.iter().any(|(rig, ..)| rig == e));
    for (rig, fighter, bones, contact, sole_height) in rigs {
        if !support.rigs.contains_key(&rig) && let Some(state) = build(world, fighter, &bones) { support.rigs.insert(rig, state); }
        let Some(state) = support.rigs.get_mut(&rig) else { continue };
        let Some(f) = fighters.get(fighter) else { continue };
        let allowed = matches!(f.action.kind, K::None | K::Guard | K::Strike(_) | K::HeavyCharge(_) | K::Charge(_) | K::Cast(_) | K::KiCharge)
            || (f.action.kind == K::GetUp && f.action.elapsed > 0.45);
        state.enabled = settings && allowed && f.hp > 0.0 && f.height <= 0.0 && f.crouch_left <= 0.0 && contact.is_some();
        if !state.enabled {
            for leg in &mut state.legs { leg.lock = FootLock::default(); }
            state.drop = 0.0;
            state.drop_velocity = 0.0;
            continue;
        }
        let actor_dt = if f.hit_stop > 0.0 { 0.0 } else { dt };
        let contact = contact.unwrap();
        let facing = Vec3::new(f.facing.x, 0.0, f.facing.z);
        if actor_dt > 0.0 {
            let angle = facing.x.atan2(facing.z) - state.last_facing.x.atan2(state.last_facing.z);
            let turn = Quat::from_rotation_y(angle);
            for leg in &mut state.legs { leg.pole = turn * leg.pole; }
            state.last_facing = facing;
        }
        let mut required_drop = 0.0f32;
        for (i, leg) in state.legs.iter_mut().enumerate() {
            let Some(pose) = world_transform(world, leg.joints[2]) else { continue };
            let (_, rotation, _) = pose.to_scale_rotation_translation();
            leg.raw = pose.translation();
            leg.contact = contact[i];
            leg.lock.step(leg.raw, ground_rotation(rotation, leg.sole_up), sole_height[i], contact[i], actor_dt);
            if leg.lock.weight > 0.5 && let Some(hip) = world_transform(world, leg.joints[0]) {
                let delta = hip.translation() - leg.lock.anchor;
                let reach = leg.lengths.iter().sum::<f32>() - 0.003;
                let horizontal = Vec2::new(delta.x, delta.z).length_squared();
                let vertical = (reach * reach - horizontal).max(0.0).sqrt();
                required_drop = required_drop.max((delta.y - vertical).max(0.0) * leg.lock.weight);
            }
        }
        (state.drop, state.drop_velocity) = spring(state.drop, state.drop_velocity, -required_drop.min(0.1), 35.0, actor_dt);
        state.drop = state.drop.clamp(-0.1, 0.0);
        // A planted foot is a reach constraint. Smooth upward recovery, but do
        // not let spring lag leave the anchor beyond an unstretched leg.
        if actor_dt > 0.0 {
            let limit = -required_drop.min(0.1);
            if state.drop > limit {
                state.drop = limit;
                state.drop_velocity = state.drop_velocity.min(0.0);
            }
        }
        if let Some(parent) = world.get::<ChildOf>(state.pelvis).map(|p| p.parent())
            && let Some(parent_pose) = world_transform(world, parent)
            && let Some(mut pelvis) = world.get_mut::<Transform>(state.pelvis)
        {
            pelvis.translation += parent_pose.affine().inverse().transform_vector3(Vec3::Y * state.drop);
        }
        for (i, leg) in state.legs.iter_mut().enumerate() {
            let Some(pose) = world_transform(world, leg.joints[2]) else { continue };
            let (_, rotation, _) = pose.to_scale_rotation_translation();
            let raw = pose.translation();
            let mut target = raw.lerp(leg.lock.anchor, leg.lock.weight);
            target.y = target.y.max(sole_height[i]);
            let wanted_rotation = rotation.slerp(leg.lock.rotation, leg.lock.weight).normalize();
            if let Some(solution) = ik::apply(world, leg.joints, target, leg.pole, facing, leg.lengths) {
                if actor_dt > 0.0 { leg.pole = solution.pole; }
                ik::rotate_world(world, leg.joints[2], wanted_rotation);
            }
            if let (Some(a), Some(b), Some(c)) = (world_transform(world, leg.joints[0]), world_transform(world, leg.joints[1]), world_transform(world, leg.joints[2])) {
                leg.solved = c.translation();
                leg.length_error = (a.translation().distance(b.translation()) - leg.lengths[0]).abs().max((b.translation().distance(c.translation()) - leg.lengths[1]).abs());
            }
        }
    }
    world.insert_resource(support);
}

pub fn snapshot(world: &World) -> serde_json::Value {
    let rows: Vec<_> = world.resource::<Support>().rigs.values().map(|s| serde_json::json!({
        "fighter": s.fighter, "enabled": s.enabled, "pelvis_drop_m": s.drop,
        "feet": s.legs.iter().map(|l| serde_json::json!({"phase": format!("{:?}", l.lock.phase).to_lowercase(), "weight": l.lock.weight, "contact": l.contact, "anchor": l.lock.anchor.to_array(), "raw": l.raw.to_array(), "solved": l.solved.to_array(), "anchor_error_m": (l.lock.phase != Phase::Swing).then_some(l.solved.distance(l.lock.anchor)), "length_error_m": l.length_error})).collect::<Vec<_>>()
    })).collect();
    serde_json::json!({"enabled": world.resource::<SupportSettings>().enabled, "fighters": rows})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_is_written_once_and_jump_release_does_not_replant_mid_blend() {
        let mut lock = FootLock::default();
        lock.step(Vec3::new(1.0, 0.1, 2.0), Quat::IDENTITY, 0.1, 1.0, 1.0 / 120.0);
        let anchor = lock.anchor;
        for _ in 0..24 { lock.step(Vec3::new(1.1, 0.1, 2.0), Quat::IDENTITY, 0.1, 1.0, 1.0 / 120.0); }
        assert_eq!(lock.phase, Phase::Planted);
        assert_eq!(lock.anchor, anchor);
        let weight = lock.weight;
        lock.step(Vec3::ZERO, Quat::IDENTITY, 0.1, 0.0, 0.0);
        assert_eq!(lock.weight, weight, "pause freezes the lock clock");
        lock.step(Vec3::new(1.3, 0.1, 2.0), Quat::IDENTITY, 0.1, 1.0, 1.0 / 120.0);
        assert_eq!(lock.phase, Phase::Release);
        assert_eq!(lock.anchor, anchor);
        for _ in 0..30 { lock.step(Vec3::new(1.3, 0.1, 2.0), Quat::IDENTITY, 0.1, 0.0, 1.0 / 120.0); }
        assert_eq!(lock.phase, Phase::Swing);
    }
}
