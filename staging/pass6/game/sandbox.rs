//! Locked sandbox: the game boots into the Studio stage with the physical body (ragdoll) already on and the
//! player's input locked. Uses the existing pieces only: `studio::open_at_startup` (paused Studio + mannequin),
//! `possession::PhysicalBody` (the stage ragdoll and its motors), `agent::Agent::lock_input` (red banner).
//! Adds: whole-body balance torque at the ankles (Physics' `phys_balance`) and `target/live/metrics.jsonl`.
//! Physics runs on a fixed 1/120 s step in FixedPostUpdate, so one "tick" here is one physics step.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_egui::egui;
use guildbound_sim::event_log::{Event as LogEvent, EventLog, Tick as LogTick};
use guildbound_sim::phys_balance as pb;
use guildbound_sim::phys_stance as st;
use std::fmt::Write as _;
use std::io::Write;
use std::sync::Mutex;

const METRICS_DIR: &str = "target/live";
const METRICS_EVERY: u64 = 120;
/// Safety cap per joint, as a lever: SAFETY_LEVER_M · M · g (≈ 2200 N·m at 75 kg). Stable PD needs no working
/// clamp; this only catches a fault (NaN or runaway), clamps it and logs it.
const SAFETY_LEVER_M: f32 = 3.0;
/// Strike: Kp × STRIKE_KP_SCALE for 6 ticks, eased to ×1 by tick 9 (phys_mass::strike_boost), 12-tick cooldown.
const STRIKE_KP_SCALE: f32 = 1.5;
/// Stance foot lateral damping gain (N·s/m) when GUILDBOUND_FOOT_DAMP=on, and the floor friction it is capped by.
const FOOT_DAMP: f32 = 40.0;
const FLOOR_MU: f32 = 0.75;

/// Gains and substeps (env). Defaults = hub ruling 07:23 on the lab gate: kp 4000, kd 126, kb 5, kbd 0.3, 12 substeps
/// (bal/ro_5_0.3_12.json). Fallback if 12 substeps costs too much: 6 substeps + GUILDBOUND_CONTACT_FREQ=3 (kb 5).
/// TODO(ruling): Avian 0.7 has no solver-iteration setting and SubstepCount is global; per-ragdoll quality is open.
/// kp = ω² (1/s²), kd = 2ζω (1/s): Stable PD scales them by the reduced inertia, so one pair fits every joint.
#[derive(Clone, Copy, Debug)]
pub struct Gains {
    pub kp: f32, pub kd: f32, pub kb: f32, pub kbd: f32, pub foot_damp: bool, pub substeps: u32,
    /// Tan et al. 2011 full numerator kp·θ − (kd + kp·dt)·ω_rel (GUILDBOUND_TAN_FULL=on). Off until the lab confirms it.
    pub tan_full: bool,
    /// Avian contact_frequency_factor override (GUILDBOUND_CONTACT_FREQ). Fallback 3.0 if substeps drop to 6.
    pub contact_freq: Option<f32>,
}
impl Default for Gains { fn default() -> Self { Gains { kp: 4000.0, kd: 126.0, kb: 5.0, kbd: 0.3, foot_damp: false, substeps: 12, tan_full: false, contact_freq: None } } }
impl Gains {
    pub fn from_env() -> Self {
        let d = Gains::default();
        let f = |k: &str, v: f32| std::env::var(k).ok().and_then(|s| s.parse().ok()).unwrap_or(v);
        Gains {
            kp: f("GUILDBOUND_KP", d.kp), kd: f("GUILDBOUND_KD", d.kd), kb: f("GUILDBOUND_KB", d.kb), kbd: f("GUILDBOUND_KBD", d.kbd),
            foot_damp: std::env::var("GUILDBOUND_FOOT_DAMP").is_ok_and(|v| v == "on"),
            // TODO(ruling): 12 substeps (global); substeps vs solver iterations still open
            tan_full: std::env::var("GUILDBOUND_TAN_FULL").is_ok_and(|v| v == "on"),
            contact_freq: std::env::var("GUILDBOUND_CONTACT_FREQ").ok().and_then(|s| s.parse().ok()),
            substeps: std::env::var("GUILDBOUND_SUBSTEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(d.substeps),
        }
    }
}
/// Anti-tunnelling on feet and shins: max spin (rad/s) and the depth that counts as "through the floor" (m).
const MAX_SPIN: f32 = 50.0;
const BELOW_FLOOR_M: f32 = -0.1;
/// Contact factor below which a lifted foot's ankle switches to its swing cap.
const SWING_C: f32 = 0.2;

/// Which motor-writer terms run (GUILDBOUND_STEPS, comma list; default all). Physics' run order is
/// `none` (motors off: the slide gate), `paired`, `paired,ff`, `paired,ff,contact`, `paired,ff,contact,balance`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Steps { pub paired: bool, pub ff: bool, pub contact: bool, pub balance: bool }
impl Steps {
    fn from_env() -> Self {
        let v = std::env::var("GUILDBOUND_STEPS").unwrap_or_else(|_| "paired,ff,contact,balance".into());
        let has = |k: &str| v.split(',').any(|x| x.trim() == k);
        Steps { paired: has("paired"), ff: has("ff"), contact: has("contact"), balance: has("balance") }
    }
    fn label(self) -> String { format!("paired={} ff={} contact={} balance={}", self.paired, self.ff, self.contact, self.balance) }
}

/// Rest-only stand test (GUILDBOUND_STAND, default on; `off` = old behaviour): no clip, agent lock off, and
/// the clip and model seen on tick 0 must not change, else the run is marked FAILED in metrics.jsonl.
#[derive(Clone, Debug, Default)]
pub struct StandLock { pub clip: Option<String>, pub model: Option<String>, pub failed: Option<String> }

/// Start-up mode. Sandbox (the default) boots locked with the ragdoll on; Paused leaves the old Studio boot.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StudioStartMode { #[default] Sandbox, Paused }

#[derive(Resource, Default)]
pub struct Sandbox {
    /// Balance and stance-hold torques on (default) or off; set only through the `SetBalance` message.
    pub balance_on: bool,
    pub tick: u64,
    pub log: EventLog,
    pub on_tick: Option<u64>,
    /// Frozen at ragdoll-on: body mass (kg) and COM height above the mean ankle (m).
    pub mass: f32,
    pub h: f32,
    pub prev_com: Option<Vec3>,
    pub balance_enabled: bool,
    pub started: bool,
    /// Foot contact with hysteresis (down < 3 cm, up > 5 cm, 6 ticks to flip), left/right.
    pub contact: [Option<pb::Contact>; 2],
    pub contact_flips: u64,
    pub knee_hold: f32,
    /// Raw sole heights above the stage floor (m), left/right, from the last step.
    pub sole: [f32; 2],
    /// Motor writer rest rig (O offsets, chains), parent spin last tick, planted factor c per foot.
    pub rest: Option<RestRig>,
    pub prev_wp: Vec<Vec3>,
    pub load_theta_err: f32,
    pub steps: Steps,
    pub stand: Option<StandLock>,
    /// Tick of the first capsule below BELOW_FLOOR_M (logged once).
    pub below_floor: Option<u64>,
    /// Per-tick torque-sum lines, flushed to torque_sum.jsonl with each metrics line.
    pub tau_log: String,
    pub gains: Gains,
    /// Tick the current strike started (Kp boost), for strike_boost and the cooldown.
    pub strike_from: Option<u64>,
    /// Safety-cap faults since ragdoll-on (should stay 0).
    pub faults: u64,
}

impl Sandbox {
    /// Start a strike's Kp boost if the 12-tick cooldown allows it (logged). Returns whether it started.
    pub fn start_strike(&mut self) -> bool {
        let since = self.strike_from.map_or(u32::MAX, |t| self.tick.saturating_sub(t).min(u32::MAX as u64) as u32);
        if !guildbound_sim::phys_mass::strike_can_restart(since) { return false; }
        self.strike_from = Some(self.tick);
        info!("[motor] strike Kp boost from tick {}", self.tick);
        let _ = append_metrics(&format!("{{\"strike\":{{\"tick\":{}}}}}", self.tick));
        true
    }
}

/// What the Physics section in the Studio shows (the UI system has no spare parameter slots).
#[derive(Clone, Debug, Default)]
pub struct Shown { pub sole: [f32; 2], pub balance_on: bool, pub knee_hold: f32, pub contact_flips: u64, pub tick: u64, pub ramp: f32, pub com: [f32; 3], pub margin: f32, pub peak_sat: f32, pub hinge_err: f32, pub mass: f32, pub h: f32, pub feet_down: u8, pub tau: f32, pub active: bool, pub error: Option<String> }
pub static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);
/// The Studio checkbox's request (a resource, no global): the checkbox stores it in egui memory, Update moves it
/// here, and the fixed step turns it into a logged `SetBalance` before physics.
#[derive(Resource, Default)]
pub struct BalanceRequest(pub Option<bool>);
const BALANCE_REQ_ID: &str = "sandbox_balance_request";

