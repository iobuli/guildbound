//! Windowed demo: the gb_lab "stand" scene with the ragdoll debug overlay (F3 toggles, on by default).
//! Defaults = pass 6 test build: 12 substeps, kb 5, kbd 0.3, Kp 4000 / Kd 126 (scaled Stable PD, no clamp),
//! foot damping 40, mu 0.8, COM pull 0.11. Run: cargo run -p gb_lab --release --features visual --bin visual
//! Keys: F3 overlay, F4-F9 layers, R restart scene, Space pause.
#[path = "visual/lab_scene.rs"]
mod lab_scene;
#[path = "../../../../staging/overlay/debug_overlay.rs"]
mod debug_overlay;

use avian3d::prelude::*;
use bevy::prelude::*;
use debug_overlay::*;
use lab_scene::Lab;

pub const DT: f32 = 1.0 / 120.0;

#[derive(Clone, Debug)]
pub struct Args {
    pub scene: String,
    pub ticks: u32,
    pub kp: f32,
    pub kd: f32,
    pub k: f32,
    pub out: Option<String>,
    pub unpaired: bool,
    pub exact: bool,
    pub leg_floor: f32,
    pub no_gff: bool,
    pub no_filter: bool,
    pub balance: bool,
    pub rest_offset: bool,
    pub no_clamp: bool,
    pub pin_pelvis: bool,
    pub one_leg: bool,
    pub scaled: bool,
    pub cp: bool,
    pub tan_full: bool,
    pub com_pull: f32,
    pub cff: f32,
    pub cdr: f32,
    pub mu: f32,
    pub ired_scalar: bool,
    pub damp_rel: bool,
    pub foot_damp: f32,
    pub anchor_k: f32,
    pub bal_rate: f32,
    pub bal_lp: f32,
    pub substeps: u32,
    pub flat_feet: bool,
    pub friction: f32,
    pub old_gff: bool,
    pub geom_edge: bool,
    pub bal_sign: f32,
    pub kb: f32,
    pub kbd: f32,
    pub hip_offset_deg: f32,
}


fn default_args() -> Args {
    Args {
        scene: "stand".into(), ticks: u32::MAX, kp: 4000.0, kd: 126.0, k: 1.2, out: None, unpaired: false, exact: false,
        leg_floor: 0.0, no_gff: false, no_filter: false, balance: true, rest_offset: true, no_clamp: true, pin_pelvis: false,
        one_leg: false, scaled: true, cp: false, tan_full: false, com_pull: 0.11, cff: 1.5, cdr: 10.0, mu: 0.8,
        ired_scalar: false, damp_rel: false, foot_damp: 40.0, anchor_k: 0.0, bal_rate: 0.0, bal_lp: 1.0, substeps: 12,
        flat_feet: false, friction: 0.8, old_gff: false, geom_edge: false, bal_sign: 1.0, kb: 5.0, kbd: 0.3, hip_offset_deg: 0.0,
    }
}

