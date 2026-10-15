//! Locked sandbox: the game boots into the Studio stage with the physical body (ragdoll) already on and the
//! player's input locked. Uses the existing pieces only: `studio::open_at_startup` (paused Studio + mannequin),
//! `possession::PhysicalBody` (the stage ragdoll and its motors), `agent::Agent::lock_input` (red banner).
//! Adds: whole-body balance torque at the ankles (Physics' `phys_balance`) and `target/live/metrics.jsonl`.
//! Physics runs every frame in PostUpdate, so one "tick" here is one physics step.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_egui::egui;
use guildbound_sim::event_log::{Event as LogEvent, EventLog, Tick as LogTick};
use guildbound_sim::phys_balance as pb;
use guildbound_sim::phys_ragdoll_fix as fix;
use guildbound_sim::phys_stance as st;
use std::io::Write;
use std::sync::Mutex;

#[path = "sandbox_core.rs"]
pub mod core;
use self::core::{split_torque, Side};

const METRICS_DIR: &str = "target/live";
const METRICS_EVERY: u64 = 120;
const OMEGA: f32 = 6.3;
const ZETA: f32 = 0.63;

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
    pub contact: [Option<st::Contact>; 2],
    pub contact_flips: u64,
    pub knee_hold: f32,
    /// Tick of the last BalanceSet{on:true}. The 60-tick ramp counts from the later of this and ragdoll-on.
    pub balance_from: Option<u64>,
}

/// What the Physics section, status strip and trails show. Held in the `SandboxShown` resource.
#[derive(Clone, Debug, Default)]
pub struct Shown { pub balance_on: bool, pub knee_hold: f32, pub contact_flips: u64, pub tick: u64, pub ramp: f32, pub com: [f32; 3], pub margin: f32, pub peak_sat: f32, pub hinge_err: f32, pub mass: f32, pub h: f32, pub feet_down: u8, pub tau: f32, pub active: bool, pub error: Option<String> }
/// Written by `balance_tick` / `report` (exclusive systems), read as `Res<SandboxShown>` by studio_kit,
/// studio_trails and `physics_section`. None until the first report. Display only, never hashed.
#[derive(Resource, Clone, Debug, Default)]
pub struct SandboxShown(pub Option<Shown>);
impl SandboxShown {
    pub fn get(&self) -> Shown { self.0.clone().unwrap_or_default() }
}

/// Ordering handle for systems that read the post-physics report (trails sampling).
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SandboxSet { Report }
/// The Studio checkbox's request, forwarded as a `SetBalance` message (the UI system has no spare parameter slots).
static BALANCE_REQUEST: Mutex<Option<bool>> = Mutex::new(None);

/// Event: turn the sandbox balance (balance torque + stance hold) on or off.
#[derive(Message, Clone, Copy, Debug)]
pub struct SetBalance(pub bool);

pub fn plugin(app: &mut App) {
    app.init_resource::<Sandbox>()
        .init_resource::<SandboxShown>()
        .init_resource::<StudioStartMode>()
        .add_message::<SetBalance>()
        .add_systems(Update, (boot, esc_opens_studio, forward_balance_request))
        // physics (and so the balance step) runs in PostUpdate here, not FixedUpdate, so the toggle goes just before it
        .add_systems(PostUpdate, apply_balance_toggle.after(crate::possession::physical_drive).before(balance_tick))
        .add_systems(PostUpdate, balance_tick.after(crate::possession::physical_drive).before(crate::ragdoll::prepare))
        // TODO(verify): pass 4 moved Avian to FixedUpdate; report goes in FixedPostUpdate after PhysicsSystems::Last
        // so studio_trails::sample_actual can order `.after(SandboxSet::Report)` in the same schedule.
        .configure_sets(FixedPostUpdate, SandboxSet::Report.after(PhysicsSystems::Last))
        .add_systems(FixedPostUpdate, report.in_set(SandboxSet::Report));
}

