//! Berserker: a rage meter that fills as blows land, and a berserk state wrapped in a pixel-flame aura.
//!
//! - `Rage` (every `Fighter`): each melee or spell `Hit` a fighter lands adds `damage / RAGE_FILL`, plus
//!   `CHAIN_BONUS` when it follows the last within `CHAIN_WINDOW` s. After `IDLE` s without a hit it drains at
//!   `DECAY` per second.
//! - Full rage: the player presses V (`Intent::berserk_pressed`), an NPC goes berserk by itself. `Berserk` lasts
//!   `DURATION` s: attack speed ×`SPEED`, damage dealt ×`MIGHT`, damage taken ×`FRAGILE` (`combat` reads the
//!   `Berserkers` set), with a roar burst (aura + fire, a slow-motion beat, a camera kick). Rage empties.
//! - The flame (`Flame`): `LAYERS` camera-facing quads up a spine of `NODES` spring-driven points rising from the
//!   fighter, each showing a frame of the pixel flame sheet (`assets/vfx/pixel_flame`, CC0) at its own offset,
//!   so the stacked frames shimmer as one shape. Width follows `taper` (widest a little below the middle, a
//!   point at the tip). Each spine point springs toward straight-up minus the fighter's ground velocity ×
//!   `LAG` × s², so moving forward bends the tip back and stopping lets it swing up and settle. Additive,
//!   unlit, no depth write, no shadow, no collider: see-through and not solid. It grows in on the trigger,
//!   breathes while berserk and shrinks out at the end.
//! - HUD: a rage bar above the hotbar while the player has rage; it pulses when full.
//! - `GB_BERSERK=<seconds>` fills the player's rage and triggers berserk at that time (smoke runs).
use super::combat::Hitstop;
use super::camera::{CameraShake, MainCamera};
use super::hud::log::{HudLog, LogKind};
use super::vfx::{self, Element, Vfx};
use super::{Fighter, Hit, Intent, Mover, Npc, Player};
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use std::collections::HashSet;

pub const RAGE_FILL: f32 = 60.0;
pub const CHAIN_BONUS: f32 = 0.04;
pub const CHAIN_WINDOW: f32 = 1.2;
pub const IDLE: f32 = 3.0;
pub const DECAY: f32 = 0.08;
pub const DURATION: f32 = 8.0;
pub const SPEED: f32 = 1.35;
pub const MIGHT: f32 = 1.3;
pub const FRAGILE: f32 = 1.2;
/// Flame: spine points, quads, height and widest width (m), lag (s), spring stiffness and damping.
const NODES: usize = 10;
const LAYERS: usize = 28;
const HEIGHT: f32 = 2.6;
const WIDTH: f32 = 1.3;
const LAG: f32 = 0.22;
const STIFF: f32 = 90.0;
const DAMP: f32 = 12.0;
/// Grow-in and fade-out times (s).
const GROW: f32 = 0.3;
const FADE: f32 = 0.5;
const SHEET: &str = "vfx/pixel_flame/flame.png";
/// The flame sheet's looping frames (`frames.json` loop 1..4 of 7 equal cells) and their rate.
const SHEET_CELLS: usize = 7;
const LOOP: [usize; 4] = [1, 2, 3, 4];
const FPS: f32 = 12.0;

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

/// The flame on a fighter: its spine (positions and velocities), age, and the quads.
#[derive(Component)]
struct Flame {
    owner: Entity,
    points: [Vec3; NODES],
    vel: [Vec3; NODES],
    age: f32,
    /// Seconds since the berserk ended (fading out), if it has.
    ending: Option<f32>,
    quads: Vec<Entity>,
}

#[derive(Resource)]
struct FlameKit {
    mesh: Handle<Mesh>,
    frames: Vec<Handle<StandardMaterial>>,
}

#[derive(Component)]
struct RageBar;
#[derive(Component)]
struct RageFill;

pub fn plugin(app: &mut App) {
    app.init_resource::<Berserkers>()
        .register_required_components::<Fighter, Rage>()
        .add_systems(Startup, load_kit)
        .add_systems(Update, (fill, smoke, trigger, tick_berserk, flame, bar).chain());
}