/// Adapter: lab `Lab` resource + Avian bodies -> overlay frame.
struct LabSource;
impl RagdollDebugSource for LabSource {
    fn fill(world: &World, frame: &mut RagdollDebugFrame) {
        let Some(lab) = world.get_resource::<Lab>() else { return };
        let mut f = RigDebug { key: 1, label: "lab capsule ragdoll".into(), profile: "capsule_ragdoll_v1".into(), ..default() };
        f.tick = lab.tick as u64;
        f.mass = lab.total_mass;
        let (mut com, mut v) = (Vec3::ZERO, Vec3::ZERO);
        for s in &lab.segs {
            if let Some(p) = world.get::<Position>(s.e) { com += p.0 * s.mass; }
            if let Some(lv) = world.get::<LinearVelocity>(s.e) { v += lv.0 * s.mass; }
        }
        f.com = com / lab.total_mass.max(1e-6);
        f.com_vel = v / lab.total_mass.max(1e-6);
        for s in lab.segs.iter().filter(|s| s.is_foot) {
            let (Some(p), Some(r)) = (world.get::<Position>(s.e), world.get::<Rotation>(s.e)) else { continue };
            let he = Vec3::new(0.05, s.foot_half_h, 0.12);
            let mut corners = Vec::new();
            for cx in [-1.0, 1.0] { for cz in [-1.0, 1.0] { corners.push(p.0 + r.0 * Vec3::new(he.x * cx, -he.y, he.z * cz)); } }
            let sole = corners.iter().map(|c| c.y).fold(f32::MAX, f32::min);
            let speed = world.get::<LinearVelocity>(s.e).map_or(0.0, |v| v.0.length());
            f.feet.push(FootSample { sole_corners: corners, sole_height: sole, speed, host_c: Some(lab.c[s.side]) });
        }
        if lab.dbg_sup != Vec3::ZERO { f.balance_target = Some(lab.dbg_sup); f.balance_torque = Some(lab.dbg_want); }
        for &(_, anchor, tau, limit) in &lab.dbg_joints { f.joints.push(JointSample { anchor, tau, limit }); }
        f.notes.push(format!("substeps 12  kb 5  kbd 0.3  Kp 4000 Kd 126  foot_damp 40  drift {:.1} cm",
            Vec2::new(f.com.x - lab.com0.x, f.com.z - lab.com0.z).length() * 100.0));
        frame.rigs.push(f);
    }
}

fn main() {
    let args = default_args();
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins.set(WindowPlugin { primary_window: Some(Window { title: "gb_lab visual - stand".into(), ..default() }), ..default() }),
        PhysicsPlugins::default(),
        RagdollDebugOverlayPlugin::<LabSource>::default(),
    ))
    .insert_resource(Time::<Fixed>::from_seconds(DT as f64))
    .insert_resource(Gravity(Vec3::NEG_Y * 9.81))
    .add_systems(Startup, setup_view)
    .add_systems(Update, (controls, draw_bodies));
    lab_scene::build(&mut app, &args, false);
    app.run();
}

fn setup_view(mut cmd: Commands) {
    cmd.spawn((Camera3d::default(), Transform::from_xyz(2.2, 1.4, 2.6).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y)));
    cmd.spawn((DirectionalLight { illuminance: 8000.0, ..default() }, Transform::from_xyz(3.0, 6.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y)));
}

/// Space pauses physics, R restarts the process-free way: respawn not supported, so R just resets drift origin.
fn controls(k: Res<ButtonInput<KeyCode>>, mut time: ResMut<Time<Physics>>, mut lab: ResMut<Lab>, q: Query<&Position>) {
    if k.just_pressed(KeyCode::Space) { if time.is_paused() { time.unpause(); } else { time.pause(); } }
    if k.just_pressed(KeyCode::KeyR) {
        let (mut s, mut m) = (Vec3::ZERO, 0.0);
        for sg in &lab.segs { if let Ok(p) = q.get(sg.e) { s += p.0 * sg.mass; m += sg.mass; } }
        lab.com0 = s / m.max(1e-6);
    }
}

/// Wire bodies with gizmos (no meshes needed): collider AABBs + floor grid.
fn draw_bodies(mut g: Gizmos, q: Query<(&ColliderAabb, &RigidBody)>) {
    for (aabb, rb) in &q {
        if *rb == RigidBody::Static { continue; }
        let c = (aabb.min + aabb.max) * 0.5;
        g.cube(Transform::from_translation(c).with_scale(aabb.max - aabb.min), Color::srgba(0.8, 0.8, 0.9, 0.6));
    }
    for i in -10..=10 {
        let x = i as f32 * 0.25;
        g.line(Vec3::new(x, 0.0, -2.5), Vec3::new(x, 0.0, 2.5), Color::srgba(0.4, 0.4, 0.4, 0.4));
        g.line(Vec3::new(-2.5, 0.0, x), Vec3::new(2.5, 0.0, x), Color::srgba(0.4, 0.4, 0.4, 0.4));
    }
}
