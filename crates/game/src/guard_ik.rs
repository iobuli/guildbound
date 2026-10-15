//! Procedural guard: while a fighter guards or holds a parry window against an incoming strike, two-bone IK
//! puts both forearms onto the attacker's striking forearm. Reimagined from ML-Agents-Fighting-Animations
//! (no licence; idea only): there a trained agent pushed a katana to meet the attacker's blade and arm IK
//! followed the weapon. Here the intercept point is computed directly from the attacker's live pose.
//! Presentation only: the sim already decided block, parry or hit.
use std::collections::HashMap;
use bevy::prelude::*;
use guildbound_sim::{ActionKind, Phase};

use crate::{anim::FighterRig, ik, motion_math::{spring, world_transform}, sim_bridge::Sim};

/// Shoulder, elbow, wrist.
struct Arm {
    joints: [Entity; 3],
    lengths: [f32; 2],
}

struct RigArms {
    fighter: usize,
    /// Left, right.
    arms: [Arm; 2],
    weight: f32,
    velocity: f32,
    /// Last intercept point; held while the weight fades out.
    target: Vec3,
    threat: Option<usize>,
}

#[derive(Resource, Default)]
pub struct GuardIk {
    rigs: HashMap<Entity, RigArms>,
}

fn build(world: &World, fighter: usize, bones: &[Entity]) -> Option<RigArms> {
    let names: HashMap<_, _> = bones.iter().filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str(), e))).collect();
    let arm = |a: &str, b: &str, c: &str| -> Option<Arm> {
        let joints = [*names.get(a)?, *names.get(b)?, *names.get(c)?];
        let p: Vec<_> = joints.iter().map(|&j| world_transform(world, j).map(|t| t.translation())).collect::<Option<_>>()?;
        let lengths = [p[0].distance(p[1]), p[1].distance(p[2])];
        lengths.iter().all(|v| v.is_finite() && (0.1..0.6).contains(v)).then_some(Arm { joints, lengths })
    };
    let arms = [arm("B_L_UpperArm", "B_L_Forearm", "B_L_Forearm.001")?, arm("B_R_UpperArm", "B_R_Forearm", "B_R_Hand")?];
    Some(RigArms { fighter, arms, weight: 0.0, velocity: 0.0, target: Vec3::ZERO, threat: None })
}

/// The point to meet: a third of the way from the attacker's striking wrist to its elbow.
fn strike_point(world: &World, arms: &[Arm; 2], toward: Vec3) -> Option<Vec3> {
    arms.iter()
        .filter_map(|a| Some((world_transform(world, a.joints[2])?.translation(), world_transform(world, a.joints[1])?.translation())))
        .min_by(|x, y| x.0.distance_squared(toward).total_cmp(&y.0.distance_squared(toward)))
        .map(|(wrist, elbow)| wrist.lerp(elbow, 0.33))
}