/// Event: turn the sandbox balance (balance torque + stance hold) on or off.
#[derive(Message, Clone, Copy, Debug)]
pub struct SetBalance(pub bool);

pub fn plugin(app: &mut App) {
    app.init_resource::<Sandbox>()
        .init_resource::<BalanceRequest>()
        .init_resource::<StudioStartMode>()
        .insert_resource(SubstepCount(Gains::from_env().substeps))
        .add_systems(Startup, contact_freq_override)
        .add_message::<SetBalance>()
        .add_systems(Update, (boot, esc_opens_studio, forward_balance_request))
        // fixed 1/120 s physics step: toggle, contacts + balance + hold, then Avian, then the post-step report
        .add_systems(FixedPostUpdate, (balance_tick, apply_balance_toggle).chain().after(crate::possession::physical_drive).before(crate::props::physics_clock).before(PhysicsSystems::First))
        .add_systems(FixedPostUpdate, report.after(PhysicsSystems::Last));
}

/// GUILDBOUND_CONTACT_FREQ: override Avian's contact stiffness factor (the 6-substep fallback is 3.0).
fn contact_freq_override(cfg: Option<ResMut<avian3d::dynamics::solver::SolverConfig>>) {
    if let (Some(f), Some(mut cfg)) = (Gains::from_env().contact_freq, cfg) { cfg.contact_frequency_factor = f; info!("[motor] contact_frequency_factor = {f}"); }
}

/// Once, at start: physical body on (fully physical), input locked. The Studio itself is opened by
/// studio::open_at_startup with the mannequin on the stage.
fn boot(mode: Res<StudioStartMode>, mut sb: ResMut<Sandbox>, mut body: ResMut<crate::possession::PhysicalBody>, mut agent: ResMut<crate::agent::Agent>) {
    if sb.started || *mode != StudioStartMode::Sandbox { return; }
    sb.started = true;
    sb.balance_enabled = true;
    // GUILDBOUND_BALANCE=off boots with balance off (logged like the Studio toggle)
    sb.balance_on = std::env::var("GUILDBOUND_BALANCE").map_or(true, |v| v != "off");
    if !sb.balance_on { let _ = sb.log.append(LogTick(0), LogEvent::BalanceSet { on: false }); }
    sb.steps = Steps::from_env();
    sb.gains = Gains::from_env();
    info!("[motor] gains {:?}", sb.gains);
    let stand = std::env::var("GUILDBOUND_STAND").map_or(true, |v| v != "off");
    if stand { sb.stand = Some(StandLock::default()); }
    body.on = true;
    body.weight = 1.0;
    body.physical_regions = [true; 3];
    // the stand test runs with the agent lock off (pass 6a) so the Pose tab stays usable
    agent.lock_input = !stand;
}

fn forward_balance_request(mut ctxs: bevy_egui::EguiContexts, mut req: ResMut<BalanceRequest>) {
    let Ok(ctx) = ctxs.ctx_mut() else { return };
    let id = egui::Id::new(BALANCE_REQ_ID);
    if let Some(on) = ctx.data_mut(|d| d.remove_temp::<bool>(id)) { req.0 = Some(on); }
}

