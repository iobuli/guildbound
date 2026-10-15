//! Body commands shared by the Pose tab, BRP and the agent (ragdoll toggle with an inertialized exit, follow-clip
//! gain, input lock), and the powered fighter ragdoll: controller hinge bends become Avian joint-motor targets and
//! pins become spherical joints to a static anchor. Event logging waits for the shared crates/sim event_log contract.
use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::prelude::*;
use serde_json::{Value, json};

use crate::body_controller::{ControllerStore, Target};

/// Body commands from the Pose tab, BRP and the agent; consumed (and logged) once per frame by `consume`.
#[derive(Clone, Debug)]
pub enum BodyCmd {
    /// None toggles.
    Ragdoll(Option<bool>),
    Follow { on: bool, gain: f32 },
    LockInput(bool),
}

impl BodyCmd {
    fn to_json(&self) -> (&'static str, Value) {
        match self {
            BodyCmd::Ragdoll(v) => ("ragdoll", json!({"on": v})),
            BodyCmd::Follow { on, gain } => ("follow_clip", json!({"on": on, "gain": gain})),
            BodyCmd::LockInput(v) => ("lock_input", json!({"on": v})),
        }
    }
}

#[derive(Resource, Default)]
pub struct Pending(pub Vec<(String, BodyCmd)>);

/// Stage ragdoll exit: the shown pose's physical weight decays to the clip on an inertialized quintic.
#[derive(Resource, Default)]
pub struct Exit {
    blend: Option<(guildbound_sim::anim_gait::Inertializer, f32, f32)>,
}

pub fn ragdoll_on(world: &World) -> bool {
    match world.resource::<ControllerStore>().target {
        Target::Stage => world.resource::<crate::possession::PhysicalBody>().on,
        Target::Fighter(i) => world.resource::<crate::sim_bridge::Sim>().world.fighters.get(i).is_some_and(|f| f.action.kind == guildbound_sim::ActionKind::Knockdown),
    }
}

/// The projection step for body commands: apply in order, log each.
pub fn consume(world: &mut World) {
    let cmds = std::mem::take(&mut world.resource_mut::<Pending>().0);
    for (source, c) in cmds {
        let (kind, data) = c.to_json();
        let target = world.resource::<ControllerStore>().target;
        let mut data = data;
        data["target"] = json!(format!("{target:?}"));
        match c {
            BodyCmd::Ragdoll(v) => {
                let on = v.unwrap_or(!ragdoll_on(world));
                data["applied"] = json!(on);
                match target {
                    Target::Stage => {
                        if on {
                            world.resource_mut::<Exit>().blend = None;
                            let mut pb = world.resource_mut::<crate::possession::PhysicalBody>();
                            pb.on = true;
                            pb.physical_regions = [true; 3];
                            if pb.weight <= 0.0 { pb.weight = 1.0; }
                        } else {
                            let w = world.resource::<crate::possession::PhysicalBody>().weight;
                            world.resource_mut::<Exit>().blend = Some((guildbound_sim::anim_gait::Inertializer::new([w, 0.0, 0.0], [0.0; 3], [0.0; 3], 0.45), 0.0, w));
                        }
                    }
                    Target::Fighter(i) => {
                        let mut sim = world.resource_mut::<crate::sim_bridge::Sim>();
                        if i < sim.world.fighters.len() {
                            if on { sim.world.knock_for(i, 3600.0); } else { sim.world.stand(i); }
                        }
                    }
                }
            }
            BodyCmd::Follow { on, gain } => {
                let mut pb = world.resource_mut::<crate::possession::PhysicalBody>();
                if on { pb.motor = gain.clamp(0.05, 3.0); pb.follow = pb.follow.max(0.0); } else { pb.motor = 0.05; pb.follow = 0.0; }
            }
            BodyCmd::LockInput(v) => {
                world.resource_mut::<crate::agent::Agent>().lock_input = v;
            }
        }
        let _ = (source, kind, data); // the shared event log (crates/sim event_log) takes these when it lands
    }
    // inertialized exit from the stage ragdoll back to the clip
    let dt = world.resource::<Time>().delta_secs();
    let step = {
        let mut ex = world.resource_mut::<Exit>();
        let s = ex.blend.as_mut().map(|(ine, t, w0)| { *t += dt; (ine.offset(*t)[0].clamp(0.0, 1.0), *t >= 0.45, *w0) });
        if s.is_some_and(|s| s.1) { ex.blend = None; }
        s
    };
    if let Some((w, fin, w0)) = step {
        let mut pb = world.resource_mut::<crate::possession::PhysicalBody>();
        pb.weight = if fin { w0 } else { w };
        if fin { pb.on = false; }
    }
}

