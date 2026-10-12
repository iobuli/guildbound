//! Possession (Studio tab, gb.possess): direct hands on every bone of the stage model, for the boss and for
//! Claude. Bones take rotation overrides (added to the clip, or absolute from the bind pose) and aims (point a
//! bone's segment at a world point); the timeline can run at a fixed frame rate so frame N is always the same
//! pose; the life layers (procedural.rs: fists, look, lean, breath) can be driven by hand. Every frame it
//! measures the body as a physical object: centre of mass from segment masses (Winter's anthropometric table),
//! its velocity, acceleration and momentum over clip time, angular momentum about it, kinetic energy, and the
//! balance margin of its ground projection inside the support polygon of the planted feet. Overrides can be
//! saved as corrections (keyed at the current clip time) or the whole pose saved for the clip generator.
use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;
use bevy_egui::egui;
use serde_json::{Value, json};

use crate::motion_math::world_transform;
use crate::procedural::{Drive, RestPose};
use crate::studio::{Correction, Corrections, Studio, StudioGizmos, StudioRig};
use crate::ui_theme::{self as theme, Kit};

/// (proximal joint, distal joint, mass fraction, centre-of-mass fraction along the segment).
/// Winter, Biomechanics and Motor Control of Human Movement, table 4.1; the head's centre sits above its joint.
const SEGMENTS: &[(&str, &str, f32, f32)] = &[
    ("B_Pelvis", "B_Spine", 0.142, 0.5),
    ("B_Spine", "B_Spine2", 0.139, 0.5),
    ("B_Spine2", "B_Neck", 0.216, 0.5),
    ("B_Neck", "B_Head", 0.081, 2.0),
    ("B_L_UpperArm", "B_L_Forearm", 0.028, 0.436),
    ("B_R_UpperArm", "B_R_Forearm", 0.028, 0.436),
    ("B_L_Forearm", "B_L_Forearm.001", 0.016, 0.43),
    ("B_R_Forearm", "B_R_Hand", 0.016, 0.43),
    ("B_L_Forearm.001", "B_L_Finger2", 0.006, 0.5),
    ("B_R_Hand", "B_R_Finger2", 0.006, 0.5),
    ("B_L_Thigh", "B_L_Calf", 0.1, 0.433),
    ("B_R_Thigh", "B_R_Calf", 0.1, 0.433),
    ("B_L_Calf", "B_L_Foot", 0.0465, 0.433),
    ("B_R_Calf", "B_R_Foot", 0.0465, 0.433),
    ("B_L_Foot", "B_L_Toe0", 0.0145, 0.5),
    ("B_R_Foot", "B_R_Toe0", 0.0145, 0.5),
];

#[derive(Clone, Copy, PartialEq)]
pub struct BoneOverride {
    /// Euler XYZ, degrees.
    pub euler: Vec3,
    /// From the bind pose instead of on top of the clip.
    pub absolute: bool,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum LookMode {
    #[default]
    Off,
    Camera,
    /// Overgrowth's idle gaze: a new point in front every 0.8-2 s.
    Wander,
    Point(Vec3),
}

#[derive(Default, Clone)]
pub struct Readout {
    pub valid: bool,
    pub com: Vec3,
    pub vel: Vec3,
    pub acc: Vec3,
    pub ang: Vec3,
    pub ke: f32,
    pub support: Vec<Vec2>,
    /// Signed distance (m) of the centre of mass inside the support polygon; None when nothing is down.
    pub margin: Option<f32>,
    /// The same for the extrapolated centre of mass (position + velocity / sqrt(g/h), Hof 2005): a moving
    /// body is balanced when this point is over its support, wherever the centre of mass itself is.
    pub dynamic_margin: Option<f32>,
}

#[derive(Resource)]
pub struct Possession {
    pub enabled: bool,
    pub fixed: bool,
    pub fps: f32,
    pub pending_steps: i32,
    clock: f32,
    pub overrides: BTreeMap<String, BoneOverride>,
    pub aims: BTreeMap<String, Vec3>,
    pub grip: Option<[f32; 2]>,
    /// With no hand-set grip: fists in fighting clips, open hands for magic, relaxed otherwise.
    pub auto_hands: bool,
    pub look: LookMode,
    pub lean: bool,
    pub breath: bool,
    pub mass_kg: f32,
    pub readout: Readout,
    /// (clip, clip time, com, velocity, segment points) of the last distinct sample.
    last: Option<(String, f32, Vec3, Vec3, Vec<Vec3>)>,
    /// Speed and momentum over the last samples, for the graph.
    pub history: VecDeque<(f32, f32)>,
    bone_filter: String,
    pub status: String,
}

impl Default for Possession {
    fn default() -> Self {
        Self {
            enabled: true, fixed: true, fps: 30.0, pending_steps: 0, clock: 0.0, overrides: BTreeMap::new(), aims: BTreeMap::new(), grip: None, auto_hands: true,
            look: LookMode::Off, lean: false, breath: false, mass_kg: 75.0, readout: Readout::default(), last: None, history: VecDeque::new(),
            bone_filter: String::new(), status: String::new(),
        }
    }
}

impl Possession {
    /// Advance the stage timeline: real time when free, whole frames of 1/fps when fixed (so a frame is a frame).
    pub fn advance(&mut self, time: &mut f32, playing: bool, dt: f32, speed: f32, duration: f32) {
        let fps = self.fps.clamp(1.0, 240.0);
        if self.pending_steps != 0 {
            *time = ((*time * fps).round() + self.pending_steps as f32) / fps;
            self.pending_steps = 0;
        } else if playing {
            if self.fixed {
                self.clock += dt * speed;
                let whole = (self.clock * fps).floor();
                self.clock -= whole / fps;
                *time += whole / fps;
            } else {
                *time += dt * speed;
            }
        }
        *time = time.rem_euclid(duration.max(0.01));
        if self.fixed && !playing {
            *time = (*time * fps).round() / fps;
        }
    }

