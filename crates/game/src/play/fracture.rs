//! Environment fractures: breakable pillars, crates and wall blocks that shatter into physics chunks.
//!
//! - `Fracturable` (half extents, strength, `Stuff`): a static box with a collider, so it blocks like any wall.
//! - What breaks it:
//!   - a `Shatter` message (a point, a radius, a force): sent by the vault slam and the dive spike (`moves`),
//!     a clash's finale (`clash`) and the berserk roar (`berserk`). Within the radius the force falls off
//!     linearly; at least `strength` breaks it.
//!   - a fighter flung into it faster than `FLUNG` m/s.
//! - Breaking: the box is split into a jittered grid of chunks (2-3 cells along each side, by size, the cut
//!   planes moved up to `JITTER` of a cell), each a dynamic body on `Phys::Debris` (it hits the world and
//!   other chunks, not characters), thrown out from the impact with spin. Chunks shrink away after
//!   `CHUNK_LIFE` s. Wood throws chips, stone a dust burst.
//! - Test yard: `YARD` (on the plaza pad) holds a ring of pillars, a stack of crates and a wall of blocks.
//!   `GB_FRACTURE=1` also rings the player with breakables at start, so any fight or move can be watched
//!   breaking them (smoke runs and recordings).
use super::fx::{Burst, BurstKind};
use super::vfx::{self, Vfx};
use super::world::ground_height;
use super::{Fighter, Mover, Phys, Player};
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;

/// Fighters moving faster than this (m/s) break what they are thrown into.
pub const FLUNG: f32 = 7.0;
/// Chunk life (s), cut jitter (share of a cell), chunk speed scale (m/s per unit force).
const CHUNK_LIFE: f32 = 4.0;
const JITTER: f32 = 0.3;
const THROW: f32 = 0.6;
/// The test yard's centre (on the plaza pad, `world::PADS`).
pub const YARD: Vec2 = Vec2::new(40.0, 72.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stuff {
    Stone,
    Wood,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Fracturable {
    pub half: Vec3,
    pub strength: f32,
    pub stuff: Stuff,
}

/// A blast that can break things: centre, radius (m), force at the centre.
#[derive(Message, Clone, Copy, Debug)]
pub struct Shatter {
    pub at: Vec3,
    pub radius: f32,
    pub force: f32,
}

#[derive(Component)]
struct Chunk {
    life: f32,
    scale: Vec3,
}

#[derive(Resource)]
struct Looks {
    stone: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_message::<Shatter>().add_systems(Startup, (looks, yard, imported_environment).chain()).add_systems(Update, (ring_player, check, age_chunks));
}

/// Local archive geometry uses matching triangle colliders, not an invisible box across doorways.
fn imported_environment(mut commands: Commands, assets: Res<AssetServer>) {
    if !super::world::duel_mode() { return; }
    #[derive(serde::Deserialize)]
    struct Module { name: String, file: String, half: [f32; 3], vertices: Vec<[f32; 3]>, indices: Vec<[u32; 3]> }
    let path = std::path::Path::new(crate::ASSETS).join("environment/sparking/manifest.json");
    let modules = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Vec<Module>>(&s).ok());
    let Some(modules) = modules else { warn!("[fracture] local Sparking environment pack unavailable"); return; };
    for (i, module) in modules.into_iter().enumerate() {
        let half = Vec3::from_array(module.half);
        let x = -10.0 + i as f32 * 10.0;
        let z = -18.0;
        let at = Vec3::new(x, ground_height(x, z) + half.y, z);
        commands.spawn((Name::new(format!("Archive {}", module.name)),
            Transform::from_translation(at), Visibility::default(), RigidBody::Static,
            Collider::trimesh(module.vertices.into_iter().map(Vec3::from_array).collect(), module.indices),
            super::world::world_layers(), Fracturable { half, strength: 55.0, stuff: Stuff::Stone },
        )).with_child((WorldAssetRoot(assets.load(format!("{}#Scene0", module.file))), Transform::default()));
    }
}

/// Force that reaches a box of half extents `half` at `centre` from a blast: linear falloff from the blast
/// centre to its radius, measured to the box's nearest point.
pub fn reach(blast: &Shatter, centre: Vec3, half: Vec3) -> f32 {
    let local = blast.at - centre;
    let nearest = centre + local.clamp(-half, half);
    let d = nearest.distance(blast.at);
    if d >= blast.radius {
        0.0
    } else {
        blast.force * (1.0 - d / blast.radius)
    }
}

/// Cut positions along one side of `2 * half` m split into `n` cells, each inner cut moved by up to `JITTER`
/// of a cell (`r` gives 0..1 values). Ends at -half and +half.
pub fn cuts(half: f32, n: usize, r: &mut impl FnMut() -> f32) -> Vec<f32> {
    let n = n.max(1);
    let cell = 2.0 * half / n as f32;
    (0..=n)
        .map(|i| {
            let base = -half + cell * i as f32;
            if i == 0 || i == n { base } else { base + (r() - 0.5) * 2.0 * JITTER * cell }
        })
        .collect()
}

fn looks(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(Looks {
        stone: materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.53, 0.5), perceptual_roughness: 0.95, ..default() }),
        wood: materials.add(StandardMaterial { base_color: Color::srgb(0.5, 0.33, 0.18), perceptual_roughness: 0.85, ..default() }),
    });
}

