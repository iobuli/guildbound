//! Berserker: a rage meter that fills as blows land, and a berserk state wrapped in a pixel-flame aura.
//!
//! - `Rage` (every `Fighter`): each melee or spell `Hit` a fighter lands adds `damage / RAGE_FILL`, plus
//!   `CHAIN_BONUS` when it follows the last within `CHAIN_WINDOW` s. After `IDLE` s without a hit it drains at
//!   `DECAY` per second.
//! - Full rage: the player presses V (`Intent::berserk_pressed`), an NPC goes berserk by itself. `Berserk` lasts
//!   `DURATION` s: attack speed ×`SPEED`, damage dealt ×`MIGHT`, damage taken ×`FRAGILE` (`combat` reads the
//!   `Berserkers` set), with a roar burst (aura + fire, a slow-motion beat, a camera kick). Rage empties.
//! - The aura (`Flame`): a pool of `RINGS` soft glowing rings rising fast up a spine of `NODES` spring-driven
//!   points (each takes `RISE` s from base to tip, then is reborn at the base with new looks). Rings face the
//!   camera, so they read as circles from any side, and sit scattered through the flame's body: each has its own
//!   angle and distance from the spine (inside `taper`, widest a little below the middle, a point at the tip),
//!   swirl, size, oval squash and roll, so they read as an irregular rush of circles. They fade in off the base
//!   and out at the tip (`ring_glow`). Most are dim (a low colour on an additive material reads as low alpha); `HIGHLIGHT` of
//!   them are bright gold-white highlights. Short lightning arcs crackle through it every `ARC_EVERY` s. Each spine point springs toward straight-up minus the fighter's ground velocity ×
//!   `LAG` × s², so moving forward bends the tip back and stopping lets it swing up and settle. Additive,
//!   unlit, no depth write, no shadow, no collider: see-through and not solid. It grows in on the trigger,
//!   breathes while berserk and shrinks out at the end.
//! - HUD: a rage bar above the hotbar while the player has rage; it pulses when full.
//! - `GB_BERSERK=<seconds>` fills the player's rage and triggers berserk at that time (smoke runs).
use super::combat::Hitstop;
use super::camera::{CameraShake, MainCamera};
use super::hud::log::{HudLog, LogKind};
use super::flipbook::{Flip, Sheet};
use super::vfx::{self, Element, Vfx};
use super::{Fighter, Hit, Intent, Mover, Npc, Player};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub const RAGE_FILL: f32 = 60.0;
pub const CHAIN_BONUS: f32 = 0.04;
pub const CHAIN_WINDOW: f32 = 1.2;
pub const IDLE: f32 = 3.0;
pub const DECAY: f32 = 0.08;
pub const DURATION: f32 = 8.0;
pub const SPEED: f32 = 1.35;
pub const MIGHT: f32 = 1.3;
pub const FRAGILE: f32 = 1.2;
/// Aura spine points.
const NODES: usize = 10;
/// Spine: height and widest width (m), lag (s), spring stiffness and damping; grow-in and fade-out times (s).
const HEIGHT: f32 = 2.6;
const WIDTH: f32 = 1.3;
const LAG: f32 = 0.22;
const STIFF: f32 = 90.0;
const DAMP: f32 = 12.0;
const GROW: f32 = 0.3;
const FADE: f32 = 0.5;
/// Aura rings: pool per fighter, rise time range (s), share of bright highlight rings, brightness levels.
const RINGS: usize = 90;
const RISE: (f32, f32) = (0.3, 0.55);
const HIGHLIGHT: f32 = 0.1;
const LEVELS: usize = 6;
/// Electricity: seconds between arcs, and their length (m).
const ARC_EVERY: f32 = 0.12;
const ARC_LEN: f32 = 0.7;

/// Width share of the flame at height share `s` (0 base, 1 tip): widest a little below the middle,
/// a point at the tip, a rounded base.
pub fn taper(s: f32) -> f32 {
    if !(0.0..=1.0).contains(&s) {
        return 0.0;
    }
    (std::f32::consts::PI * s).sin().max(0.0).powf(0.8) * (1.0 - 0.35 * s)
}

