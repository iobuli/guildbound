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
    /// Fights back: deterministic sparring AI from the sim (approach, strings, guards).
    Spar,
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
    /// Press edges gathered since the last tick; the next tick consumes them exactly once.
    pub edges: gs::Intent,
    /// Contacts drained from the sim for presentation (sparks, log).
    pub recent: Vec<(gs::Contact, f32)>,
    /// Mob brains by fighter index (None for the player and the sparring opponent).
    pub brains: Vec<Option<gs::mobs::MobBrain>>,
    /// Time actually advanced by the fixed-step core this rendered frame.
    pub frame_step_s: f32,
    acc: f32,
}

impl Sim {
    pub fn new() -> Self {
        let mut world = gs::World::sparring(gs::Tuning { arena_half_size: crate::props::WORLD_HALF, ..gs::Tuning::default() });
        world.stock_floor();
        world.obstacles = crate::props::obstacles();
        let n = world.fighters.len();
        Self { world, paused: false, speed: 1.0, pending_steps: 0, opponent: OpponentMode::Idle, scripted: vec![None; n], keyboard: gs::Intent::default(), edges: gs::Intent::default(), recent: Vec::new(), brains: vec![None; n], frame_step_s: 0.0, acc: 0.0 }
    }

    pub fn reset(&mut self) {
        let tuning = self.world.tuning.clone();
        let combat = self.world.combat;
        let magic = self.world.magic;
        let lock = self.world.fighters[0].lock_target;
        let kit = self.world.fighters[0].kit.clone();
        self.world = gs::World::sparring(tuning);
        self.world.stock_floor();
        self.world.obstacles = crate::props::obstacles();
        // the player keeps what they carry and wear across a rematch
        self.world.fighters[0].kit = kit;
        self.world.refresh_vitality(0);
        self.world.fighters[0].hp = self.world.fighters[0].max_hp;
        self.world.combat = combat;
        self.world.magic = magic;
        for f in &mut self.world.fighters {
            f.guard = combat.guard_max;
            f.ki = f.ki.min(magic.ki_max);
        }
        self.world.fighters[0].lock_target = lock.filter(|&i| i > 0 && i < self.world.fighters.len());
        self.scripted = vec![None; self.world.fighters.len()];
        self.brains = vec![None; self.world.fighters.len()];
        self.keyboard = gs::Intent::default();
        self.edges = gs::Intent::default();
        self.pending_steps = 0;
        self.recent.clear();
        self.frame_step_s = 0.0;
        self.acc = 0.0;
    }

    /// Menu/focus loss must not turn a held charge into a released shot on resume.
    pub fn clear_player_input(&mut self) {
        self.keyboard = gs::Intent::default();
        self.edges = gs::Intent::default();
        self.world.cancel_held_action(0);
    }

    fn tick(&mut self) {
        let mut player = self.keyboard;
        let e = if self.world.fighters[0].hit_stop > 0.0 {
            // Presses made during local hit-stop wait for the actor's next rule
            // tick. Held intent is still sampled normally.
            gs::Intent::default()
        } else {
            std::mem::take(&mut self.edges)
        };
        (player.light, player.heavy, player.guard_pressed, player.step, player.jump, player.roll, player.cast) = (e.light, e.heavy, e.guard_pressed, e.step, e.jump, e.roll, e.cast);
        (player.interact, player.bag) = (e.interact, e.bag);
        let mut intents = vec![player, self.opponent_intent()];
        for i in 2..self.world.fighters.len() {
            let intent = match self.brains.get_mut(i).and_then(|b| b.as_mut()) {
                Some(brain) => gs::mobs::mob_intent(&self.world, i, brain),
                None => gs::Intent::default(),
            };
            intents.push(intent);
        }
        for (i, slot) in self.scripted.iter_mut().enumerate() {
            if i >= intents.len() {
                break;
            }
            if let Some(s) = slot {
                intents[i] = s.intent;
                // Scripted controller presses use the same actor clock as
                // local presses: hit-stop must not eat a queued rush input.
                if self.world.fighters[i].hit_stop > 0.0 { continue; }
                // press edges in a script fire on its first tick only
                s.intent.light = false;
                s.intent.heavy = false;
                s.intent.step = false;
                s.intent.jump = false;
                s.intent.roll = false;
                s.intent.guard_pressed = false;
                s.intent.cast = None;
                s.intent.interact = false;
                s.intent.bag = None;
                s.ticks_left -= 1;
                if s.ticks_left == 0 {
                    *slot = None;
                }
            }
        }
        self.world.step(&intents);
    }