fn load_kit(mut commands: Commands, assets: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    // Pixel art: sampled nearest so the frames keep their pixels.
    let sheet: Handle<Image> = assets.load_with_settings(SHEET, |s: &mut ImageLoaderSettings| s.sampler = ImageSampler::nearest());
    let cell = 1.0 / SHEET_CELLS as f32;
    let frames = LOOP
        .iter()
        .map(|&i| {
            materials.add(StandardMaterial {
                base_color: Color::LinearRgba(LinearRgba::rgb(2.2, 1.3, 0.8)),
                base_color_texture: Some(sheet.clone()),
                uv_transform: bevy::math::Affine2::from_scale_angle_translation(Vec2::new(cell, 1.0), 0.0, Vec2::new(i as f32 * cell, 0.0)),
                unlit: true,
                alpha_mode: AlphaMode::Add,
                cull_mode: None,
                ..default()
            })
        })
        .collect();
    commands.insert_resource(FlameKit { mesh: meshes.add(Rectangle::new(1.0, 1.0)), frames });
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

/// `GB_BERSERK=<s>`: the player's rage fills at that time and berserk follows.
fn smoke(time: Res<Time>, mut done: Local<bool>, mut players: Query<&mut Rage, With<Player>>) {
    if *done {
        return;
    }
    let Some(at) = std::env::var("GB_BERSERK").ok().and_then(|v| v.trim().parse::<f32>().ok()) else {
        *done = true;
        return;
    };
    if time.elapsed_secs() >= at {
        *done = true;
        for mut r in &mut players {
            r.value = 1.0;
        }
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
        hitstop.trigger(0.25);
        if player {
            shake.add(0.45);
        }
        let who = name.map_or(if player { "You" } else { "?" }.to_string(), |n| n.to_string());
        info!("[berserk] {who} goes berserk");
        log.write(HudLog::new(LogKind::Warn, format!("{who}: BERSERK!")));
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

/// Spawn, drive and retire the flame on every berserk fighter.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn flame(
    time: Res<Time>,
    mut commands: Commands,
    kit: Option<Res<FlameKit>>,
    owners: Query<(Entity, &Transform, &Mover, Has<Berserk>), Without<Flame>>,
    camera: Query<&GlobalTransform, With<MainCamera>>,
    mut flames: Query<(Entity, &mut Flame)>,
    mut quads: Query<(&mut Transform, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility), (Without<Flame>, Without<Mover>)>,
) {
    let Some(kit) = kit else { return };
    let dt = time.delta_secs().clamp(0.0, 0.05);
    let t = time.elapsed_secs();
    let eye = camera.single().map(|g| g.translation()).unwrap_or(Vec3::new(0.0, 3.0, 10.0));
    // New flames.
    let lit: HashSet<Entity> = flames.iter().map(|(_, f)| f.owner).collect();
    for (e, tf, _, berserk) in &owners {
        if berserk && !lit.contains(&e) {
            let base = tf.translation - Vec3::Y * 0.8;
            let points = std::array::from_fn(|i| base + rest_offset(i as f32 / (NODES - 1) as f32, Vec3::ZERO));
            let quads = (0..LAYERS)
                .map(|k| {
                    commands
                        .spawn((
                            Name::new("Flame layer"),
                            Mesh3d(kit.mesh.clone()),
                            MeshMaterial3d(kit.frames[k % kit.frames.len()].clone()),
                            Transform::from_translation(base),
                            Visibility::Hidden,
                            bevy::light::NotShadowCaster,
                        ))
                        .id()
                })
                .collect();
            commands.spawn((Name::new("Berserk flame"), Flame { owner: e, points, vel: [Vec3::ZERO; NODES], age: 0.0, ending: None, quads }));
        }
    }
    for (fe, mut f) in &mut flames {
        let Ok((_, tf, mover, berserk)) = owners.get(f.owner) else {
            for q in f.quads.drain(..) {
                commands.entity(q).try_despawn();
            }
            commands.entity(fe).despawn();
            continue;
        };
        f.age += dt;
        if !berserk {
            let e = f.ending.get_or_insert(0.0);
            *e += dt;
        }
        let size = (f.age / GROW).min(1.0) * f.ending.map_or(1.0, |e| (1.0 - e / FADE).max(0.0)) * (1.0 + 0.06 * (t * 7.0).sin());
        if f.ending.is_some_and(|e| e >= FADE) {
            for q in f.quads.drain(..) {
                commands.entity(q).try_despawn();
            }
            commands.entity(fe).despawn();
            continue;
        }
        // Spine: the base rides the fighter; every other point springs toward its rest, bent back by motion.
        let base = tf.translation - Vec3::Y * 0.8;
        let v = mover.velocity;
        let n = ((dt / 0.004).ceil() as usize).clamp(1, 16);
        let h = dt / n as f32;
        for _ in 0..n {
            f.points[0] = base;
            for i in 1..NODES {
                let s = i as f32 / (NODES - 1) as f32;
                let want = base + rest_offset(s, v) * size.max(0.05) + Vec3::new((t * 3.1 + s * 4.0).sin(), 0.0, (t * 2.3 + s * 3.0).cos()) * 0.04 * s;
                let pull = STIFF * (want - f.points[i]) - DAMP * f.vel[i];
                f.vel[i] += pull * h;
                let step = f.vel[i] * h;
                f.points[i] += step;
            }
        }
        // Quads: frames advance at FPS, each layer offset, sized by the taper, facing the camera about the spine.
        let frame = (t * FPS) as usize;
        for (k, &q) in f.quads.iter().enumerate() {
            let Ok((mut qt, mut mat, mut vis)) = quads.get_mut(q) else { continue };
            let s = (k as f32 + 0.5) / LAYERS as f32;
            let x = s * (NODES - 1) as f32;
            let i = (x.floor() as usize).min(NODES - 2);
            let u = x - i as f32;
            let at = f.points[i].lerp(f.points[i + 1], u);
            let up = (f.points[i + 1] - f.points[i]).normalize_or(Vec3::Y);
            let to_eye = (eye - at).normalize_or(Vec3::Z);
            let side = up.cross(to_eye).normalize_or(Vec3::X);
            let face = side.cross(up);
            let w = WIDTH * taper(s) * size;
            let layer_h = HEIGHT * size * 2.4 / LAYERS as f32 * 2.0;
            *qt = Transform::from_translation(at + face * 0.01 * k as f32)
                .with_rotation(Quat::from_mat3(&Mat3::from_cols(side, up, face)))
                .with_scale(Vec3::new(w.max(0.001), layer_h * (0.6 + 0.8 * taper(s)), 1.0));
            mat.0 = kit.frames[(frame + k * 3) % kit.frames.len()].clone();
            *vis = if w > 0.01 { Visibility::Visible } else { Visibility::Hidden };
        }
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