/// Once, at start: physical body on (fully physical), input locked. The Studio itself is opened by
/// studio::open_at_startup with the mannequin on the stage.
fn boot(mode: Res<StudioStartMode>, mut sb: ResMut<Sandbox>, mut body: ResMut<crate::possession::PhysicalBody>, mut agent: ResMut<crate::agent::Agent>) {
    if sb.started || *mode != StudioStartMode::Sandbox { return; }
    sb.started = true;
    sb.balance_enabled = true;
    sb.balance_on = true;
    body.on = true;
    body.weight = 1.0;
    body.physical_regions = [true; 3];
    agent.lock_input = true;
}

fn forward_balance_request(mut out: MessageWriter<SetBalance>) {
    if let Some(on) = BALANCE_REQUEST.lock().ok().and_then(|mut r| r.take()) { out.write(SetBalance(on)); }
}

/// Applies `SetBalance` and logs it in the event log.
pub fn apply_balance_toggle(mut msgs: MessageReader<SetBalance>, mut sb: ResMut<Sandbox>) {
    for &SetBalance(on) in msgs.read() {
        if sb.balance_on == on { continue; }
        sb.balance_on = on;
        let t = sb.tick;
        let _ = sb.log.append(LogTick(t), LogEvent::BalanceSet { on });
        // switching balance on restarts the smoothstep ramp from this event's tick (replay-identical)
        if on { sb.balance_from = Some(t); }
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
            let radius = world.get::<Collider>(link.body).and_then(|c| c.shape().as_capsule().map(|c| c.radius)).unwrap_or(0.045);
            let ankle = t.translation + t.rotation * link.origin;
            let toe = t.translation - t.rotation * link.origin;
            m.ankles[s] = Some(ankle);
            m.sole_half[s] = ankle.distance(toe) * 0.5 + radius;
            m.sole_h[s] = ankle.y.min(toe.y) - radius - ground;
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

/// Before the physics step: ankle motors carry Physics' whole-body balance torque.
pub fn balance_tick(world: &mut World) {
    let mut sb = world.remove_resource::<Sandbox>().unwrap_or_default();
    let body = world.resource::<crate::possession::PhysicalBody>();
    let Some(doll) = body.doll() else {
        if sb.on_tick.is_some() { let t = sb.tick; let _ = sb.log.append(LogTick(t), LogEvent::RagdollOff); }
        sb.on_tick = None;
        sb.contact = [None; 2];
        sb.prev_com = None;
        world.insert_resource(sb);
        return;
    };
    let tick = sb.tick;
    sb.tick += 1;
    let raw = measure(world, doll, [false; 2]);
    let mut planted = [false; 2];
    for s in 0..2 {
        let c = sb.contact[s].get_or_insert_with(st::Contact::new);
        let before = c.planted;
        planted[s] = c.step(raw.sole_h[s]);
        if planted[s] != before { sb.contact_flips += 1; }
    }
    let m = measure(world, doll, planted);
    if sb.on_tick.is_none() {
        // ragdoll just came on: log it, freeze m and h (COM height above the mean ankle)
        let _ = sb.log.append(LogTick(tick), LogEvent::RagdollOn);
        sb.on_tick = Some(tick);
        sb.mass = m.mass;
        let ankles: Vec<Vec3> = m.ankles.iter().flatten().copied().collect();
        let ankle_y = if ankles.is_empty() { crate::studio::STAGE.y } else { ankles.iter().map(|a| a.y).sum::<f32>() / ankles.len() as f32 };
        sb.h = (m.com.y - ankle_y).max(0.05);
    }
    let anchor = sb.on_tick.unwrap_or(tick).max(sb.balance_from.unwrap_or(0));
    let since_on = tick.saturating_sub(anchor);
    let dt = world.resource::<Time>().delta_secs().max(1e-4);
    let com_vel = sb.prev_com.map_or(Vec3::ZERO, |p| (m.com - p) / dt);
    sb.prev_com = Some(m.com);
    let cap = pb::balance_cap(sb.mass);
    let tau = if sb.balance_enabled && sb.balance_on {
        pb::balance_torque(sb.mass, sb.h, m.com.to_array(), com_vel.to_array(), &m.poly, pb::balance_gains(OMEGA, ZETA), since_on, cap)
    } else { [0.0; 3] };
    // each planted ankle can only push while the pressure point stays on its sole (phys_ragdoll_fix::ankle_cap)
    let n = m.planted.iter().filter(|p| **p).count().max(1) as f32;
    let stance: Vec<(u16, Side, f32)> = [(0u16, Side::Left), (1u16, Side::Right)].into_iter()
        .map(|(b, s)| (b, s, fix::ankle_cap(sb.mass / n, m.sole_half[b as usize]).min(cap)))
        .collect();
    let torques = split_torque(tau, &stance, m.planted);
    // stance-leg gravity hold (phys_stance): each planted leg's hip, knee and ankle carry its share of body weight.
    // Pairs are (child, parent, torque on child); the parent gets the opposite, so the motors stay internal.
    let ramp = pb::ramp(since_on);
    let feet: Vec<([f32; 3], bool)> = (0..2).map(|s| (m.contact_pt[s].to_array(), m.planted[s])).collect();
    let shares = st::weight_shares(m.com.to_array(), &feet);
    let mut pairs: Vec<(Entity, Entity, Vec3)> = Vec::new();
    let mut knee_hold = 0.0f32;
    for s in 0..2 {
        if !m.planted[s] || !sb.balance_on { continue; }
        let (Some((hip, thigh)), Some(knee), Some(ankle), Some(pelvis), Some(calf), Some(foot)) = (m.hips[s], m.knees[s], m.ankles[s], m.pelvis, m.calves[s], m.feet[s]) else { continue };
        let hold = st::leg_hold(&[hip.to_array(), knee.to_array(), ankle.to_array()], m.contact_pt[s].to_array(), shares[s], sb.mass);
        let hold = hold.map(|h| Vec3::from_array(h) * ramp);
        knee_hold += hold[1].length();
        // the balance torque turns the shin (body above the ankle), so on the foot (the child) it enters negated
        let ankle_total = hold[2] - Vec3::from_array(torques.get(s).map_or([0.0; 3], |t| t.1));
        pairs.push((thigh, pelvis, hold[0].clamp_length_max(cap)));
        pairs.push((calf, thigh, hold[1].clamp_length_max(cap)));
        pairs.push((foot, calf, ankle_total.clamp_length_max(cap)));
    }
    // a lifted foot still gets its balance share (zero by split_torque), so nothing else to add
    sb.knee_hold = knee_hold;
    let mut q = world.query::<Forces>();
    for (child, parent, t) in pairs {
        if t == Vec3::ZERO { continue; }
        if let Ok(mut f) = q.get_mut(world, child) { f.apply_torque(t); }
        if let Ok(mut f) = q.get_mut(world, parent) { f.apply_torque(-t); }
    }
    let peak = pb::saturation(&torques).iter().fold(0.0f32, |a, s| a.max(s.1));
    let tau_mag = Vec3::from_array(tau).length();
    {
        let mut s = world.resource_mut::<SandboxShown>();
        let mut v = s.0.take().unwrap_or_default();
        v.peak_sat = peak;
        v.tau = tau_mag;
        v.ramp = pb::ramp(since_on);
        v.knee_hold = sb.knee_hold;
        v.balance_on = sb.balance_on;
        v.contact_flips = sb.contact_flips;
        s.0 = Some(v);
    }
    world.insert_resource(sb);
}

/// After the physics step: the final pose gives COM, margin and the hinge check; every 120 ticks a metrics line.
pub fn report(world: &mut World) {
    let Some(sb) = world.get_resource::<Sandbox>() else { return };
    let (tick, on, mass, h) = (sb.tick.saturating_sub(1), sb.on_tick, sb.mass, sb.h);
    let contact = [0, 1].map(|s| sb.contact[s].is_some_and(|c| c.planted));
    let body = world.resource::<crate::possession::PhysicalBody>();
    let Some(doll) = body.doll() else {
        if let Some(v) = world.resource_mut::<SandboxShown>().0.as_mut() { v.active = false; }
        return;
    };
    let Some(on) = on else { return };
    let m = measure(world, doll, contact);
    let margin = pb::stability_margin(&m.poly, [m.com.x, m.com.z]);
    let hinge = hinge_axis_error(world, doll);
    let since_on = tick.saturating_sub(on);
    let mut shown = world.resource::<SandboxShown>().get();
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
            "{{\"tick\":{since_on},\"ramp\":{:.3},\"com\":[{:.4},{:.4},{:.4}],\"margin\":{margin_s},\"peak_sat\":{:.3},\"max_hinge_axis_error_rad\":{hinge:.5},\"feet_down\":{},\"knee_hold\":{:.1},\"contact_flips\":{},\"balance_on\":{},\"mass\":{mass:.2},\"h\":{h:.3}}}",
            pb::ramp(since_on), shown.com[0], shown.com[1], shown.com[2], shown.peak_sat, shown.feet_down, shown.knee_hold, shown.contact_flips, shown.balance_on
        );
        shown.error = append_metrics(&line).err().map(|e| e.to_string());
    }
    world.resource_mut::<SandboxShown>().0 = Some(shown);
}