    /// Add a mob near `at`; melee or keeper tactics alternate.
    pub fn spawn_mob(&mut self, at: gs::V2) -> usize {
        let i = gs::mobs::spawn(&mut self.world, at, 60.0);
        const DROPS: [gs::rpg::ItemId; 5] = [gs::rpg::HEALING_DRAUGHT, gs::rpg::LEATHER_CAP, gs::rpg::KI_TONIC, gs::rpg::TRAVEL_BOOTS, gs::rpg::QUILTED_TROUSERS];
        self.world.fighters[i].loot = Some(DROPS[i % DROPS.len()]);
        let tactics = if i % 2 == 0 { gs::mobs::Tactics::Melee } else { gs::mobs::Tactics::Keeper };
        self.brains.resize(self.world.fighters.len(), None);
        self.scripted.resize(self.world.fighters.len(), None);
        self.brains[i] = Some(gs::mobs::MobBrain::new(at, tactics));
        i
    }

    fn opponent_intent(&self) -> gs::Intent {
        let [player, opp] = [&self.world.fighters[0], &self.world.fighters[1]];
        match self.opponent {
            OpponentMode::Idle => gs::Intent::default(),
            OpponentMode::Chase => {
                let to = player.position - opp.position;
                if to.length() > 1.2 { gs::Intent { move_dir: to.normalized(), run: to.length() > 4.0, ..default() } } else { gs::Intent::default() }
            }
            OpponentMode::Spar => gs::combat::spar_intent(&self.world, 1, 0),
        }
    }
}

#[derive(Component)]
pub struct FighterView(pub usize);

/// The placeholder capsule; shown only when the panel's "capsules" overlay is on.
#[derive(Component)]
pub struct CapsuleView;