/// Applies `SetBalance` and logs it in the event log.
pub fn apply_balance_toggle(mut msgs: MessageReader<SetBalance>, mut sb: ResMut<Sandbox>) {
    for &SetBalance(on) in msgs.read() {
        if sb.balance_on == on { continue; }
        sb.balance_on = on;
        let t = sb.tick;
        let _ = sb.log.append(LogTick(t), LogEvent::BalanceSet { on });
    }
}

/// Esc opens the Studio (F2) when it is closed. agent::toggle also releases the input lock on Esc.
fn esc_opens_studio(keys: Res<ButtonInput<KeyCode>>, mut studio: ResMut<crate::studio::Studio>) {
    if keys.just_pressed(KeyCode::Escape) && !studio.open { studio.toggle_request = true; }
}

struct Measure { mass: f32, com: Vec3, poly: Vec<[f32; 2]>, planted: [bool; 2], sole_h: [f32; 2], foot_pts: [Vec<[f32; 2]>; 2], contact_pt: [Vec3; 2], hips: [Option<(Vec3, Entity)>; 2], knees: [Option<Vec3>; 2], pelvis: Option<Entity>, thighs: [Option<Entity>; 2], ankles: [Option<Vec3>; 2], sole_half: [f32; 2], feet: [Option<Entity>; 2], calves: [Option<Entity>; 2] }

fn side_of(name: &str) -> Option<usize> {
    if name.starts_with("B_L_") { Some(0) } else if name.starts_with("B_R_") { Some(1) } else { None }
}

/// COM, support polygon and feet from the stage doll's bodies. Sole length = the foot body (ankle to toe).
fn measure(world: &World, doll: &crate::ragdoll::Doll, contact: [bool; 2]) -> Measure {
    let ground = crate::studio::STAGE.y;
    let mut m = Measure { mass: 0.0, com: Vec3::ZERO, poly: Vec::new(), planted: contact, sole_h: [f32::INFINITY; 2], foot_pts: [Vec::new(), Vec::new()], contact_pt: [Vec3::ZERO; 2], hips: [None; 2], knees: [None; 2], pelvis: None, thighs: [None; 2], ankles: [None; 2], sole_half: [0.0; 2], feet: [None; 2], calves: [None; 2] };
    let mut pts = Vec::new();
    for link in &doll.links {
        let Some(t) = world.get::<Transform>(link.body) else { continue };
        let mass = world.get::<Mass>(link.body).map_or(1.0, |m| m.0);
        m.mass += mass;
        m.com += t.translation * mass;
        let name = world.get::<Name>(link.bone).map(|n| n.as_str().to_owned()).unwrap_or_default();
        if link.parent.is_none() { m.pelvis = Some(link.body); }
        let Some(s) = side_of(&name) else { continue };
        let joint = t.translation + t.rotation * link.origin; // this segment's start = its joint with the parent
        if name.ends_with("_Thigh") { m.thighs[s] = Some(link.body); m.hips[s] = Some((joint, link.body)); }
        if name.ends_with("_Calf") { m.calves[s] = Some(link.body); m.knees[s] = Some(joint); }
        if name.ends_with("_Foot") {
            m.feet[s] = Some(link.body);
            let (radius, half) = capsule_dims(world, link.body).unwrap_or((0.045, 0.0));
            let ankle = t.translation + t.rotation * link.origin;
            let toe = t.translation - t.rotation * link.origin;
            m.ankles[s] = Some(ankle);
            m.sole_half[s] = ankle.distance(toe) * 0.5 + radius;
            // Exact lowest point of the capsule: the ends of its inner segment minus the radius. The old
            // `min(ankle, toe).y - radius` used the capsule TIPS, which already include the radius, so a foot
            // pitched by θ read r·sinθ too low (the −2.65 cm "rest offset" at r = 4.5 cm). No offset needed now.
            m.sole_h[s] = t.translation.y - (t.rotation * Vec3::Y).y.abs() * half - radius - ground;
            let mid = (ankle + toe) * 0.5;
            m.contact_pt[s] = Vec3::new(mid.x, ground, mid.z);
            let along = Vec3::new(toe.x - ankle.x, 0.0, toe.z - ankle.z).normalize_or(Vec3::Z);
            let across = Vec3::Y.cross(along) * radius;
            m.foot_pts[s] = [ankle + across, ankle - across, toe + across, toe - across].iter().map(|p| [p.x, p.z]).collect();
        }
    }
    for s in 0..2 { if m.planted[s] { pts.extend(m.foot_pts[s].iter().copied()); } }
    m.com /= m.mass.max(1e-3);
    m.poly = pb::support_polygon(&pts);
    m
}

/// Capsule radius and inner half-segment length (ragdoll capsules lie along their body's local Y).
fn capsule_dims(world: &World, e: Entity) -> Option<(f32, f32)> {
    world.get::<Collider>(e).and_then(|c| c.shape().as_capsule().map(|c| (c.radius, c.half_height())))
}

/// Hinge axis error of the doll's revolute joints (same check as ragdoll::snapshot, for the stage doll).
fn hinge_axis_error(world: &World, doll: &crate::ragdoll::Doll) -> f32 {
    let mut worst = 0.0f32;
    for &e in &doll.joints {
        if let Some(j) = world.get::<RevoluteJoint>(e)
            && let Some(a) = world.get::<Transform>(j.body1)
            && let Some(b) = world.get::<Transform>(j.body2)
            && let Some(ha) = j.local_hinge_axis1()
            && let Some(hb) = j.local_hinge_axis2()
        {
            worst = worst.max((a.rotation * ha).dot(b.rotation * hb).clamp(-1.0, 1.0).acos());
        }
    }
    worst
}

