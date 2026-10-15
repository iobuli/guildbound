//! Studio motion trails (display only: gizmos, never the event log).
//! Extends: crates/game/src/studio.rs (toolbar toggle; options in the Pose and Physics tabs) and
//! studio_kit.rs (pure ring buffers + helpers in studio_kit_core.rs). Pose-order slot: reads the FINAL
//! pose after physics (slot 6, Avian/ragdoll) each fixed tick.
//! Look (animator motion arcs): solid fading Actual trail; dashed thinner Target ghost at ~50% alpha;
//! spacing dots every 4 ticks; divergence connectors every 8 ticks (feet planted: 1/3 cm, others 2/8 cm);
//! key diamonds on the target; flat squares where a foot plants/lifts (phys_stance Contact, same as
//! balance); peak-speed tick on hand arcs; brighter+thicker inside anim_flow_cut strike windows;
//! thick cyan COM trail + faint floor projection; optional onion-skin stick figures every 15 ticks.
//! Keys: T trails, G ghost, O onion. Default set: hands, feet, head + COM. Cap 600 samples (5 s).
//! STAGED, UNCOMPILED (no Bevy/egui on the box). Pure parts are tested in studio_kit_core.rs.
use bevy::prelude::*;
use bevy_egui::egui;
use crate::studio_kit::core::*;
use crate::ui_theme as t;
use guildbound_sim::anim_flow_cut::{inside_strike_window, StrikePhases};

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct TrailGizmos;
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct TrailThickGizmos;

#[derive(Resource)]
pub struct Trails {
    pub on: bool,
    pub ghost: bool,
    pub onion: bool,
    /// Samples drawn per bone (30..=600); storage is always the 600 cap.
    pub n: usize,
    pub set: TrailSet,
    pub color: TrailColor,
    pub freeze_on_pause: bool,
    pub data: TrailData,
    /// Per bone (rig.bones order): name, parent bone index, driver colour index.
    pub names: Vec<String>,
    pub parents: Vec<Option<u16>>,
    pub drivers: Vec<usize>,
    /// Current clip's strike phases and the fixed tick the clip started at (set by Animation/possession).
    /// TODO(verify): who owns StrikePhases for the playing clip; until set, no strike highlight.
    pub strike: Option<(StrikePhases, u64)>,
    rig_for: Option<Entity>,
}
impl Default for Trails {
    fn default() -> Self {
        Trails { on: true, ghost: true, onion: false, n: TRAIL_DEFAULT, set: TrailSet::Preset, color: TrailColor::Speed,
                 freeze_on_pause: true, data: TrailData::default(), names: Vec::new(), parents: Vec::new(), drivers: Vec::new(), strike: None, rig_for: None }
    }
}
impl Trails {
    /// Animation's entry point (also used by `push_targets` below).
    pub fn push_target(&mut self, bone: u16, tick: u64, pos: [f32; 3]) { self.data.push_target(bone, tick, pos); }
    fn shown(&self, bone: usize, selected: Option<&str>) -> bool {
        let name = self.names.get(bone).map(String::as_str).unwrap_or("");
        match self.set { TrailSet::All => true, TrailSet::Selected => selected == Some(name), TrailSet::Preset => in_preset(name) }
    }
    fn in_strike(&self, tick: u64) -> bool {
        self.strike.is_some_and(|(ph, start)| tick >= start && inside_strike_window(ph, tick - start))
    }
}

/// What Animation fills so the game can call `anim_trail_target::trail_targets` each fixed tick.
/// TODO(verify): Animation owns filling this from anim_interp (pose per skin joint at the current tick).
#[derive(Resource, Default)]
pub struct TrailTargetSource {
    pub rig: Option<guildbound_sim::anim_rig::Rig>,
    /// Local rotation per skin joint for this tick (None = rest).
    pub pose: Vec<Option<guildbound_sim::anim_rig::Q>>,
    /// Bones to evaluate (skin-joint indices = BoneIdx order = rig.bones order). TODO(verify) same order.
    pub bones: Vec<u16>,
    /// Sorted clip key ticks (absolute fixed ticks).
    pub key_ticks: Vec<u64>,
    /// Model space -> world (the stage model's root transform).
    pub model_to_world: Transform,
}

/// Fixed tick counter shared with the status strip.
fn tick_now(kit: &crate::studio_kit::Kit) -> u64 { kit.fixed_tick }