/// Rest offset of a spine point at height share `s` for ground velocity `v`: up, bent back against the motion.
pub fn rest_offset(s: f32, v: Vec3) -> Vec3 {
    let v = v.with_y(0.0);
    Vec3::Y * HEIGHT * s - v * LAG * s * s
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Rage {
    pub value: f32,
    /// Seconds since the last landed hit.
    pub idle: f32,
}

impl Rage {
    /// A landed hit of `damage`.
    pub fn gain(&mut self, damage: f32) {
        let chained = self.idle < CHAIN_WINDOW;
        self.value = (self.value + damage.max(0.0) / RAGE_FILL + if chained { CHAIN_BONUS } else { 0.0 }).min(1.0);
        self.idle = 0.0;
    }
    pub fn tick(&mut self, dt: f32) {
        let before = self.idle;
        self.idle += dt;
        // Only the time past the idle delay drains.
        let draining = (self.idle - before.max(IDLE)).max(0.0);
        self.value = (self.value - DECAY * draining).max(0.0);
    }
    pub fn full(&self) -> bool {
        self.value >= 1.0 - 1e-4
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Berserk {
    pub left: f32,
}

/// Fighters berserk this frame (read by `combat`: attack speed, damage dealt and taken).
#[derive(Resource, Default)]
pub struct Berserkers(pub HashSet<Entity>);

/// One rising ring of the aura: where it is up the taper (0 base .. 1 tip), how fast it rises, its own size
/// angle round the spine and swirl, distance out (share of the taper), size, oval squash and roll, thickness and
/// whether it is a bright highlight.
#[derive(Clone, Copy, Debug)]
struct Ring {
    entity: Entity,
    s: f32,
    rate: f32,
    angle: f32,
    swirl: f32,
    out: f32,
    size: f32,
    squash: Vec2,
    roll: f32,
    mesh: usize,
    bright: bool,
}

/// The aura on a fighter: its spine (positions and velocities), age, its rings and the arc clock.
#[derive(Component)]
struct Flame {
    owner: Entity,
    points: [Vec3; NODES],
    vel: [Vec3; NODES],
    age: f32,
    /// Seconds since the berserk ended (fading out), if it has.
    ending: Option<f32>,
    rings: Vec<Ring>,
    arc: f32,
    seed: u32,
}

#[derive(Resource)]
struct FlameKit {
    /// Ring meshes: medium, thin, hairline.
    meshes: [Handle<Mesh>; 3],
    /// Brightness levels (dim to full) for the aura colour, and for the highlight colour.
    dim: Vec<Handle<StandardMaterial>>,
    bright: Vec<Handle<StandardMaterial>>,
}

/// xorshift32 → 0..1.
fn rand01(state: &mut u32) -> f32 {
    let mut x = (*state).max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

fn new_ring(entity: Entity, seed: &mut u32, s: f32) -> Ring {
    let mut r = || rand01(seed);
    let bright = r() < HIGHLIGHT;
    Ring {
        entity,
        s,
        rate: 1.0 / (RISE.0 + (RISE.1 - RISE.0) * r()),
        angle: r() * std::f32::consts::TAU,
        swirl: (r() - 0.5) * 6.0,
        out: r().sqrt() * 0.75,
        size: 0.18 + 0.5 * r() * r() + if bright { 0.0 } else { 0.08 },
        squash: Vec2::new(0.8 + 0.4 * r(), 0.8 + 0.4 * r()),
        roll: r() * std::f32::consts::TAU,
        mesh: if bright { 2 } else { (r() * 2.0) as usize },
        bright,
    }
}

/// Brightness of a ring at height share `s`: fades in off the base and out toward the tip.
pub fn ring_glow(s: f32) -> f32 {
    if !(0.0..=1.0).contains(&s) {
        return 0.0;
    }
    (std::f32::consts::PI * s).sin().max(0.0).powf(0.7)
}

#[derive(Component)]
struct RageBar;
#[derive(Component)]
struct RageFill;

/// The old spring-driven ring aura is retired in favour of the pixel flame billboards (`pixel_aura`).
const RING_AURA: bool = false;

pub fn plugin(app: &mut App) {
    app.init_resource::<Berserkers>()
        .register_required_components::<Fighter, Rage>()
        .add_systems(Startup, load_kit)
        .add_systems(Update, (fill, smoke, trigger, tick_berserk, flame, pixel_aura, bar).chain());
}

fn load_kit(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // Additive and dim: a ring adds its glow over the scene, so a low colour reads as a low alpha.
    let level = |materials: &mut Assets<StandardMaterial>, c: LinearRgba, k: usize| {
        let a = (k + 1) as f32 / LEVELS as f32;
        vfx::glow(materials, LinearRgba::rgb(c.red * a, c.green * a, c.blue * a))
    };
    // Many rings overlap along any line of sight, so each is faint; highlights stay hot.
    let aura = LinearRgba::rgb(0.32, 0.09, 0.025);
    let hot = LinearRgba::rgb(1.5, 1.1, 0.6);
    let dim = (0..LEVELS).map(|k| level(&mut materials, aura, k)).collect();
    let bright = (0..LEVELS).map(|k| level(&mut materials, hot, k)).collect();
    commands.insert_resource(FlameKit {
        meshes: [meshes.add(vfx::soft_ring(0.6, 0.85, 1.0)), meshes.add(vfx::soft_ring(0.75, 0.9, 1.0)), meshes.add(vfx::soft_ring(0.84, 0.94, 1.0))],
        dim,
        bright,
    });
}

/// Landed hits fill their attacker's rage; idle rage drains.
fn fill(time: Res<Time>, mut hits: MessageReader<Hit>, mut rages: Query<&mut Rage>) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for mut r in &mut rages {
        r.tick(dt);
    }
    for h in hits.read() {
        if h.attacker != h.target
            && h.damage > 0.0
            && let Ok(mut r) = rages.get_mut(h.attacker)
        {
            r.gain(h.damage);
        }
    }
}

/// `GB_BERSERK=<s>`: the player's rage fills at that time and berserk follows. With `GB_REEL=<dir>` it also saves
/// stills of the aura (`berserk_0.png` ..) a few seconds apart.
fn smoke(time: Res<Time>, mut commands: Commands, mut filled: Local<Option<f32>>, mut shots: Local<usize>, mut players: Query<&mut Rage, With<Player>>) {
    const STILLS: [f32; 4] = [1.2, 2.6, 4.0, 5.4];
    let Some(at) = std::env::var("GB_BERSERK").ok().and_then(|v| v.trim().parse::<f32>().ok()) else { return };
    let now = time.elapsed_secs();
    if filled.is_none() && now >= at {
        *filled = Some(now);
        for mut r in &mut players {
            r.value = 1.0;
        }
    }
    if let (Some(since), Some(dir)) = (*filled, super::flipbook::reel_dir())
        && *shots < STILLS.len()
        && now - since >= STILLS[*shots]
    {
        super::flipbook::shoot(&mut commands, format!("{dir}/berserk_{}.png", *shots));
        *shots += 1;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn trigger(
    mut commands: Commands,
    vfx: Option<Res<Vfx>>,
    mut hitstop: ResMut<Hitstop>,
    mut shake: ResMut<CameraShake>,
    mut log: MessageWriter<HudLog>,
    mut fighters: Query<(Entity, &Transform, &mut Rage, &Fighter, Option<&Intent>, Has<Player>, Has<Npc>, Option<&Name>), Without<Berserk>>,
) {
    let smoke = std::env::var_os("GB_BERSERK").is_some();
    for (e, tf, mut rage, f, intent, player, npc, name) in &mut fighters {
        if f.dead || !rage.full() {
            continue;
        }
        let pressed = intent.is_some_and(|i| i.berserk_pressed);
        if !(pressed || npc || (player && smoke)) {
            continue;
        }
        rage.value = 0.0;
        commands.entity(e).try_insert(Berserk { left: DURATION });
        if let Some(vfx) = &vfx {
            vfx::burst(&mut commands, vfx.burst(Element::Aura), tf.translation, 1.8);
            vfx::burst(&mut commands, vfx.burst(Element::Fire), tf.translation + Vec3::Y * 0.6, 1.4);
            vfx::burst(&mut commands, vfx.shock(), tf.translation - Vec3::Y * 0.8, 2.0);
        }
        let feet = tf.translation - Vec3::Y * 0.8;
        commands.write_message(Flip::new(Sheet::Sunburn, feet + Vec3::Y * 1.2, 6.0, 1.1).glow(1.3));
        commands.write_message(Flip::new(Sheet::Vortex, feet + Vec3::Y * 0.1, 6.5, 1.0).flat().spin(4.0).tinted(Color::srgb(1.0, 0.35, 0.1), 0.65));
        commands.write_message(super::fracture::Shatter { at: tf.translation, radius: 4.5, force: 50.0 });
        hitstop.trigger(0.25);
        if player {
            shake.add(0.45);
        }
        let who = name.map_or(if player { "You" } else { "?" }.to_string(), |n| n.to_string());
        info!("[berserk] {who} goes berserk");
        log.write(HudLog::new(LogKind::Warn, format!("{who}: BERSERK!")));
    }
}

/// The berserk aura: flat pixel-effect planes stacked up the body. Each climbs along Y while its edge shrinks to a point,
/// so the stack reads as a spinning cone. One plane leaves the feet every `AURA_RISE / AURA_LAYERS` seconds.
const AURA_RISE: f32 = 1.5;
const AURA_LAYERS: f32 = 14.0;
const AURA_HEIGHT: f32 = 2.8;
const AURA_SIZE: f32 = 3.6;
/// Sheets taken in turn, with the dark smoke plane every `SMOKE_EVERY` planes.
const AURA_SHEETS: [Sheet; 3] = [Sheet::Vortex, Sheet::FireSpin, Sheet::Vortex];
const SMOKE_EVERY: u32 = 3;

fn pixel_aura(time: Res<Time>, mut out: MessageWriter<Flip>, fighters: Query<Entity, With<Berserk>>, mut clocks: Local<HashMap<Entity, (f32, u32)>>) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    clocks.retain(|e, _| fighters.contains(*e));
    for e in &fighters {
        let c = clocks.entry(e).or_insert((0.0, 0));
        c.0 += dt;
        while c.0 >= AURA_RISE / AURA_LAYERS {
            c.0 -= AURA_RISE / AURA_LAYERS;
            c.1 += 1;
            let dir = if c.1 % 2 == 0 { 1.0 } else { -1.0 };
            let at = Vec3::Y * -0.75;
            // Level cameras see flat planes edge-on, so the default planes face the camera; `GB_AURA=flat` lays them down.
            let flat = std::env::var("GB_AURA").is_ok_and(|v| v == "flat");
            let lay = |f: Flip| if flat { f.flat() } else { f };
            if c.1 % SMOKE_EVERY == 0 {
                out.write(lay(Flip::new(Sheet::Midnight, Vec3::ZERO, AURA_SIZE * 0.8, AURA_RISE * 1.2).rising(AURA_HEIGHT).spin(dir * 1.5).smoke().tinted(Color::srgb(0.35, 0.05, 0.02), 0.95).follow(e, at)));
            } else {
                let sheet = AURA_SHEETS[c.1 as usize % AURA_SHEETS.len()];
                out.write(lay(Flip::new(sheet, Vec3::ZERO, AURA_SIZE, AURA_RISE).rising(AURA_HEIGHT).spin(dir * 3.0).tinted(Color::srgb(1.0, 0.4, 0.12), 0.45).glow(1.6).follow(e, at)));
            }
        }
    }
}

/// Count berserk down and keep the `Berserkers` set in step.
fn tick_berserk(time: Res<Time>, mut commands: Commands, mut set: ResMut<Berserkers>, mut q: Query<(Entity, &mut Berserk, &Fighter)>) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    set.0.clear();
    for (e, mut b, f) in &mut q {
        b.left -= dt;
        if b.left <= 0.0 || f.dead {
            commands.entity(e).try_remove::<Berserk>();
            info!("[berserk] ends");
        } else {
            set.0.insert(e);
        }
    }
}

/// Spawn, drive and retire the ring aura on every berserk fighter.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn flame(
    time: Res<Time>,
    mut commands: Commands,
    kit: Option<Res<FlameKit>>,
    owners: Query<(Entity, &Transform, &Mover, Has<Berserk>), Without<Flame>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flames: Query<(Entity, &mut Flame)>,
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    mut rings: Query<(&mut Transform, &mut Mesh3d, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility), (Without<Flame>, Without<Mover>)>,
) {
    let Some(kit) = kit else { return };
    let dt = time.delta_secs().clamp(0.0, 0.05);
    let t = time.elapsed_secs();
    let eye = cameras.iter().next().map(|c| c.translation());
    // New auras: a pool of rings spread up the taper so it starts full.
    let lit: HashSet<Entity> = flames.iter().map(|(_, f)| f.owner).collect();
    for (e, tf, _, berserk) in &owners {
        if RING_AURA && berserk && !lit.contains(&e) {
            let base = tf.translation - Vec3::Y * 0.8;
            let points = std::array::from_fn(|i| base + rest_offset(i as f32 / (NODES - 1) as f32, Vec3::ZERO));
            let mut seed = (e.to_bits() as u32) | 1;
            let rings = (0..RINGS)
                .map(|k| {
                    let id = commands
                        .spawn((
                            Name::new("Aura ring"),
                            Mesh3d(kit.meshes[0].clone()),
                            MeshMaterial3d(kit.dim[0].clone()),
                            Transform::from_translation(base),
                            Visibility::Hidden,
                            bevy::light::NotShadowCaster,
                        ))
                        .id();
                    new_ring(id, &mut seed, k as f32 / RINGS as f32)
                })
                .collect();
            commands.spawn((Name::new("Berserk aura"), Flame { owner: e, points, vel: [Vec3::ZERO; NODES], age: 0.0, ending: None, rings, arc: 0.0, seed }));
        }
    }
    for (fe, mut f) in &mut flames {
        let Ok((_, tf, mover, berserk)) = owners.get(f.owner) else {
            for r in f.rings.drain(..) {
                commands.entity(r.entity).try_despawn();
            }
            commands.entity(fe).despawn();
            continue;
        };
        f.age += dt;
        if !berserk {
            *f.ending.get_or_insert(0.0) += dt;
        }
        if f.ending.is_some_and(|e| e >= FADE) {
            for r in f.rings.drain(..) {
                commands.entity(r.entity).try_despawn();
            }
            commands.entity(fe).despawn();
            continue;
        }
        let size = (f.age / GROW).min(1.0) * f.ending.map_or(1.0, |e| (1.0 - e / FADE).max(0.0)) * (1.0 + 0.05 * (t * 7.0).sin());
        // Spine: the base rides the fighter; every other point springs toward its rest, bent back by motion.
        let base = tf.translation - Vec3::Y * 0.8;
        let v = mover.velocity;
        let n = ((dt / 0.004).ceil() as usize).clamp(1, 16);
        let h = dt / n as f32;
        for _ in 0..n {
            f.points[0] = base;
            for i in 1..NODES {
                let s = i as f32 / (NODES - 1) as f32;
                let want = base + rest_offset(s, v) * size.max(0.05);
                let pull = STIFF * (want - f.points[i]) - DAMP * f.vel[i];
                f.vel[i] += pull * h;
                let step = f.vel[i] * h;
                f.points[i] += step;
            }
        }
        let points = f.points;
        let along = |s: f32| -> (Vec3, Vec3) {
            let x = s.clamp(0.0, 1.0) * (NODES - 1) as f32;
            let i = (x.floor() as usize).min(NODES - 2);
            let u = x - i as f32;
            (points[i].lerp(points[i + 1], u), (points[i + 1] - points[i]).normalize_or(Vec3::Y))
        };
        // Rings rise fast up the taper; a ring past the tip is reborn at the base with new looks.
        let mut seed = f.seed;
        for r in f.rings.iter_mut() {
            r.s += r.rate * dt;
            if r.s >= 1.0 {
                let id = r.entity;
                *r = new_ring(id, &mut seed, r.s - 1.0);
            }
            let Ok((mut rt, mut mesh, mut mat, mut vis)) = rings.get_mut(r.entity) else { continue };
            let (on, up) = along(r.s);
            let body = 0.5 * WIDTH * taper(r.s) * size;
            let side = Quat::from_rotation_arc(Vec3::Y, up) * Vec3::new((r.angle + t * r.swirl).cos(), 0.0, (r.angle + t * r.swirl).sin());
            let at = on + side * body * r.out;
            // The ring's size shrinks with the taper too, so the tip stays a point.
            let radius = WIDTH * r.size * (0.35 + 0.65 * taper(r.s)) * size.min(1.0) * if r.s > 0.7 { taper(r.s) / taper(0.7) } else { 1.0 };
            let glow = ring_glow(r.s) * size.min(1.0);
            let level = ((glow * LEVELS as f32) as usize).min(LEVELS - 1);
            let face = eye.map_or(Vec3::Z, |e| (e - at).normalize_or(Vec3::Z));
            *rt = Transform::from_translation(at)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, face) * Quat::from_rotation_y(r.roll + t * r.swirl))
                .with_scale(Vec3::new(radius * r.squash.x, 1.0, radius * r.squash.y));
            mesh.0 = kit.meshes[r.mesh].clone();
            mat.0 = if r.bright { kit.bright[level].clone() } else { kit.dim[level].clone() };
            *vis = if radius > 0.01 && glow > 0.05 { Visibility::Visible } else { Visibility::Hidden };
        }
        // Electricity: short arcs crackle through the aura.
        f.arc -= dt;
        if f.arc <= 0.0 && size > 0.3 {
            f.arc = ARC_EVERY;
            let s0 = 0.15 + 0.6 * rand01(&mut seed);
            let (a, _) = along(s0);
            let ang = rand01(&mut seed) * std::f32::consts::TAU;
            let out = Vec3::new(ang.cos(), 0.0, ang.sin()) * 0.5 * WIDTH * taper(s0) * size;
            let b = a + out + Vec3::new(rand01(&mut seed) - 0.5, rand01(&mut seed) * 0.8, rand01(&mut seed) - 0.5).normalize_or(Vec3::Y) * ARC_LEN;
            vfx::spawn_bolt(&mut commands, &mut meshes, &mut materials, a + out * 0.5, b, seed);
        }
        f.seed = seed;
    }
}