/// Load-time rest data for the single motor writer (brief 3.4), measured once at ragdoll-on.
pub struct JointRest {
    pub link: usize,
    pub parent: usize,
    /// O for parent and child capsule: bone_world = capsule_world · O.
    pub o_p: Quat,
    pub o_c: Quat,
    /// Skin-joint chain (anim_rig joint indices) from the parent capsule's bone down to this one.
    pub chain: Vec<usize>,
    pub kind: JointKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointKind { Hip(usize), Knee(usize), Ankle(usize), Arm, Trunk }

pub struct RestRig { pub rig: guildbound_sim::anim_rig::Rig, pub joints: Vec<JointRest> }

fn kind_of(name: &str) -> JointKind {
    let s = side_of(name).unwrap_or(0);
    if name.ends_with("_Thigh") { JointKind::Hip(s) }
    else if name.ends_with("_Calf") { JointKind::Knee(s) }
    else if name.ends_with("_Foot") { JointKind::Ankle(s) }
    else if name.contains("Arm") { JointKind::Arm }
    else { JointKind::Trunk }
}

fn q64(q: Quat) -> [f64; 4] { [q.x as f64, q.y as f64, q.z as f64, q.w as f64] }
fn mat_v(m: &pb::M3, v: [f32; 3]) -> [f32; 3] { [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2]) }
/// A body's own world inertia tensor (row-major; symmetric, so rows = columns).
fn world_inertia(world: &World, e: Entity, r: Quat) -> [[f32; 3]; 3] {
    world.get::<ComputedAngularInertia>(e).map_or([[1e-3, 0.0, 0.0], [0.0, 1e-3, 0.0], [0.0, 0.0, 1e-3]], |ia| ia.rotated(r).tensor().to_cols_array_2d())
}
/// Shortest-arc rotation vector (world) of q.
fn log_q(q: Quat) -> Vec3 { let q = if q.w < 0.0 { -q } else { q }; let (axis, angle) = q.to_axis_angle(); if angle.abs() < 1e-9 { Vec3::ZERO } else { axis * angle } }

/// Build the rest rig from the stage skeleton (the anim_rig load log comes from it) and the doll.
fn build_rest(world: &World, doll: &crate::ragdoll::Doll) -> Option<RestRig> {
    use guildbound_sim::anim_rig as ar;
    let root = world.resource::<crate::studio::Studio>().entity?;
    let bones = world.get::<crate::studio::StudioRig>(root)?.bones.clone();
    let nodes: Vec<ar::Node> = bones.iter().enumerate().map(|(i, &e)| {
        let t = world.get::<Transform>(e).copied().unwrap_or_default();
        let parent = world.get::<ChildOf>(e).and_then(|c| bones.iter().position(|&b| b == c.parent())).map_or(-1, |p| p as i32);
        let name = world.get::<Name>(e).map(|n| n.as_str().to_owned()).unwrap_or_default();
        ar::Node { name, joint: i as i32, parent, t: [t.translation.x as f64, t.translation.y as f64, t.translation.z as f64], r: ar::qnorm(q64(t.rotation)) }
    }).collect();
    let rig = ar::Rig::from_nodes(nodes);
    let names: Vec<&str> = doll.links.iter().filter_map(|l| world.get::<Name>(l.bone).map(|n| n.as_str())).collect();
    info!("{}", rig.report("stage", &names).trim_end());
    let mut joints = Vec::new();
    for (i, l) in doll.links.iter().enumerate() {
        let Some(p) = l.parent else { continue };
        let pl = &doll.links[p];
        let (Some(from), Some(to)) = (bones.iter().position(|&b| b == pl.bone), bones.iter().position(|&b| b == l.bone)) else { continue };
        let Some(chain) = guildbound_sim::anim_abilities::chain(&rig, from, to) else { warn!("[motor] no chain {from}->{to}"); continue };
        let name = world.get::<Name>(l.bone).map(|n| n.as_str().to_owned()).unwrap_or_default();
        joints.push(JointRest { link: i, parent: p, o_p: pl.rotation, o_c: l.rotation, chain, kind: kind_of(&name) });
    }
    Some(RestRig { rig, joints })
}

/// Target local rotation of a capsule joint: Animation's chain_target over the chain's rest locals
/// (no animation target plays in the sandbox yet, so every joint targets the rest pose).
fn joint_target(rest: &RestRig, j: &JointRest, tick: u64) -> guildbound_sim::anim_ability::BoneTarget {
    let local = |_t: i64, k: usize| { let r = rest.rig.nodes[rest.rig.joints[k]].r; [r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32] };
    guildbound_sim::anim_abilities::chain_target(&j.chain, &local, tick)
}

fn theta_err(r_p: Quat, r_c: Quat, j: &JointRest, local: Quat) -> Vec3 {
    log_q((r_p * j.o_p * local * j.o_c.inverse()) * r_c.inverse())
}