pub fn plugin(app: &mut App) {
    app.init_resource::<Trails>()
        .init_resource::<TrailTargetSource>()
        .init_gizmo_group::<TrailGizmos>()
        .init_gizmo_group::<TrailThickGizmos>()
        .add_systems(Startup, |mut store: ResMut<GizmoConfigStore>| {
            store.config_mut::<TrailGizmos>().0.line.width = 2.0;
            store.config_mut::<TrailThickGizmos>().0.line.width = 4.5;
            store.config_mut::<TrailGizmos>().0.depth_bias = -0.5;
            store.config_mut::<TrailThickGizmos>().0.depth_bias = -0.5;
        })
        // TODO(verify): Avian in FixedUpdate -> PhysicsSystems::Last exists in FixedPostUpdate or FixedUpdate;
        // run after it so samples are the final post-physics pose.
        .add_systems(FixedPostUpdate, (sample_actual, push_targets).after(avian3d::prelude::PhysicsSystems::Last))
        .add_systems(Update, (keys, draw));
}

fn keys(keys: Res<ButtonInput<KeyCode>>, studio: Res<crate::studio::Studio>, mut tr: ResMut<Trails>, mut ctx: bevy_egui::EguiContexts) {
    if !studio.open || ctx.ctx_mut().map(|c| c.wants_keyboard_input()).unwrap_or(false) { return; }
    if keys.just_pressed(KeyCode::KeyT) { tr.on = !tr.on; }
    if keys.just_pressed(KeyCode::KeyG) { tr.ghost = !tr.ghost; }
    if keys.just_pressed(KeyCode::KeyO) { tr.onion = !tr.onion; }
}

/// Actual samples for every bone (so onion-skin can draw whole poses) + COM, after physics.
#[allow(clippy::too_many_arguments)]
fn sample_actual(
    studio: Res<crate::studio::Studio>,
    kit: Res<crate::studio_kit::Kit>,
    sim: Res<crate::sim_bridge::Sim>,
    sandbox: Res<crate::sandbox::Sandbox>,
    mut tr: ResMut<Trails>,
    rigs: Query<&crate::studio::StudioRig>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    xf: Query<&GlobalTransform>,
) {
    if !studio.open { return; }
    if tr.freeze_on_pause && sim.paused { return; }
    let Some(rig) = studio.entity.and_then(|e| rigs.get(e).ok()) else { return };
    if tr.rig_for != studio.entity {
        tr.rig_for = studio.entity;
        tr.data.clear();
        tr.names = rig.bones.iter().map(|&b| names.get(b).map(|n| n.as_str().to_owned()).unwrap_or_default()).collect();
        tr.parents = rig.bones.iter().map(|&b| parents.get(b).ok().and_then(|p| rig.bones.iter().position(|&x| x == p.parent())).map(|i| i as u16)).collect();
        tr.drivers = rig.bones.iter().enumerate().map(|(i, _)| kit.bones.get(i).map_or(0, |r| r.driver)).collect();
    }
    let tick = tick_now(&kit);
    let mut sum = Vec3::ZERO;
    let mut n = 0.0;
    for (i, &b) in rig.bones.iter().enumerate() {
        let Ok(g) = xf.get(b) else { continue };
        let p = g.translation();
        sum += p; n += 1.0;
        let name = &tr.names[i];
        // Same Contact the balance uses (sandbox.contact = phys_stance::Contact with hysteresis).
        let planted = if is_foot(name) {
            let side = foot_side(name);
            side.and_then(|s| sandbox.contact[s].as_ref().map(|c| c.planted))
        } else { None };
        tr.data.push_actual(i as u16, tick, p.to_array(), planted);
    }
    // COM: the sandbox's mass-weighted COM when the physics body is on, else the mean bone position.
    let shown = crate::sandbox::SHOWN.lock().ok().and_then(|s| s.clone());
    let com = match shown { Some(s) if s.active => Vec3::from_array(s.com), _ if n > 0.0 => sum / n, _ => return };
    tr.data.push_com(tick, com.to_array());
}

/// L/R from the rig name (B_L_Foot, LeftFoot, foot.l). TODO(verify) sandbox.contact index 0 = left.
fn foot_side(name: &str) -> Option<usize> {
    let n = name.to_ascii_lowercase();
    if n.contains("_l_") || n.contains("left") || n.ends_with(".l") { Some(0) } else if n.contains("_r_") || n.contains("right") || n.ends_with(".r") { Some(1) } else { None }
}

