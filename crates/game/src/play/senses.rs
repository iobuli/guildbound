//! Enemy senses (`Update / PlaySet::Think`, before `ai`): detection, not combat. Rays are code only
//! (no visuals unless a debug view is on) and cheap: each enemy senses at `RATE` Hz, staggered.
//! - Sight: the player inside an enemy's cone (`Senses.range`, `half_angle`; shorter at night) is
//!   seen if one ray from its eyes to the player's chest meets no world geometry.
//! - Aura as a magnet: the player's `AuraPresence.power` pulls the enemy's fan of `RAYS` sense
//!   rays like gravity. A ray passing `d` metres from the chest feels `power / (d + 0.5)²`; only a
//!   pull of `CAPTURE` or more bends it onto the player (weak pulls do nothing), and the bent ray
//!   is still stopped by objects. Strong auras are easy to sense; crouching, the Veil art
//!   (`Veiled`) and a spent Battlewill pool quieten it, an Aura Ward or sprinting makes it louder.
//! - `Awareness` (0..1) rises with what each tick caught and decays without it; at 1 the enemy is
//!   alerted (log line, HUD line) and only then may `ai` pick the player as a foe. It forgets
//!   after `FORGET` s with no contact. A blow from the player or the player within `TOUCH` m
//!   alerts at once.
//! - Enrolment: NPCs that are not civilians get `Senses` + `Awareness` when they appear (so unit
//!   tests that build bare NPCs keep their old behaviour: no `Awareness` = always aware).
//! - Debug views (`debug::View`): vision volumes (a procedural cone mesh per enemy drawn with the
//!   wind-aura shader, tinted by awareness), sense rays, the player's aura capture radius, and an
//!   awareness bar over each enemy.
use super::debug::{DebugToggles, View, shown};
use super::magic::{Energy, Ward};
use super::occupations::Civilian;
use super::stats::Stats;
use super::wind_aura::{WindAura, WindAuraParams};
use super::{HALF_HEIGHT, Hit, Mover, Npc, Phys, PlaySet, Player};
use super::hud::log::{HudLog, LogKind};
use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Senses per second per enemy; rays in the fan.
const RATE: f32 = 6.0;
const RAYS: usize = 7;
/// Eyes above the body centre; the player's chest above theirs.
const EYE: f32 = 0.62;
const CHEST: f32 = 0.35;
/// Default sight: range (m), cone half-angle (deg); night multiplies the range.
const RANGE: f32 = 20.0;
const HALF_ANGLE: f32 = 55.0;
const NIGHT_RANGE: f32 = 0.6;
/// Pull a ray needs to bend onto the player.
pub const CAPTURE: f32 = 0.5;
/// Awareness per second at full strength, decay per second, seconds to forget, touch distance.
const NOTICE_RATE: f32 = 0.9;
const DECAY: f32 = 0.2;
const FORGET: f32 = 8.0;
const TOUCH: f32 = 2.0;
/// Aura power modifiers.
const CROUCH: f32 = 0.5;
const SPRINT: f32 = 1.4;
const WARDED: f32 = 1.6;
pub const VEILED: f32 = 0.1;

/// How an enemy senses.
#[derive(Component, Clone, Debug)]
pub struct Senses {
    pub range: f32,
    pub half_angle: f32,
    /// Seconds to the next sense tick.
    next: f32,
    /// Last tick's rays for the debug view: (from, to, kind) — 0 missed, 1 blocked, 2 sight, 3 pulled.
    pub rays: Vec<(Vec3, Vec3, u8)>,
}

impl Default for Senses {
    fn default() -> Self {
        Self { range: RANGE, half_angle: HALF_ANGLE.to_radians(), next: 0.0, rays: Vec::new() }
    }
}

/// What an enemy knows of the player (written here only).
#[derive(Component, Clone, Debug, Default)]
pub struct Awareness {
    pub level: f32,
    pub alerted: bool,
    /// Seconds since the last contact.
    since: f32,
}

impl Awareness {
    pub fn alert(&mut self) {
        self.level = 1.0;
        self.alerted = true;
        self.since = 0.0;
    }
}

/// The player's aura as others feel it (written here only).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct AuraPresence {
    pub power: f32,
}