pub fn setup_scene(
    mut commands: Commands,
) {
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(6.0, 12.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

}

/// One view (capsule + pack model + physics body) per sim fighter, spawned and despawned as the sim's
/// fighter list changes (mobs join at runtime; a reset removes them).
pub fn ensure_views(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
    libs: Res<crate::anim::Libraries>,
    sim: Res<Sim>,
    views: Query<(Entity, &FighterView)>,
    bodies: Query<(Entity, &crate::props::FighterBody)>,
) {
    let n = sim.world.fighters.len();
    for (e, v) in &views {
        if v.0 >= n {
            commands.entity(e).despawn();
        }
    }
    for (e, b) in &bodies {
        if b.0 >= n {
            commands.entity(e).despawn();
        }
    }
    let have: Vec<usize> = views.iter().map(|(_, v)| v.0).collect();
    let r = sim.world.tuning.body_radius;
    let model = libs.current().map(|l| assets.load(GltfAssetLabel::Scene(0).from_asset(l.path.clone())));
    for i in (0..n).filter(|i| !have.contains(i)) {
        let color = crate::anim::color_of(i);
        let mat = materials.add(StandardMaterial { base_color: color, ..default() });
        let capsule_visibility = if model.is_some() { Visibility::Hidden } else { Visibility::Inherited };
        let body = meshes.add(Capsule3d::new(r, 1.8 - 2.0 * r));
        let nose = meshes.add(Cuboid::new(0.12, 0.12, 0.25));
        let view = commands
            .spawn((FighterView(i), Transform::default(), Visibility::default()))
            .with_children(|p| {
                p.spawn((CapsuleView, Mesh3d(body), MeshMaterial3d(mat.clone()), Transform::from_xyz(0.0, 0.9, 0.0), capsule_visibility))
                    .with_child((Mesh3d(nose), MeshMaterial3d(mat), Transform::from_xyz(0.0, 0.55, -r - 0.08)));
            })
            .id();
        if let Some(scene) = &model {
            crate::anim::spawn_rig(&mut commands, view, i, scene.clone());
        }
        commands.spawn((
            crate::props::FighterBody(i),
            avian3d::prelude::RigidBody::Kinematic,
            avian3d::prelude::Collider::capsule(r, 1.8 - 2.0 * r),
            avian3d::prelude::CollisionLayers::new(2u32, 1u32),
            Transform::from_xyz(0.0, 0.9, 0.0),
        ));
    }
}

pub fn show_capsules(panel: Res<PanelState>, mut capsules: Query<&mut Visibility, With<CapsuleView>>) {
    if !panel.is_changed() {
        return;
    }
    for mut v in &mut capsules {
        *v = if panel.show_capsules { Visibility::Inherited } else { Visibility::Hidden };
    }
}

/// Keymap (Decision 18 + boss 2026-10-12): WASD move (camera-relative), Shift run, LMB light, RMB guard
/// (tap = parry), Mouse4 heavy (W/S + Mouse4 = rising/sweep), Mouse5 lock-on, Ctrl step/dash.
/// Space jump, C crouch/roll, Q ki, E interact (pick up / open), 1-4 spells.
/// I inventory, Tab menu (dev panel), P pause, . step one tick while paused.
pub fn read_player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    wants: Res<EguiWantsInput>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    camera: Single<&Transform, With<OrbitCamera>>,
    mut panel: ResMut<PanelState>,
    mut sim: ResMut<Sim>,
    mut had_focus: Local<bool>,
) {
    if !window.focused {
        if *had_focus {
            sim.clear_player_input();
        } else {
            sim.keyboard = gs::Intent::default();
            sim.edges = gs::Intent::default();
        }
        *had_focus = false;
        return;
    }
    *had_focus = true;
    let typing = wants.wants_any_keyboard_input();
    let over_panel = (!panel.hidden && window.cursor_position().is_some_and(|p| p.x <= panel.width)) || wants.wants_any_pointer_input();
    if !typing {
        if keys.just_pressed(KeyCode::Tab) {
            panel.hidden = !panel.hidden;
        }
        if keys.just_pressed(KeyCode::KeyI) {
            panel.show_inventory = !panel.show_inventory;
        }
        if keys.just_pressed(KeyCode::KeyP) {
            sim.paused = !sim.paused;
        }
        if keys.just_pressed(KeyCode::Period) && sim.paused {
            sim.pending_steps += 1;
        }
        if mouse.just_pressed(MouseButton::Forward) {
            let lock = sim.world.fighters[0].lock_target;
            sim.world.fighters[0].lock_target = if lock.is_some() { None } else { Some(1) };
        }
    }
    if sim.paused && sim.pending_steps == 0 {
        sim.keyboard = gs::Intent::default();
        sim.edges = gs::Intent::default();
        return;
    }
    let fwd = camera.forward().as_vec3().with_y(0.0).normalize_or_zero();
    let right = camera.right().as_vec3().with_y(0.0).normalize_or_zero();
    let mut m = Vec3::ZERO;
    if !typing {
        if keys.pressed(KeyCode::KeyW) { m += fwd; }
        if keys.pressed(KeyCode::KeyS) { m -= fwd; }
        if keys.pressed(KeyCode::KeyD) { m += right; }
        if keys.pressed(KeyCode::KeyA) { m -= right; }
    }
    let m = m.normalize_or_zero();
    let mouse_ok = !over_panel;
    let guard_held = mouse_ok && mouse.pressed(MouseButton::Right);
    sim.keyboard = gs::Intent {
        move_dir: gs::V2::new(m.x, m.z),
        run: !typing && (keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)),
        guard: guard_held,
        heavy_held: !typing && (keys.pressed(KeyCode::KeyF) || (mouse_ok && mouse.pressed(MouseButton::Back))),
        cast_held: !typing && [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].iter().any(|k| keys.pressed(*k)),
        ki_charge: !typing && keys.pressed(KeyCode::KeyQ),
        ..default()
    };
    // edges accumulate until a sim tick consumes them
    let e = &mut sim.edges;
    e.light |= mouse_ok && mouse.just_pressed(MouseButton::Left);
    e.heavy |= !typing && (keys.just_pressed(KeyCode::KeyF) || (mouse_ok && mouse.just_pressed(MouseButton::Back)));
    e.guard_pressed |= mouse_ok && mouse.just_pressed(MouseButton::Right);
    e.step |= !typing && (keys.just_pressed(KeyCode::ControlLeft) || keys.just_pressed(KeyCode::ControlRight));
    e.jump |= !typing && keys.just_pressed(KeyCode::Space);
    // 1-4 cast the spell in that slot (hold to charge a chargeable one), Q held charges ki
    let slots = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];
    if !typing {
        if let Some(k) = slots.iter().position(|k| keys.just_pressed(*k)) {
            e.cast = Some(k as u8);
        }
    }
    e.roll |= !typing && keys.just_pressed(KeyCode::KeyC);
    e.interact |= !typing && keys.just_pressed(KeyCode::KeyE);
}

