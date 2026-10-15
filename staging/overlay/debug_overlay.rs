//! Ragdoll debug overlay for Guildbound (Bevy 0.19.1, Avian 0.7.0). Self-contained: no game imports.
//!
//! Draws with Gizmos, on by default (F3 toggles the whole overlay):
//!   F4 contact   - per-foot sole box: green = on, red = off. Hysteresis on < 1 cm, off > 2.5 cm,
//!                  measured RELATIVE to the settled sole rest height (learned while the foot is still).
//!   F5 com       - whole-body COM sphere + vertical drop line to the ground + ground cross.
//!   F6 support   - support polygon (convex hull of the sole corners of planted feet) + its centre.
//!   F7 balance   - balance target (support centre) and error vector COM_xz - target, plus torque want.
//!   F8 torques   - one line per joint along tau, length = |tau| * TORQUE_SCALE, colour by saturation
//!                  |tau| / limit (green 0 -> yellow 0.5 -> red >= 1).
//!   F9 text      - small text panel (tick, COM, contact, peak saturation).
//!   F10 voxel    - merged voxel collision boxes (wireframe) + debris mass/COM markers.
//!   F11 labels   - floating label above every rig: model/rig name + profile, feet, saturation, locomotion.
//! Multi-rig: the source pushes one `RigDebug` per powered ragdoll (hero, mannequin, NPC ...), so it
//! works in live gameplay, not only the sandbox. COM layer also draws the capture point (cyan).
//!
//! Hooking: implement `RagdollDebugSource` for any zero-sized marker type; its `fill` reads the world
//! and writes a `RagdollDebugFrame`. Then `app.add_plugins(RagdollDebugOverlayPlugin::<MySource>::default())`.
//! Everything is drawn in PostUpdate, after transform propagation and after `PhysicsSystems::Last`.
use avian3d::prelude::PhysicsSystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use std::marker::PhantomData;

pub const CONTACT_ON_M: f32 = 0.01;
pub const CONTACT_OFF_M: f32 = 0.025;
/// metres of line per N*m of joint torque
pub const TORQUE_SCALE: f32 = 0.004;

/// One foot as the host sees it this frame.
#[derive(Clone, Debug, Default)]
pub struct FootSample {
    /// World-space sole corners (any count >= 1; 4 for a box sole).
    pub sole_corners: Vec<Vec3>,
    /// Lowest sole point height above the ground (m), raw (may be negative when sunk in).
    pub sole_height: f32,
    /// Foot body linear speed (m/s), used to decide when the rest height is settled.
    pub speed: f32,
    /// The host's own planted factor c in [0, 1] if it has one (shown in text only).
    pub host_c: Option<f32>,
}

/// One driven joint.
#[derive(Clone, Debug, Default)]
pub struct JointSample { pub anchor: Vec3, pub tau: Vec3, pub limit: f32 }

/// One powered-ragdoll rig (hero, mannequin, NPC, any rig profile).
#[derive(Clone, Debug, Default)]
pub struct RigDebug {
    /// Stable key (e.g. root entity bits) so contact/rest state follows the rig.
    pub key: u64,
    /// Floating label: model / rig name and profile.
    pub label: String,
    pub profile: String,
    pub tick: u64,
    /// COM velocity (m/s) for the capture point xi = com_xz + v_xz / sqrt(g / h).
    pub com_vel: Vec3,
    /// Optional locomotion state text (e.g. "walk 1.2 m/s").
    pub locomotion: Option<String>,
    pub ground_y: f32,
    /// World point for the floating label (default com + 1.1 m).
    pub label_anchor: Option<Vec3>,
    pub com: Vec3,
    pub mass: f32,
    /// Index 0 = right/first foot, 1 = left/second (host order, just kept stable).
    pub feet: Vec<FootSample>,
    /// Host balance target on the ground (support centre); None = overlay uses its own polygon centre.
    pub balance_target: Option<Vec3>,
    /// Host balance torque request (N*m), drawn from the COM if present.
    pub balance_torque: Option<Vec3>,
    pub joints: Vec<JointSample>,
    /// Extra text lines from the host.
    pub notes: Vec<String>,
}