impl AuraPresence {
    /// Distance (m) from the chest within which a passing ray is captured.
    pub fn capture_radius(self) -> f32 {
        ((self.power / CAPTURE).sqrt() - 0.5).max(0.0)
    }
}

/// The Veil art's hush on the aura: seconds left.
#[derive(Component, Clone, Copy, Debug)]
pub struct Veiled(pub f32);

/// What one sense tick found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contact {
    /// In the cone, nothing in the way: strength by distance.
    Sight(f32),
    /// A fan ray bent onto the player by the aura: strength by pull.
    Aura(f32),
}

impl Contact {
    pub fn strength(self) -> f32 {
        match self {
            Contact::Sight(s) | Contact::Aura(s) => s,
        }
    }
}

/// Fan directions across the cone (horizontal), unit vectors.
pub fn fan(forward: Vec3, half_angle: f32) -> [Vec3; RAYS] {
    let f = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::Z);
    std::array::from_fn(|i| {
        let t = i as f32 / (RAYS - 1) as f32 * 2.0 - 1.0;
        Quat::from_rotation_y(t * half_angle) * f
    })
}

/// One sense tick, without the world: `clear(from, to)` says whether nothing blocks a ray.
/// Sight wins over aura; an aura ray is captured only at `CAPTURE` pull or more.
pub fn sense(
    eye: Vec3,
    forward: Vec3,
    range: f32,
    half_angle: f32,
    target: Vec3,
    aura: AuraPresence,
    mut clear: impl FnMut(Vec3, Vec3) -> bool,
    mut record: impl FnMut(Vec3, Vec3, u8),
) -> Option<Contact> {
    let to = target - eye;
    let dist = to.length();
    let flat_fwd = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::Z);
    let in_cone = dist < range && Vec3::new(to.x, 0.0, to.z).normalize_or(flat_fwd).angle_between(flat_fwd) <= half_angle;
    if in_cone {
        if clear(eye, target) {
            record(eye, target, 2);
            return Some(Contact::Sight((1.0 - 0.6 * dist / range).clamp(0.2, 1.0)));
        }
        record(eye, target, 1);
    }
    let mut best: Option<f32> = None;
    for dir in fan(forward, half_angle) {
        // Closest approach of the ray (eye → eye + dir·range) to the target.
        let along = to.dot(dir).clamp(0.0, range);
        let d = (eye + dir * along).distance(target);
        let pull = aura.power / ((d + 0.5) * (d + 0.5));
        if pull >= CAPTURE && along > 0.0 {
            best = Some(best.map_or(pull, |b: f32| b.max(pull)));
            record(eye, eye + dir * along, 3);
        } else {
            record(eye, eye + dir * range, 0);
        }
    }
    let pull = best?;
    if !clear(eye, target) {
        record(eye, target, 1);
        return None;
    }
    record(eye, target, 3);
    Some(Contact::Aura((pull / (CAPTURE * 4.0)).clamp(0.15, 1.0)))
}

/// Awareness after `dt` s with `contact` (pure; `true` when it just became alerted).
pub fn integrate(a: &mut Awareness, contact: Option<Contact>, dt: f32) -> bool {
    let was = a.alerted;
    match contact {
        Some(c) => {
            a.since = 0.0;
            a.level = (a.level + c.strength() * NOTICE_RATE * dt).min(1.0);
            if a.level >= 1.0 {
                a.alerted = true;
            }
        }
        None => {
            a.since += dt;
            if a.alerted {
                if a.since >= FORGET {
                    a.alerted = false;
                    a.level = 0.29;
                }
            } else {
                a.level = (a.level - DECAY * dt).max(0.0);
            }
        }
    }
    a.alerted && !was
}

/// The player's aura power from level, Battlewill and what they are doing.
pub fn aura_power(level: u32, battlewill: f32, crouch: bool, sprint: bool, warded: bool, veiled: bool) -> f32 {
    let mut p = (1.0 + 0.08 * level as f32) * (0.6 + 0.8 * battlewill.clamp(0.0, 1.0));
    if crouch {
        p *= CROUCH;
    }
    if sprint {
        p *= SPRINT;
    }
    if warded {
        p *= WARDED;
    }
    if veiled {
        p *= VEILED;
    }
    p
}