pub fn spawn_breakable(commands: &mut Commands, meshes: &mut Assets<Mesh>, looks_stone: &Handle<StandardMaterial>, looks_wood: &Handle<StandardMaterial>, at: Vec3, half: Vec3, stuff: Stuff, yaw: f32) {
    let strength = match stuff {
        Stuff::Stone => 30.0 + 20.0 * half.volume_scale(),
        Stuff::Wood => 12.0 + 10.0 * half.volume_scale(),
    };
    commands.spawn((
        Name::new(match stuff {
            Stuff::Stone => "Breakable stone",
            Stuff::Wood => "Breakable crate",
        }),
        Fracturable { half, strength, stuff },
        Mesh3d(meshes.add(Cuboid::from_size(half * 2.0))),
        MeshMaterial3d(match stuff {
            Stuff::Stone => looks_stone.clone(),
            Stuff::Wood => looks_wood.clone(),
        }),
        Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
        RigidBody::Static,
        Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
        super::world::world_layers(),
    ));
}

trait VolumeScale {
    fn volume_scale(&self) -> f32;
}
impl VolumeScale for Vec3 {
    /// Volume relative to a 1 m cube (half extents 0.5).
    fn volume_scale(&self) -> f32 {
        self.x * self.y * self.z * 8.0
    }
}

/// The test yard: a ring of stone pillars, a stack of crates and a wall of stone blocks.
fn yard(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, looks: Res<Looks>) {
    let g = |x: f32, z: f32| ground_height(x, z);
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        let (x, z) = (YARD.x + a.cos() * 5.0, YARD.y + a.sin() * 5.0);
        let half = Vec3::new(0.3, 1.2, 0.3);
        spawn_breakable(&mut commands, &mut meshes, &looks.stone, &looks.wood, Vec3::new(x, g(x, z) + half.y, z), half, Stuff::Stone, a);
    }
    for (i, (dx, dz, level)) in [(0.0, 0.0, 0), (0.9, 0.0, 0), (0.45, 0.1, 1)].into_iter().enumerate() {
        let (x, z) = (YARD.x - 1.5 + dx, YARD.y - 1.0 + dz);
        let half = Vec3::splat(0.42);
        spawn_breakable(&mut commands, &mut meshes, &looks.stone, &looks.wood, Vec3::new(x, g(x, z) + half.y * (1.0 + 2.0 * level as f32), z), half, Stuff::Wood, i as f32 * 0.3);
    }
    for i in 0..4 {
        for row in 0..2 {
            let (x, z) = (YARD.x + 1.0 + i as f32 * 0.82, YARD.y + 2.2);
            let half = Vec3::new(0.4, 0.35, 0.25);
            spawn_breakable(&mut commands, &mut meshes, &looks.stone, &looks.wood, Vec3::new(x, g(x, z) + half.y * (1.0 + 2.0 * row as f32), z), half, Stuff::Stone, 0.0);
        }
    }
}