/// Everything the overlay draws, refilled by the source each frame.
#[derive(Resource, Clone, Debug, Default)]
pub struct RagdollDebugFrame {
    pub rigs: Vec<RigDebug>,
    /// Voxel layer: world AABBs (min, max) of merged collision boxes (phys_voxel::merge_boxes).
    pub voxel_boxes: Vec<(Vec3, Vec3)>,
    /// Voxel layer: debris pieces (world COM, mass kg).
    pub debris: Vec<(Vec3, f32)>,
    /// Global notes.
    pub notes: Vec<String>,
}

/// Adapter to real game types. `fill` runs as an exclusive read of the world each frame.
pub trait RagdollDebugSource: Send + Sync + 'static {
    fn fill(world: &World, frame: &mut RagdollDebugFrame);
}

/// Per-layer toggles. Defaults: everything on (open and look).
#[derive(Resource, Clone, Debug)]
pub struct OverlayToggles { pub enabled: bool, pub labels: bool, pub voxels: bool, pub contact: bool, pub com: bool, pub support: bool, pub balance: bool, pub torques: bool, pub text: bool }
impl Default for OverlayToggles {
    fn default() -> Self { Self { enabled: true, labels: true, voxels: true, contact: true, com: true, support: true, balance: true, torques: true, text: true } }
}

/// Overlay-side contact state per foot (same hysteresis and 6-tick hold as pass 6 `Contact`).
#[derive(Clone, Copy, Debug)]
pub struct FootState { pub rest: Option<f32>, rest_acc: f32, rest_n: u32, pub on: bool, held: u32, pub rel: f32 }
impl Default for FootState { fn default() -> Self { Self { rest: None, rest_acc: 0.0, rest_n: 0, on: true, held: 6, rel: 0.0 } } }

/// rest_h locks after this many contact ticks (pass6 v5b).
pub const REST_LOCK_TICKS: u32 = 60;

#[derive(Default, Debug, Clone)]
pub struct RigState { pub feet: Vec<FootState>, pub last_tick: u64, pub peak_sat: f32, pub seen: bool }

#[derive(Resource, Default, Debug)]
pub struct OverlayState { pub rigs: std::collections::HashMap<u64, RigState> }

#[derive(Component)]
struct RigLabel(u64);

#[derive(Component)]
struct OverlayText;

pub struct RagdollDebugOverlayPlugin<S: RagdollDebugSource>(PhantomData<S>);
impl<S: RagdollDebugSource> Default for RagdollDebugOverlayPlugin<S> { fn default() -> Self { Self(PhantomData) } }

impl<S: RagdollDebugSource> Plugin for RagdollDebugOverlayPlugin<S> {
    fn build(&self, app: &mut App) {
        app.init_resource::<RagdollDebugFrame>()
            .init_resource::<OverlayToggles>()
            .init_resource::<OverlayState>()
            .add_systems(Startup, spawn_text)
            .add_systems(Update, keys)
            .add_systems(
                PostUpdate,
                (collect::<S>, update_contact, draw, update_text, update_labels)
                    .chain()
                    .after(TransformSystems::Propagate)
                    .after(PhysicsSystems::Last),
            );
    }
}

fn collect<S: RagdollDebugSource>(world: &mut World) {
    let mut frame = world.remove_resource::<RagdollDebugFrame>().unwrap_or_default();
    frame.rigs.clear();
    frame.voxel_boxes.clear();
    frame.debris.clear();
    frame.notes.clear();
    S::fill(world, &mut frame);
    world.insert_resource(frame);
}