/// Target ghost samples from Animation's pure sim `anim_trail_target::trail_targets`.
fn push_targets(studio: Res<crate::studio::Studio>, kit: Res<crate::studio_kit::Kit>, src: Res<TrailTargetSource>, mut tr: ResMut<Trails>) {
    if !studio.open || !tr.ghost { return; }
    let Some(rig) = src.rig.as_ref() else { return };
    let tick = tick_now(&kit);
    let pose = |j: usize| src.pose.get(j).copied().flatten();
    for (bone, tk, pos, is_key) in guildbound_sim::anim_trail_target::trail_targets(rig, tick, &pose, &src.bones, &src.key_ticks) {
        let w = src.model_to_world.transform_point(Vec3::from_array(pos));
        tr.push_target(bone, tk, w.to_array());
        if is_key {
            let slot = bone as usize;
            if let Some(bt) = tr.data.bones.get_mut(slot) && bt.keys.last() != Some(&tk) {
                bt.keys.push(tk);
                let extra = bt.keys.len().saturating_sub(TRAIL_CAP); bt.keys.drain(..extra);
            }
        }
    }
}

fn col(rgb: [f32; 3], a: f32) -> Color { Color::srgba(rgb[0], rgb[1], rgb[2], a.clamp(0.0, 1.0)) }
fn c32(c: egui::Color32) -> [f32; 3] { [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0] }
fn v(p: [f32; 3]) -> Vec3 { Vec3::from_array(p) }

fn draw(studio: Res<crate::studio::Studio>, tr: Res<Trails>, mut g: Gizmos<TrailGizmos>, mut gt: Gizmos<TrailThickGizmos>) {
    if !studio.open || !tr.on { return; }
    let sel = studio.selected.as_deref();
    let shown: Vec<usize> = (0..tr.data.bones.len()).filter(|&i| tr.shown(i, sel)).collect();
    let stride = if tr.set == TrailSet::All { stride_for(shown.len(), tr.n, SEGMENT_BUDGET) } else { 1 };
    let cyan = c32(t::CYAN);
    for &i in &shown {
        let bt = &tr.data.bones[i];
        let name = tr.names.get(i).map(String::as_str).unwrap_or("");
        let hue = match tr.color { TrailColor::Driver => c32(t::DRIVER_COLORS[tr.drivers.get(i).copied().unwrap_or(0).min(3)]), _ => cyan };
        let act = bt.actual.last(tr.n);
        let len = act.len();
        // Actual: solid, fading with age; speed colour per segment; strike windows brighter + thicker.
        let mut k = stride;
        while k < len {
            let (a, b) = (act[k - stride], act[k]);
            let alpha = trail_alpha(k, len);
            let rgb = if tr.color == TrailColor::Speed {
                let dt = (b.tick - a.tick).max(1) as f32 / 120.0;
                speed_rgb(v(a.pos).distance(v(b.pos)) / dt, 2.0)
            } else { hue };
            if tr.in_strike(b.tick) { gt.line(v(a.pos), v(b.pos), col(rgb.map(|c| (c * 1.25).min(1.0)), alpha.max(0.6))); }
            else { g.line(v(a.pos), v(b.pos), col(rgb, alpha)); }
            k += stride;
        }
        // Spacing dots every 4 ticks (wide = fast, bunched = slowing).
        for (j, s) in act.iter().enumerate().filter(|(_, s)| is_dot_tick(s.tick)) {
            g.sphere(Isometry3d::from_translation(v(s.pos)), 0.006, col(hue, trail_alpha(j, len)));
        }
        // Peak-speed tick on hand arcs.
        if name.to_ascii_lowercase().contains("hand") && let Some((pi, _)) = peak_speed_index(&act) {
            let p = v(act[pi].pos);
            gt.line(p - Vec3::Y * 0.02, p + Vec3::Y * 0.02, col(c32(t::SELECTION), 1.0));
        }
        // Contact marks: flat squares where the foot plants (green) / lifts (amber).
        for (p, planted) in contact_marks(&act) {
            let c = if planted { c32(t::GOOD) } else { c32(t::SELECTION) };
            g.rect(Isometry3d::new(v(p), Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)), Vec2::splat(0.05), col(c, 0.9));
        }
        if tr.ghost {
            // Target ghost: dashed, thinner (half-alpha, same hue).
            let tg = bt.target.last(tr.n);
            for j in 1..tg.len() {
                if dash_on(j) { g.line(v(tg[j - 1].pos), v(tg[j].pos), col(hue, 0.5 * trail_alpha(j, tg.len()))); }
            }
            // Key-pose diamonds.
            for p in key_markers(bt, tr.n) {
                let p = v(p); let d = 0.015;
                let c = col(hue, 0.8);
                g.line(p + Vec3::Y * d, p + Vec3::X * d, c); g.line(p + Vec3::X * d, p - Vec3::Y * d, c);
                g.line(p - Vec3::Y * d, p - Vec3::X * d, c); g.line(p - Vec3::X * d, p + Vec3::Y * d, c);
            }
            // Divergence connectors every 8 ticks, coloured by error (planted feet stricter).
            for (tp, ap, err, planted) in divergence(bt, tr.n) {
                let c = match err_class_for(err, planted && is_foot(name)) { ErrClass::Good => t::GOOD, ErrClass::Warn => t::SELECTION, ErrClass::Bad => t::DANGER };
                g.line(v(tp), v(ap), col(c32(c), 0.9));
            }
        }
    }
    // COM: thick cyan + faint ground projection (stage floor).
    let com = tr.data.com.last(tr.n);
    let floor = crate::studio::STAGE.y; // TODO(verify): stage floor height
    for j in 1..com.len() {
        let (a, b) = (v(com[j - 1].pos), v(com[j].pos));
        gt.line(a, b, col(cyan, trail_alpha(j, com.len())));
        g.line(Vec3::new(a.x, floor + 0.002, a.z), Vec3::new(b.x, floor + 0.002, b.z), col(cyan, 0.18));
    }
    // Onion-skin: faint stick figures every 15 ticks along the shown window.
    if tr.onion {
        for s in com.iter().filter(|s| is_onion_tick(s.tick)) {
            for (i, bt) in tr.data.bones.iter().enumerate() {
                let Some(Some(p)) = tr.parents.get(i) else { continue };
                let (Some(a), Some(b)) = (bt.actual.at_tick(s.tick), tr.data.bones.get(*p as usize).and_then(|pb| pb.actual.at_tick(s.tick))) else { continue };
                g.line(v(a.pos), v(b.pos), col(c32(t::MUTED), 0.22));
            }
        }
    }
}