/// Motor torque limit (N m) from crates/sim phys_mass: hold the hanging chain level against gravity with 1.5x
/// headroom plus 20 rad/s^2 on its inertia about the joint (segment as a solid cylinder, radius 0.04 m approx).
pub fn torque_limit_for(chain_mass: f32, chain_com_dist: f32, length: f32) -> f32 {
    use guildbound_sim::phys_mass as pm;
    let props = pm::segment_props(pm::Segment { mass: chain_mass, length: length.max(0.05), radius: 0.04 }, &[]);
    pm::arm_torque_limit(chain_mass, chain_com_dist, props.inertia_joint[0][0].max(props.inertia_joint[2][2]), 20.0, 1.5, 0.0)
}
/// Measured tracking of the powered fighter ragdoll.
#[derive(Resource, Default)]
pub struct RagdollTrack {
    pub joints: Vec<(String, f32, f32, f32)>,
    pub pins: Vec<(String, f32)>,
    pub max_speed: f32,
    entry_flex: HashMap<Entity, f32>,
    pins_live: HashMap<String, (Entity, Entity, Vec3)>,
}

impl RagdollTrack {
    pub fn to_json(&self) -> Value {
        let r = |v: f32| (v * 10.0).round() / 10.0;
        json!({"motors": self.joints.iter().map(|(n, want, err, lim)| json!({"bone": n, "target_flex_deg": r(*want), "error_deg": r(*err), "max_torque_nm": r(*lim)})).collect::<Vec<_>>(),
            "pins": self.pins.iter().map(|(n, e)| json!({"bone": n, "error_m": (e * 1000.0).round() / 1000.0})).collect::<Vec<_>>(), "max_body_speed_mps": r(self.max_speed)})
    }
}

fn flex(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    (b - a).normalize_or_zero().angle_between((c - b).normalize_or_zero()).to_degrees()
}