/// Before the physics step (FixedPostUpdate, before PhysicsSystems::First): contact, then the single motor
/// writer (brief 3.4). One torque per joint per tick, one clamp per joint, +tau on the child, -tau on the parent.
pub fn balance_tick(world: &mut World) {
    let mut sb = world.remove_resource::<Sandbox>().unwrap_or_default();
    if let Some(on) = world.resource_mut::<BalanceRequest>().0.take() { world.write_message(SetBalance(on)); }
    let body = world.resource::<crate::possession::PhysicalBody>();
    let Some(doll) = body.doll() else {
        if sb.on_tick.is_some() { let t = sb.tick; let _ = sb.log.append(LogTick(t), LogEvent::RagdollOff); }
        sb.on_tick = None;
        sb.contact = [None; 2];
        sb.prev_com = None;
        sb.rest = None;
        world.insert_resource(sb);
        return;
    };
    let tick = sb.tick;
    sb.tick += 1;
    let mut motors_off: Vec<(Entity, RevoluteJoint)> = Vec::new();
    let mut anti_tunnel: Vec<(Entity, bool)> = Vec::new();
    let mut stop_clip = false;
    let raw = measure(world, doll, [false; 2]);
    if sb.on_tick.is_none() {
        // ragdoll just came on: log it, freeze m, h and the rest rig (O, chains)
        let _ = sb.log.append(LogTick(tick), LogEvent::RagdollOn);
        sb.on_tick = Some(tick);
        sb.below_floor = None;
        sb.mass = raw.mass;
        let ankles: Vec<Vec3> = raw.ankles.iter().flatten().copied().collect();
        let ankle_y = if ankles.is_empty() { crate::studio::STAGE.y } else { ankles.iter().map(|a| a.y).sum::<f32>() / ankles.len() as f32 };
        sb.h = (raw.com.y - ankle_y).max(0.05);
        sb.rest = build_rest(world, doll);
        sb.prev_wp = vec![Vec3::ZERO; doll.links.len()];
        sb.contact = [Some(pb::Contact::planted()); 2];
        // Avian joint motors off on the sandbox body: the motor writer is the only torque source
        motors_off = doll.joints.iter().filter_map(|&e| world.get::<RevoluteJoint>(e).filter(|j| j.motor.enabled).map(|j| { let mut j = j.clone(); j.motor.enabled = false; (e, j) })).collect();
        // anti-tunnelling: every capsule spins at most MAX_SPIN; feet, toes and shins also get swept CCD
        anti_tunnel = doll.links.iter().map(|l| {
            let n = world.get::<Name>(l.bone).map(|n| n.as_str().to_owned()).unwrap_or_default();
            (l.body, n.ends_with("_Foot") || n.ends_with("_Toe0") || n.ends_with("_Calf"))
        }).collect();
        // stand test: freeze the clip and model seen now, stop any clip so nothing but rest drives the stage
        if let Some(lock) = sb.stand.as_mut() {
            let studio = world.resource::<crate::studio::Studio>();
            stop_clip = true; // applied in finish_on (the doll borrows the world until then)
            *lock = StandLock { clip: studio.clip.clone(), model: studio.model.clone(), failed: None };
            info!("[stand] tick 0: clip {:?}, model {:?}, steps {}", lock.clip, lock.model, sb.steps.label());
            let _ = append_metrics(&format!("{{\"stand\":{{\"tick\":0,\"clip\":{},\"model\":{},\"steps\":\"{}\"}}}}", js(&lock.clip), js(&lock.model), sb.steps.label()));
        }
        if let Some(rest) = &sb.rest {
            let mut worst = 0.0f32;
            for j in &rest.joints {
                let (Some(tp), Some(tc)) = (world.get::<Transform>(doll.links[j.parent].body), world.get::<Transform>(doll.links[j.link].body)) else { continue };
                let local = Quat::from_array(joint_target(rest, j, 0).rot.0);
                worst = worst.max(theta_err(tp.rotation, tc.rotation, j, local).length());
            }
            sb.load_theta_err = worst;
            let ok = if worst < 1e-4 { "ok" } else { "FAIL" };
            info!("[motor] load check: max |theta_err| at rest = {worst:.2e} rad over {} joints, sole L {:.4} R {:.4} m ({ok})", rest.joints.len(), raw.sole_h[0], raw.sole_h[1]);
            let _ = append_metrics(&format!("{{\"load_check\":{{\"max_theta_err_rad\":{worst:.3e},\"joints\":{},\"sole\":[{},{}],\"ok\":{}}}}}", rest.joints.len(), fmt_m(raw.sole_h[0]), fmt_m(raw.sole_h[1]), worst < 1e-4));
        }
    }
    let since_on = tick - sb.on_tick.unwrap_or(tick);
    // stand test: the clip or model changing mid-run invalidates it (pass 5 switched model ~8 times)
    if let Some(lock) = sb.stand.as_mut() && lock.failed.is_none() && since_on > 0 {
        let studio = world.resource::<crate::studio::Studio>();
        let why = if studio.model != lock.model { Some(format!("model {:?} -> {:?}", lock.model, studio.model)) }
            else if studio.clip != lock.clip || studio.playing { Some(format!("clip {:?} -> {:?} (playing {})", lock.clip, studio.clip, studio.playing)) } else { None };
        if let Some(why) = why {
            warn!("[stand] FAILED at tick {since_on}: {why}");
            let _ = append_metrics(&format!("{{\"stand\":{{\"tick\":{since_on},\"failed\":{}}}}}", js(&Some(why.clone()))));
            lock.failed = Some(why);
        }
    }
    // contact: hysteresis on the exact sole height, then the 0.25/tick filter gives c (steps.contact off = raw 0/1)
    let mut planted = [false; 2];
    let mut c = [0.0f32; 2];
    for s in 0..2 {
        let ct = sb.contact[s].get_or_insert_with(pb::Contact::planted);
        let flip = ct.step(raw.sole_h[s]);
        planted[s] = ct.on;
        c[s] = if sb.steps.contact { ct.c } else if ct.on { 1.0 } else { 0.0 };
        if flip { sb.contact_flips += 1; }
    }
    sb.sole = raw.sole_h;
    let m = measure(world, doll, planted);
    let dt = 1.0 / 120.0;
    let com_vel = sb.prev_com.map_or(Vec3::ZERO, |p| (m.com - p) / dt);
    sb.prev_com = Some(m.com);
    let ramp = pb::ramp(since_on);
    let feet: Vec<([f32; 3], bool)> = (0..2).map(|s| (m.contact_pt[s].to_array(), m.planted[s])).collect();
    let shares = st::weight_shares(m.com.to_array(), &feet);
    // tau_bal: whole-body balance (raw, ramped inside); off = 0. Split below: ankles first, rest to stance hips.
    // support centre = c-weighted foot centres (as the lab)
    let csum = c[0] + c[1];
    let centre = (0..2).fold(Vec3::ZERO, |a, s| a + m.contact_pt[s] * c[s]) / csum.max(1e-6);
    let tau_bal = if sb.balance_enabled && sb.balance_on && sb.steps.balance && csum > 0.05 {
        Vec3::from_array(pb::balance_kb(sb.mass, m.com.to_array(), com_vel.to_array(), [centre.x, centre.z], sb.gains.kb, sb.gains.kbd, since_on))
    } else { Vec3::ZERO };
    let split_in: Vec<(f32, f32, f32)> = (0..2).map(|s| (c[s], shares[s], ankle_cap(&m, s, -tau_bal, c[s] * shares[s] * sb.mass))).collect();
    let split: Vec<(Vec3, Vec3)> = pb::split_balance(tau_bal.to_array(), &split_in).into_iter().map(|(a, h)| (Vec3::from_array(a), Vec3::from_array(h))).collect();
    let Some(rest) = sb.rest.take() else { finish_on(world, motors_off, &anti_tunnel, stop_clip); world.insert_resource(sb); return };
    let links: Vec<(Entity, Quat, Vec3, Vec3, f32)> = doll.links.iter().map(|l| {
        let t = world.get::<Transform>(l.body).copied().unwrap_or_default();
        let w = world.get::<AngularVelocity>(l.body).map_or(Vec3::ZERO, |v| v.0);
        (l.body, t.rotation, t.translation, w, world.get::<Mass>(l.body).map_or(1.0, |m| m.0))
    }).collect();
    let inertia: Vec<[[f32; 3]; 3]> = links.iter().map(|k| world_inertia(world, k.0, k.1)).collect();
    // stance foot lateral damping (off by default): F = -c·40·(v_foot - v_com) in x,z, capped at mu·share·M·g
    let mut foot_forces: Vec<(Entity, Vec3)> = Vec::new();
    if sb.gains.foot_damp {
        for s in 0..2 {
            let Some(foot) = m.feet[s] else { continue };
            let v = world.get::<LinearVelocity>(foot).map_or(Vec3::ZERO, |v| v.0);
            let f = -(c[s] * FOOT_DAMP) * (v - com_vel).with_y(0.0);
            let f = f.clamp_length_max(FLOOR_MU * shares[s] * sb.mass * guildbound_sim::phys_math::G);
            if f != Vec3::ZERO { foot_forces.push((foot, f)); }
        }
    }
    let strike = sb.strike_from.map_or(0.0, |t| guildbound_sim::phys_mass::strike_boost(tick.saturating_sub(t).min(u32::MAX as u64) as u32));
    let safety = SAFETY_LEVER_M * sb.mass * guildbound_sim::phys_math::G;
    let mut fault: Option<(usize, f32)> = None;
    // first capsule through the floor (lowest point of its capsule below BELOW_FLOOR_M), logged once
    if sb.below_floor.is_none() {
        let ground = crate::studio::STAGE.y;
        let low = doll.links.iter().zip(&links).filter_map(|(l, k)| {
            let (r, half) = capsule_dims(world, l.body)?;
            Some((l.bone, k.2.y - (k.1 * Vec3::Y).y.abs() * half - r - ground))
        }).min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((bone, y)) = low && y < BELOW_FLOOR_M {
            sb.below_floor = Some(since_on);
            let name = world.get::<Name>(bone).map(|n| n.as_str().to_owned()).unwrap_or_default();
            warn!("[floor] {name} is {y:.3} m below the floor at tick {since_on}");
            let _ = append_metrics(&format!("{{\"below_floor\":{{\"tick\":{since_on},\"bone\":\"{name}\",\"y\":{y:.4}}}}}"));
        }
    }
    // subtree per link, built once per tick (was rebuilt per joint)
    let mut kids_of: Vec<Vec<usize>> = vec![Vec::new(); doll.links.len()];
    for (i, l) in doll.links.iter().enumerate() { if let Some(p) = l.parent { kids_of[p].push(i); } }
    let subtree = |i: usize| -> Vec<usize> { let mut c = vec![i]; let mut k = 0; while k < c.len() { c.extend(kids_of[c[k]].iter().copied()); k += 1; } c };
    let mut writes: Vec<(Entity, Entity, Vec3)> = Vec::new();
    let (mut peak, mut ankle_tau, mut knee_ff) = (0.0f32, 0.0f32, 0.0f32);
    for j in &rest.joints {
        let (_, r_p, _, w_p, _) = links[j.parent];
        let (_, r_c, _, w_c, _) = links[j.link];
        let t = joint_target(&rest, j, since_on);
        let err = theta_err(r_p, r_c, j, Quat::from_array(t.rot.0));
        // target rates are in the PARENT BONE frame: transport with B_p = R_p·O_p, not R_p (ANIMATION_MATH §4)
        let b_p = r_p * j.o_p;
        let (w_star, a_star) = guildbound_sim::anim_abilities::rates_to_world_rel(b_p.to_array(), w_p.to_array(), t.omega.0, t.alpha.0);
        let (w_star, a_star) = (Vec3::from_array(w_star), Vec3::from_array(a_star));
        let dwp = (w_p - sb.prev_wp.get(j.parent).copied().unwrap_or(w_p)) / dt;
        let w_rel = w_c - w_p;
        // full 3x3 reduced inertia (I_c^-1 + I_p^-1)^-1 from each body's OWN world inertia (Avian applies torque per body)
        let i_red = pb::reduced_inertia3(inertia[j.parent], inertia[j.link]);
        let ff = Vec3::from_array(mat_v(&i_red, (a_star + dwp).to_array()));
        let jp = links[j.link].2 + links[j.link].1 * doll.links[j.link].origin;
        let below = subtree(j.link);
        let m_below: f32 = below.iter().map(|&k| links[k].4).sum();
        let com_below = below.iter().map(|&k| links[k].2 * links[k].4).sum::<Vec3>() / m_below.max(1e-3);
        // leg joints carry their foot's c and weight share; arms and trunk are always "swing" (c = 0)
        let (cj, share, cop) = match j.kind { JointKind::Hip(s) | JointKind::Knee(s) | JointKind::Ankle(s) => (c[s], shares[s], m.contact_pt[s]), _ => (0.0, 0.0, jp) };
        // Stable PD (implicit in dt) on the reduced inertia; the strike boost raises Kp (arms only), never a cap
        let kp = sb.gains.kp * if matches!(j.kind, JointKind::Arm) { 1.0 + (STRIKE_KP_SCALE - 1.0) * strike } else { 1.0 };
        // Tan full numerator adds −kp·dt·ω_rel: same as stable_pd3 with the rate term scaled by (kd + kp·dt)/kd
        let w_err = (w_rel - w_star) * if sb.gains.tan_full { (sb.gains.kd + kp * dt) / sb.gains.kd.max(1e-6) } else { 1.0 };
        let pd = Vec3::from_array(pb::stable_pd3(i_red, err.to_array(), w_err.to_array(), kp, sb.gains.kd, dt));
        // gravity feed-forward, reaction form at this foot's CoP (swing = plain subtree compensation)
        let grav = if sb.steps.ff {
            Vec3::from_array(pb::gravity_ff(jp.to_array(), com_below.to_array(), m_below, cop.to_array(), share, sb.mass, cj))
        } else { Vec3::ZERO };
        // balance enters negated on the child (it turns the body above: the parent side)
        let bal = match j.kind { JointKind::Ankle(s) => -split[s].0, JointKind::Hip(s) => -split[s].1, _ => Vec3::ZERO };
        let raw_tau = ff + pd + grav + bal;
        let sat_c = if matches!(j.kind, JointKind::Arm | JointKind::Trunk) { 1.0 } else { cj }; // legs at c < 0.2 are not counted
        if let Some(r) = pb::sat_ratio(raw_tau.to_array(), safety, sat_c) { peak = peak.max(r); }
        // the one cap is a fault catcher only: NaN or past SAFETY_LEVER_M·M·g
        let tau = if !raw_tau.is_finite() { fault.get_or_insert((j.link, f32::NAN)); Vec3::ZERO }
            else if raw_tau.length() > safety { fault.get_or_insert((j.link, raw_tau.length())); raw_tau * (safety / raw_tau.length()) }
            else { raw_tau };
        match j.kind { JointKind::Ankle(_) => ankle_tau = ankle_tau.max(tau.length()), JointKind::Knee(_) => knee_ff += grav.length(), _ => {} }
        writes.push((links[j.link].0, links[j.parent].0, tau));
    }
    sb.prev_wp = links.iter().map(|l| l.3).collect();
    if let Some((link, t)) = fault {
        sb.faults += 1;
        let name = world.get::<Name>(doll.links[link].bone).map(|n| n.as_str().to_owned()).unwrap_or_default();
        if sb.faults <= 20 { warn!("[motor] FAULT tick {since_on}: {name} |tau| {t:.1} > safety {safety:.0} N m"); }
        let _ = append_metrics(&format!("{{\"fault\":{{\"tick\":{since_on},\"bone\":\"{name}\",\"tau\":{},\"count\":{}}}}}", fmt_m(t), sb.faults));
    }
    finish_on(world, motors_off, &anti_tunnel, stop_clip);
    // apply: +tau child, -tau parent (steps.paired off = child only, to test the slide theory). Στ is what was applied.
    let motors = sb.steps.paired || sb.steps.ff || sb.steps.balance;
    let (mut sum, mut max_t, mut unpaired) = (Vec3::ZERO, 0.0f32, 0u32);
    let mut q = world.query::<Forces>();
    if motors {
        for (child, parent, t) in writes {
            if t == Vec3::ZERO { continue; }
            max_t = max_t.max(t.length());
            let mut hit = 0;
            if let Ok(mut f) = q.get_mut(world, child) { f.apply_torque(t); sum += t; hit += 1; }
            if sb.steps.paired && let Ok(mut f) = q.get_mut(world, parent) { f.apply_torque(-t); sum -= t; hit += 1; }
            if hit < 2 { unpaired += 1; }
        }
    }
    // foot damping is a ground-friction stand-in: an external force, not part of Στ
    for (e, f) in foot_forces { if let Ok(mut fo) = q.get_mut(world, e) { fo.apply_force(f); } }
    let _ = writeln!(sb.tau_log, "{{\"tick\":{since_on},\"sum_tau\":[{:.4},{:.4},{:.4}],\"max_tau\":{max_t:.2},\"unpaired\":{unpaired},\"com_v\":[{:.4},{:.4}],\"c\":[{:.3},{:.3}]}}", sum.x, sum.y, sum.z, com_vel.x, com_vel.z, c[0], c[1]);
    if since_on % METRICS_EVERY == 0 {
        let lines = std::mem::take(&mut sb.tau_log);
        if let Err(e) = append_file("torque_sum.jsonl", lines.trim_end()) { warn!("[motor] torque_sum.jsonl: {e}"); }
    }
    sb.knee_hold = knee_ff;
    sb.rest = Some(rest);
    if let Ok(mut s) = SHOWN.lock() {
        let mut v = s.take().unwrap_or_default();
        v.peak_sat = peak;
        v.tau = ankle_tau;
        v.ramp = ramp;
        v.knee_hold = sb.knee_hold;
        v.balance_on = sb.balance_on;
        v.sole = sb.sole;
        v.contact_flips = sb.contact_flips;
        v.error = sb.stand.as_ref().and_then(|l| l.failed.clone()).map(|w| format!("Stand test FAILED: {w}"));
        *s = Some(v);
    }
    world.insert_resource(sb);
}