/// Senses run before the AI reads `Awareness`.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SensesSet;

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (enroll, presence, tick).chain().in_set(PlaySet::Think).in_set(SensesSet),
    )
    .add_systems(
        Update,
        (
            volumes,
            rays.run_if(shown(View::SenseRays)),
            aura_radius.run_if(shown(View::AuraRadius)),
            meters.run_if(shown(View::Awareness)),
        ),
    );
}

#[allow(clippy::type_complexity)]
fn enroll(mut commands: Commands, npcs: Query<Entity, (With<Npc>, Without<Senses>, Without<Civilian>)>) {
    for (i, e) in npcs.iter().enumerate() {
        commands.entity(e).try_insert((
            Senses { next: (e.to_bits() as f32 * 0.37 + i as f32 * 0.11).fract() / RATE, ..default() },
            Awareness::default(),
        ));
    }
}

#[allow(clippy::type_complexity)]
fn presence(
    time: Res<Time>,
    mut commands: Commands,
    mut players: Query<
        (Entity, &Mover, Option<&Stats>, Option<&Energy>, Has<Ward>, Option<&mut Veiled>, Option<&mut AuraPresence>),
        With<Player>,
    >,
) {
    for (e, mover, stats, energy, warded, veiled, aura) in &mut players {
        let mut hushed = false;
        if let Some(mut v) = veiled {
            v.0 -= time.delta_secs();
            hushed = v.0 > 0.0;
            if !hushed {
                commands.entity(e).try_remove::<Veiled>();
            }
        }
        let bw = energy.map_or(1.0, |en| en.battlewill / en.max_battlewill.max(1.0));
        let power = aura_power(stats.map_or(1, |s| s.level), bw, mover.crouch, mover.sprinting, warded, hushed);
        match aura {
            Some(mut a) => a.power = power,
            None => {
                commands.entity(e).try_insert(AuraPresence { power });
            }
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tick(
    time: Res<Time>,
    toggles: Res<DebugToggles>,
    spatial: SpatialQuery,
    clock: Option<Res<super::sky::Clock>>,
    mut hits: MessageReader<Hit>,
    mut log: MessageWriter<HudLog>,
    players: Query<(Entity, &Transform, &AuraPresence), With<Player>>,
    mut enemies: Query<(Entity, &Transform, &mut Senses, &mut Awareness, Option<&Name>, &super::Fighter)>,
) {
    let Ok((player, ptf, aura)) = players.single() else { return };
    let dt = time.delta_secs();
    let struck: Vec<Entity> = hits.read().filter(|h| h.attacker == player).map(|h| h.target).collect();
    let night = clock.is_some_and(|c| c.is_night());
    let chest = ptf.translation + Vec3::Y * CHEST;
    let debug = toggles.get(View::SenseRays);
    let filter = SpatialQueryFilter::from_mask(Phys::World);
    for (e, tf, mut senses, mut aware, name, fighter) in &mut enemies {
        if fighter.dead {
            continue;
        }
        let who = name.map_or("Someone", |n| n.as_str());
        if struck.contains(&e) || tf.translation.distance(ptf.translation) < TOUCH {
            if !aware.alerted {
                info!("[senses] {who} alerted by touch");
            }
            aware.alert();
            continue;
        }
        senses.next -= dt;
        if senses.next > 0.0 {
            continue;
        }
        let step = 1.0 / RATE - senses.next.min(0.0);
        senses.next = 1.0 / RATE;
        let eye = tf.translation + Vec3::Y * EYE;
        let forward = tf.rotation * Vec3::Z;
        let range = senses.range * if night { NIGHT_RANGE } else { 1.0 };
        let mut recorded = Vec::new();
        let contact = sense(
            eye,
            forward,
            range,
            senses.half_angle,
            chest,
            *aura,
            |a, b| {
                let d = b - a;
                let len = d.length();
                Dir3::new(d).map_or(true, |dir| spatial.cast_ray(a, dir, len, true, &filter).is_none_or(|h| h.distance >= len - 0.05))
            },
            |a, b, k| {
                if debug {
                    recorded.push((a, b, k));
                }
            },
        );
        senses.rays = recorded;
        if integrate(&mut aware, contact, step) {
            let how = match contact {
                Some(Contact::Sight(_)) => "spots you",
                _ => "senses your aura",
            };
            info!("[senses] {who} alerted by {} (aura power {:.2})", if how == "spots you" { "sight" } else { "aura" }, aura.power);
            log.write(HudLog::new(LogKind::Warn, format!("{who} {how}")).key(7200 + e.index_u32() as u64));
        } else if !aware.alerted && aware.level == 0.29 {
            info!("[senses] {who} lost you");
        }
    }
}

/// A capped cone along +Z: apex at the origin, base `range` out, `segments` around.
pub fn vision_cone_mesh(range: f32, half_angle: f32, segments: usize) -> Mesh {
    let n = segments.max(3);
    let r = range * half_angle.tan();
    let mut positions = vec![[0.0, 0.0, 0.0]];
    let mut normals = vec![[0.0, 0.0, -1.0]];
    let mut uvs = vec![[0.5, 0.0]];
    let slope = half_angle.cos();
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        positions.push([c * r, s * r, range]);
        let side = Vec3::new(c * slope, s * slope, -half_angle.sin()).normalize();
        normals.push(side.to_array());
        uvs.push([i as f32 / n as f32, 1.0]);
    }
    // Cap centre.
    positions.push([0.0, 0.0, range]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push([0.5, 1.0]);
    let centre = (n + 1) as u32;
    let mut indices = Vec::with_capacity(n * 6);
    for i in 0..n as u32 {
        let (a, b) = (1 + i, 1 + (i + 1) % n as u32);
        indices.extend_from_slice(&[0, b, a, centre, a, b]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

/// The vision volume of one enemy (child entity) and its material.
#[derive(Component)]
struct Volume {
    owner: Entity,
    material: Handle<WindAura>,
}

/// Spawn, tint and show or hide the vision volumes.
#[allow(clippy::type_complexity)]
fn volumes(
    toggles: Res<DebugToggles>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WindAura>>,
    enemies: Query<(Entity, &Senses, &Awareness)>,
    mut cones: Query<(Entity, &Volume, &mut Visibility)>,
    mut made: Local<Vec<Entity>>,
) {
    let on = toggles.get(View::VisionVolumes);
    if on {
        for (e, senses, _) in &enemies {
            if made.contains(&e) {
                continue;
            }
            made.push(e);
            let material = materials.add(WindAura {
                params: WindAuraParams {
                    color: Vec4::new(0.3, 0.6, 1.0, 0.35),
                    wind: Vec4::new(0.0, 1.0, 1.5, 0.8),
                    clock: Vec4::new(0.0, 1.6, 0.0, 0.0),
                },
            });
            commands.spawn((
                Name::new("Vision volume"),
                Volume { owner: e, material: material.clone() },
                Mesh3d(meshes.add(vision_cone_mesh(senses.range, senses.half_angle, 24))),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, EYE, 0.0),
                ChildOf(e),
                bevy::light::NotShadowCaster,
            ));
        }
    }
    for (cone, v, mut vis) in &mut cones {
        let Ok((_, _, aware)) = enemies.get(v.owner) else {
            commands.entity(cone).try_despawn();
            made.retain(|m| *m != v.owner);
            continue;
        };
        *vis = if on { Visibility::Inherited } else { Visibility::Hidden };
        if on && let Some(mut m) = materials.get_mut(&v.material) {
            let l = aware.level;
            let c = if aware.alerted {
                Vec3::new(1.0, 0.2, 0.15)
            } else {
                Vec3::new(0.3, 0.6, 1.0).lerp(Vec3::new(1.0, 0.75, 0.2), l)
            };
            m.params.color = c.extend(0.35 + 0.4 * l);
            m.params.wind.z = 1.5 + 3.0 * l;
        }
    }
}

fn rays(mut gizmos: Gizmos, enemies: Query<&Senses>) {
    for s in &enemies {
        for &(a, b, k) in &s.rays {
            let c = match k {
                1 => Color::srgb(1.0, 0.2, 0.2),
                2 => Color::srgb(1.0, 1.0, 0.3),
                3 => Color::srgb(0.4, 0.9, 1.0),
                _ => Color::srgba(0.6, 0.6, 0.6, 0.35),
            };
            gizmos.line(a, b, c);
        }
    }
}

fn aura_radius(mut gizmos: Gizmos, players: Query<(&Transform, &AuraPresence), With<Player>>) {
    for (tf, a) in &players {
        let r = a.capture_radius();
        let chest = tf.translation + Vec3::Y * CHEST;
        gizmos.sphere(Isometry3d::from_translation(chest), r.max(0.05), Color::srgb(0.4, 0.9, 1.0));
    }
}

fn meters(mut gizmos: Gizmos, enemies: Query<(&Transform, &Awareness)>, camera: Query<&Transform, With<super::camera::MainCamera>>) {
    let right = camera.iter().next().map_or(Vec3::X, |c| c.rotation * Vec3::X);
    for (tf, a) in &enemies {
        let at = tf.translation + Vec3::Y * (HALF_HEIGHT + 0.45);
        let start = at - right * 0.4;
        gizmos.line(start, start + right * 0.8, Color::srgb(0.25, 0.25, 0.25));
        let c = if a.alerted { Color::srgb(1.0, 0.2, 0.15) } else { Color::srgb(1.0, 0.8, 0.2) };
        gizmos.line(start + Vec3::Y * 0.02, start + Vec3::Y * 0.02 + right * 0.8 * a.level, c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EYE_AT: Vec3 = Vec3::new(0.0, 1.6, 0.0);

    fn run(target: Vec3, power: f32, open: bool) -> Option<Contact> {
        sense(EYE_AT, Vec3::Z, 20.0, 55f32.to_radians(), target, AuraPresence { power }, |_, _| open, |_, _, _| {})
    }

    #[test]
    fn sight_sees_in_the_cone_and_walls_block_it() {
        assert!(matches!(run(Vec3::new(1.0, 1.4, 8.0), 0.0, true), Some(Contact::Sight(_))));
        assert_eq!(run(Vec3::new(1.0, 1.4, 8.0), 0.0, false), None);
        // Behind the enemy, no aura: unseen.
        assert_eq!(run(Vec3::new(0.0, 1.4, -6.0), 0.0, true), None);
    }

    #[test]
    fn the_aura_pulls_rays_only_when_strong() {
        // Just outside the cone's edge (a fan ray passes ~1.5 m away).
        let off = Quat::from_rotation_y(68f32.to_radians()) * Vec3::Z * 8.0 + Vec3::Y * 1.6;
        assert_eq!(run(off, 1.0, true), None, "a weak pull does nothing");
        assert!(matches!(run(off, 6.0, true), Some(Contact::Aura(_))), "a strong aura bends the ray");
        assert_eq!(run(off, 6.0, false), None, "objects still block a bent ray");
        // Veiled, the same strong aura is a whisper.
        assert_eq!(run(off, 6.0 * VEILED, true), None);
    }

    #[test]
    fn awareness_rises_alerts_decays_and_forgets() {
        let mut a = Awareness::default();
        let mut alerted = false;
        for _ in 0..30 {
            alerted |= integrate(&mut a, Some(Contact::Sight(1.0)), 0.1);
        }
        assert!(alerted && a.alerted);
        for _ in 0..(FORGET * 10.0) as usize + 2 {
            integrate(&mut a, None, 0.1);
        }
        assert!(!a.alerted && a.level < 0.3);
        let mut b = Awareness { level: 0.5, ..default() };
        integrate(&mut b, None, 1.0);
        assert!(b.level < 0.5 && !b.alerted);
    }

    #[test]
    fn aura_power_follows_what_the_player_does() {
        let base = aura_power(5, 1.0, false, false, false, false);
        assert!(aura_power(5, 1.0, true, false, false, false) < base);
        assert!(aura_power(5, 1.0, false, false, true, false) > base);
        assert!(aura_power(5, 1.0, false, false, false, true) < base * 0.2);
        assert!(aura_power(5, 0.0, false, false, false, false) < base);
    }

    #[test]
    fn the_cone_mesh_has_its_apex_at_the_origin() {
        let m = vision_cone_mesh(10.0, 0.5, 12);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!() };
        assert_eq!(p.len(), 12 + 2);
        assert_eq!(p[0], [0.0, 0.0, 0.0]);
        assert!(p[1..13].iter().all(|v| (v[2] - 10.0).abs() < 1e-4));
    }
}