/// `GB_FRACTURE=1`: a ring of breakables round the player once it stands in the world.
fn ring_player(mut done: Local<bool>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, looks: Res<Looks>, players: Query<&Transform, With<Player>>, time: Res<Time>) {
    if *done || (!super::world::duel_mode() && std::env::var_os("GB_FRACTURE").is_none()) || time.elapsed_secs() < 1.0 {
        return;
    }
    let Ok(tf) = players.single() else { return };
    *done = true;
    let c = tf.translation;
    for i in 0..10 {
        let a = i as f32 / 10.0 * std::f32::consts::TAU;
        let r = 7.0 + (i % 2) as f32 * 1.0;
        let (x, z) = (c.x + a.cos() * r, c.z + a.sin() * r);
        let (half, stuff) = if i % 3 == 0 { (Vec3::splat(0.4), Stuff::Wood) } else { (Vec3::new(0.28, 1.0, 0.28), Stuff::Stone) };
        spawn_breakable(&mut commands, &mut meshes, &looks.stone, &looks.wood, Vec3::new(x, ground_height(x, z) + half.y, z), half, stuff, a);
    }
    info!("[fracture] ringed the player with 10 breakables");
    // Two small modular buildings: separate wall blocks, columns and roof planks can be reused.
    // Keep the middle lane clear for the duel and the camera boom.
    for side in [-1.0, 1.0] {
        let origin = c + Vec3::new(side * 10.0, 0.0, 4.0);
        for row in 0..3 {
            for column in 0..5 {
                let half = Vec3::new(0.48, 0.45, 0.25);
                let x = origin.x + (column as f32 - 2.0);
                let z = origin.z + 2.0;
                spawn_breakable(&mut commands, &mut meshes, &looks.stone, &looks.wood,
                    Vec3::new(x, ground_height(x, z) + 0.45 + row as f32 * 0.9, z), half, Stuff::Stone, 0.0);
            }
        }
        for xoff in [-2.0, 2.0] {
            let x = origin.x + xoff;
            spawn_breakable(&mut commands, &mut meshes, &looks.stone, &looks.wood,
                Vec3::new(x, ground_height(x, origin.z) + 1.35, origin.z), Vec3::new(0.2, 1.35, 0.2), Stuff::Wood, 0.0);
        }
        for strip in 0..5 {
            let x = origin.x + strip as f32 - 2.0;
            spawn_breakable(&mut commands, &mut meshes, &looks.stone, &looks.wood,
                Vec3::new(x, ground_height(x, origin.z) + 2.85, origin.z + 1.0), Vec3::new(0.48, 0.1, 1.3), Stuff::Wood, 0.0);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check(
    mut commands: Commands,
    mut blasts: MessageReader<Shatter>,
    mut bursts: MessageWriter<Burst>,
    vfx: Option<Res<Vfx>>,
    looks: Res<Looks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut things: Query<(Entity, &Transform, &mut Fracturable)>,
    fighters: Query<(&Transform, &Mover), With<Fighter>>,
) {
    let blasts: Vec<Shatter> = blasts.read().copied().collect();
    for (e, tf, mut f) in &mut things {
        let centre = tf.translation;
        // The strongest blast that reaches it, or a body flung into it.
        let mut hit: Option<(Vec3, f32)> = blasts.iter().map(|b| {
            let local = Shatter { at: tf.rotation.inverse() * (b.at - centre), ..*b };
            (b.at, reach(&local, Vec3::ZERO, f.half))
        }).filter(|h| h.1 > 0.0).max_by(|a, b| a.1.total_cmp(&b.1));
        for (ftf, m) in &fighters {
            let speed = m.velocity.length();
            let local = tf.rotation.inverse() * (ftf.translation - centre);
            if speed > FLUNG && local.abs().cmple(f.half + Vec3::splat(0.45)).all() {
                hit = Some((ftf.translation, speed * 4.0));
            }
        }
        let Some((from, force)) = hit else { continue };
        f.strength -= force;
        if f.strength > 0.0 {
            continue;
        }
        commands.entity(e).despawn();
        let mut seed = (e.to_bits() as u32) | 1;
        let mut r = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        let n = |h: f32| if h > 0.6 { 3 } else { 2 };
        let (xs, ys, zs) = (cuts(f.half.x, n(f.half.x), &mut r), cuts(f.half.y, n(f.half.y), &mut r), cuts(f.half.z, n(f.half.z), &mut r));
        let mat = match f.stuff {
            Stuff::Stone => looks.stone.clone(),
            Stuff::Wood => looks.wood.clone(),
        };
        for i in 0..xs.len() - 1 {
            for j in 0..ys.len() - 1 {
                for k in 0..zs.len() - 1 {
                    let lo = Vec3::new(xs[i], ys[j], zs[k]);
                    let hi = Vec3::new(xs[i + 1], ys[j + 1], zs[k + 1]);
                    let size = (hi - lo).max(Vec3::splat(0.04));
                    let pos = centre + tf.rotation * ((lo + hi) * 0.5);
                    let out = (pos - from).normalize_or(Vec3::Y);
                    let vel = (out + Vec3::Y * 0.6).normalize() * force.min(60.0) * THROW * (0.6 + 0.6 * r()) / (1.0 + size.length());
                    commands.spawn((
                        Name::new("Fracture chunk"),
                        Chunk { life: CHUNK_LIFE, scale: Vec3::ONE },
                        Mesh3d(meshes.add(Cuboid::from_size(size))),
                        MeshMaterial3d(mat.clone()),
                        Transform::from_translation(pos).with_rotation(tf.rotation),
                        RigidBody::Dynamic,
                        Collider::cuboid(size.x, size.y, size.z),
                        CollisionLayers::new(Phys::Debris, [Phys::World, Phys::Debris]),
                        LinearVelocity(vel),
                        AngularVelocity(Vec3::new(r() - 0.5, r() - 0.5, r() - 0.5) * 12.0),
                    ));
                }
            }
        }
        match f.stuff {
            Stuff::Wood => {
                bursts.write(Burst { kind: BurstKind::Chips, at: centre, dir: Vec3::Y, count: 24 });
            }
            Stuff::Stone => {
                bursts.write(Burst { kind: BurstKind::Sparks, at: centre, dir: Vec3::Y, count: 10 });
                if let Some(v) = &vfx {
                    vfx::burst(&mut commands, v.shock(), centre - Vec3::Y * f.half.y, 0.8 + f.half.y);
                }
            }
        }
        info!("[fracture] {:?} broke into {} chunks", f.stuff, (xs.len() - 1) * (ys.len() - 1) * (zs.len() - 1));
    }
}

/// Chunks shrink away at the end of their life.
fn age_chunks(time: Res<Time>, mut commands: Commands, mut chunks: Query<(Entity, &mut Chunk, &mut Transform)>) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (e, mut c, mut tf) in &mut chunks {
        c.life -= dt;
        if c.life <= 0.0 {
            commands.entity(e).despawn();
        } else if c.life < 0.6 {
            tf.scale = c.scale * (c.life / 0.6);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blast_falls_off_to_its_radius_measured_to_the_nearest_face() {
        let b = Shatter { at: Vec3::ZERO, radius: 4.0, force: 40.0 };
        let half = Vec3::new(0.5, 1.0, 0.5);
        assert!((reach(&b, Vec3::new(2.5, 0.0, 0.0), half) - 20.0).abs() < 1e-4, "2 m from its face");
        assert_eq!(reach(&b, Vec3::new(9.0, 0.0, 0.0), half), 0.0);
        assert!((reach(&b, Vec3::ZERO, half) - 40.0).abs() < 1e-4, "inside");
    }

    #[test]
    fn cuts_span_the_box_and_stay_ordered() {
        let mut s = 3u32;
        let mut r = move || {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (s >> 8) as f32 / (1u32 << 24) as f32
        };
        for _ in 0..50 {
            let c = cuts(0.6, 3, &mut r);
            assert_eq!(c.len(), 4);
            assert!((c[0] + 0.6).abs() < 1e-6 && (c[3] - 0.6).abs() < 1e-6);
            assert!(c.windows(2).all(|w| w[1] > w[0]), "{c:?}");
        }
    }
}