/// Toolbar toggle (top bar).
pub fn toolbar_toggle(ui: &mut egui::Ui, tr: &mut Trails) {
    if t::button(ui, "Trails", tr.on).on_hover_text("Motion trails  [T]").clicked() { tr.on = !tr.on; }
}

/// Options block for the Pose and Physics tabs. Common first, advanced collapsed.
pub fn options(ui: &mut egui::Ui, tr: &mut Trails) {
    t::panel(ui, "Motion trails", |ui| {
        ui.horizontal(|ui| {
            t::checkbox(ui, &mut tr.on, "Trails").on_hover_text("Show motion trails  [T]");
            t::checkbox(ui, &mut tr.ghost, "Target ghost").on_hover_text("Dashed animation target trail  [G]");
            t::checkbox(ui, &mut tr.onion, "Onion skin").on_hover_text("Faint whole-body poses every 15 ticks  [O]");
        });
        ui.horizontal(|ui| {
            for (s, l, tip) in [(TrailSet::Preset, "Hands+feet+head", "Default set"), (TrailSet::Selected, "Selected", "Only the selected bone"), (TrailSet::All, "All bones", "Every bone (capped at ~10k segments)")] {
                if t::button(ui, l, tr.set == s).on_hover_text(tip).clicked() { tr.set = s; }
            }
        });
        let mut n = tr.n as f32;
        if t::slider(ui, "Length (ticks)", &mut n, TRAIL_RANGE.0 as f32..=TRAIL_RANGE.1 as f32).changed() { tr.n = (n.round() as usize).clamp(TRAIL_RANGE.0, TRAIL_RANGE.1); }
        t::advanced(ui, "trail_adv", "Advanced", |ui| {
            ui.horizontal(|ui| {
                for (c, l) in [(TrailColor::Speed, "By speed"), (TrailColor::Driver, "By driver"), (TrailColor::Plain, "Plain")] {
                    if t::button(ui, l, tr.color == c).clicked() { tr.color = c; }
                }
            });
            t::checkbox(ui, &mut tr.freeze_on_pause, "Freeze on pause").on_hover_text("Stop sampling while the sim is paused  [Space]");
            if t::button(ui, "Clear trails", false).clicked() { tr.data.clear(); }
            t::hint(ui, "Dots every 4 ticks: wide = fast, bunched = easing. Connectors every 8 ticks: green/amber/red error (feet 1/3 cm, hands 2/8 cm).");
        });
    });
}