/// Fixed-step accumulator on real frame time, scaled by `speed`. Capped so a stall cannot spiral.
pub fn step_sim(time: Res<Time>, mut sim: ResMut<Sim>) {
    let start_tick = sim.world.tick;
    if sim.paused {
        sim.acc = 0.0;
        while sim.pending_steps > 0 {
            sim.pending_steps -= 1;
            sim.tick();
        }
        sim.frame_step_s = (sim.world.tick - start_tick) as f32 * gs::STEP;
        collect_contacts(&mut sim);
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
    sim.frame_step_s = (sim.world.tick - start_tick) as f32 * gs::STEP;
    collect_contacts(&mut sim);
}

fn collect_contacts(sim: &mut Sim) {
    let fresh = sim.world.drain_contacts();
    sim.recent.extend(fresh.into_iter().map(|c| (c, c.tick as f32 * gs::STEP)));
    let keep_from = sim.recent.len().saturating_sub(12);
    sim.recent.drain(..keep_from);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_discards_stale_inputs_contacts_and_removed_target_but_keeps_tuning() {
        let mut sim = Sim::new();
        let mob = sim.spawn_mob(gs::V2::new(5.0, 0.0));
        sim.world.fighters[0].lock_target = Some(mob);
        sim.world.combat.guard_max = 75.0;
        sim.world.magic.ki_max = 80.0;
        sim.keyboard.move_dir = gs::V2::new(1.0, 0.0);
        sim.edges.light = true;
        sim.pending_steps = 20;
        sim.scripted[0] = Some(Scripted { intent: sim.keyboard, ticks_left: 60 });
        sim.recent.push((gs::Contact { attacker: 0, defender: 1, source: gs::Source::Move(gs::combat::JAB), outcome: gs::Outcome::Hit, point: gs::V2::ZERO, aim: gs::combat::Aim::Head, tick: 1, damage: 4.0, crit: false }, 0.0));
        sim.reset();
        assert_eq!(sim.world.fighters.len(), 2);
        assert_eq!(sim.world.fighters[0].lock_target, None);
        assert_eq!(sim.world.combat.guard_max, 75.0);
        assert_eq!(sim.world.magic.ki_max, 80.0);
        assert_eq!(sim.pending_steps, 0);
        assert!(sim.recent.is_empty());
        assert!(sim.scripted.iter().all(Option::is_none));
        sim.tick();
        assert_eq!(sim.world.fighters[0].position, gs::V2::new(0.0, -1.5));
        assert_eq!(sim.world.fighters[0].action.kind, gs::ActionKind::None);
    }

    #[test]
    fn a_press_during_hit_stop_is_consumed_once_when_the_actor_resumes() {
        let mut sim = Sim::new();
        sim.world.fighters[0].hit_stop = 0.02;
        sim.edges.light = true;
        for _ in 0..3 { sim.tick(); }
        assert_eq!(sim.world.fighters[0].action.kind, gs::ActionKind::None);
        sim.tick();
        assert!(matches!(sim.world.fighters[0].action.kind, gs::ActionKind::Strike(_)));
        let seq = sim.world.fighters[0].action.seq;
        sim.tick();
        assert_eq!(sim.world.fighters[0].action.seq, seq);
        assert!(!sim.edges.light);
    }

    #[test]
    fn a_scripted_rush_press_waits_through_hit_stop() {
        let mut sim = Sim::new();
        sim.world.fighters[0].hit_stop = 0.02;
        sim.scripted[0] = Some(Scripted { intent: gs::Intent { light: true, ..Default::default() }, ticks_left: 1 });
        for _ in 0..3 { sim.tick(); }
        assert_eq!(sim.world.fighters[0].action.kind, gs::ActionKind::None);
        assert!(sim.scripted[0].is_some());
        sim.tick();
        assert!(matches!(sim.world.fighters[0].action.kind, gs::ActionKind::Strike(_)));
        assert!(sim.scripted[0].is_none());
    }
}

pub fn sync_views(sim: Res<Sim>, mut views: Query<(&FighterView, &mut Transform)>) {
    for (view, mut t) in &mut views {
        let Some(f) = sim.world.fighters.get(view.0) else { continue };
        t.translation = Vec3::new(f.position.x, f.height, f.position.z);
        t.look_to(Vec3::new(f.facing.x, 0.0, f.facing.z), Vec3::Y);
    }
}

pub fn draw_gizmos(sim: Res<Sim>, panel: Res<PanelState>, mut gizmos: Gizmos) {
    // contact sparks: white hit, yellow counter-hit, blue block, orange guard break, green parry
    let now = sim.world.tick as f32 * gs::STEP;
    for (c, at) in &sim.recent {
        let age = now - at;
        if age > 0.35 {
            continue;
        }
        let profile = crate::feedback::profile(c.outcome);
        let k = (1.0 - age / profile.duration_s).clamp(0.0, 1.0);
        let color = profile.color.with_alpha(k);
        let height = match c.aim {
            gs::combat::Aim::Head => 1.5,
            gs::combat::Aim::Body => 1.0,
            gs::combat::Aim::Low => 0.4,
        };
        let p = Vec3::new(c.point.x, height, c.point.z);
        let r = 0.04 + profile.radius_m * (1.0 - k);
        for d in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(1.0, 1.0, 0.0).normalize(), Vec3::new(0.0, 1.0, 1.0).normalize(), Vec3::new(1.0, -1.0, 1.0).normalize()] {
            gizmos.line(p - d * r, p + d * r, color);
        }
    }
    for f in &sim.world.fighters {
        let p = Vec3::new(f.position.x, 0.03, f.position.z);
        let flat = Isometry3d::new(p, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
        if f.action.kind == gs::ActionKind::Guard {
            let k = f.guard / sim.world.combat.guard_max;
            gizmos.circle(flat, 0.45, Color::srgba(0.4, 0.7, 1.0, 0.3 + 0.6 * k));
        }
        if f.parry_left > 0.0 {
            gizmos.circle(flat, 0.6, Color::srgb(0.3, 1.0, 0.5));
        }
        if f.counter_left > 0.0 {
            gizmos.circle(flat, 0.7, Color::srgba(1.0, 0.9, 0.2, 0.7));
        }
    }
    let t = &sim.world.tuning;
    let h = t.arena_half_size;
    let y = 0.01;
    let edge = Color::srgb(0.55, 0.85, 1.0);
    let corners = [Vec3::new(-h, y, -h), Vec3::new(h, y, -h), Vec3::new(h, y, h), Vec3::new(-h, y, h)];
    if !panel.hidden {
        for i in 0..4 {
            gizmos.line(corners[i], corners[(i + 1) % 4], edge);
        }
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
        if !panel.hidden {
            gizmos.arrow(p, p + Vec3::new(f.facing.x, 0.0, f.facing.z) * 0.8, Color::srgb(1.0, 0.85, 0.2));
        }
        if panel.show_velocity && f.velocity.length_squared() > 1e-6 {
            gizmos.line(p, p + Vec3::new(f.velocity.x, 0.0, f.velocity.z) * 0.5, Color::srgb(0.3, 1.0, 0.4));
        }
        if panel.show_bodies {
            gizmos.circle(Isometry3d::new(p, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), t.body_radius, Color::srgba(1.0, 1.0, 1.0, 0.5));
        }
    }
}
