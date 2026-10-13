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
use crate::ui_theme as theme;

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
    /// Blend sequence: clips played back to back, each crossfading into the next over `seq_fade` seconds.
    pub sequence: Vec<String>,
    pub seq_fade: f32,
    pub seq_time: f32,
    /// (clip, weight) pairs playing this frame, for the readout.
    pub seq_now: Vec<(String, f32)>,
    clock: f32,
    pub overrides: BTreeMap<String, BoneOverride>,
    pub aims: BTreeMap<String, Vec3>,
    pub grip: Option<[f32; 2]>,
    /// Posing respects anatomy: a hand pose that takes the selected bone's joint (or its knee) out of range is
    /// refused and the bone stays at its last legal angle.
    pub anatomy_lock: bool,
    /// The bone being posed and the direction of the last drag step per axis (for the anatomical lock).
    pub last_good: Option<(String, Vec3)>,
    /// A copied pose (local rotations by bone name): pastes onto any model of the same skeleton.
    pub clipboard: Option<BTreeMap<String, Quat>>,
    pub copy_request: bool,
    pub paste_request: bool,
    /// Live dimensions of the stage model (m): height, shoulder width, arm, leg, torso.
    pub dims: [f32; 5],
    /// Length scales applied to the stage model: arms, legs, spine, neck/head.
    pub dims_scale: [f32; 4],
    /// With no hand-set grip: fists in fighting clips, open hands for magic, relaxed otherwise.
    pub auto_hands: bool,
    pub look: LookMode,
    pub lean: bool,
    pub breath: bool,
    pub mass_kg: f32,
    pub readout: Readout,
    /// (clip, clip time, com, velocity, segment points) of the last distinct sample.
    last: Option<(Entity, String, f32, Vec3, Vec3, Vec<Vec3>)>,
    /// Speed and momentum over the last samples, for the graph.
    pub history: VecDeque<(f32, f32)>,
    bone_filter: String,
    pub status: String,
    pub motion: Option<crate::generated_motion::Motion>,
    pub plant_anchors: [Option<Vec3>; 2],
    pub reference_corrector: bool,
}