fn append_metrics(line: &str) -> std::io::Result<()> {
    let dir = format!("{}/{METRICS_DIR}", crate::ROOT);
    std::fs::create_dir_all(&dir)?;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(format!("{dir}/metrics.jsonl"))?;
    writeln!(f, "{line}")
}

/// Studio Physics tab: "Balance" section (HUD theme). Common readouts first, the rest under Advanced.
/// The toggle is the existing one (BALANCE_REQUEST -> SetBalance -> BalanceSet in the log); not duplicated.
pub fn physics_section(ui: &mut egui::Ui, shown: &SandboxShown) {
    use crate::ui_theme as t;
    t::panel(ui, "Balance", |ui| {
        let Some(s) = shown.0.clone() else { t::hint(ui, "Balance starts when the physics body is on."); return };
        if !s.active { t::hint(ui, "Physics body is off. Turn on \"Physics body\" below to start balance."); return; }
        let mut on = s.balance_on;
        if t::checkbox(ui, &mut on, "Balance").on_hover_text("Balance torque and stance-leg hold. Off = passive ragdoll; metrics keep writing. Turning it on restarts the 0.5 s ramp.  [B]").changed()
            && let Ok(mut r) = BALANCE_REQUEST.lock() { *r = Some(on); }
        let (mtxt, mcol) = if !s.margin.is_finite() { ("no feet down".to_string(), t::DANGER) }
            else { (format!("{:+.1} cm", s.margin * 100.0), if s.margin > 0.03 { t::GOOD } else if s.margin >= 0.0 { t::SELECTION } else { t::DANGER }) };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Stability margin").size(12.0).color(t::MUTED));
            t::chip(ui, &mtxt, mcol);
        });
        t::stat(ui, "Balance ramp", format!("{:.2}", s.ramp), if s.ramp >= 1.0 { t::GOOD } else { t::SELECTION });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Ankle torque").size(12.0).color(t::MUTED));
            let col = if s.peak_sat > 0.9 { t::DANGER } else if s.peak_sat > 0.7 { t::SELECTION } else { t::CYAN };
            ui.add(egui::ProgressBar::new(s.peak_sat.clamp(0.0, 1.0)).desired_width(120.0).fill(col)
                .text(egui::RichText::new(format!("{:.1} N m  {:.0}%", s.tau, s.peak_sat * 100.0)).font(t::number_font(10.5))));
        });
        t::stat(ui, "Feet on the ground", format!("{}", s.feet_down), t::PAPER);
        t::advanced(ui, "sandbox_bal_adv", "Advanced", |ui| {
            t::stat(ui, "Body", format!("{:.1} kg, COM {:.3} m above ankles", s.mass, s.h), t::PAPER);
            t::stat(ui, "Ticks since ragdoll on", format!("{}", s.tick), t::PAPER);
            t::stat(ui, "COM (stage, m)", format!("{:.3}  {:.3}  {:.3}", s.com[0], s.com[1], s.com[2]), t::PAPER);
            t::stat(ui, "Knee hold", format!("{:.1} N m", s.knee_hold), t::PAPER);
            t::stat(ui, "Contact flips", format!("{}", s.contact_flips), t::PAPER);
            t::stat(ui, "Hinge axis error", format!("{:.4} rad", s.hinge_err), if s.hinge_err < 0.05 { t::GOOD } else { t::SELECTION });
            t::hint(ui, "Every 120 ticks a line goes to target/live/metrics.jsonl.");
        });
        if let Some(e) = &s.error { t::stat(ui, "Metrics file", e.clone(), t::DANGER); }
    });
}