fn keys(k: Res<ButtonInput<KeyCode>>, mut t: ResMut<OverlayToggles>) {
    if k.just_pressed(KeyCode::F3) { t.enabled = !t.enabled; }
    if k.just_pressed(KeyCode::F4) { t.contact = !t.contact; }
    if k.just_pressed(KeyCode::F5) { t.com = !t.com; }
    if k.just_pressed(KeyCode::F6) { t.support = !t.support; }
    if k.just_pressed(KeyCode::F7) { t.balance = !t.balance; }
    if k.just_pressed(KeyCode::F8) { t.torques = !t.torques; }
    if k.just_pressed(KeyCode::F9) { t.text = !t.text; }
    if k.just_pressed(KeyCode::F10) { t.voxels = !t.voxels; }
    if k.just_pressed(KeyCode::F11) { t.labels = !t.labels; }
}

/// rest_h = mean sole height over the first 60 planted ticks, then LOCKED (pass6 v5b).
/// Contact uses rel = sole - rest_h, so a foot resting 0.8 cm into the floor reads 0, not -0.8 cm.
fn update_contact(frame: Res<RagdollDebugFrame>, mut st: ResMut<OverlayState>) {
    for r in st.rigs.values_mut() { r.seen = false; }
    for rig in &frame.rigs {
        let rs = st.rigs.entry(rig.key).or_default();
        rs.seen = true;
        if rs.feet.len() != rig.feet.len() { rs.feet = vec![FootState::default(); rig.feet.len()]; }
        if rig.tick == rs.last_tick && rig.tick != 0 { continue; } // one contact step per sim tick
        rs.last_tick = rig.tick;
        for (fs, f) in rs.feet.iter_mut().zip(rig.feet.iter()) {
            if fs.rest.is_none() && fs.on && f.sole_height < CONTACT_OFF_M {
                fs.rest_acc += f.sole_height; fs.rest_n += 1;
                if fs.rest_n >= REST_LOCK_TICKS { fs.rest = Some(fs.rest_acc / fs.rest_n as f32); }
            }
            let rest = fs.rest.unwrap_or(fs.rest_acc / fs.rest_n.max(1) as f32).min(0.0);
            fs.rel = f.sole_height - rest;
            fs.held = fs.held.saturating_add(1);
            let want = if fs.on { fs.rel <= CONTACT_OFF_M } else { fs.rel < CONTACT_ON_M };
            if want != fs.on && fs.held >= 6 { fs.on = want; fs.held = 0; }
        }
    }
    st.rigs.retain(|_, r| r.seen);
}