    pub fn frame(&self, time: f32) -> i64 {
        (time * self.fps).round() as i64
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Possession>().add_systems(Update, draw);
}

fn bone_map(world: &World, bones: &[Entity]) -> BTreeMap<String, Entity> {
    bones.iter().filter_map(|&e| world.get::<Name>(e).map(|n| (n.as_str().to_owned(), e))).collect()
}

fn depth(world: &World, mut e: Entity) -> usize {
    let mut d = 0;
    while let Some(p) = world.get::<ChildOf>(e) {
        e = p.parent();
        d += 1;
    }
    d
}

fn euler_quat(e: Vec3) -> Quat {
    Quat::from_euler(EulerRot::XYZ, e.x.to_radians(), e.y.to_radians(), e.z.to_radians())
}

fn stage(world: &World) -> Option<(Entity, Vec<Entity>)> {
    let e = world.resource::<Studio>().entity?;
    Some((e, world.get::<StudioRig>(e)?.bones.clone()))
}

/// Overrides, aims and the life layers on the stage model (after corrections, before the authority rules).
pub fn apply(world: &mut World) {
    let p = world.resource::<Possession>();
    if !p.enabled || !world.resource::<Studio>().open {
        return;
    }
    let (overrides, aims, look, lean, breath) = (p.overrides.clone(), p.aims.clone(), p.look, p.lean, p.breath);
    let grip = p.grip.or_else(|| p.auto_hands.then(|| clip_grip(world.resource::<Studio>().clip.as_deref().unwrap_or(""))));
    let r = p.readout.clone();
    let dt = 1.0 / p.fps.clamp(1.0, 240.0);
    let Some((root, bones)) = stage(world) else { return };
    let map = bone_map(world, &bones);
    let rest = world.get::<RestPose>(root).map(|r| r.0.clone()).unwrap_or_default();
    for (name, o) in &overrides {
        let Some(&e) = map.get(name) else { continue };
        let base = if o.absolute { rest.get(&e).copied() } else { world.get::<Transform>(e).map(|t| t.rotation) };
        if let (Some(base), Some(mut t)) = (base, world.get_mut::<Transform>(e)) {
            t.rotation = (base * euler_quat(o.euler)).normalize();
        }
    }
    let mut ordered: Vec<(usize, Entity, Vec3)> = aims.iter().filter_map(|(n, p)| map.get(n).map(|&e| (depth(world, e), e, *p))).collect();
    ordered.sort_by_key(|a| a.0);
    for (_, e, target) in ordered {
        let child = bones.iter().copied().find(|&c| world.get::<ChildOf>(c).is_some_and(|p| p.parent() == e));
        let (Some(child), Some(g)) = (child, world_transform(world, e)) else { continue };
        let Some(cp) = world_transform(world, child).map(|t| t.translation()) else { continue };
        let (cur, want) = ((cp - g.translation()).normalize_or_zero(), (target - g.translation()).normalize_or_zero());
        if cur != Vec3::ZERO && want != Vec3::ZERO {
            let (_, rot, _) = g.to_scale_rotation_translation();
            crate::ik::rotate_world(world, e, Quat::from_rotation_arc(cur, want) * rot);
        }
    }
    let forward = world.get::<Transform>(root).map_or(Vec3::Z, |t| t.rotation * Vec3::Z);
    let head = map.get("B_Head").and_then(|&e| world_transform(world, e)).map_or(Vec3::Y * 1.6, |t| t.translation());
    let look = match look {
        LookMode::Off => None,
        LookMode::Point(p) => Some((p, 1.0)),
        LookMode::Camera => world.query_filtered::<&GlobalTransform, With<crate::camera::OrbitCamera>>().iter(world).next().map(|g| (g.translation(), 1.0)),
        LookMode::Wander => {
            // a new gaze point in front every 0.8-2 s, from a hash of the slot (deterministic)
            let t = world.resource::<Time>().elapsed_secs();
            let slot = (t / 1.4).floor();
            let h = |k: f32| ((slot * 12.9898 + k * 78.233).sin() * 43758.547).fract().abs();
            let side = Vec3::Y.cross(forward).normalize_or_zero();
            let dir = (forward * (0.6 + 0.4 * h(1.0)) + side * (h(2.0) * 2.0 - 1.0) * 0.8 + Vec3::Y * (h(3.0) * 0.6 - 0.3)).normalize_or_zero();
            Some((head + dir * 10.0, 1.0))
        }
    };
    let drive = Drive { grip, look, forward, chest_yaw: 0.0, breath: if breath { 1.0 } else { 0.0 }, exertion: (r.vel.length() / 6.0).min(1.0), lean: (lean && r.valid).then_some((r.vel, r.acc)), dt };
    crate::procedural::apply(world, root, &bones, &drive);
}

/// The hands a clip implies: fists to fight, open to cast, relaxed otherwise.
pub fn clip_grip(clip: &str) -> [f32; 2] {
    let c = clip.to_lowercase();
    let fight = ["attack", "strafe", "punch", "kick", "block", "guard", "hp_", "bp_", "hk_", "lk_", "bk_", "hb_", "lb_", "bb_", "hd_", "ld_", "bd_", "finisher", "parry"];
    let down = ["knockdown", "getup", "fall", "death", "dizzy"].iter().any(|f| c.contains(f));
    if c.starts_with("mage") || c.contains("magic") || c.contains("cast") || c.contains("spell") {
        [0.1, 0.1]
    } else if down {
        [0.45, 0.45]
    } else if c.starts_with("unarmed-") || fight.iter().any(|f| c.contains(f)) {
        [1.0, 1.0]
    } else {
        [0.35, 0.35]
    }
}

/// Physics readout of the final stage pose (runs after the authority rules).
pub fn measure(world: &mut World) {
    if !world.resource::<Studio>().open {
        return;
    }
    let Some((_, bones)) = stage(world) else { return };
    let map = bone_map(world, &bones);
    let pos = |n: &str| map.get(n).and_then(|&e| world_transform(world, e)).map(|t| t.translation());
    let mut points = Vec::new();
    let mut masses = Vec::new();
    for &(a, b, m, f) in SEGMENTS {
        if let (Some(pa), Some(pb)) = (pos(a), pos(b)) {
            points.push(pa + (pb - pa) * f);
            masses.push(m);
        }
    }
    let total: f32 = masses.iter().sum();
    if total < 0.5 {
        return;
    }
    let com = points.iter().zip(&masses).map(|(p, m)| *p * *m).sum::<Vec3>() / total;
    let floor = crate::studio::STAGE.y;
    let mut support = Vec::new();
    for s in ["L", "R"] {
        let (Some(ankle), Some(ball)) = (pos(&format!("B_{s}_Foot")), pos(&format!("B_{s}_Toe0"))) else { continue };
        if ball.y - floor < 0.09 || ankle.y - floor < 0.14 {
            let fwd = Vec3::new(ball.x - ankle.x, 0.0, ball.z - ankle.z);
            let side = Vec3::Y.cross(fwd.normalize_or_zero()) * 0.045;
            for p in [ankle - fwd * 0.35 + side, ankle - fwd * 0.35 - side, ball + fwd * 0.45 + side, ball + fwd * 0.45 - side] {
                support.push(Vec2::new(p.x, p.z));
            }
        }
    }
    // a body on its hands, knees, hips or back is supported by them too (getting up, knocked down)
    for (n, reach) in [("B_L_Calf", 0.12), ("B_R_Calf", 0.12), ("B_L_Forearm.001", 0.1), ("B_R_Hand", 0.1), ("B_Pelvis", 0.2), ("B_Spine2", 0.2), ("B_Head", 0.16)] {
        if let Some(p) = pos(n).filter(|p| p.y - floor < reach) {
            support.push(Vec2::new(p.x, p.z));
        }
    }
    let hull = convex_hull(support);
    let margin = (hull.len() >= 3).then(|| inside_margin(&hull, Vec2::new(com.x, com.z)));
    let (clip, time) = {
        let s = world.resource::<Studio>();
        (s.clip.clone().unwrap_or_default(), s.time)
    };
    let mut p = world.resource_mut::<Possession>();
    let mass = p.mass_kg;
    let mut r = Readout { valid: false, com, support: hull, margin, ..default() };
    let had_vel = p.readout.valid;
    if let Some((lc, lt, lcom, lvel, lpts)) = &p.last {
        let dt = time - lt;
        if *lc == clip && dt.abs() < 1e-5 {
            // same frame (fixed fps between steps): keep the last readout
            return;
        }
        if *lc == clip && dt > 0.0 && dt < 0.25 && lpts.len() == points.len() {
            r.valid = true;
            r.vel = (com - *lcom) / dt;
            // the first moving sample has no earlier velocity to differ from
            r.acc = if had_vel { (r.vel - *lvel) / dt } else { Vec3::ZERO };
            let unit = mass / total;
            for ((pt, lp), m) in points.iter().zip(lpts).zip(&masses) {
                let v = (*pt - *lp) / dt;
                r.ang += (*pt - com).cross(v - r.vel) * (m * unit);
                r.ke += 0.5 * m * unit * v.length_squared();
            }
        }
    }
    if r.valid && r.support.len() >= 3 {
        let h = (com.y - floor).max(0.2);
        let x = com + r.vel * (h / 9.81).sqrt();
        r.dynamic_margin = Some(inside_margin(&r.support, Vec2::new(x.x, x.z)));
    }
    p.last = Some((clip, time, com, r.vel, points));
    if r.valid {
        let speed = r.vel.length();
        p.history.push_back((speed, speed * mass));
        while p.history.len() > 240 {
            p.history.pop_front();
        }
    }
    p.readout = r;
}

fn convex_hull(mut pts: Vec<Vec2>) -> Vec<Vec2> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut hull: Vec<Vec2> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let iter: Box<dyn Iterator<Item = &Vec2>> = if pass == 0 { Box::new(pts.iter()) } else { Box::new(pts.iter().rev()) };
        for &p in iter {
            while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// Distance from p to the nearest hull edge, positive inside (hull counter-clockwise).
fn inside_margin(hull: &[Vec2], p: Vec2) -> f32 {
    let mut best = f32::INFINITY;
    let mut inside = true;
    for i in 0..hull.len() {
        let (a, b) = (hull[i], hull[(i + 1) % hull.len()]);
        let e = b - a;
        if e.perp_dot(p - a) < 0.0 {
            inside = false;
        }
        let t = ((p - a).dot(e) / e.length_squared().max(1e-9)).clamp(0.0, 1.0);
        best = best.min((a + e * t).distance(p));
    }
    if inside { best } else { -best }
}

fn draw(studio: Res<Studio>, p: Res<Possession>, mut gizmos: Gizmos<StudioGizmos>) {
    if !studio.open || !p.enabled {
        return;
    }
    let r = &p.readout;
    let floor = crate::studio::STAGE.y + 0.01;
    let ok = r.margin.is_some_and(|m| m > 0.0);
    let col = if r.margin.is_none() { Color::srgb(0.6, 0.7, 1.0) } else if ok { Color::srgb(0.43, 0.86, 0.5) } else { Color::srgb(1.0, 0.33, 0.28) };
    gizmos.sphere(Isometry3d::from_translation(r.com), 0.035, col);
    gizmos.line(r.com, Vec3::new(r.com.x, floor, r.com.z), col.with_alpha(0.5));
    gizmos.circle(Isometry3d::new(Vec3::new(r.com.x, floor, r.com.z), Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 0.03, col);
    if r.valid {
        gizmos.arrow(r.com, r.com + r.vel * 0.25, Color::srgb(1.0, 0.8, 0.3));
    }
    for i in 0..r.support.len() {
        let (a, b) = (r.support[i], r.support[(i + 1) % r.support.len()]);
        gizmos.line(Vec3::new(a.x, floor, a.y), Vec3::new(b.x, floor, b.y), Color::srgb(0.35, 0.86, 0.78));
    }
    for t in p.aims.values() {
        gizmos.sphere(Isometry3d::from_translation(*t), 0.025, Color::srgb(1.0, 0.4, 0.9));
    }
    if let LookMode::Point(t) = p.look {
        gizmos.sphere(Isometry3d::from_translation(t), 0.04, Color::srgb(0.6, 0.9, 1.0));
    }
}

/// Offsets become correction keys at the current clip phase (merged into the ledger).
pub fn save_corrections(p: &Possession, studio: &Studio, corr: &mut Corrections) -> usize {
    let Some(clip) = studio.clip.clone() else { return 0 };
    let t = (studio.time / studio.duration.max(0.01)).clamp(0.0, 1.0);
    let mut n = 0;
    for (bone, o) in p.overrides.iter().filter(|(_, o)| !o.absolute) {
        let key = [t, o.euler.x, o.euler.y, o.euler.z];
        match corr.list.iter_mut().find(|c| c.clip == clip && &c.bone == bone) {
            Some(c) => {
                c.keys.retain(|k| (k[0] - t).abs() > 1e-3);
                c.keys.push(key);
                c.keys.sort_by(|a, b| a[0].total_cmp(&b[0]));
            }
            None => corr.list.push(Correction { clip: clip.clone(), bone: bone.clone(), keys: vec![key] }),
        }
        n += 1;
    }
    corr.save();
    n
}

/// The stage's current local pose as a named key pose (assets/poses/NAME.json), for the clip generator.
pub fn save_pose(world: &World, name: &str) -> Option<String> {
    let (_, bones) = stage(world)?;
    let s = world.resource::<Studio>();
    let pose: serde_json::Map<String, Value> = bones
        .iter()
        .filter_map(|&e| Some((world.get::<Name>(e)?.as_str().to_owned(), world.get::<Transform>(e)?)))
        .map(|(n, t)| (n, json!({"rot": t.rotation.to_array(), "pos": t.translation.to_array()})))
        .collect();
    let dir = format!("{}/assets/poses", crate::ROOT);
    std::fs::create_dir_all(&dir).ok()?;
    let path = format!("{dir}/{name}.json");
    let body = json!({"model": s.model, "clip": s.clip, "time": s.time, "frame": world.resource::<Possession>().frame(s.time), "bones": pose});
    std::fs::write(&path, serde_json::to_string_pretty(&body).ok()?).ok()?;
    Some(path)
}

/// Claude's view (gb.possess): settings, overrides, physics, the stage numbers, and optionally every bone.
pub fn snapshot(world: &World, with_bones: bool) -> Value {
    let p = world.resource::<Possession>();
    let s = world.resource::<Studio>();
    let r = &p.readout;
    let v3 = |v: Vec3| [(v.x * 1000.0).round() / 1000.0, (v.y * 1000.0).round() / 1000.0, (v.z * 1000.0).round() / 1000.0];
    let mut out = json!({
        "enabled": p.enabled, "fixed": p.fixed, "fps": p.fps, "frame": p.frame(s.time), "frames": (s.duration * p.fps).round(),
        "model": s.model, "clip": s.clip, "time": s.time, "duration": s.duration,
        "overrides": p.overrides.iter().map(|(b, o)| (b.clone(), json!({"euler_deg": o.euler.to_array(), "absolute": o.absolute}))).collect::<serde_json::Map<_, _>>(),
        "aims": p.aims.iter().map(|(b, t)| (b.clone(), json!(t.to_array()))).collect::<serde_json::Map<_, _>>(),
        "grip": p.grip, "look": match p.look { LookMode::Off => json!("off"), LookMode::Camera => json!("camera"), LookMode::Wander => json!("wander"), LookMode::Point(t) => json!(t.to_array()) },
        "lean": p.lean, "breath": p.breath,
        "physics": {"mass_kg": p.mass_kg, "valid": r.valid, "com": v3(r.com), "com_height_m": r.com.y - crate::studio::STAGE.y, "velocity": v3(r.vel), "speed_mps": r.vel.length(),
                    "momentum_kgmps": r.vel.length() * p.mass_kg, "accel": v3(r.acc), "angular_momentum": v3(r.ang), "kinetic_j": r.ke,
                    "balance_margin_m": r.margin, "dynamic_margin_m": r.dynamic_margin, "feet_down": r.support.len() >= 3, "accel_g": r.acc.length() / 9.81},
        "numbers": s.numbers,
    });
    if with_bones && let Some((_, bones)) = stage(world) {
        let rows: Vec<Value> = bones
            .iter()
            .filter_map(|&e| {
                let name = world.get::<Name>(e)?.as_str().to_owned();
                let parent = world.get::<ChildOf>(e).and_then(|c| world.get::<Name>(c.parent())).map(|n| n.as_str().to_owned());
                let (_, wr, g) = world_transform(world, e)?.to_scale_rotation_translation();
                let (x, y, z) = world.get::<Transform>(e)?.rotation.to_euler(EulerRot::XYZ);
                Some(json!({"bone": name, "parent": parent, "world": v3(g), "world_rot": wr.to_array(), "local_euler_deg": [x.to_degrees().round(), y.to_degrees().round(), z.to_degrees().round()]}))
            })
            .collect();
        out["bones"] = json!(rows);
    }
    out
}

/// gb.possess commands. Returns a status line.
pub fn command(world: &mut World, p: &Value) -> String {
    let mut log = Vec::new();
    {
        let mut pos = world.resource_mut::<Possession>();
        if let Some(v) = p.get("enable").and_then(Value::as_bool) { pos.enabled = v; }
        if let Some(v) = p.get("fixed").and_then(Value::as_bool) { pos.fixed = v; }
        if let Some(v) = p.get("fps").and_then(Value::as_f64) { pos.fps = (v as f32).clamp(1.0, 240.0); }
        if let Some(v) = p.get("step").and_then(Value::as_i64) { pos.pending_steps += v as i32; }
        if let Some(v) = p.get("lean").and_then(Value::as_bool) { pos.lean = v; }
        if let Some(v) = p.get("breath").and_then(Value::as_bool) { pos.breath = v; }
        if let Some(v) = p.get("auto_hands").and_then(Value::as_bool) { pos.auto_hands = v; }
        if let Some(v) = p.get("mass_kg").and_then(Value::as_f64) { pos.mass_kg = v as f32; }
        match p.get("grip") {
            Some(Value::Array(a)) if a.len() == 2 => pos.grip = Some([a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32]),
            Some(Value::Null) => pos.grip = None,
            _ => {}
        }
        match p.get("look") {
            Some(Value::String(s)) => pos.look = match s.as_str() { "camera" => LookMode::Camera, "wander" => LookMode::Wander, _ => LookMode::Off },
            Some(Value::Array(a)) if a.len() == 3 => pos.look = LookMode::Point(Vec3::new(a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32)),
            _ => {}
        }
        match p.get("clear") {
            Some(Value::Bool(true)) => {
                pos.overrides.clear();
                pos.aims.clear();
            }
            Some(Value::Array(a)) => {
                for b in a.iter().filter_map(Value::as_str) {
                    pos.overrides.remove(b);
                    pos.aims.remove(b);
                }
            }
            _ => {}
        }
        let absolute = p.get("absolute").and_then(Value::as_bool).unwrap_or(false);
        let vec = |v: &Value| v.as_array().filter(|a| a.len() == 3).map(|a| Vec3::new(a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32));
        if let Some(m) = p.get("set").and_then(Value::as_object) {
            for (b, v) in m {
                if let Some(e) = vec(v) {
                    pos.overrides.insert(b.clone(), BoneOverride { euler: e, absolute });
                }
            }
        }
        if let Some(m) = p.get("aim").and_then(Value::as_object) {
            for (b, v) in m {
                if let Some(t) = vec(v) {
                    pos.aims.insert(b.clone(), t);
                }
            }
        }
    }
    if let Some(v) = p.get("frame").and_then(Value::as_i64) {
        let fps = world.resource::<Possession>().fps;
        let mut s = world.resource_mut::<Studio>();
        s.time = v as f32 / fps;
        s.playing = false;
    }
    if p.get("save").and_then(Value::as_str) == Some("correction") {
        world.resource_scope(|world, mut corr: Mut<Corrections>| {
            let n = save_corrections(world.resource::<Possession>(), world.resource::<Studio>(), &mut corr);
            log.push(format!("saved {n} overrides as corrections"));
        });
    }
    if let Some(name) = p.get("pose").and_then(Value::as_str) {
        log.push(save_pose(world, name).map_or_else(|| "pose not saved".to_owned(), |path| format!("pose saved to {path}")));
    }
    log.join("; ")
}

/// The Possession tab.
pub fn tab(ui: &mut egui::Ui, p: &mut Possession, studio: &mut Studio, corr: &mut Corrections, kit: Option<&Kit>, bone_names: &[String]) {
    theme::section(ui, "Possession");
    ui.horizontal(|ui| {
        ui.checkbox(&mut p.enabled, "Possess the stage model");
        ui.checkbox(&mut p.fixed, "Fixed fps");
        ui.add(egui::DragValue::new(&mut p.fps).range(1.0..=240.0).speed(1.0).suffix(" fps"));
    });
    ui.horizontal(|ui| {
        let frames = (studio.duration * p.fps).round();
        ui.label(egui::RichText::new(format!("frame {} / {frames:.0}", p.frame(studio.time))).monospace().color(theme::TEAL));
        for (label, step) in [("|<", i32::MIN), ("-1", -1), ("+1", 1), (">|", i32::MAX)] {
            if theme::button(ui, kit, label, false).clicked() {
                studio.playing = false;
                match step {
                    i32::MIN => studio.time = 0.0,
                    i32::MAX => studio.time = studio.duration - 1.0 / p.fps,
                    s => p.pending_steps += s,
                }
            }
        }
    });

    theme::section(ui, "Bone");
    if let Some(bone) = studio.selected.clone() {
        let mut o = p.overrides.get(&bone).copied().unwrap_or(BoneOverride { euler: Vec3::ZERO, absolute: false });
        let before = o;
        ui.label(egui::RichText::new(&bone).strong().color(theme::PAPER));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut o.euler.x).speed(0.5).prefix("x ").suffix(" deg"));
            ui.add(egui::DragValue::new(&mut o.euler.y).speed(0.5).prefix("y ").suffix(" deg"));
            ui.add(egui::DragValue::new(&mut o.euler.z).speed(0.5).prefix("z ").suffix(" deg"));
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut o.absolute, "From bind pose");
            if theme::button(ui, kit, "Release bone", false).clicked() {
                p.overrides.remove(&bone);
                p.aims.remove(&bone);
                o = before;
            } else if o != before {
                p.overrides.insert(bone.clone(), o);
            }
        });
    } else {
        ui.label(egui::RichText::new("Click a bone on the model, or pick one below.").color(theme::MUTED));
    }
    ui.add(egui::TextEdit::singleline(&mut p.bone_filter).hint_text("filter bones"));
    egui::ScrollArea::vertical().id_salt("possess_bones").max_height(140.0).show(ui, |ui| {
        let f = p.bone_filter.to_lowercase();
        for n in bone_names.iter().filter(|n| f.is_empty() || n.to_lowercase().contains(&f)) {
            let held = p.overrides.contains_key(n) || p.aims.contains_key(n);
            let text = egui::RichText::new(n).size(11.5).color(if held { theme::WARN } else { theme::PAPER });
            if ui.selectable_label(studio.selected.as_deref() == Some(n.as_str()), text).clicked() {
                studio.selected = Some(n.clone());
            }
        }
    });

    theme::section(ui, "Life layers (Overgrowth)");
    ui.checkbox(&mut p.auto_hands, "Auto hands (fists to fight, open to cast)");
    let mut drive_hands = p.grip.is_some();
    ui.checkbox(&mut drive_hands, "Set grip by hand");
    match (drive_hands, p.grip.as_mut()) {
        (true, Some(g)) => {
            ui.add(egui::Slider::new(&mut g[0], 0.0..=1.0).text("left grip"));
            ui.add(egui::Slider::new(&mut g[1], 0.0..=1.0).text("right grip"));
        }
        (true, None) => p.grip = Some([1.0, 1.0]),
        (false, _) => p.grip = None,
    }
    ui.horizontal(|ui| {
        ui.label("Look");
        for (label, mode) in [("off", LookMode::Off), ("camera", LookMode::Camera), ("wander", LookMode::Wander)] {
            if ui.selectable_label(p.look == mode, label).clicked() {
                p.look = mode;
            }
        }
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut p.lean, "Lean into motion");
        ui.checkbox(&mut p.breath, "Breathe");
    });

    theme::section(ui, "Physics");
    ui.add(egui::DragValue::new(&mut p.mass_kg).range(30.0..=200.0).speed(0.5).prefix("mass ").suffix(" kg"));
    let r = p.readout.clone();
    theme::stat(ui, "Centre of mass height", format!("{:.3} m", r.com.y - crate::studio::STAGE.y), theme::PAPER);
    theme::stat(ui, "Speed", if r.valid { format!("{:.2} m/s", r.vel.length()) } else { "-".into() }, theme::PAPER);
    theme::stat(ui, "Momentum", if r.valid { format!("{:.1} kg m/s", r.vel.length() * p.mass_kg) } else { "-".into() }, theme::PAPER);
    theme::stat(ui, "Acceleration", if r.valid { format!("{:.2} m/s2", r.acc.length()) } else { "-".into() }, theme::PAPER);
    theme::stat(ui, "Angular momentum", if r.valid { format!("{:.2} kg m2/s", r.ang.length()) } else { "-".into() }, theme::PAPER);
    theme::stat(ui, "Kinetic energy", if r.valid { format!("{:.1} J", r.ke) } else { "-".into() }, theme::PAPER);
    let (text, col) = match r.dynamic_margin.or(r.margin) {
        None => ("airborne / nothing down".to_owned(), theme::MUTED),
        Some(m) if m >= 0.0 => (format!("balanced, {:.1} cm inside", m * 100.0), theme::GOOD),
        Some(m) => (format!("falling, {:.1} cm outside", -m * 100.0), theme::BAD),
    };
    theme::stat(ui, "Balance (moving)", text, col);
    theme::stat(ui, "Centre of mass over support", r.margin.map_or("-".into(), |m| format!("{:.1} cm", m * 100.0)), theme::MUTED);
    if p.history.len() > 2 {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), egui::Sense::hover());
        let top = p.history.iter().map(|h| h.0).fold(0.5_f32, f32::max);
        let pts: Vec<egui::Pos2> = p.history.iter().enumerate().map(|(i, h)| egui::pos2(rect.left() + rect.width() * i as f32 / 240.0, rect.bottom() - rect.height() * h.0 / top)).collect();
        ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(1.0_f32, theme::TEAL_DIM), egui::StrokeKind::Inside);
        ui.painter().add(egui::Shape::line(pts, egui::Stroke::new(1.5_f32, theme::WARN)));
        ui.label(egui::RichText::new(format!("speed over the last {} samples (peak {top:.2} m/s)", p.history.len())).size(10.5).color(theme::MUTED));
    }

    theme::section(ui, "Keep");
    ui.horizontal(|ui| {
        if theme::button(ui, kit, "Save as correction", false).clicked() {
            let n = save_corrections(p, studio, corr);
            p.status = format!("{n} bone offsets saved as corrections at this frame");
        }
        if theme::button(ui, kit, "Release all", false).clicked() {
            p.overrides.clear();
            p.aims.clear();
        }
    });
    if !p.status.is_empty() {
        ui.label(egui::RichText::new(&p.status).size(11.0).color(theme::MUTED));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_margin_is_positive_inside_the_feet_and_negative_outside() {
        let hull = convex_hull(vec![Vec2::new(-0.1, -0.1), Vec2::new(0.1, -0.1), Vec2::new(0.1, 0.2), Vec2::new(-0.1, 0.2), Vec2::new(0.0, 0.0)]);
        assert_eq!(hull.len(), 4);
        assert!((inside_margin(&hull, Vec2::ZERO) - 0.1).abs() < 1e-5);
        assert!((inside_margin(&hull, Vec2::new(0.3, 0.0)) + 0.2).abs() < 1e-5);
    }

    #[test]
    fn fixed_fps_lands_on_whole_frames_and_steps_exactly() {
        let mut p = Possession { fps: 30.0, ..default() };
        let mut t = 0.0;
        for _ in 0..7 {
            p.advance(&mut t, true, 1.0 / 144.0, 1.0, 2.0);
        }
        // 7 render frames at 144 Hz = 0.0486 s: one whole 30 fps frame has passed
        assert!((t - 1.0 / 30.0).abs() < 1e-6, "{t}");
        p.pending_steps = 5;
        p.advance(&mut t, false, 0.0, 1.0, 2.0);
        assert_eq!(p.frame(t), 6);
        p.pending_steps = -10;
        p.advance(&mut t, false, 0.0, 1.0, 2.0);
        assert_eq!(p.frame(t), 56, "steps wrap around the clip");
    }
}