/// Ragdoll-on side effects that need `&mut World`: re-insert the motor-off joints, add the anti-tunnel parts.
fn finish_on(world: &mut World, motors_off: Vec<(Entity, RevoluteJoint)>, anti_tunnel: &[(Entity, bool)], stop_clip: bool) {
    if stop_clip { world.resource_mut::<crate::studio::Studio>().playing = false; }
    for (e, j) in motors_off { world.entity_mut(e).insert(j); }
    for &(e, ccd) in anti_tunnel {
        let mut ent = world.entity_mut(e);
        ent.insert(MaxAngularSpeed(MAX_SPIN));
        if ccd { ent.insert((SweptCcd::default(), SpeculativeMargin(0.05))); }
    }
}

fn js(v: &Option<String>) -> String { v.as_ref().map_or("null".into(), |s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))) }

/// Ankle clamp c·share·M·g·d_edge: d_edge = ankle to the sole edge the torque tips the foot toward.
/// The ground must move its pressure point by r = (-tau_z, 0, tau_x)/N to resist tau on the foot.
fn ankle_cap(m: &Measure, s: usize, tau: Vec3, mass_share: f32) -> f32 {
    let (Some(ankle), half) = (m.ankles[s], m.sole_half[s]) else { return 0.0 };
    let Some(_) = m.feet[s] else { return 0.0 };
    let pts = &m.foot_pts[s];
    if pts.len() < 4 || mass_share <= 0.0 { return 0.0; }
    let toe = Vec3::new((pts[2][0] + pts[3][0]) * 0.5, 0.0, (pts[2][1] + pts[3][1]) * 0.5);
    let a = Vec3::new(ankle.x, 0.0, ankle.z);
    let along = (toe - a).normalize_or(Vec3::Z);
    let across = Vec3::Y.cross(along);
    let radius = Vec3::new(pts[0][0] - pts[1][0], 0.0, pts[0][1] - pts[1][1]).length() * 0.5;
    let r = Vec3::new(-tau.z, 0.0, tau.x);
    let Some(u) = r.try_normalize() else { return mass_share * guildbound_sim::phys_math::G * radius };
    let (ua, ux) = (u.dot(along), u.dot(across));
    let foot_len = (half - radius) * 2.0;
    let edge_along = if ua >= 0.0 { foot_len + radius } else { radius }; // ankle sits at the heel end of the foot capsule
    let d_edge = (edge_along / ua.abs().max(1e-6)).min(radius / ux.abs().max(1e-6));
    guildbound_sim::phys_ragdoll_fix::ankle_cap(mass_share, d_edge)
}