impl Default for Possession {
    fn default() -> Self {
        Self {
            enabled: true, fixed: true, fps: 30.0, pending_steps: 0, sequence: Vec::new(), seq_fade: 0.25, seq_time: 0.0, seq_now: Vec::new(), clock: 0.0, overrides: BTreeMap::new(), aims: BTreeMap::new(), grip: None, anatomy_lock: true, last_good: None, clipboard: None, copy_request: false, paste_request: false, dims: [0.0; 5], dims_scale: [1.0; 4], auto_hands: true,
            look: LookMode::Off, lean: false, breath: false, mass_kg: 75.0, readout: Readout::default(), last: None, history: VecDeque::new(),
            bone_filter: String::new(), status: String::new(),
            motion: None, plant_anchors: [None; 2],
            // a Studio tool: on gameplay fighters it replaced the locomotion blend with one dominant clip
            reference_corrector: false,
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
        if self.fixed && !playing {
            *time = (*time * fps).round() / fps;
        }
        *time = if self.motion.as_ref().is_some_and(|m| !m.looping) {
            time.clamp(0.0, duration)
        } else {
            time.rem_euclid(duration.max(0.01))
        };
    }

    pub fn frame(&self, time: f32) -> i64 {
        (time * self.fps).round() as i64
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Possession>().insert_resource(PhysicalBody { weight: 1.0, assist: 0.5, motor: 2.5, physical_regions: [true; 3], damping: 1.0, hip_gain: 0.6, balance_pos: 0.8, balance_vel: 0.25, assist_kp: 300.0, assist_kd: 35.0, ..default() })
        .init_resource::<crate::procedural::LifeTuning>().add_systems(Update, draw);
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
    let (aims, look, lean, breath) = (p.aims.clone(), p.look, p.lean, p.breath);
    let time = world.resource::<Studio>().time;
    let sample = p.motion.as_ref().map(|m| m.sample(time));
    let grip = p.grip.or_else(|| sample.as_ref().and_then(|s| s.grip)).or_else(|| p.auto_hands.then(|| clip_grip(world.resource::<Studio>().clip.as_deref().unwrap_or(""))));
    let Some((root, bones)) = stage(world) else { return };
    let map = bone_map(world, &bones);
    let rest = world.get::<RestPose>(root).map(|r| r.0.clone()).unwrap_or_default();
    let forward = world.get::<Transform>(root).map_or(Vec3::Z, |t| t.rotation * Vec3::Z);
    let head = map.get("B_Head").and_then(|&e| world_transform(world, e)).map_or(Vec3::Y * 1.6, |t| t.translation());
    let look = match look {
        LookMode::Off => None,
        LookMode::Point(p) => Some((p, 1.0)),
        LookMode::Camera => world.query_filtered::<&GlobalTransform, With<crate::camera::OrbitCamera>>().iter(world).next().map(|g| (g.translation(), 1.0)),
        LookMode::Wander => {
            // a new gaze point in front every 0.8-2 s, from a hash of the slot (deterministic)
            let slot = (time / 1.4).floor();
            let h = |k: f32| ((slot * 12.9898 + k * 78.233).sin() * 43758.547).fract().abs();
            let side = Vec3::Y.cross(forward).normalize_or_zero();
            let dir = (forward * (0.6 + 0.4 * h(1.0)) + side * (h(2.0) * 2.0 - 1.0) * 0.8 + Vec3::Y * (h(3.0) * 0.6 - 0.3)).normalize_or_zero();
            Some((head + dir * 10.0, 1.0))
        }
    };
    let rotation = world.get::<Transform>(root).map_or(Quat::IDENTITY, |t| t.rotation);
    let drive = Drive { grip, look, forward, chest_yaw: 0.0, breath: if breath { 1.0 } else { 0.0 },
        exertion: sample.as_ref().map_or(0.0, |s| (s.velocity.length() / 6.0).min(1.0)),
        lean: sample.as_ref().filter(|_| lean).map(|s| (rotation * s.velocity, rotation * s.acceleration)), dt: 0.0 };
    crate::procedural::sample(world, root, &bones, &drive, time);
    // paste a copied pose: every bone it names becomes an override from the bind pose
    if world.resource::<Possession>().paste_request {
        let clip = world.resource::<Possession>().clipboard.clone();
        let mut p = world.resource_mut::<Possession>();
        p.paste_request = false;
        if let Some(clip) = clip {
            for (name, q) in clip {
                if let Some(&e) = map.get(&name) {
                    let r = rest.get(&e).copied().unwrap_or(Quat::IDENTITY);
                    let (x, y, z) = (r.inverse() * q).to_euler(EulerRot::XYZ);
                    p.overrides.insert(name, BoneOverride { euler: Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees()), absolute: true });
                }
            }
        }
    }
    let overrides = world.resource::<Possession>().overrides.clone();
    // Manual control wins over automatic fingers, head look and breathing; anatomy still runs afterwards.
    for (name, o) in &overrides {
        let Some(&e) = map.get(name) else { continue };
        let base = if o.absolute { rest.get(&e).copied() } else { world.get::<Transform>(e).map(|t| t.rotation) };
        if let (Some(base), Some(mut t)) = (base, world.get_mut::<Transform>(e)) {
            t.rotation = (base * euler_quat(o.euler)).normalize();
        }
    }
    let mut ordered: Vec<_> = aims.iter().filter_map(|(n, p)| map.get(n).map(|&e| (depth(world, e), e, *p))).collect();
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
    let Some((root, bones)) = stage(world) else { return };
    let map = bone_map(world, &bones);
    // copy the shown pose; measure the model's dimensions
    if world.resource::<Possession>().copy_request {
        let copied: BTreeMap<String, Quat> = map.iter().filter_map(|(n, &e)| world.get::<Transform>(e).map(|t| (n.clone(), t.rotation))).collect();
        let mut p = world.resource_mut::<Possession>();
        p.clipboard = Some(copied);
        p.copy_request = false;
        p.status = "Pose copied: load another model and Paste".into();
    }
    {
        let at = |n: &str| map.get(n).and_then(|&e| world_transform(world, e)).map(|t| t.translation());
        let d = |a: &str, b: &str| at(a).zip(at(b)).map_or(0.0, |(p, q)| p.distance(q));
        let floor = crate::studio::STAGE.y;
        let height = map.values().filter_map(|&e| world_transform(world, e)).map(|t| t.translation().y).fold(floor, f32::max) - floor + 0.1;
        let dims = [height, d("B_L_UpperArm", "B_R_UpperArm"), d("B_L_UpperArm", "B_L_Forearm") + d("B_L_Forearm", "B_L_Forearm.001"),
            d("B_L_Thigh", "B_L_Calf") + d("B_L_Calf", "B_L_Foot"), d("B_Pelvis", "B_Neck")];
        world.resource_mut::<Possession>().dims = dims;
    }
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
        let p = world.resource::<Possession>();
        (if p.motion.is_some() { "generated motion".into() } else if !p.sequence.is_empty() { "blend sequence".into() } else { s.clip.clone().unwrap_or_default() }, s.time)
    };
    let mut p = world.resource_mut::<Possession>();
    let mass = p.mass_kg;
    let mut r = Readout { valid: false, com, support: hull, margin, ..default() };
    let had_vel = p.readout.valid;
    if let Some((lr, lc, lt, lcom, lvel, lpts)) = &p.last {
        let dt = time - lt;
        if *lr == root && *lc == clip && dt.abs() < 1e-5 && *lpts == points {
            // An unchanged frame preserves derivatives. Paused edits still refresh geometry below.
            return;
        }
        if *lr == root && *lc == clip && dt > 0.0 && dt < 0.25 && lpts.len() == points.len() {
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
    p.last = Some((root, clip, time, com, r.vel, points));
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
    let col = Color::srgb(0.6, 0.7, 1.0);
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
    // the pose under a generated motion or a blend sequence is not the selected clip at its phase
    if p.motion.is_some() || !p.sequence.is_empty() {
        return 0;
    }
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
        "reference": crate::reference_pose::snapshot(world),
        "playing": s.playing,
        "motion": p.motion.as_ref().map(|m| json!({"keys": m.keys.len(), "duration_s": m.duration, "loop": m.looping, "plants": m.plants})),
        "enabled": p.enabled, "fixed": p.fixed, "fps": p.fps, "frame": p.frame(s.time), "frames": (s.duration * p.fps).round(),
        "model": s.model, "clip": s.clip, "time": s.time, "duration": s.duration,
        "overrides": p.overrides.iter().map(|(b, o)| (b.clone(), json!({"euler_deg": o.euler.to_array(), "absolute": o.absolute}))).collect::<serde_json::Map<_, _>>(),
        "aims": p.aims.iter().map(|(b, t)| (b.clone(), json!(t.to_array()))).collect::<serde_json::Map<_, _>>(),
        "grip": p.grip, "look": match p.look { LookMode::Off => json!("off"), LookMode::Camera => json!("camera"), LookMode::Wander => json!("wander"), LookMode::Point(t) => json!(t.to_array()) },
        "lean": p.lean, "breath": p.breath, "sequence": p.sequence, "sequence_time": p.seq_time, "blend": p.seq_now.iter().map(|(c, w)| json!([c, w])).collect::<Vec<_>>(),
        "physics": {"mass_kg": p.mass_kg, "valid": r.valid, "com": v3(r.com), "com_height_m": r.com.y - crate::studio::STAGE.y, "velocity": v3(r.vel), "speed_mps": r.vel.length(),
                    "momentum_kgmps": r.vel.length() * p.mass_kg, "accel": v3(r.acc), "angular_momentum": v3(r.ang), "kinetic_j": r.ke,
                    "balance_estimate_only": true, "balance_margin_m": r.margin, "dynamic_margin_m": r.dynamic_margin, "feet_down": r.support.len() >= 3, "accel_g": r.acc.length() / 9.81},
        "numbers": s.numbers,
    });
    if let Some(pb) = world.get_resource::<PhysicalBody>() {
    out["physical"] = json!({"regions_physical": {"legs": pb.physical_regions[0], "body": pb.physical_regions[1], "arms": pb.physical_regions[2]},
        "tracking_error_deg": pb.track_err.iter().map(|(n, e)| (n.clone(), json!((e * 10.0).round() / 10.0))).collect::<serde_json::Map<_, _>>(), "on": pb.on, "built": pb.doll.is_some(), "assist": pb.assist, "motor": pb.motor, "standing": pb.standing, "pelvis_height_m": pb.pelvis_h});
    }
    if let Some((root, bones)) = stage(world) {
        out["clearance"] = crate::body_clearance::snapshot(world, root, &bones);
        out["clip_support"] = if p.motion.is_none() { crate::support::stage_snapshot(world) } else { Value::Null };
        let map = bone_map(world, &bones);
        out["foot_anchor_error_m"] = json!(["L", "R"].map(|side| {
            let i = if side == "L" { 0 } else { 1 };
            p.plant_anchors[i].and_then(|anchor| map.get(&format!("B_{side}_Foot")).and_then(|&e| world_transform(world, e)).map(|g| g.translation().distance(anchor)))
        }));
    }
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
pub fn command(world: &mut World, p: &Value) -> Result<String, String> {
    let mut log = Vec::new();
    if let Some(value) = p.get("motion") {
        let motion = if value.is_null() { None } else {
            let (_, bones) = stage(world).ok_or("load a stage model before submitting motion")?;
            let known: Vec<_> = bone_map(world, &bones).into_keys().collect();
            if known.is_empty() { return Err("stage model is still loading".into()); }
            Some(crate::generated_motion::Motion::parse(value, &known)?)
        };
        let mut pos = world.resource_mut::<Possession>();
        pos.motion = motion;
        pos.plant_anchors = [None; 2];
        pos.clock = 0.0;
        pos.pending_steps = 0;
        pos.last = None;
        pos.history.clear();
        pos.readout = Readout::default();
        drop(pos);
        let mut s = world.resource_mut::<Studio>();
        s.time = 0.0;
        s.playing = false;
        s.tab = 1;
        log.push("motion updated".into());
    }
    {
        let mut pos = world.resource_mut::<Possession>();
        if let Some(v) = p.get("enable").and_then(Value::as_bool) { pos.enabled = v; }
        if let Some(v) = p.get("reference_corrector").and_then(Value::as_bool) { pos.reference_corrector = v; }
        if let Some(v) = p.get("fixed").and_then(Value::as_bool) { pos.fixed = v; }
        if let Some(v) = p.get("fps").and_then(Value::as_f64) { pos.fps = (v as f32).clamp(1.0, 240.0); }
        if let Some(v) = p.get("step").and_then(Value::as_i64) { pos.pending_steps += v as i32; }
        if let Some(v) = p.get("lean").and_then(Value::as_bool) { pos.lean = v; }
        if let Some(a) = p.get("sequence").and_then(Value::as_array) {
            pos.sequence = a.iter().filter_map(|c| c.as_str().map(str::to_owned)).collect();
            pos.seq_time = 0.0;
        }
        if let Some(v) = p.get("fade").and_then(Value::as_f64) { pos.seq_fade = (v as f32).clamp(0.0, 1.0); }
        if let Some(v) = p.get("breath").and_then(Value::as_bool) { pos.breath = v; }
        if let Some(v) = p.get("auto_hands").and_then(Value::as_bool) { pos.auto_hands = v; }
        if let Some(v) = p.get("mass_kg").and_then(Value::as_f64) {
            pos.mass_kg = (v as f32).clamp(10.0, 300.0);
            pos.last = None;
        }
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
    if let Some(mut pb) = world.get_resource_mut::<PhysicalBody>() {
        if let Some(v) = p.get("physical").and_then(Value::as_bool) { pb.on = v; }
        if let Some(v) = p.get("assist").and_then(Value::as_f64) { pb.assist = (v as f32).clamp(0.0, 1.0); }
        if let Some(v) = p.get("motor").and_then(Value::as_f64) { pb.motor = (v as f32).clamp(0.0, 4.0); }
        for (k, i) in [("legs", 0), ("body", 1), ("arms", 2)] {
            if let Some(v) = p.get("physical_regions").and_then(|r| r.get(k)).and_then(Value::as_bool) { pb.physical_regions[i] = v; }
        }
    }
    if let Some(v) = p.get("frame").and_then(Value::as_i64) {
        let fps = world.resource::<Possession>().fps;
        let mut s = world.resource_mut::<Studio>();
        s.time = v as f32 / fps;
        s.playing = false;
    }
    if let Some(v) = p.get("play").and_then(Value::as_bool) { world.resource_mut::<Studio>().playing = v; }
    if p.get("save").and_then(Value::as_str) == Some("correction") {
        world.resource_scope(|world, mut corr: Mut<Corrections>| {
            let n = save_corrections(world.resource::<Possession>(), world.resource::<Studio>(), &mut corr);
            log.push(format!("saved {n} overrides as corrections"));
        });
    }
    if let Some(name) = p.get("pose").and_then(Value::as_str) {
        log.push(save_pose(world, name).map_or_else(|| "pose not saved".to_owned(), |path| format!("pose saved to {path}")));
    }
    Ok(log.join("; "))
}

/// The Possession tab.
pub fn tab(ui: &mut egui::Ui, p: &mut Possession, studio: &mut Studio, corr: &mut Corrections, bone_names: &[String]) {
    theme::section(ui, "Possession");
    ui.horizontal(|ui| {
        ui.checkbox(&mut p.enabled, "Possess the stage model");
        ui.checkbox(&mut p.reference_corrector, "Reference joint correction");
        ui.checkbox(&mut p.fixed, "Fixed fps");
        ui.add(egui::DragValue::new(&mut p.fps).range(1.0..=240.0).speed(1.0).suffix(" fps"));
    });
    if let Some(motion) = &p.motion {
        ui.label(egui::RichText::new(format!("Generated motion: {} keys, {:.2} s", motion.keys.len(), motion.duration)).color(theme::TEAL));
        if theme::button(ui, "Return to clip", false).clicked() {
            p.motion = None;
            studio.time = 0.0;
            p.last = None;
        }
    }
    ui.horizontal(|ui| {
        let frames = (studio.duration * p.fps).round();
        ui.label(egui::RichText::new(format!("frame {} / {frames:.0}", p.frame(studio.time))).monospace().color(theme::TEAL));
        for (label, step) in [("|<", i32::MIN), ("-1", -1), ("+1", 1), (">|", i32::MAX)] {
            if theme::button(ui, label, false).clicked() {
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
            if theme::button(ui, "Release bone", false).clicked() {
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
    ui.checkbox(&mut p.anatomy_lock, "Anatomical lock (posing stops at each joint's range)");
    ui.label(egui::RichText::new("Pose the selected bone: hold 1, 2 or 3 and drag with the middle mouse button (X, Y, Z).").size(11.0).color(theme::MUTED));
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
        ui.checkbox(&mut p.lean, "Lean into generated motion");
        ui.checkbox(&mut p.breath, "Breathe");
    });

    theme::section(ui, "Pose clipboard");
    ui.horizontal(|ui| {
        if theme::button(ui, "Copy pose", false).clicked() {
            p.copy_request = true;
        }
        if theme::button(ui, "Paste pose", p.clipboard.is_some()).clicked() && p.clipboard.is_some() {
            p.paste_request = true;
        }
        if theme::button(ui, "Clear pose", false).clicked() {
            p.overrides.clear();
            p.aims.clear();
        }
    });
    ui.label(egui::RichText::new("Copy on one model, cycle to another (same skeleton), Paste: the pose carries over as bone overrides.").size(11.0).color(theme::MUTED));

    theme::section(ui, "Dimensions");
    let d = p.dims;
    theme::stat(ui, "height", format!("{:.3} m", d[0]), theme::PAPER);
    theme::stat(ui, "shoulder width", format!("{:.3} m", d[1]), theme::PAPER);
    theme::stat(ui, "arm (shoulder to wrist)", format!("{:.3} m", d[2]), theme::PAPER);
    theme::stat(ui, "leg (hip to ankle)", format!("{:.3} m", d[3]), theme::PAPER);
    theme::stat(ui, "torso (pelvis to neck)", format!("{:.3} m", d[4]), theme::PAPER);
    for (i, label) in ["arm length x", "leg length x", "spine length x", "neck and head x"].iter().enumerate() {
        ui.add(egui::Slider::new(&mut p.dims_scale[i], 0.7..=1.3).text(*label));
    }
    if theme::button(ui, "Reset dimensions", false).clicked() {
        p.dims_scale = [1.0; 4];
    }

    theme::section(ui, "Motion estimates");
    ui.add(egui::DragValue::new(&mut p.mass_kg).range(30.0..=200.0).speed(0.5).prefix("mass ").suffix(" kg"));
    let r = p.readout.clone();
    theme::stat(ui, "Centre of mass height", format!("{:.3} m", r.com.y - crate::studio::STAGE.y), theme::PAPER);
    theme::stat(ui, "Speed", if r.valid { format!("{:.2} m/s", r.vel.length()) } else { "-".into() }, theme::PAPER);
    theme::stat(ui, "Momentum", if r.valid { format!("{:.1} kg m/s", r.vel.length() * p.mass_kg) } else { "-".into() }, theme::PAPER);
    theme::stat(ui, "Acceleration", if r.valid { format!("{:.2} m/s2", r.acc.length()) } else { "-".into() }, theme::PAPER);
    theme::stat(ui, "Angular momentum", if r.valid { format!("{:.2} kg m2/s", r.ang.length()) } else { "-".into() }, theme::PAPER);
    theme::stat(ui, "Kinetic energy", if r.valid { format!("{:.1} J", r.ke) } else { "-".into() }, theme::PAPER);
    let text = match r.dynamic_margin.or(r.margin) {
        None => "no estimated support polygon".to_owned(),
        Some(m) if m >= 0.0 => format!("{:.1} cm inside estimated support", m * 100.0),
        Some(m) => format!("{:.1} cm outside estimated support", -m * 100.0),
    };
    theme::stat(ui, "Support estimate", text, theme::MUTED);
    ui.label(egui::RichText::new("Diagnostic only; this does not establish balance or a fall.").size(10.5).color(theme::MUTED));
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
        if theme::button(ui, "Save as correction", false).clicked() {
            let n = save_corrections(p, studio, corr);
            p.status = format!("{n} bone offsets saved as corrections at this frame");
        }
        if theme::button(ui, "Release all", false).clicked() {
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

/// The physical stage body (Possession "Physical"): ragdoll capsules and joints that track the clip with joint
/// motors and must hold themselves up on their own feet. Assist (0..1) adds a pull on the pelvis toward the
/// clip; at 0 it is pure physics, so whether it stands says how physically sound the clip is.
#[derive(Resource, Default)]
pub struct PhysicalBody {
    doll: Option<crate::ragdoll::Doll>,
    /// The stage root the doll was built for.
    doll_root: Option<Entity>,
    /// How much the shown pose follows the bodies: 0 = the animation, 1 = fully physical.
    pub weight: f32,
    /// Damping ratio of the joint motors, the pelvis's own upright motor, the balance feedback on position and
    /// velocity (SIMBICON), and the assist pull's stiffness and damping.
    pub damping: f32,
    pub hip_gain: f32,
    pub balance_pos: f32,
    pub balance_vel: f32,
    pub assist_kp: f32,
    pub assist_kd: f32,
    pub on: bool,
    pub assist: f32,
    pub motor: f32,
    pub standing: bool,
    pub pelvis_h: f32,
    /// Which regions are physical (legs, body, arms); the rest follow the clip exactly as kinematic anchors.
    pub physical_regions: [bool; 3],
    /// Mean angle (deg) each physical limb lags its keyframe, by bone, for finding keys a body cannot follow.
    pub track_err: Vec<(String, f32)>,
}

fn region(bone: &str) -> usize {
    if bone.ends_with("_Thigh") || bone.ends_with("_Calf") || bone.ends_with("_Foot") {
        0
    } else if bone.contains("UpperArm") || bone.contains("Forearm") {
        2
    } else {
        1
    }
}

pub fn physical_drive(world: &mut World) {
    use avian3d::prelude::*;
    let mut pb = world.remove_resource::<PhysicalBody>().unwrap_or_default();
    let open = world.resource::<Studio>().open;
    let stage = stage(world);
    let want = pb.on && open && stage.is_some();
    if !want {
        if let Some(mut d) = pb.doll.take() {
            crate::ragdoll::remove_bodies(world, &mut d);
        }
        world.insert_resource(pb);
        return;
    }
    let (root, bones) = stage.unwrap();
    if pb.doll_root != Some(root) && let Some(mut d) = pb.doll.take() {
        crate::ragdoll::remove_bodies(world, &mut d);
    }
    if pb.doll.is_none() && !bones.is_empty() {
        pb.doll_root = Some(root);
        let fwd = world.get::<Transform>(root).map_or(Vec3::Z, |t| t.rotation * Vec3::Z);
        pb.doll = crate::ragdoll::build_with(world, &bones, usize::MAX, Vec3::ZERO, fwd);
        if pb.doll.is_none() {
            pb.on = false;
        }
    }
    if let Some(doll) = &pb.doll {
        // kinematic anchors: regions that follow the clip exactly
        let mut anchored = Vec::new();
        for link in &doll.links {
            let name = world.get::<Name>(link.bone).map(|n| n.as_str().to_owned()).unwrap_or_default();
            let physical = pb.physical_regions[region(&name)];
            let want = if physical { RigidBody::Dynamic } else { RigidBody::Kinematic };
            if world.get::<RigidBody>(link.body) != Some(&want) {
                world.entity_mut(link.body).insert(want);
            }
            if !physical && let Some(bone) = world_transform(world, link.bone) {
                let (_, br, bp) = bone.to_scale_rotation_translation();
                let rot = br * link.rotation.inverse();
                anchored.push((link.body, bp - rot * link.origin, rot));
            }
        }
        for (body, pos, rot) in anchored {
            if let Some(mut t) = world.get_mut::<Transform>(body) {
                t.translation = pos;
                t.rotation = rot;
            }
            if let Some(mut v) = world.get_mut::<LinearVelocity>(body) {
                v.0 = Vec3::ZERO;
            }
        }
        let kp = 420.0 * pb.motor.max(0.05);
        let kd = 2.0 * pb.damping * kp.sqrt();
        // balance (SIMBICON-style): the centre of mass drifting off the feet tilts the legs' targets toward the
        // drift so the base moves under it
        let body_pos = |w: &World, e: Entity| w.get::<Transform>(e).map(|t| t.translation);
        let mut m_sum = 0.0;
        let mut com = Vec3::ZERO;
        let mut com_v = Vec3::ZERO;
        for l in &doll.links {
            let m = world.get::<Mass>(l.body).map_or(1.0, |m| m.0);
            com += body_pos(world, l.body).unwrap_or_default() * m;
            com_v += world.get::<LinearVelocity>(l.body).map_or(Vec3::ZERO, |v| v.0) * m;
            m_sum += m;
        }
        com /= m_sum.max(1e-3);
        com_v /= m_sum.max(1e-3);
        let feet: Vec<Vec3> = doll.links.iter().filter(|l| world.get::<Name>(l.bone).is_some_and(|n| n.as_str().ends_with("_Foot"))).filter_map(|l| body_pos(world, l.body)).collect();
        let base = if feet.is_empty() { com } else { feet.iter().sum::<Vec3>() / feet.len() as f32 };
        let drift = Vec3::new(com.x - base.x, 0.0, com.z - base.z) * pb.balance_pos + Vec3::new(com_v.x, 0.0, com_v.z) * pb.balance_vel;
        let tilt = if drift.length() > 1e-4 { Quat::from_axis_angle(Vec3::Y.cross(drift).normalize(), drift.length().min(0.5)) } else { Quat::IDENTITY };
        let mut jobs = Vec::new();
        for (i, link) in doll.links.iter().enumerate() {
            let (Some(bone), Some(body)) = (world_transform(world, link.bone), world.get::<Transform>(link.body).copied()) else { continue };
            let (_, br, bp) = bone.to_scale_rotation_translation();
            let mut target = br * link.rotation.inverse();
            if world.get::<Name>(link.bone).is_some_and(|n| n.as_str().ends_with("_Thigh") || n.as_str().ends_with("_Calf")) {
                target = tilt * target;
            }
            let mut err = target * body.rotation.inverse();
            if err.w < 0.0 {
                err = -err;
            }
            let (axis, angle) = err.to_axis_angle();
            let w = world.get::<AngularVelocity>(link.body).map_or(Vec3::ZERO, |v| v.0);
            let parent_w = link.parent.and_then(|p| world.get::<AngularVelocity>(doll.links[p].body)).map_or(Vec3::ZERO, |v| v.0);
            let mut acc = axis * angle * kp - (w - parent_w) * kd;
            let mut lin = Vec3::ZERO;
            if link.parent.is_none() {
                // the pelvis has no motor; only the assist holds it toward the clip
                let centre = bp - target * link.origin;
                let v = world.get::<LinearVelocity>(link.body).map_or(Vec3::ZERO, |v| v.0);
                lin = ((centre - body.translation) * pb.assist_kp - v * pb.assist_kd) * pb.assist;
                // the pelvis's own motor reacts against the thighs (an internal hip torque: the floor has to carry it)
                acc = axis * angle * kp * pb.hip_gain - w * kd;
            }
            jobs.push((i, acc, lin));
        }
        // tracking error of the physical links (smoothed), the keyframes a body struggles to follow
        let mut errs = Vec::new();
        for link in &doll.links {
            let name = world.get::<Name>(link.bone).map(|n| n.as_str().to_owned()).unwrap_or_default();
            if !pb.physical_regions[region(&name)] {
                continue;
            }
            if let (Some(bone), Some(body)) = (world_transform(world, link.bone), world.get::<Transform>(link.body)) {
                let target = bone.to_scale_rotation_translation().1 * link.rotation.inverse();
                let e = target.angle_between(body.rotation).to_degrees();
                let prev = pb.track_err.iter().find(|(n, _)| *n == name).map_or(e, |p| p.1);
                errs.push((name, prev + (e - prev) * 0.2));
            }
        }
        pb.track_err = errs;
        let jobs: Vec<_> = jobs.into_iter().filter(|(i, ..)| {
            let name = world.get::<Name>(doll.links[*i].bone).map(|n| n.as_str().to_owned()).unwrap_or_default();
            pb.physical_regions[region(&name)]
        }).collect();
        let mass = |w: &World, e: Entity| w.get::<Mass>(e).map_or(1.0, |m| m.0);
        let mut q = world.query::<Forces>();
        for (i, acc, lin) in jobs {
            let link = &doll.links[i];
            let ratio = link.parent.map(|p| mass(world, link.body) / mass(world, doll.links[p].body).max(0.1));
            if let Ok(mut f) = q.get_mut(world, link.body) {
                f.apply_angular_acceleration(acc);
                if lin != Vec3::ZERO {
                    f.apply_linear_acceleration(lin);
                }
            }
            if link.parent.is_none() {
                let thighs: Vec<Entity> = doll.links.iter().filter(|l| world.get::<Name>(l.bone).is_some_and(|n| n.as_str().ends_with("_Thigh"))).map(|l| l.body).collect();
                let share = mass(world, link.body) / thighs.iter().map(|&t| mass(world, t)).sum::<f32>().max(0.1);
                for t in thighs {
                    if let Ok(mut f) = q.get_mut(world, t) {
                        f.apply_angular_acceleration(-acc * share);
                    }
                }
            }
            // equal and opposite on the parent: motors are internal, balance has to come from the feet
            if let (Some(p), Some(r)) = (link.parent, ratio)
                && let Ok(mut f) = q.get_mut(world, doll.links[p].body)
            {
                f.apply_angular_acceleration(-acc * r);
            }
        }
        if let Some(t) = world.get::<Transform>(doll.links[0].body) {
            pb.pelvis_h = t.translation.y - crate::studio::STAGE.y;
            pb.standing = pb.pelvis_h > 0.55;
        }
    }
    world.insert_resource(pb);
}

/// After physics: the stage skin follows the bodies.
pub fn physical_skin(world: &mut World) {
    let mut pb = world.remove_resource::<PhysicalBody>().unwrap_or_default();
    if let Some(doll) = pb.doll.as_mut() {
        // only physical regions take their pose from the bodies; anchored regions keep the clip
        let regions = pb.physical_regions;
        let keep: Vec<bool> = doll.links.iter().map(|l| regions[region(world.get::<Name>(l.bone).map_or("", |n| n.as_str()))]).collect();
        crate::ragdoll::skin_some(world, doll, &keep, pb.weight);
    }
    world.insert_resource(pb);
}