pub fn apply(world: &mut World) {
    let mut state = world.remove_resource::<GuardIk>().unwrap_or_default();
    let sim = world.resource::<Sim>();
    let dt = sim.frame_step_s;
    let fighters = sim.world.fighters.clone();
    let mut query = world.query::<(Entity, &FighterRig)>();
    let rigs: Vec<_> = query.iter(world).map(|(e, r)| (e, r.fighter, r.bones.clone())).collect();
    state.rigs.retain(|e, _| rigs.iter().any(|(r, ..)| r == e));
    for (rig, fighter, bones) in &rigs {
        if !state.rigs.contains_key(rig) && let Some(arms) = build(world, *fighter, bones) {
            state.rigs.insert(*rig, arms);
        }
    }
    // attacker arms by fighter index, so a defender can read the incoming forearm
    let by_fighter: HashMap<usize, Entity> = state.rigs.iter().map(|(&e, r)| (r.fighter, e)).collect();

    let rig_keys: Vec<Entity> = state.rigs.keys().copied().collect();
    for key in rig_keys {
        let i = state.rigs[&key].fighter;
        let Some(f) = fighters.get(i) else { continue };
        let guarding = f.action.kind == ActionKind::Guard || (f.parry_left > 0.0 && f.action.free());
        if f.hp <= 0.0 || f.kit.weapon().is_some() || !(guarding || f.action.kind == ActionKind::None) {
            // knocked about, striking or casting: the clip owns the arms
            let r = state.rigs.get_mut(&key).unwrap();
            (r.weight, r.velocity, r.threat) = (0.0, 0.0, None);
            continue;
        }
        let me = Vec3::new(f.position.x, f.height + guildbound_sim::terrain::ground(f.position.x, f.position.z), f.position.z);
        let forward = Vec3::new(f.facing.x, 0.0, f.facing.z).normalize_or(Vec3::NEG_Z);
        let chest = me + Vec3::Y * 1.3;
        // the nearest live strike aimed at this fighter, still winding up or landing
        let threat = guarding.then(|| {
            fighters.iter().enumerate()
                .filter(|(j, o)| *j != i && o.hp > 0.0 && matches!(o.action.kind, ActionKind::Strike(_)) && o.action.phase() != Phase::Recovery)
                .filter(|(_, o)| o.strike_target == Some(i) || (o.position - f.position).length() < 2.2)
                .min_by(|a, b| (a.1.position - f.position).length().total_cmp(&(b.1.position - f.position).length()))
                .map(|(j, _)| j)
        }).flatten();
        let point = threat.and_then(|j| by_fighter.get(&j)).and_then(|e| strike_point(world, &state.rigs[e].arms, chest));
        let r = state.rigs.get_mut(&key).unwrap();
        let actor_dt = if f.hit_stop > 0.0 { 0.0 } else { dt };
        if let Some(p) = point {
            // meet it in front of the chest: 0.28-0.5 m out, between hip and brow height
            let mut to = p - chest;
            if to.dot(forward) < 0.1 { to += forward * (0.1 - to.dot(forward)); }
            let reach = to.length().clamp(0.28, if f.parry_left > 0.0 { 0.5 } else { 0.4 });
            let mut t = chest + to.normalize_or(forward) * reach;
            t.y = t.y.clamp(me.y + 0.8, me.y + 1.75);
            r.target = if r.weight < 0.05 { t } else { r.target.lerp(t, (actor_dt * 30.0).min(1.0)) };
        }
        r.threat = threat;
        let want = if point.is_some() { if f.parry_left > 0.0 { 1.0 } else { 0.85 } } else { 0.0 };
        let stiffness = if f.parry_left > 0.0 { 55.0 } else { 30.0 };
        (r.weight, r.velocity) = spring(r.weight, r.velocity, want, stiffness, actor_dt);
        r.weight = r.weight.clamp(0.0, 1.0);
        if r.weight < 0.001 { continue; }
        let (weight, target) = (r.weight, r.target);
        let right = forward.cross(Vec3::Y);
        // lead with the hand on the threat's side; the other forearm stacks just inside and behind it
        let side = (target - chest).dot(right);
        let lead = if side >= 0.0 { 1 } else { 0 };
        for (k, arm) in r.arms.iter().enumerate() {
            let Some(wrist) = world_transform(world, arm.joints[2]).map(|t| t.translation()) else { continue };
            let Some(shoulder) = world_transform(world, arm.joints[0]).map(|t| t.translation()) else { continue };
            let Some(elbow) = world_transform(world, arm.joints[1]).map(|t| t.translation()) else { continue };
            let goal = if k == lead { target } else { target - right * side.signum() * 0.13 - forward * 0.06 + Vec3::Y * 0.04 };
            let out = if k == 1 { right } else { -right };
            // the animated elbow is the pole; fallback keeps elbows down and out
            ik::apply(world, arm.joints, wrist.lerp(goal, weight), elbow - shoulder, Vec3::NEG_Y + out * 0.6, arm.lengths);
        }
    }
    world.insert_resource(state);
}

pub fn snapshot(world: &World) -> serde_json::Value {
    let rows: Vec<_> = world.resource::<GuardIk>().rigs.values().map(|r| serde_json::json!({
        "fighter": r.fighter, "weight": r.weight, "threat": r.threat, "target": r.target.to_array(),
    })).collect();
    serde_json::json!({ "fighters": rows })
}