/// After the physics step: the final pose gives COM, margin and the hinge check; every 120 ticks a metrics line.
pub fn report(world: &mut World) {
    let Some(sb) = world.get_resource::<Sandbox>() else { return };
    let (tick, on, mass, h, load_err, below) = (sb.tick.saturating_sub(1), sb.on_tick, sb.mass, sb.h, sb.load_theta_err, sb.below_floor);
    let stand_failed = sb.stand.as_ref().is_some_and(|l| l.failed.is_some());
    let contact = [0, 1].map(|s| sb.contact[s].is_some_and(|c| c.on));
    let body = world.resource::<crate::possession::PhysicalBody>();
    let Some(doll) = body.doll() else {
        if let Ok(mut s) = SHOWN.lock() && let Some(v) = s.as_mut() { v.active = false; }
        return;
    };
    let Some(on) = on else { return };
    let m = measure(world, doll, contact);
    let margin = pb::stability_margin(&m.poly, [m.com.x, m.com.z]);
    let hinge = hinge_axis_error(world, doll);
    let since_on = tick.saturating_sub(on);
    let mut shown = SHOWN.lock().ok().and_then(|s| s.clone()).unwrap_or_default();
    shown.tick = since_on;
    shown.com = [m.com.x, m.com.y - crate::studio::STAGE.y, m.com.z - crate::studio::STAGE.z];
    shown.margin = margin;
    shown.hinge_err = hinge;
    shown.mass = mass;
    shown.h = h;
    shown.feet_down = m.planted.iter().filter(|p| **p).count() as u8;
    shown.active = true;
    if since_on % METRICS_EVERY == 0 {
        let margin_s = if margin.is_finite() { format!("{margin:.4}") } else { "null".into() };
        let line = format!(
            "{{\"tick\":{since_on},\"ramp\":{:.3},\"com\":[{:.4},{:.4},{:.4}],\"margin\":{margin_s},\"peak_sat\":{:.3},\"max_hinge_axis_error_rad\":{hinge:.5},\"feet_down\":{},\"knee_hold\":{:.1},\"contact_flips\":{},\"balance_on\":{},\"sole_l\":{},\"sole_r\":{},\"mass\":{mass:.2},\"h\":{h:.3},\"load_theta_err\":{:.2e},\"below_floor_tick\":{},\"stand_failed\":{}}}",
            pb::ramp(since_on), shown.com[0], shown.com[1], shown.com[2], shown.peak_sat, shown.feet_down, shown.knee_hold, shown.contact_flips, shown.balance_on, fmt_m(shown.sole[0]), fmt_m(shown.sole[1]), load_err, below.map_or("null".into(), |t| t.to_string()), stand_failed
        );
        if let Err(e) = append_metrics(&line) { shown.error = Some(format!("metrics.jsonl: {e}")); }
    }
    if let Ok(mut s) = SHOWN.lock() { *s = Some(shown); }
}