fn hull(mut p: Vec<Vec2>) -> Vec<Vec2> {
    p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    p.dedup();
    if p.len() < 3 { return p; }
    let cr = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut h: Vec<Vec2> = Vec::new();
    for pass in 0..2 {
        let start = h.len();
        let it: Box<dyn Iterator<Item = &Vec2>> = if pass == 0 { Box::new(p.iter()) } else { Box::new(p.iter().rev()) };
        for &q in it {
            while h.len() >= start + 2 && cr(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 { h.pop(); }
            h.push(q);
        }
        h.pop();
    }
    h
}

fn sat_color(s: f32) -> Color {
    let s = s.clamp(0.0, 1.0);
    if s < 0.5 { Color::srgb(s * 2.0, 1.0, 0.0) } else { Color::srgb(1.0, 2.0 - s * 2.0, 0.0) }
}

fn draw(mut g: Gizmos, frame: Res<RagdollDebugFrame>, t: Res<OverlayToggles>, mut st: ResMut<OverlayState>) {
    if !t.enabled { return; }
    if t.voxels {
        for (mn, mx) in &frame.voxel_boxes {
            g.cube(Transform::from_translation((*mn + *mx) * 0.5).with_scale(*mx - *mn), Color::srgb(0.3, 0.9, 0.9));
        }
        for (c, m) in &frame.debris {
            let col = Color::srgb(1.0, 0.6, 0.1);
            g.sphere(*c, 0.03 + 0.01 * m.max(0.0).cbrt(), col);
            g.cross(*c, 0.06, col);
        }
    }
    for rig in &frame.rigs {
        let Some(rs) = st.rigs.get_mut(&rig.key) else { continue };
        draw_rig(&mut g, rig, rs, &t);
    }
}

fn draw_rig(g: &mut Gizmos, rig: &RigDebug, rs: &mut RigState, t: &OverlayToggles) {
    let gy = rig.ground_y + 0.002;
    let mut pts = Vec::new();
    for (i, f) in rig.feet.iter().enumerate() {
        if rs.feet.get(i).is_some_and(|s| s.on) { pts.extend(f.sole_corners.iter().map(|c| Vec2::new(c.x, c.z))); }
    }
    let poly = hull(pts);
    let centre = if poly.is_empty() { None } else { Some(poly.iter().copied().sum::<Vec2>() / poly.len() as f32) };
    if t.contact {
        for (i, f) in rig.feet.iter().enumerate() {
            let on = rs.feet.get(i).is_some_and(|s| s.on);
            let col = if on { Color::srgb(0.1, 1.0, 0.2) } else { Color::srgb(1.0, 0.15, 0.1) };
            if f.sole_corners.len() >= 2 {
                let h = hull(f.sole_corners.iter().map(|c| Vec2::new(c.x, c.z)).collect());
                let y = f.sole_corners.iter().map(|c| c.y).fold(f32::MAX, f32::min).max(gy);
                let mut ring: Vec<Vec3> = h.iter().map(|p| Vec3::new(p.x, y, p.y)).collect();
                if let Some(&first) = ring.first() { ring.push(first); }
                g.linestrip(ring, col);
            }
            for c in &f.sole_corners { g.sphere(*c, 0.012, col); }
        }
    }
    if t.support && poly.len() >= 2 {
        let mut ring: Vec<Vec3> = poly.iter().map(|p| Vec3::new(p.x, gy + 0.001, p.y)).collect();
        ring.push(ring[0]);
        g.linestrip(ring, Color::srgb(0.2, 0.6, 1.0));
        if let Some(c) = centre { g.cross(Vec3::new(c.x, gy, c.y), 0.04, Color::srgb(0.2, 0.6, 1.0)); }
    }
    let com_g = Vec3::new(rig.com.x, gy, rig.com.z);
    if t.com {
        let col = Color::srgb(1.0, 0.9, 0.1);
        g.sphere(rig.com, 0.04, col);
        g.line(rig.com, com_g, col);
        g.cross(com_g, 0.05, col);
        // capture point xi = x + v / w0, w0 = sqrt(g / h)
        let h = (rig.com.y - rig.ground_y).max(0.3);
        let xi = com_g + rig.com_vel.with_y(0.0) / (9.81 / h).sqrt();
        let cpc = Color::srgb(0.0, 1.0, 1.0);
        g.sphere(xi, 0.02, cpc);
        g.line(com_g, xi, cpc);
    }
    if t.balance {
        let target = rig.balance_target.map(|v| Vec3::new(v.x, gy, v.z)).or(centre.map(|c| Vec3::new(c.x, gy, c.y)));
        if let Some(tg) = target {
            g.sphere(tg, 0.025, Color::srgb(1.0, 0.0, 1.0));
            if (com_g - tg).length() > 1e-4 { g.arrow(tg, com_g, Color::srgb(1.0, 0.0, 1.0)); }
        }
        if let Some(w) = rig.balance_torque { g.arrow(rig.com, rig.com + w * TORQUE_SCALE, Color::srgb(0.9, 0.4, 1.0)); }
    }
    let mut peak = 0.0f32;
    for j in &rig.joints {
        let s = j.tau.length() / j.limit.max(1.0);
        peak = peak.max(s);
        if t.torques && j.tau.length() > 1e-3 {
            g.line(j.anchor, j.anchor + j.tau * TORQUE_SCALE, sat_color(s));
            g.sphere(j.anchor, 0.008, sat_color(s));
        }
    }
    rs.peak_sat = rs.peak_sat.max(peak);
}

/// Floating label above each rig (UI text placed at the projected head point).
fn update_labels(
    mut cmd: Commands, frame: Res<RagdollDebugFrame>, t: Res<OverlayToggles>, st: Res<OverlayState>,
    cams: Query<(&Camera, &GlobalTransform)>, mut q: Query<(Entity, &RigLabel, &mut Text, &mut Node, &mut Visibility)>,
) {
    let cam = cams.iter().find(|(c, _)| c.is_active);
    let mut have = std::collections::HashSet::new();
    for (e, l, mut txt, mut node, mut vis) in &mut q {
        let Some(rig) = frame.rigs.iter().find(|r| r.key == l.0) else { cmd.entity(e).despawn(); continue };
        have.insert(l.0);
        let head = rig.label_anchor.unwrap_or(rig.com + Vec3::Y * 1.1);
        let pos = cam.and_then(|(c, gt)| c.world_to_viewport(gt, head).ok());
        match pos {
            Some(p) if t.enabled && t.labels => {
                *vis = Visibility::Inherited;
                node.left = Val::Px(p.x - 60.0); node.top = Val::Px(p.y - 30.0);
                let rs = st.rigs.get(&rig.key);
                let feet: String = rs.map_or(String::new(), |r| r.feet.iter().map(|f| if f.on { 'O' } else { '-' }).collect());
                txt.0 = format!("{} [{}]\nfeet {}  sat {:.2}{}", rig.label, rig.profile, feet, rs.map_or(0.0, |r| r.peak_sat),
                    rig.locomotion.as_ref().map_or(String::new(), |l| format!("\n{l}")));
            }
            _ => *vis = Visibility::Hidden,
        }
    }
    for rig in &frame.rigs {
        if have.contains(&rig.key) { continue; }
        cmd.spawn((RigLabel(rig.key), Text::new(""), TextFont { font_size: FontSize::Px(13.0), ..default() }, TextColor(Color::srgb(1.0, 1.0, 0.6)),
            Node { position_type: PositionType::Absolute, ..default() }, Visibility::Hidden));
    }
}

fn spawn_text(mut cmd: Commands) {
    cmd.spawn((
        OverlayText,
        Text::new(""),
        TextFont { font_size: FontSize::Px(14.0), ..default() },
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, top: Val::Px(8.0), left: Val::Px(8.0), ..default() },
    ));
}