/// Fighter ragdoll: controller bends on hinge joints become motor targets; pins become spherical joints to a static
/// anchor. Runs after ragdoll::prepare so the doll exists, before the physics step.
pub fn ragdoll_motors(world: &mut World) {
    let (enabled, target, nodes) = { let s = world.resource::<ControllerStore>(); (s.enabled, s.target, s.nodes.clone()) };
    let doll = match target {
        Target::Fighter(i) => world.resource::<crate::ragdoll::Ragdolls>().by_fighter(i).map(|d| (d.links.iter().map(|l| (l.bone, l.body, l.parent)).collect::<Vec<_>>(), d.joints.clone())),
        Target::Stage => None,
    };
    let mut track = world.remove_resource::<RagdollTrack>().unwrap_or_default();
    let Some((links, joints)) = doll.filter(|_| enabled) else {
        for (_, (anchor, joint, _)) in track.pins_live.drain() {
            if let Ok(e) = world.get_entity_mut(joint) { e.despawn(); }
            if let Ok(e) = world.get_entity_mut(anchor) { e.despawn(); }
        }
        track.entry_flex.clear();
        track.joints.clear();
        track.pins.clear();
        world.insert_resource(track);
        return;
    };
    let name = |w: &World, e: Entity| w.get::<Name>(e).map(|n| n.as_str().to_owned()).unwrap_or_default();
    let pos = |w: &World, e: Entity| w.get::<Transform>(e).map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let bone_world = |w: &World, e: Entity| crate::motion_math::world_transform(w, e).map(|g| g.translation()).unwrap_or(Vec3::ZERO);
    track.joints.clear();
    track.max_speed = links.iter().filter_map(|l| world.get::<LinearVelocity>(l.1).map(|v| v.0.length())).fold(0.0, f32::max);
    for &j in &joints {
        let Some(rj) = world.get::<RevoluteJoint>(j).cloned() else { continue };
        let Some(ci) = links.iter().position(|l| l.1 == rj.body2) else { continue };
        let Some(pi) = links[ci].2 else { continue };
        let bone = name(world, links[ci].0);
        // the segment's far end: the next link's start, else the bone's first child
        let child_end = links.iter().find(|l| l.2 == Some(ci)).map(|l| pos(world, l.1));
        let (pp, cp) = (pos(world, links[pi].1), pos(world, links[ci].1));
        let cur = flex(pp, cp, child_end.unwrap_or(cp + (cp - pp)));
        let cur = if child_end.is_some() { cur } else {
            let pr = world.get::<Rotation>(links[pi].1).map(|r| r.0).unwrap_or_default();
            let cr = world.get::<Rotation>(links[ci].1).map(|r| r.0).unwrap_or_default();
            (pr.inverse() * cr).angle_between(Quat::IDENTITY).to_degrees()
        };
        let entry = *track.entry_flex.entry(j).or_insert(cur);
        let want = nodes.get(&bone).filter(|n| n.enabled && n.euler != Vec3::ZERO).map(|n| Quat::from_euler(EulerRot::XYZ, n.euler.x.to_radians(), n.euler.y.to_radians(), n.euler.z.to_radians()).angle_between(Quat::IDENTITY).to_degrees());
        // the chain hanging below this joint: this segment and every descendant link
        let mut chain = vec![ci];
        let mut k = 0;
        while k < chain.len() { let c = chain[k]; chain.extend(links.iter().enumerate().filter(|(_, l)| l.2 == Some(c)).map(|(i, _)| i)); k += 1; }
        let m = |i: usize| world.get::<Mass>(links[i].1).map_or(1.0, |m| m.0);
        let total: f32 = chain.iter().map(|&i| m(i)).sum();
        let com = chain.iter().map(|&i| pos(world, links[i].1) * m(i)).sum::<Vec3>() / total.max(1e-3);
        let limit = torque_limit_for(total, com.distance(cp), child_end.map_or(0.25, |e| e.distance(cp)));
        let mut joint = rj.clone();
        match want {
            Some(w) => {
                joint.motor = AngularMotor::new(MotorModel::SpringDamper { frequency: 6.0, damping_ratio: 1.0 }).with_target_position((w - entry).to_radians()).with_max_torque(limit);
                track.joints.push((bone, w, (cur - w).abs(), limit));
            }
            None => joint.motor.enabled = false,
        }
        if joint.motor != rj.motor { world.entity_mut(j).insert(joint); }
    }
    // pins: a static anchor and a spherical joint on the segment that ends at the pinned bone
    track.pins.clear();
    let pinned: Vec<(String, Vec3)> = nodes.iter().filter(|(_, n)| n.enabled && n.pin).filter_map(|(k, n)| n.anchor.map(|a| (k.clone(), a))).collect();
    let stale: Vec<String> = track.pins_live.keys().filter(|k| !pinned.iter().any(|(p, _)| p == *k)).cloned().collect();
    for k in stale {
        if let Some((anchor, joint, _)) = track.pins_live.remove(&k) {
            if let Ok(e) = world.get_entity_mut(joint) { e.despawn(); }
            if let Ok(e) = world.get_entity_mut(anchor) { e.despawn(); }
        }
    }
    for (bone, anchor) in pinned {
        let mut q = world.query::<(Entity, &Name)>();
        let Some(be) = q.iter(world).filter(|(e, n)| n.as_str() == bone && links.iter().any(|l| world.get::<ChildOf>(*e).is_some_and(|p| p.parent() == l.0))).map(|(e, _)| e).next() else { continue };
        let Some(seg) = world.get::<ChildOf>(be).map(|p| p.parent()).and_then(|p| links.iter().find(|l| l.0 == p)).map(|l| l.1) else { continue };
        if !track.pins_live.contains_key(&bone) {
            let tip = bone_world(world, be);
            let (bp, br) = world.get::<Transform>(seg).map_or((Vec3::ZERO, Quat::IDENTITY), |t| (t.translation, t.rotation));
            let local = br.inverse() * (tip - bp);
            let a = world.spawn((RigidBody::Static, Transform::from_translation(anchor), Name::new(format!("pin anchor {bone}")))).id();
            let j = world.spawn((SphericalJoint::new(a, seg).with_local_anchor2(local), JointCollisionDisabled)).id();
            track.pins_live.insert(bone.clone(), (a, j, local));
        }
        let (_, _, local) = track.pins_live[&bone];
        let (bp, br) = world.get::<Transform>(seg).map_or((Vec3::ZERO, Quat::IDENTITY), |t| (t.translation, t.rotation));
        track.pins.push((bone, (bp + br * local).distance(anchor)));
    }
    world.insert_resource(track);
}

/// Visible banner while the agent holds the body and the user's gameplay input is locked out. Bottom centre,
/// above the status strip and click-through, so it never covers the Studio title or tab bar (pass 6).
fn banner(mut contexts: bevy_egui::EguiContexts, agent: Res<crate::agent::Agent>) {
    if !agent.lock_input { return; }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    bevy_egui::egui::Area::new("agent_lock_banner".into()).anchor(bevy_egui::egui::Align2::CENTER_BOTTOM, [0.0, -44.0]).interactable(false).order(bevy_egui::egui::Order::Foreground).show(ctx, |ui| {
        bevy_egui::egui::Frame::new().fill(bevy_egui::egui::Color32::from_rgba_unmultiplied(150, 40, 30, 230)).inner_margin(10).corner_radius(6).show(ui, |ui| {
            ui.label(bevy_egui::egui::RichText::new(format!("AGENT CONTROL - your input is locked ({}). Esc or the Pose tab toggle exits.", agent.status)).size(13.0).strong().color(bevy_egui::egui::Color32::WHITE));
        });
    });
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Pending>()
        .init_resource::<Exit>()
        .init_resource::<RagdollTrack>()
        .add_systems(Update, consume)
        .add_systems(PostUpdate, ragdoll_motors.after(crate::ragdoll::prepare).before(PhysicsSystems::First))
        .add_systems(bevy_egui::EguiPrimaryContextPass, banner);
}