fn fmt_m(v: f32) -> String { if v.is_finite() { format!("{v:.4}") } else { "null".into() } }

fn append_metrics(line: &str) -> std::io::Result<()> { append_file("metrics.jsonl", line) }

fn append_file(name: &str, text: &str) -> std::io::Result<()> {
    let dir = format!("{}/{METRICS_DIR}", crate::ROOT);
    std::fs::create_dir_all(&dir)?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(format!("{dir}/{name}"))?;
    writeln!(f, "{text}")
}

/// Studio Tuning tab: "Physics: balance" section, shown after the Possession block.
pub fn physics_section(ui: &mut egui::Ui) {
    use crate::ui_theme as t;
    let Some(s) = SHOWN.lock().ok().and_then(|s| s.clone()) else { t::hint(ui, "Balance starts when the physics body is on."); return };
    if !s.active { t::hint(ui, "Physics body is off. Turn on \"Physics body\" below to start balance."); return; }
    let mut on = s.balance_on;
    if ui.checkbox(&mut on, "Balance").on_hover_text("Balance torque and stance-leg hold. Off = passive ragdoll; metrics keep writing.").changed()
    { ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(BALANCE_REQ_ID), on)); }
    t::stat(ui, "Body", format!("{:.1} kg, COM {:.3} m above ankles", s.mass, s.h), t::PAPER);
    t::stat(ui, "Ticks since ragdoll on", format!("{}", s.tick), t::PAPER);
    t::stat(ui, "Balance ramp", format!("{:.2}", s.ramp), if s.ramp >= 1.0 { t::GOOD } else { t::WARN });
    t::stat(ui, "COM (stage, m)", format!("{:.3}  {:.3}  {:.3}", s.com[0], s.com[1], s.com[2]), t::PAPER);
    let margin = if s.margin.is_finite() { format!("{:+.3} m", s.margin) } else { "no feet down".into() };
    t::stat(ui, "Stability margin", margin, if s.margin >= 0.0 { t::GOOD } else { t::BAD });
    t::stat(ui, "Feet on the ground", format!("{}", s.feet_down), t::PAPER);
    t::stat(ui, "Ankle torque", format!("{:.1} N m (peak {:.0}%)", s.tau, s.peak_sat * 100.0), if s.peak_sat > 0.9 { t::BAD } else { t::GOOD });
    t::stat(ui, "Knee gravity hold", format!("{:.1} N m", s.knee_hold), t::PAPER);
    t::stat(ui, "Contact flips", format!("{}", s.contact_flips), t::PAPER);
    t::stat(ui, "Hinge axis error", format!("{:.4} rad", s.hinge_err), if s.hinge_err < 0.05 { t::GOOD } else { t::WARN });
    t::hint(ui, "Every 120 ticks a line goes to target/live/metrics.jsonl.");
    if let Some(e) = &s.error { t::stat(ui, "Metrics file", e.clone(), t::BAD); }
}