fn update_text(frame: Res<RagdollDebugFrame>, t: Res<OverlayToggles>, st: Res<OverlayState>, mut q: Query<&mut Text, With<OverlayText>>) {
    let Ok(mut txt) = q.single_mut() else { return };
    if !t.enabled || !t.text { if !txt.0.is_empty() { txt.0.clear(); } return; }
    let mut s = String::from("physics overlay  F3 all  F4 contact  F5 com/capture  F6 support  F7 balance  F8 torque  F9 text  F10 voxel  F11 labels\n");
    if frame.rigs.is_empty() { s.push_str("(no powered ragdoll rigs)\n"); }
    for rig in &frame.rigs {
        s.push_str(&format!("{} [{}] tick {}  mass {:.1} kg  com h {:.3}  v {:.2} m/s\n", rig.label, rig.profile, rig.tick, rig.mass, rig.com.y - rig.ground_y, rig.com_vel.with_y(0.0).length()));
        if let Some(rs) = st.rigs.get(&rig.key) {
            for (i, (f, fs)) in rig.feet.iter().zip(rs.feet.iter()).enumerate() {
                s.push_str(&format!("  foot{i}: {}  sole {:+.1} cm  rest_h {}  rel {:+.1} cm{}\n",
                    if fs.on { "ON " } else { "off" }, f.sole_height * 100.0,
                    fs.rest.map_or("settling".to_string(), |r| format!("{:+.1} cm (locked)", r * 100.0)), fs.rel * 100.0,
                    f.host_c.map_or(String::new(), |c| format!("  c {c:.2}"))));
            }
            s.push_str(&format!("  joints {}  peak sat {:.2}\n", rig.joints.len(), rs.peak_sat));
        }
        for n in &rig.notes { s.push_str("  "); s.push_str(n); s.push('\n'); }
    }
    if !frame.voxel_boxes.is_empty() || !frame.debris.is_empty() { s.push_str(&format!("voxel boxes {}  debris {}\n", frame.voxel_boxes.len(), frame.debris.len())); }
    for n in &frame.notes { s.push_str(n); s.push('\n'); }
    txt.0 = s;
}