/// The player's rage bar above the hotbar.
fn bar(time: Res<Time>, mut commands: Commands, players: Query<(&Rage, Has<Berserk>), With<Player>>, bars: Query<Entity, With<RageBar>>, mut fill: Query<(&mut Node, &mut BackgroundColor), With<RageFill>>) {
    let Ok((rage, berserk)) = players.single() else { return };
    let show = rage.value > 0.001 || berserk;
    if !show {
        for e in &bars {
            commands.entity(e).despawn();
        }
        return;
    }
    if bars.is_empty() {
        commands
            .spawn((
                RageBar,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(112),
                    left: Val::Percent(42.0),
                    width: Val::Percent(16.0),
                    height: px(8),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(Color::srgba(1.0, 0.6, 0.3, 0.8)),
                BackgroundColor(Color::srgba(0.1, 0.03, 0.02, 0.7)),
            ))
            .with_child((RageFill, Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgb(0.95, 0.35, 0.1))));
    }
    let pulse = if rage.full() || berserk { 0.75 + 0.25 * (time.elapsed_secs() * 8.0).sin() } else { 1.0 };
    for (mut n, mut bg) in &mut fill {
        n.width = Val::Percent(if berserk { 100.0 } else { rage.value * 100.0 });
        bg.0 = Color::srgb(0.95 * pulse, 0.35 * pulse, 0.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flame_is_widest_below_the_middle_and_ends_in_a_point() {
        assert!(taper(1.0) < 1e-3, "tip {}", taper(1.0));
        assert_eq!(taper(-0.1), 0.0);
        let (mut best, mut at) = (0.0, 0.0);
        for i in 0..=100 {
            let s = i as f32 / 100.0;
            assert!(taper(s) >= 0.0);
            if taper(s) > best {
                (best, at) = (taper(s), s);
            }
        }
        assert!((0.35..0.5).contains(&at), "widest at {at}");
    }

    #[test]
    fn moving_forward_bends_the_tip_back() {
        let still = rest_offset(1.0, Vec3::ZERO);
        let run = rest_offset(1.0, Vec3::new(6.0, 0.0, 0.0));
        assert!(still.x.abs() < 1e-6);
        assert!(run.x < -0.5, "tip trails behind: {run}");
        // The base barely moves; the bend grows toward the tip.
        assert!(rest_offset(0.2, Vec3::new(6.0, 0.0, 0.0)).x.abs() < run.x.abs() * 0.1);
    }

    #[test]
    fn rings_fade_in_off_the_base_and_out_at_the_tip() {
        assert!(ring_glow(0.0) < 1e-3 && ring_glow(1.0) < 1e-3);
        assert!(ring_glow(0.5) > 0.99);
        assert_eq!(ring_glow(1.5), 0.0);
        // Ring looks vary: sizes, squash and rise rates differ ring to ring.
        let mut seed = 7;
        let rings: Vec<Ring> = (0..40).map(|k| new_ring(Entity::PLACEHOLDER, &mut seed, k as f32 / 40.0)).collect();
        let sizes: Vec<f32> = rings.iter().map(|r| r.size).collect();
        assert!(sizes.iter().cloned().fold(0.0, f32::max) - sizes.iter().cloned().fold(9.0, f32::min) > 0.5);
        assert!(rings.iter().any(|r| r.bright) && rings.iter().any(|r| !r.bright));
        assert!(rings.iter().all(|r| (1.0 / RISE.1..=1.0 / RISE.0).contains(&r.rate)));
    }

    #[test]
    fn rage_fills_from_hits_chains_and_drains_when_idle() {
        let mut r = Rage { value: 0.0, idle: 10.0 };
        r.gain(30.0);
        assert!((r.value - 0.5).abs() < 1e-5, "{}", r.value);
        r.tick(0.5);
        r.gain(30.0);
        assert!(r.full(), "a chained hit fills it: {}", r.value);
        r.tick(IDLE + 1.0);
        assert!(r.value < 1.0 && r.value > 0.9);
        r.tick(30.0);
        assert_eq!(r.value, 0.0);
    }
}
