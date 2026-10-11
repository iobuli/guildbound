//! Owns the deterministic sim, feeds it intents (keyboard, opponent, remote scripts) at a fixed 1/120 s step,
//! and mirrors its state onto the scene. Presentation reads the sim; it never writes sim state directly.

use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use guildbound_sim as gs;

use crate::camera::OrbitCamera;
use crate::panel::PanelState;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpponentMode {
    Idle,
    Chase,
}

/// An intent held for a number of sim ticks, set by the remote API.
#[derive(Clone, Copy)]
pub struct Scripted {
    pub intent: gs::Intent,
    pub ticks_left: u32,
}

#[derive(Resource)]
pub struct Sim {
    pub world: gs::World,
    pub paused: bool,
    pub speed: f32,
    pub pending_steps: u32,
    pub opponent: OpponentMode,
    pub scripted: Vec<Option<Scripted>>,
    pub keyboard: gs::Intent,
    acc: f32,
}

impl Sim {
    pub fn new() -> Self {
        let world = gs::World::sparring(gs::Tuning::default());
        let n = world.fighters.len();
        Self { world, paused: false, speed: 1.0, pending_steps: 0, opponent: OpponentMode::Idle, scripted: vec![None; n], keyboard: gs::Intent::default(), acc: 0.0 }
    }

    pub fn reset(&mut self) {
        let tuning = self.world.tuning.clone();
        let lock = self.world.fighters[0].lock_target;
        self.world = gs::World::sparring(tuning);
        self.world.fighters[0].lock_target = lock;
        self.scripted = vec![None; self.world.fighters.len()];
        self.acc = 0.0;
    }

    fn tick(&mut self) {
        let mut intents = vec![self.keyboard, self.opponent_intent()];
        for (i, slot) in self.scripted.iter_mut().enumerate() {
            if let Some(s) = slot {
                intents[i] = s.intent;
                s.ticks_left -= 1;
                if s.ticks_left == 0 {
                    *slot = None;
                }
            }
        }
        self.world.step(&intents);
    }

    fn opponent_intent(&self) -> gs::Intent {
        let [player, opp] = [&self.world.fighters[0], &self.world.fighters[1]];
        match self.opponent {
            OpponentMode::Idle => gs::Intent::default(),
            OpponentMode::Chase => {
                let to = player.position - opp.position;
                if to.length() > 1.2 { gs::Intent { move_dir: to.normalized(), run: to.length() > 4.0 } } else { gs::Intent::default() }
            }
        }
    }
}

#[derive(Component)]
pub struct FighterView(pub usize);

pub fn setup_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sim: Res<Sim>) {
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 40.0))),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.22, 0.24, 0.22), perceptual_roughness: 0.95, ..default() })),
    ));
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(6.0, 12.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let colors = [Color::srgb(0.25, 0.55, 0.95), Color::srgb(0.9, 0.3, 0.25)];
    let r = sim.world.tuning.body_radius;
    let body = meshes.add(Capsule3d::new(r, 1.8 - 2.0 * r));
    let nose = meshes.add(Cuboid::new(0.12, 0.12, 0.25));
    for (i, color) in colors.iter().enumerate().take(sim.world.fighters.len()) {
        let mat = materials.add(StandardMaterial { base_color: *color, ..default() });
        commands
            .spawn((FighterView(i), Mesh3d(body.clone()), MeshMaterial3d(mat.clone()), Transform::from_xyz(0.0, 0.9, 0.0)))
            .with_child((Mesh3d(nose.clone()), MeshMaterial3d(mat), Transform::from_xyz(0.0, 0.55, -r - 0.08)));
    }
}

/// WASD relative to the camera, Shift runs. Space pauses, period steps one tick while paused.
pub fn read_player_input(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    camera: Single<&Transform, With<OrbitCamera>>,
    mut sim: ResMut<Sim>,
) {
    if wants.wants_any_keyboard_input() {
        sim.keyboard = gs::Intent::default();
        return;
    }
    if keys.just_pressed(KeyCode::Space) {
        sim.paused = !sim.paused;
    }
    if keys.just_pressed(KeyCode::Period) && sim.paused {
        sim.pending_steps += 1;
    }
    let fwd = camera.forward().as_vec3().with_y(0.0).normalize_or_zero();
    let right = camera.right().as_vec3().with_y(0.0).normalize_or_zero();
    let mut m = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) { m += fwd; }
    if keys.pressed(KeyCode::KeyS) { m -= fwd; }
    if keys.pressed(KeyCode::KeyD) { m += right; }
    if keys.pressed(KeyCode::KeyA) { m -= right; }
    let m = m.normalize_or_zero();
    sim.keyboard = gs::Intent {
        move_dir: gs::V2::new(m.x, m.z),
        run: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
    };
}

/// Fixed-step accumulator on real frame time, scaled by `speed`. Capped so a stall cannot spiral.
pub fn step_sim(time: Res<Time>, mut sim: ResMut<Sim>) {
    if sim.paused {
        sim.acc = 0.0;
        while sim.pending_steps > 0 {
            sim.pending_steps -= 1;
            sim.tick();
        }
        return;
    }
    sim.acc += time.delta_secs() * sim.speed;
    let mut n = 0;
    while sim.acc >= gs::STEP && n < 24 {
        sim.acc -= gs::STEP;
        sim.tick();
        n += 1;
    }
    if n == 24 {
        sim.acc = 0.0;
    }
}

pub fn sync_views(sim: Res<Sim>, mut views: Query<(&FighterView, &mut Transform)>) {
    for (view, mut t) in &mut views {
        let Some(f) = sim.world.fighters.get(view.0) else { continue };
        t.translation = Vec3::new(f.position.x, 0.9, f.position.z);
        t.look_to(Vec3::new(f.facing.x, 0.0, f.facing.z), Vec3::Y);
    }
}

pub fn draw_gizmos(sim: Res<Sim>, panel: Res<PanelState>, mut gizmos: Gizmos) {
    let t = &sim.world.tuning;
    let h = t.arena_half_size;
    let y = 0.01;
    let edge = Color::srgb(0.55, 0.85, 1.0);
    let corners = [Vec3::new(-h, y, -h), Vec3::new(h, y, -h), Vec3::new(h, y, h), Vec3::new(-h, y, h)];
    for i in 0..4 {
        gizmos.line(corners[i], corners[(i + 1) % 4], edge);
    }
    if panel.show_grid {
        let grid = Color::srgba(1.0, 1.0, 1.0, 0.08);
        let mut x = -h.floor();
        while x <= h {
            gizmos.line(Vec3::new(x, y, -h), Vec3::new(x, y, h), grid);
            gizmos.line(Vec3::new(-h, y, x), Vec3::new(h, y, x), grid);
            x += 1.0;
        }
    }
    for f in &sim.world.fighters {
        let p = Vec3::new(f.position.x, 0.05, f.position.z);
        gizmos.arrow(p, p + Vec3::new(f.facing.x, 0.0, f.facing.z) * 0.8, Color::srgb(1.0, 0.85, 0.2));
        if panel.show_velocity && f.velocity.length_squared() > 1e-6 {
            gizmos.line(p, p + Vec3::new(f.velocity.x, 0.0, f.velocity.z) * 0.5, Color::srgb(0.3, 1.0, 0.4));
        }
        if panel.show_bodies {
            gizmos.circle(Isometry3d::new(p, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), t.body_radius, Color::srgba(1.0, 1.0, 1.0, 0.5));
        }
    }
}
