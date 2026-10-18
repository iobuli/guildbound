//! World setup: camera entity, sun + ambient, a ~120x120 m heightfield terrain (hills from
//! `guildbound_sim::terrain::ground`, flat training courtyard at the centre), a pond with a
//! `Water` volume, climbable walls/cliffs/ledges (`Climbable`), stairs and boxes for mantling,
//! interactables (`Interactable`: chest, crop plot, tree, well, table, chair, anvil, lever,
//! torch), training dummies / an opponent, and the player via `spawn_character`.
//! Exposes `pub fn ground_height(x, z) -> f32` for the analytic terrain.
//!
//! Layout (y up, the player spawns at the south edge facing +Z):
//! - courtyard |x|,|z| ≤ 15 m, exactly y = 0; hills fade in over 12 m outside it, a rim ridge
//!   (too steep to walk) closes the 120 m map, invisible walls back it up.
//! - west: pond centred at `POND_CENTER` (bowl `POND_DEPTH` deep, surface `WATER_LEVEL`);
//!   mantle box row (0.5 / 0.9 / 0.9 / 1.2 / 1.6 m) at x = −9; stairs (0.25 m rise) up to a
//!   1.5 m platform in the north-west corner.
//! - east: climb walls 2.5 / 4 / 6 m at x ≈ 13.5; training dummies on a sand pad south-east.
//! - north: chest, table (with the axe) + chair, anvil, torch, lever, well, a voxel tree to chop;
//!   more voxel trees on the hills (`trees::spawn_tree`); farm plots south-west.
//!
//! Interactable roots sit on the ground (y = 0); `stand` is the feet offset from the root and
//! `face_yaw` faces the object. Movable bits are named children (`Chest lid`, `Lever arm`,
//! `Crop`, `Torch flame`, `Tree canopy`) for `interact` to animate; the axe is a `tools::ToolItem`.
//!
//! Smoke-test env vars: `GB_AUTOQUIT=<seconds>` sends `AppExit` after that much real time (waits
//! up to 5 s more for a pending screenshot); `GB_SCREENSHOT=<path>` saves the primary window at
//! ~3 s (`GB_SCREENSHOT_AT=<seconds>` to choose). `GB_SPAWN=x,z,yaw_deg` moves the player there. `GB_CAST=<ids>` makes the player cast those
//! abilities at the nearest NPC (see `magic::demo`). `GB_RIVAL=1` / `GB_HUNTER=1` bring in the
//! guests at start.
//!
//! F7 / F8 toggle extra enemies (`Guest`): the Rival (team 2, greatsword, Earthbreaker and Frost
//! Bloom) and the Hunter (team 3, bow). Every NPC fights the nearest foe, so they fight each
//! other as readily as the player.
use super::armor::spawn_stand;
use super::hud::log::{HudLog, LogKind};
use super::magic::Grimoire;
use super::occupations::{Civilian, Occupation};
use super::skills::{Sheet, xp_for_level};
use super::stats::{Attr, Stats};
use super::surfaces::{PROP_TILE, Surfaces, TERRAIN_TILE, world_uvs};
use super::camera::MainCamera;
use super::tools::{AXE_SCALE, Tool, ToolItem};
use super::items::HillTrees;
use super::trees::{TreeSpec, spawn_tree};
use super::{Climbable, Fighter, Interactable, Loadout, Npc, Phys, Player, Water, spawn_character};
use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::world_serialization::WorldAssetRoot;
use std::f32::consts::{FRAC_PI_2, PI};

/// Half size of the flat training courtyard (y = 0 inside).
pub const COURTYARD_HALF: f32 = 15.0;
/// Half size of the terrain mesh (m).
pub const TERRAIN_HALF: f32 = 60.0;
/// Terrain vertex spacing (m).
pub const GRID: f32 = 0.5;
/// Hills fade in over this distance outside the courtyard.
const COURTYARD_BLEND: f32 = 12.0;
/// World metres → `guildbound_sim::terrain` units (pulls its hills and ridges into the map).
const HILL_SCALE: f32 = 1.4;
/// Rim ridge closing the map edge.
const RIM_HEIGHT: f32 = 10.0;
const RIM_WIDTH: f32 = 16.0;
/// Pond.
pub const POND_CENTER: Vec2 = Vec2::new(-24.0, 2.0);
pub const POND_RADIUS: f32 = 6.0;
pub const POND_DEPTH: f32 = 2.1;
pub const WATER_LEVEL: f32 = -0.35;
/// Player spawn (feet) and facing; the opponent stands 4 m in front.
pub const SPAWN_FEET: Vec3 = Vec3::new(0.0, 0.0, -12.5);
pub const SPAWN_YAW: f32 = 0.0;
const SCREENSHOT_AT: f32 = 3.0;

pub fn plugin(app: &mut App) {
    app.insert_resource(PlayerSpawn {
        feet: SPAWN_FEET,
        yaw: SPAWN_YAW,
    })
    .insert_resource(Smoke::from_env())
    .add_systems(Startup, setup)
    .add_systems(Update, (smoke_test, guests));
}

/// Extra enemies brought in and sent away by key (each on its own team, so the brain (`ai`)
/// sends every one at whoever is nearest: the Opponent, each other, or the player).
/// - F7 **Rival**: greatsword brawler, Earthbreaker and Frost Bloom (`GB_RIVAL=1` at start);
/// - F8 **Hunter**: an archer with a bow who keeps its distance (`GB_HUNTER=1` at start).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guest {
    Rival,
    Hunter,
}

impl Guest {
    const ALL: [Guest; 2] = [Guest::Rival, Guest::Hunter];
    fn key(self) -> KeyCode {
        match self {
            Guest::Rival => KeyCode::F7,
            Guest::Hunter => KeyCode::F8,
        }
    }
    fn env(self) -> &'static str {
        match self {
            Guest::Rival => "GB_RIVAL",
            Guest::Hunter => "GB_HUNTER",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Guest::Rival => "Rival",
            Guest::Hunter => "Hunter",
        }
    }
    /// Feet (x, z) and team.
    fn post(self) -> (Vec3, u8) {
        match self {
            Guest::Rival => (Vec3::new(5.0, 0.0, -6.0), 2),
            Guest::Hunter => (Vec3::new(-6.0, 0.0, -3.0), 3),
        }
    }
}

/// F7 / F8: bring a guest in, or send it away. One lying in a ragdoll leaves once it is back up
/// (its physics segments belong to the body).
fn guests(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    present: Query<(Entity, &Guest, &super::Mover)>,
    mut log: MessageWriter<HudLog>,
    mut leaving: Local<Vec<Guest>>,
    mut started: Local<bool>,
) {
    let at_start = !*started && time.elapsed_secs() > 0.5;
    if at_start {
        *started = true;
    }
    for g in Guest::ALL {
        let from_env = at_start && std::env::var(g.env()).is_ok_and(|v| v.trim() == "1");
        if !(keys.just_pressed(g.key()) || from_env) {
            continue;
        }
        let key = format!("{:?}", g.key()).replace("Key", "");
        if present.iter().any(|(_, k, _)| *k == g) {
            if !leaving.contains(&g) {
                leaving.push(g);
            }
            log.write(HudLog::new(LogKind::System, format!("The {} withdraws  [{key}]", g.name().to_lowercase())).key(7101));
        } else {
            leaving.retain(|l| *l != g);
            spawn_guest(&mut commands, g);
            log.write(HudLog::new(LogKind::Warn, format!("A {} enters the courtyard  [{key}]", g.name().to_lowercase())).key(7101));
        }
    }
    for (e, g, mover) in &present {
        if leaving.contains(g) && mover.mode != super::MoveMode::Ragdoll {
            commands.entity(e).try_despawn();
        }
    }
    leaving.retain(|g| present.iter().any(|(_, k, _)| k == g));
}

fn spawn_guest(commands: &mut Commands, g: Guest) {
    let (post, team) = g.post();
    let feet = post.with_y(ground_height(post.x, post.z));
    let opponent = SPAWN_FEET + Vec3::new(SPAWN_YAW.sin(), 0.0, SPAWN_YAW.cos()) * 4.0;
    let to = opponent - feet;
    let (weapon, tint) = match g {
        Guest::Rival => ("greatsword", Color::srgb(0.36, 0.30, 0.78)),
        Guest::Hunter => ("bow", Color::srgb(0.30, 0.62, 0.32)),
    };
    let e = spawn_character(
        commands,
        g.name(),
        feet,
        to.x.atan2(to.z),
        Fighter::new(team, 100.0),
        Loadout {
            slots: vec![weapon.to_string()],
            current: 0,
        },
        Some(tint),
    );
    let mut sheet = Sheet::default();
    let (stats, grimoire) = match g {
        // A heavy brawler: strength, endurance and vigor, a greatsword and the skills behind it;
        // it slams the ground (Earthbreaker) and throws frost.
        Guest::Rival => {
            sheet.grant("swordsmanship", xp_for_level(15));
            sheet.grant("defence", xp_for_level(8));
            sheet.grant("aura_arts", xp_for_level(10));
            sheet.grant("frost", xp_for_level(5));
            sheet.abilities.extend(["earthbreaker", "frost_bloom"]);
            let build = [Attr::Strength, Attr::Endurance, Attr::Vigor];
            (Stats::preset(4, &build), Grimoire { art: "earthbreaker", spell: "frost_bloom", ..default() })
        }
        // A deft archer: dexterity first; it shoots, so it never casts.
        Guest::Hunter => {
            sheet.grant("archery", xp_for_level(15));
            let build = [Attr::Dexterity, Attr::Dexterity, Attr::Vigor];
            (Stats::preset(3, &build), Grimoire::default())
        }
    };
    commands.entity(e).insert((Npc, g, stats, sheet, grimoire));
}

/// Where `R` puts the player back.
#[derive(Resource, Clone, Copy, Debug)]
pub struct PlayerSpawn {
    pub feet: Vec3,
    pub yaw: f32,
}

/// Marker on the terrain entity.
#[derive(Component)]
pub struct Terrain;

/// Static training dummy (capsule collider on `Phys::Character`, root at the capsule centre).
#[derive(Component)]
pub struct TrainingDummy;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Signed distance to the square courtyard (≤ 0 inside; contours round off outside its corners).
pub fn courtyard_distance(x: f32, z: f32) -> f32 {
    let q = Vec2::new(x.abs() - COURTYARD_HALF, z.abs() - COURTYARD_HALF);
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0)
}

/// The guild terrace east of the courtyard (x, z bounds): flat ground for the Guild Hall, its
/// training ground and the path from the courtyard (`hall`).
pub const TERRACE_MIN: Vec2 = Vec2::new(14.0, -18.0);
pub const TERRACE_MAX: Vec2 = Vec2::new(46.0, 14.0);
const TERRACE_BLEND: f32 = 8.0;

/// Signed distance to the guild terrace (≤ 0 on it).
pub fn terrace_distance(x: f32, z: f32) -> f32 {
    let c = (TERRACE_MIN + TERRACE_MAX) * 0.5;
    let h = (TERRACE_MAX - TERRACE_MIN) * 0.5;
    let q = (Vec2::new(x, z) - c).abs() - h;
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0)
}

/// The ruins on the north hill (`ruins`): a raised plateau of radius `RUINS_RADIUS` at
/// `ruins_level()`, blending into the hills over `RUINS_BLEND`.
pub const RUINS_CENTER: Vec2 = Vec2::new(0.0, 33.0);
pub const RUINS_RADIUS: f32 = 9.0;
const RUINS_BLEND: f32 = 6.0;

/// Signed distance to the ruins plateau (≤ 0 on it).
pub fn ruins_distance(x: f32, z: f32) -> f32 {
    Vec2::new(x, z).distance(RUINS_CENTER) - RUINS_RADIUS
}

/// The plateau's height: the hills at its centre, lifted a little so it reads as a terrace.
pub fn ruins_level() -> f32 {
    guildbound_sim::terrain::ground(RUINS_CENTER.x * HILL_SCALE, RUINS_CENTER.y * HILL_SCALE).max(0.4) + 0.5
}

/// On or by the ruins: no hill rocks, trees or items there.
pub fn in_ruins(x: f32, z: f32) -> bool {
    ruins_distance(x, z) < 2.5
}

/// Worn footpaths (x, z polylines): courtyard to the pond, and up to the ruins.
const PATHS: [&[[f32; 2]]; 2] = [
    &[[-15.0, 2.0], [-17.5, 2.5]],
    &[[0.0, 15.0], [1.5, 19.0], [-0.5, 23.0], [0.0, 24.5]],
];
const PATH_WIDTH: f32 = 1.1;

/// Distance (m) from a point to the nearest footpath.
pub fn path_distance(x: f32, z: f32) -> f32 {
    let p = Vec2::new(x, z);
    let mut best = f32::MAX;
    for path in PATHS {
        for w in path.windows(2) {
            let (a, b) = (Vec2::from(w[0]), Vec2::from(w[1]));
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
            best = best.min(p.distance(a + ab * t));
        }
    }
    best
}

/// On (or right by) the guild grounds: no hill rocks, trees or items there.
pub fn in_guild_grounds(x: f32, z: f32) -> bool {
    terrace_distance(x, z) < 3.0
}

/// Analytic terrain height (m). Exactly 0 inside the courtyard and on the guild terrace.
pub fn ground_height(x: f32, z: f32) -> f32 {
    if !(x.is_finite() && z.is_finite()) {
        return 0.0;
    }
    let hills = guildbound_sim::terrain::ground(x * HILL_SCALE, z * HILL_SCALE)
        * smoothstep(0.0, COURTYARD_BLEND, courtyard_distance(x, z))
        * smoothstep(0.0, TERRACE_BLEND, terrace_distance(x, z));
    let plateau = smoothstep(0.0, RUINS_BLEND, ruins_distance(x, z));
    let hills = ruins_level() * (1.0 - plateau) + hills * plateau;
    let edge = x.abs().max(z.abs());
    let rim = smoothstep(TERRAIN_HALF - RIM_WIDTH, TERRAIN_HALF, edge).powi(2) * RIM_HEIGHT;
    let pond = Vec2::new(x, z).distance(POND_CENTER);
    let meadow = smoothstep(POND_RADIUS + 7.0, POND_RADIUS + 1.5, pond);
    let bowl = smoothstep(POND_RADIUS, POND_RADIUS * 0.35, pond) * POND_DEPTH;
    (hills + rim) * (1.0 - meadow) - bowl
}

/// Terrain normal by central differences.
pub fn ground_normal(x: f32, z: f32) -> Vec3 {
    let e = 0.25;
    Vec3::new(
        ground_height(x - e, z) - ground_height(x + e, z),
        2.0 * e,
        ground_height(x, z - e) - ground_height(x, z + e),
    )
    .normalize_or(Vec3::Y)
}

/// Low-poly stone of radius `s` and a collider that matches its faces exactly (a sphere collider
/// would bulge above the flat facets, leaving feet floating).
fn stone_shape(s: f32) -> (Mesh, Collider) {
    let mesh = Sphere::new(s).mesh().ico(1).unwrap_or_else(|_| Sphere::new(s).into());
    let collider = Collider::convex_hull_from_mesh(&mesh).unwrap_or_else(|| Collider::sphere(s));
    (mesh, collider)
}

fn hash2(x: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27D4_EB2D) ^ (z as u32).wrapping_mul(0x1656_67B1) ^ 0xA511_E9B3;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
}

/// Smooth value noise 0..1.
pub fn value_noise(x: f32, z: f32) -> f32 {
    let (ix, iz) = (x.floor(), z.floor());
    let (fx, fz) = (smoothstep(0.0, 1.0, x - ix), smoothstep(0.0, 1.0, z - iz));
    let (ix, iz) = (ix as i32, iz as i32);
    let a = hash2(ix, iz) + (hash2(ix + 1, iz) - hash2(ix, iz)) * fx;
    let b = hash2(ix, iz + 1) + (hash2(ix + 1, iz + 1) - hash2(ix, iz + 1)) * fx;
    a + (b - a) * fz
}

/// Vertex colour (linear RGBA): grass / packed dirt / sand + mud / rock by slope and height.
pub fn ground_color(x: f32, z: f32, y: f32, n: Vec3) -> [f32; 4] {
    let srgb = |r: f32, g: f32, b: f32| Vec3::new(r, g, b);
    let patch = value_noise(x * 0.3, z * 0.3);
    let fine = value_noise(x * 1.7 + 40.0, z * 1.7 - 13.0);
    let mut c = srgb(0.27, 0.43, 0.17).lerp(srgb(0.40, 0.55, 0.22), patch);
    let yard = 1.0 - smoothstep(-1.5, 1.0, courtyard_distance(x, z).min(terrace_distance(x, z) + 2.0));
    c = c.lerp(srgb(0.55, 0.46, 0.33).lerp(srgb(0.62, 0.53, 0.39), fine), yard);
    // Worn footpaths and the ruins' trodden ground: packed earth with a lighter centre line.
    let path = smoothstep(PATH_WIDTH, PATH_WIDTH * 0.35, path_distance(x, z) + (fine - 0.5) * 0.5);
    let ruins = smoothstep(1.5, -1.0, ruins_distance(x, z) + (patch - 0.5) * 2.0);
    let worn = path.max(ruins * 0.85);
    c = c.lerp(srgb(0.47, 0.40, 0.30).lerp(srgb(0.56, 0.49, 0.37), fine), worn);
    let pond = Vec2::new(x, z).distance(POND_CENTER);
    c = c.lerp(srgb(0.70, 0.63, 0.46), smoothstep(POND_RADIUS + 2.0, POND_RADIUS, pond));
    c = c.lerp(srgb(0.28, 0.24, 0.18), smoothstep(WATER_LEVEL + 0.05, WATER_LEVEL - 0.6, y));
    let steep = smoothstep(0.9, 0.72, n.y);
    let high = smoothstep(7.0, 12.0, y) * 0.8;
    c = c.lerp(srgb(0.46, 0.44, 0.41).lerp(srgb(0.55, 0.53, 0.50), fine), steep.max(high));
    // Alpha carries the worn mask for the ground shader (`ground.wgsl`); the material is opaque.
    let mut out = Color::srgb(c.x, c.y, c.z).to_linear().to_f32_array();
    out[3] = worn;
    out
}

/// Terrain render mesh + trimesh collider (same vertices).
fn build_terrain() -> (Mesh, Collider) {
    let n = (2.0 * TERRAIN_HALF / GRID).round() as usize;
    let stride = n + 1;
    let mut positions = Vec::with_capacity(stride * stride);
    let mut normals = Vec::with_capacity(stride * stride);
    let mut colors = Vec::with_capacity(stride * stride);
    for iz in 0..=n {
        for ix in 0..=n {
            let x = -TERRAIN_HALF + ix as f32 * GRID;
            let z = -TERRAIN_HALF + iz as f32 * GRID;
            let y = ground_height(x, z);
            let normal = ground_normal(x, z);
            positions.push(Vec3::new(x, y, z));
            normals.push(normal.to_array());
            colors.push(ground_color(x, z, y, normal));
        }
    }
    let mut triangles = Vec::with_capacity(n * n * 2);
    for iz in 0..n {
        for ix in 0..n {
            let a = (iz * stride + ix) as u32;
            let b = a + 1;
            let c = a + stride as u32;
            let d = c + 1;
            triangles.extend([[a, c, b], [b, c, d]]);
        }
    }
    let collider = Collider::trimesh(positions.clone(), triangles.clone());
    let uvs: Vec<[f32; 2]> = positions.iter().map(|p| [p.x / TERRAIN_TILE, p.z / TERRAIN_TILE]).collect();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            positions.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(triangles.into_iter().flatten().collect()));
    if let Err(e) = mesh.generate_tangents() {
        warn!("[world] terrain tangents: {e}");
    }
    (mesh, collider)
}

fn world_layers() -> CollisionLayers {
    CollisionLayers::new(Phys::World, LayerMask::ALL)
}

/// Yaw that faces from `stand` back toward the object origin.
fn face_toward(stand: Vec3) -> f32 {
    let d = -stand;
    if d.x.abs() + d.z.abs() < 1e-5 { 0.0 } else { d.x.atan2(d.z) }
}

pub(crate) struct Palette {
    pub(crate) stone: Handle<StandardMaterial>,
    pub(crate) wall: Handle<StandardMaterial>,
    pub(crate) wood: Handle<StandardMaterial>,
    pub(crate) dark_wood: Handle<StandardMaterial>,
    pub(crate) metal: Handle<StandardMaterial>,
    pub(crate) soil: Handle<StandardMaterial>,
    pub(crate) crop: Handle<StandardMaterial>,
    pub(crate) leaves: Handle<StandardMaterial>,
    pub(crate) straw: Handle<StandardMaterial>,
    pub(crate) sand: Handle<StandardMaterial>,
    pub(crate) rock: Handle<StandardMaterial>,
    pub(crate) flame: Handle<StandardMaterial>,
    pub(crate) water: Handle<StandardMaterial>,
    pub(crate) cloth: Handle<StandardMaterial>,
    /// White base for vertex-coloured voxel meshes (trees).
    pub(crate) voxel: Handle<StandardMaterial>,
}

impl Palette {
    /// Colours stay as tuned; textured entries multiply them by a `surfaces` detail map and add
    /// its normal map (props get world-scale UVs from `Kit`).
    fn new(mats: &mut Assets<StandardMaterial>, surf: &Surfaces) -> Self {
        let mut solid = |r: f32, g: f32, b: f32, rough: f32, surface: Option<&super::surfaces::Surface>| {
            let base = StandardMaterial {
                base_color: Color::srgb(r, g, b),
                perceptual_roughness: rough,
                ..default()
            };
            mats.add(match surface {
                Some(s) => s.material(base),
                None => base,
            })
        };
        // Detail maps average ≈0.85, so textured colours are lifted to keep their old brightness.
        let stone = solid(0.64, 0.63, 0.60, 0.9, Some(&surf.stone));
        let wall = solid(0.58, 0.55, 0.50, 0.95, Some(&surf.stone));
        let wood = solid(0.64, 0.45, 0.26, 0.85, Some(&surf.wood));
        let dark_wood = solid(0.39, 0.26, 0.15, 0.85, Some(&surf.wood));
        let soil = solid(0.35, 0.25, 0.17, 1.0, Some(&surf.ground));
        let crop = solid(0.55, 0.70, 0.20, 0.8, None);
        let leaves = solid(0.22, 0.42, 0.18, 0.9, None);
        let straw = solid(0.90, 0.80, 0.48, 0.9, Some(&surf.cloth));
        let sand = solid(0.88, 0.78, 0.56, 1.0, Some(&surf.sand));
        let rock = solid(0.55, 0.54, 0.51, 0.95, Some(&surf.rock));
        let cloth = solid(0.70, 0.21, 0.17, 0.9, Some(&surf.cloth));
        let voxel = solid(1.0, 1.0, 1.0, 0.9, None);
        let metal = mats.add(surf.metal.material(StandardMaterial {
            base_color: Color::srgb(0.31, 0.32, 0.34),
            metallic: 0.8,
            perceptual_roughness: 0.45,
            ..default()
        }));
        let flame = mats.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.6, 0.2),
            emissive: LinearRgba::rgb(9.0, 3.6, 0.8),
            unlit: true,
            ..default()
        });
        let water = mats.add(StandardMaterial {
            base_color: Color::srgba(0.16, 0.38, 0.52, 0.62),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.08,
            reflectance: 0.6,
            ..default()
        });
        Self {
            stone,
            wall,
            wood,
            dark_wood,
            metal,
            soil,
            crop,
            leaves,
            straw,
            sand,
            rock,
            flame,
            water,
            cloth,
            voxel,
        }
    }
}

/// Spawning helpers bundling commands, mesh storage and the palette.
pub(crate) struct Kit<'a, 'w, 's> {
    pub(crate) commands: &'a mut Commands<'w, 's>,
    pub(crate) meshes: &'a mut Assets<Mesh>,
    pub(crate) pal: Palette,
}

impl Kit<'_, '_, '_> {
    /// A standalone static solid (its own static body when it has a collider).
    pub(crate) fn solid(
        &mut self,
        name: &str,
        mesh: impl Into<Mesh>,
        material: &Handle<StandardMaterial>,
        transform: Transform,
        collider: Option<Collider>,
    ) -> Entity {
        let mut mesh = mesh.into();
        world_uvs(&mut mesh, transform.scale, PROP_TILE);
        let mesh = self.meshes.add(mesh);
        let mut e = self.commands.spawn((
            Name::new(name.to_string()),
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            transform,
        ));
        if let Some(collider) = collider {
            e.insert((RigidBody::Static, collider, world_layers()));
        }
        e.id()
    }

    /// A child piece of `parent` (a collider attaches to the parent's static body).
    pub(crate) fn part(
        &mut self,
        parent: Entity,
        name: &str,
        mesh: impl Into<Mesh>,
        material: &Handle<StandardMaterial>,
        transform: Transform,
        collider: Option<Collider>,
    ) -> Entity {
        let mut mesh = mesh.into();
        world_uvs(&mut mesh, transform.scale, PROP_TILE);
        let mesh = self.meshes.add(mesh);
        let mut e = self.commands.spawn((
            Name::new(name.to_string()),
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            transform,
            ChildOf(parent),
        ));
        if let Some(collider) = collider {
            e.insert((collider, world_layers()));
        }
        e.id()
    }

    /// An empty static root at feet level.
    pub(crate) fn root(&mut self, name: &str, at: Vec3) -> Entity {
        self.commands
            .spawn((
                Name::new(name.to_string()),
                Transform::from_translation(at),
                Visibility::default(),
                RigidBody::Static,
            ))
            .id()
    }
}

/// What an interactable does (`Interactable` minus derived fields).
pub(crate) struct UseSpec {
    pub(crate) kind: &'static str,
    pub(crate) prompt: &'static str,
    pub(crate) track: &'static str,
    pub(crate) duration: f32,
    pub(crate) looping: bool,
    pub(crate) stand: Vec3,
    /// Override for `face_yaw` (default: face the object).
    pub(crate) face: Option<f32>,
    pub(crate) hands: [Option<Vec3>; 2],
    /// Moving child part the `hands` are relative to.
    pub(crate) grip: Option<&'static str>,
    /// When the use lands (clip seconds) and when the hands hold fully (see `Interactable`).
    pub(crate) beat: Option<f32>,
    pub(crate) reach: Option<(f32, f32)>,
}

impl UseSpec {
    fn component(&self) -> Interactable {
        Interactable {
            kind: self.kind.to_string(),
            prompt: self.prompt.to_string(),
            track: self.track.to_string(),
            duration: self.duration,
            looping: self.looping,
            stand: self.stand,
            face_yaw: self.face.unwrap_or_else(|| face_toward(self.stand)),
            hands: self.hands,
            grip: self.grip.map(str::to_string),
            range: 1.8,
            requires: None,
            beat: self.beat,
            reach: self.reach,
            radial: None,
        }
    }
}

fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let surfaces = Surfaces::build(&mut images);
    let pal = Palette::new(&mut materials, &surfaces);
    // Stone and wood weather (moss, grime) in `ground`.
    commands.insert_resource(super::ground::Weathering::new([pal.stone.clone(), pal.rock.clone(), pal.wall.clone(), pal.wood.clone(), pal.dark_wood.clone()]));
    spawn_sky_and_camera(&mut commands);
    // The vertex colours carry the terrain's look; the ground map adds soil grain and pebbles.
    let terrain_material = materials.add(surfaces.ground.material(StandardMaterial {
        base_color: Color::linear_rgb(1.18, 1.18, 1.18),
        perceptual_roughness: 0.95,
        ..default()
    }));
    commands.insert_resource(surfaces);
    let mut kit = Kit {
        commands: &mut commands,
        meshes: &mut meshes,
        pal,
    };
    spawn_terrain(&mut kit, terrain_material);
    spawn_pond(&mut kit);
    super::beam::spawn_tightrope(&mut kit);
    spawn_traversal(&mut kit);
    spawn_training_ground(&mut kit);
    spawn_props(&mut kit);
    spawn_interactables(&mut kit, &assets);
    super::hall::spawn_hall(&mut kit);
    super::ruins::spawn_ruins(&mut kit);
    spawn_characters(&mut commands);
}

fn spawn_sky_and_camera(commands: &mut Commands) {
    commands.spawn((
        Name::new("Sun"),
        super::sky::Sun,
        DirectionalLight {
            illuminance: 9000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-0.4, -0.78, 0.48), Vec3::Y),
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 10.0,
            maximum_distance: 80.0,
            ..default()
        }
        .build(),
    ));
    let eye = SPAWN_FEET + Vec3::new(0.0, 2.8, -5.0);
    commands.spawn((
        Name::new("Main camera"),
        MainCamera,
        Camera3d::default(),
        // HDR + gentle bloom: spells, arts and flames brighter than 1 glow.
        bevy::camera::Hdr,
        bevy::post_process::bloom::Bloom {
            intensity: 0.12,
            ..bevy::post_process::bloom::Bloom::NATURAL
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 60f32.to_radians(),
            ..default()
        }),
        Transform::from_translation(eye).looking_at(SPAWN_FEET + Vec3::Y * 1.35, Vec3::Y),
        DistanceFog {
            color: Color::srgb(0.60, 0.71, 0.83),
            falloff: FogFalloff::Linear {
                start: 50.0,
                end: 150.0,
            },
            ..default()
        },
    ));
}

fn spawn_terrain(kit: &mut Kit, material: Handle<StandardMaterial>) {
    let (mesh, collider) = build_terrain();
    let mesh = kit.meshes.add(mesh);
    kit.commands.spawn((
        Name::new("Terrain"),
        Terrain,
        RigidBody::Static,
        collider,
        world_layers(),
        Friction::new(0.8),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::default(),
    ));
    // Invisible walls just inside the rim ridge.
    let half = TERRAIN_HALF - 0.5;
    for (pos, size) in [
        (Vec3::new(half + 0.5, 20.0, 0.0), Vec3::new(1.0, 60.0, 2.0 * TERRAIN_HALF)),
        (Vec3::new(-half - 0.5, 20.0, 0.0), Vec3::new(1.0, 60.0, 2.0 * TERRAIN_HALF)),
        (Vec3::new(0.0, 20.0, half + 0.5), Vec3::new(2.0 * TERRAIN_HALF, 60.0, 1.0)),
        (Vec3::new(0.0, 20.0, -half - 0.5), Vec3::new(2.0 * TERRAIN_HALF, 60.0, 1.0)),
    ] {
        kit.commands.spawn((
            Name::new("Boundary"),
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            world_layers(),
            Transform::from_translation(pos),
        ));
    }
}

fn spawn_pond(kit: &mut Kit) {
    let bottom = -POND_DEPTH - 0.2;
    let half = Vec3::new(POND_RADIUS * 0.9, (WATER_LEVEL - bottom) * 0.5, POND_RADIUS * 0.9);
    let centre = Vec3::new(POND_CENTER.x, (WATER_LEVEL + bottom) * 0.5, POND_CENTER.y);
    let surface = kit.meshes.add(Circle::new(POND_RADIUS * 0.97).mesh().resolution(64));
    let water = kit.pal.water.clone();
    kit.commands
        .spawn((
            Name::new("Pond"),
            Water { half_extents: half },
            Transform::from_translation(centre),
            Visibility::default(),
            RigidBody::Static,
            Sensor,
            Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
            CollisionLayers::new(Phys::Trigger, [Phys::Character]),
        ))
        .with_child((
            Name::new("Pond surface"),
            Mesh3d(surface),
            MeshMaterial3d(water),
            NotShadowCaster,
            Transform::from_xyz(0.0, half.y, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        ));
    // Reeds and stones along the bank.
    let stone = kit.pal.rock.clone();
    let leaves = kit.pal.leaves.clone();
    for i in 0..9 {
        let a = i as f32 * 0.83 + 0.4;
        let r = POND_RADIUS + 0.6 + hash2(i, 7) * 1.4;
        let (x, z) = (POND_CENTER.x + a.cos() * r, POND_CENTER.y + a.sin() * r);
        let y = ground_height(x, z);
        if i % 3 == 0 {
            let s = 0.3 + hash2(i, 3) * 0.3;
            let (mesh, collider) = stone_shape(s);
            kit.solid("Bank stone", mesh, &stone, Transform::from_xyz(x, y + s * 0.3, z), Some(collider));
        } else {
            kit.solid(
                "Reeds",
                Cylinder::new(0.04, 1.0),
                &leaves,
                Transform::from_xyz(x, y + 0.5, z),
                None,
            );
        }
    }
}

/// Climb walls, mantle boxes, stairs and the platform.
fn spawn_traversal(kit: &mut Kit) {
    let wall = kit.pal.wall.clone();
    let stone = kit.pal.stone.clone();
    for (z, height) in [(-3.0, 2.5), (1.0, 4.0), (5.0, 6.0)] {
        let size = Vec3::new(1.0, height, 3.0);
        let e = kit.solid(
            &format!("Climb wall {height} m"),
            Cuboid::from_size(size),
            &wall,
            Transform::from_xyz(13.5, height * 0.5, z),
            Some(Collider::cuboid(size.x, size.y, size.z)),
        );
        kit.commands.entity(e).insert(Climbable { ledge: height });
    }
    // Mantle row: a waist-high run with lower and higher outliers.
    for (z, height) in [(-6.0, 0.5), (-4.0, 0.9), (-2.0, 0.9), (0.0, 1.2), (2.0, 1.6)] {
        let size = Vec3::new(1.2, height, 1.4);
        let e = kit.solid(
            &format!("Mantle box {height} m"),
            Cuboid::from_size(size),
            &stone,
            Transform::from_xyz(-9.0, height * 0.5, z),
            Some(Collider::cuboid(size.x, size.y, size.z)),
        );
        kit.commands.entity(e).insert(Climbable { ledge: height });
    }
    // Stairs (6 × 0.25 m rise, 0.45 m run) up to a 1.5 m platform.
    let (x, width, run, rise) = (-12.0, 2.0, 0.45, 0.25);
    let z0 = 9.0 - 6.0 * run;
    for i in 1..=6 {
        let h = rise * i as f32;
        let size = Vec3::new(width, h, run);
        kit.solid(
            "Stair step",
            Cuboid::from_size(size),
            &stone,
            Transform::from_xyz(x, h * 0.5, z0 + (i as f32 - 0.5) * run),
            Some(Collider::cuboid(size.x, size.y, size.z)),
        );
    }
    let size = Vec3::new(3.0, 1.5, 3.0);
    let e = kit.solid(
        "Platform",
        Cuboid::from_size(size),
        &stone,
        Transform::from_xyz(x, 0.75, 10.5),
        Some(Collider::cuboid(size.x, size.y, size.z)),
    );
    kit.commands.entity(e).insert(Climbable { ledge: 1.5 });
}

fn spawn_training_ground(kit: &mut Kit) {
    let centre = Vec3::new(8.5, 0.0, -9.0);
    let (sand, wood) = (kit.pal.sand.clone(), kit.pal.dark_wood.clone());
    kit.solid(
        "Training pad",
        Cuboid::new(6.0, 0.02, 5.0),
        &sand,
        Transform::from_translation(centre + Vec3::Y * 0.01),
        None,
    );
    for (dx, dz) in [(-3.0, -2.5), (3.0, -2.5), (-3.0, 2.5), (3.0, 2.5)] {
        kit.solid(
            "Training post",
            Cylinder::new(0.06, 1.0),
            &wood,
            Transform::from_translation(centre + Vec3::new(dx, 0.5, dz)),
            Some(Collider::cylinder(0.06, 1.0)),
        );
    }
    for (i, offset) in [Vec3::new(-1.5, 0.0, 1.0), Vec3::new(0.5, 0.0, -0.5), Vec3::new(2.0, 0.0, 1.5)]
        .into_iter()
        .enumerate()
    {
        spawn_dummy(kit, centre + offset, PI + (i as f32 - 1.0) * 0.4, &format!("Training dummy {}", i + 1));
    }
}

/// A straw training dummy standing at `feet`, facing `yaw`.
pub(crate) fn spawn_dummy(kit: &mut Kit, feet: Vec3, yaw: f32, name: &str) {
    let (wood, straw, cloth) = (kit.pal.dark_wood.clone(), kit.pal.straw.clone(), kit.pal.cloth.clone());
    let (radius, length) = (0.28, 1.0);
    let half = radius + length * 0.5;
    let dummy = kit
        .commands
        .spawn((
            Name::new(name.to_string()),
            TrainingDummy,
            Transform::from_translation(feet + Vec3::Y * half).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            RigidBody::Static,
            Collider::capsule(radius, length),
            CollisionLayers::new(Phys::Character, [Phys::World, Phys::Character, Phys::Projectile]),
        ))
        .id();
    kit.part(dummy, "Dummy body", Capsule3d::new(radius, length), &straw, Transform::default(), None);
    kit.part(dummy, "Dummy head", Sphere::new(0.17), &cloth, Transform::from_xyz(0.0, half + 0.05, 0.0), None);
    kit.part(dummy, "Dummy arms", Cuboid::new(1.1, 0.09, 0.09), &wood, Transform::from_xyz(0.0, 0.35, 0.0), None);
}

/// Crates, barrels, rocks and trees.
fn spawn_props(kit: &mut Kit) {
    let (wood, dark, rock, voxel, metal) = (
        kit.pal.wood.clone(),
        kit.pal.dark_wood.clone(),
        kit.pal.rock.clone(),
        kit.pal.voxel.clone(),
        kit.pal.metal.clone(),
    );
    for (pos, yaw) in [
        (Vec3::new(12.0, 0.5, -13.2), 0.1),
        (Vec3::new(12.1, 1.5, -13.1), 0.5),
        (Vec3::new(10.8, 0.5, -13.5), -0.3),
        (Vec3::new(-13.2, 0.5, -3.0), 0.2),
    ] {
        kit.solid(
            "Crate",
            Cuboid::new(1.0, 1.0, 1.0),
            &wood,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Some(Collider::cuboid(1.0, 1.0, 1.0)),
        );
    }
    for (x, z) in [(3.6, 10.8), (4.3, 11.5), (-13.0, -1.6), (11.0, 8.2)] {
        let barrel = kit.solid(
            "Barrel",
            Cylinder::new(0.38, 1.0),
            &dark,
            Transform::from_xyz(x, 0.5, z),
            Some(Collider::cylinder(0.38, 1.0)),
        );
        for y in [-0.3, 0.3] {
            kit.part(barrel, "Barrel hoop", Cylinder::new(0.395, 0.05), &metal, Transform::from_xyz(0.0, y, 0.0), None);
        }
    }
    // Rocks and trees on the hills (deterministic scatter, away from the pond).
    let mut hill_trees = Vec::new();
    for i in 0..26 {
        let a = i as f32 * 2.399_963;
        let r = 21.0 + hash2(i, 1) * 26.0;
        let (x, z) = (a.cos() * r, a.sin() * r);
        if Vec2::new(x, z).distance(POND_CENTER) < POND_RADIUS + 4.0 || in_guild_grounds(x, z) || in_ruins(x, z) {
            continue;
        }
        let y = ground_height(x, z);
        if i % 3 == 0 {
            let spec = TreeSpec {
                radius: 0.26 + hash2(i, 3) * 0.08,
                height: 2.8 + hash2(i, 4) * 0.6,
                canopy: 1.3 + hash2(i, 5) * 0.8,
                seed: i as u32,
            };
            spawn_tree(kit.commands, kit.meshes, &voxel, Vec3::new(x, y, z), spec);
            hill_trees.push(Vec3::new(x, y, z));
        } else {
            let s = 0.5 + hash2(i, 9) * 1.1;
            let (mesh, collider) = stone_shape(s);
            kit.solid(
                "Rock",
                mesh,
                &rock,
                Transform::from_xyz(x, y + s * 0.25, z).with_rotation(Quat::from_rotation_y(hash2(i, 2) * PI)),
                Some(collider),
            );
        }
    }
    // Apples fall under the hill trees (`items` scatters them).
    kit.commands.insert_resource(HillTrees(hill_trees));
}

/// Spawn an interactable root on the ground at `at`.
pub(crate) fn place(kit: &mut Kit, name: &str, at: Vec3, spec: UseSpec) -> Entity {
    let e = kit.root(name, at);
    kit.commands.entity(e).insert(spec.component());
    e
}

fn spawn_interactables(kit: &mut Kit, assets: &AssetServer) {
    let p = &kit.pal;
    let (wood, dark, metal, stone, soil, crop, voxel, flame, water, straw) = (
        p.wood.clone(),
        p.dark_wood.clone(),
        p.metal.clone(),
        p.stone.clone(),
        p.soil.clone(),
        p.crop.clone(),
        p.voxel.clone(),
        p.flame.clone(),
        p.water.clone(),
        p.straw.clone(),
    );
    let front = Vec3::new(0.0, 0.0, -0.9);

    // Chest: body + lid (the lid hinges at the back edge).
    let chest = place(kit, "Chest", Vec3::new(-5.0, 0.0, 11.5), UseSpec {
        kind: "chest",
        prompt: "Open chest",
        track: "ual2/Chest_Open",
        duration: 2.0,
        looping: false,
        stand: front,
        face: None,
        hands: [Some(Vec3::new(0.3, 0.1, -0.58)), Some(Vec3::new(-0.3, 0.1, -0.58))],
        grip: Some("Chest lid"),
        beat: None,
        reach: None,
    });
    kit.part(chest, "Chest body", Cuboid::new(1.0, 0.5, 0.6), &wood, Transform::from_xyz(0.0, 0.25, 0.0), Some(Collider::cuboid(1.0, 0.5, 0.6)));
    let lid = kit.commands.spawn((Name::new("Chest lid"), Transform::from_xyz(0.0, 0.5, 0.3), Visibility::default(), ChildOf(chest))).id();
    kit.part(lid, "Chest lid board", Cuboid::new(1.02, 0.14, 0.62), &dark, Transform::from_xyz(0.0, 0.07, -0.3), None);

    // Table with the axe on it, and a chair beside it. `PickUp_Table` reaches with the LEFT
    // hand (about 0.45 m ahead at 0.87 m, 0.2-0.3 s in), so the axe lies across the front-left
    // of the top with its mid-haft there; the left wrist target sits just behind that grip.
    let table = place(kit, "Table", Vec3::new(-1.5, 0.0, 12.0), UseSpec {
        kind: "table",
        prompt: "Pick up axe",
        track: "ual1/PickUp_Table",
        duration: 0.833,
        looping: false,
        stand: front,
        face: None,
        hands: [Some(Vec3::new(0.1, 0.84, -0.47)), None],
        grip: None,
        beat: Some(0.27),
        reach: Some((0.2, 0.3)),
    });
    kit.part(table, "Table top", Cuboid::new(1.6, 0.08, 0.9), &wood, Transform::from_xyz(0.0, 0.74, 0.0), Some(Collider::cuboid(1.6, 0.08, 0.9)));
    for (x, z) in [(-0.7, -0.35), (0.7, -0.35), (-0.7, 0.35), (0.7, 0.35)] {
        kit.part(table, "Table leg", Cuboid::new(0.08, 0.7, 0.08), &dark, Transform::from_xyz(x, 0.35, z), Some(Collider::cuboid(0.08, 0.7, 0.08)));
    }
    // Lying flat, head toward +X, edge toward the user; mid-haft at (0.1, 0.81, -0.4).
    let rest = Transform::from_xyz(0.01, 0.806, -0.4)
        .with_rotation(Quat::from_rotation_y(-FRAC_PI_2))
        .with_scale(Vec3::splat(AXE_SCALE));
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset("packs/weapons/battleaxe.glb"));
    kit.commands.spawn((
        Name::new("Axe"),
        WorldAssetRoot(scene),
        rest,
        Visibility::default(),
        ToolItem::new(Tool::Axe, table, rest),
        ChildOf(table),
    ));

    // Chair faces −Z (back toward +Z); sit on the seat, facing away from the backrest.
    let chair = place(kit, "Chair", Vec3::new(-3.3, 0.0, 11.6), UseSpec {
        kind: "chair",
        prompt: "Sit",
        track: "ual1/Sitting_Idle_Loop",
        duration: 4.0,
        looping: true,
        stand: Vec3::new(0.0, 0.0, -0.15),
        face: Some(PI),
        hands: [None, None],
        grip: None,
        beat: None,
        reach: None,
    });
    kit.part(chair, "Chair seat", Cuboid::new(0.5, 0.06, 0.5), &wood, Transform::from_xyz(0.0, 0.45, 0.0), None);
    kit.part(chair, "Chair back", Cuboid::new(0.5, 0.55, 0.06), &wood, Transform::from_xyz(0.0, 0.75, 0.22), Some(Collider::cuboid(0.5, 0.55, 0.06)));
    for (x, z) in [(-0.21, -0.21), (0.21, -0.21), (-0.21, 0.21), (0.21, 0.21)] {
        kit.part(chair, "Chair leg", Cuboid::new(0.05, 0.45, 0.05), &dark, Transform::from_xyz(x, 0.225, z), None);
    }

    // Anvil on a stump.
    let anvil = place(kit, "Anvil", Vec3::new(2.5, 0.0, 12.0), UseSpec {
        kind: "anvil",
        prompt: "Repair",
        track: "ual1/Fixing_Kneeling",
        duration: 3.0,
        looping: true,
        stand: Vec3::new(0.0, 0.0, -0.85),
        face: None,
        hands: [None, None],
        grip: None,
        beat: None,
        reach: None,
    });
    kit.part(anvil, "Anvil stump", Cylinder::new(0.3, 0.45), &dark, Transform::from_xyz(0.0, 0.225, 0.0), Some(Collider::cylinder(0.3, 0.45)));
    kit.part(anvil, "Anvil body", Cuboid::new(0.7, 0.22, 0.28), &metal, Transform::from_xyz(0.0, 0.56, 0.0), Some(Collider::cuboid(0.7, 0.22, 0.28)));
    kit.part(anvil, "Anvil horn", Cone { radius: 0.11, height: 0.3 }, &metal, Transform::from_xyz(0.48, 0.6, 0.0).with_rotation(Quat::from_rotation_z(-FRAC_PI_2)), None);

    // Torch on a pole with a light.
    let torch = place(kit, "Torch", Vec3::new(5.0, 0.0, 13.2), UseSpec {
        kind: "torch",
        prompt: "Take torch",
        track: "ual1/Idle_Torch_Loop",
        duration: 3.0,
        looping: true,
        stand: Vec3::new(0.15, 0.0, -0.45),
        face: Some(0.0),
        hands: [None, Some(Vec3::new(0.0, 1.1, 0.0))],
        grip: None,
        beat: None,
        reach: None,
    });
    kit.part(torch, "Torch pole", Cylinder::new(0.04, 1.5), &dark, Transform::from_xyz(0.0, 0.75, 0.0), Some(Collider::cylinder(0.04, 1.5)));
    let flame_entity = kit.part(torch, "Torch flame", Sphere::new(0.11), &flame, Transform::from_xyz(0.0, 1.6, 0.0), None);
    kit.commands.entity(flame_entity).insert((
        NotShadowCaster,
        PointLight {
            color: Color::srgb(1.0, 0.65, 0.3),
            intensity: 60_000.0,
            range: 9.0,
            shadow_maps_enabled: false,
            ..default()
        },
    ));

    // Lever on a stone base (the arm pivots at its bottom).
    let lever = place(kit, "Lever", Vec3::new(7.5, 0.0, 13.2), UseSpec {
        kind: "lever",
        prompt: "Pull lever",
        track: "ual1/Interact",
        duration: 1.2,
        looping: false,
        stand: Vec3::new(0.15, 0.0, -0.5),
        face: Some(0.0),
        hands: [None, Some(Vec3::new(0.0, 0.62, 0.0))],
        grip: Some("Lever arm"),
        beat: None,
        reach: None,
    });
    kit.part(lever, "Lever base", Cuboid::new(0.5, 0.5, 0.4), &stone, Transform::from_xyz(0.0, 0.25, 0.0), Some(Collider::cuboid(0.5, 0.5, 0.4)));
    let arm = kit
        .commands
        .spawn((
            Name::new("Lever arm"),
            Transform::from_xyz(0.0, 0.5, 0.0).with_rotation(Quat::from_rotation_x(-0.5)),
            Visibility::default(),
            ChildOf(lever),
        ))
        .id();
    kit.part(arm, "Lever shaft", Cylinder::new(0.03, 0.6), &metal, Transform::from_xyz(0.0, 0.3, 0.0), None);
    kit.part(arm, "Lever grip", Sphere::new(0.06), &dark, Transform::from_xyz(0.0, 0.62, 0.0), None);

    // Guild noticeboard near the spawn: two posts, a plank board under a little roof, pinned
    // notices. F reads it (`contracts` opens the WORK tab).
    let board = place(kit, "Noticeboard", Vec3::new(3.2, 0.0, -9.8), UseSpec {
        kind: "board",
        prompt: "Read the notices",
        track: "ual1/Interact",
        duration: 1.2,
        looping: false,
        stand: Vec3::new(0.0, 0.0, -0.8),
        face: Some(0.0),
        hands: [None, Some(Vec3::new(0.15, 1.35, -0.06))],
        grip: None,
        beat: None,
        reach: None,
    });
    for x in [-0.75, 0.75] {
        kit.part(board, "Board post", Cuboid::new(0.1, 2.1, 0.1), &dark, Transform::from_xyz(x, 1.05, 0.0), Some(Collider::cuboid(0.1, 2.1, 0.1)));
    }
    kit.part(board, "Board planks", Cuboid::new(1.4, 0.9, 0.05), &wood, Transform::from_xyz(0.0, 1.35, 0.0), Some(Collider::cuboid(1.4, 0.9, 0.05)));
    kit.part(board, "Board roof", Cuboid::new(1.75, 0.06, 0.45), &dark, Transform::from_xyz(0.0, 2.12, -0.05).with_rotation(Quat::from_rotation_x(-0.25)), None);
    for (x, y, tilt) in [(-0.4, 1.5, 0.06), (0.05, 1.4, -0.04), (0.45, 1.55, 0.08), (-0.2, 1.08, -0.07), (0.35, 1.1, 0.03)] {
        kit.part(board, "Notice", Cuboid::new(0.26, 0.32, 0.01), &straw, Transform::from_xyz(x, y, -0.035).with_rotation(Quat::from_rotation_z(tilt)), None);
    }

    // Well: stone ring, water disc, two posts and a roof beam.
    let well = place(kit, "Well", Vec3::new(10.0, 0.0, 10.5), UseSpec {
        kind: "well",
        prompt: "Drink",
        track: "sekiro/Drinking",
        duration: 2.5,
        looping: false,
        stand: Vec3::new(0.0, 0.0, -1.3),
        face: None,
        hands: [None, None],
        grip: None,
        beat: None,
        reach: None,
    });
    kit.part(well, "Well ring", Cylinder::new(0.9, 0.8), &stone, Transform::from_xyz(0.0, 0.4, 0.0), Some(Collider::cylinder(0.9, 0.8)));
    kit.part(well, "Well water", Circle::new(0.75), &water, Transform::from_xyz(0.0, 0.81, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)), None);
    for x in [-0.8, 0.8] {
        kit.part(well, "Well post", Cuboid::new(0.12, 1.9, 0.12), &dark, Transform::from_xyz(x, 0.95, 0.0), None);
    }
    kit.part(well, "Well beam", Cuboid::new(1.9, 0.12, 0.12), &dark, Transform::from_xyz(0.0, 1.9, 0.0), None);

    // Armour stand: a wooden mannequin wearing the plate set (`armor`), facing the courtyard.
    spawn_stand(kit.commands, kit.meshes, &dark, Vec3::new(1.0, 0.0, 8.0), PI);

    // Voxel tree to chop (needs the axe from the table).
    let spec = TreeSpec {
        radius: 0.33,
        height: 3.2,
        canopy: 1.6,
        seed: 101,
    };
    spawn_tree(kit.commands, kit.meshes, &voxel, Vec3::new(-5.0, 0.0, 6.0), spec);

    // Farm: crop plot, watering spot, seed plot (soil is visual only, walkable).
    let plot = |kit: &mut Kit, parent: Entity, size: Vec2| {
        kit.part(parent, "Soil", Cuboid::new(size.x, 0.06, size.y), &soil, Transform::from_xyz(0.0, 0.03, 0.0), None)
    };
    let crops = place(kit, "Crop plot", Vec3::new(-11.0, 0.0, -11.5), UseSpec {
        kind: "crop",
        prompt: "Harvest",
        track: "ual2/Farm_Harvest",
        duration: 2.5,
        looping: false,
        stand: Vec3::new(0.0, 0.0, -1.3),
        face: None,
        hands: [None, None],
        grip: None,
        beat: None,
        reach: None,
    });
    plot(kit, crops, Vec2::new(2.0, 2.0));
    for ix in 0..3 {
        for iz in 0..3 {
            let (x, z) = (-0.6 + ix as f32 * 0.6, -0.6 + iz as f32 * 0.6);
            kit.part(crops, "Crop", Cone { radius: 0.14, height: 0.45 }, &crop, Transform::from_xyz(x, 0.28, z), None);
        }
    }
    let watering = place(kit, "Watering spot", Vec3::new(-9.25, 0.0, -11.5), UseSpec {
        kind: "watering",
        prompt: "Water plants",
        track: "ual2/Farm_Watering",
        duration: 3.0,
        looping: false,
        stand: Vec3::new(0.0, 0.0, -1.0),
        face: None,
        hands: [None, None],
        grip: None,
        beat: None,
        reach: None,
    });
    plot(kit, watering, Vec2::new(1.2, 1.2));
    kit.part(watering, "Bucket", Cylinder::new(0.16, 0.3), &wood, Transform::from_xyz(0.35, 0.15, -0.35), None);
    let seeds = place(kit, "Seed plot", Vec3::new(-7.5, 0.0, -11.5), UseSpec {
        kind: "seed",
        prompt: "Plant seeds",
        track: "ual2/Farm_PlantSeed",
        duration: 2.5,
        looping: false,
        stand: Vec3::new(0.0, 0.0, -1.3),
        face: None,
        hands: [None, None],
        grip: None,
        beat: None,
        reach: None,
    });
    plot(kit, seeds, Vec2::new(2.0, 2.0));
    kit.part(seeds, "Seed sack", Sphere::new(0.2), &straw, Transform::from_xyz(1.2, 0.18, -0.9), None);
}

fn spawn_characters(commands: &mut Commands) {
    let player = spawn_character(
        commands,
        "Player",
        SPAWN_FEET.with_y(ground_height(SPAWN_FEET.x, SPAWN_FEET.z)),
        SPAWN_YAW,
        Fighter::new(0, 100.0),
        Loadout {
            // Weapons come from the chest (`armory`); fists until then.
            slots: vec!["unarmed".to_string()],
            current: 0,
        },
        None,
    );
    commands.entity(player).insert(Player);
    let forward = Vec3::new(SPAWN_YAW.sin(), 0.0, SPAWN_YAW.cos());
    let feet = SPAWN_FEET + forward * 4.0;
    let npc = spawn_character(
        commands,
        "Opponent",
        feet.with_y(ground_height(feet.x, feet.z)),
        SPAWN_YAW + PI,
        Fighter::new(1, 100.0),
        Loadout {
            slots: vec!["katana".to_string()],
            current: 0,
        },
        Some(Color::srgb(0.80, 0.30, 0.26)),
    );
    // A seasoned swordsman: a few levels into strength, dexterity and vigor, and the skills to go
    // with them.
    let build = [Attr::Strength, Attr::Dexterity, Attr::Vigor];
    let mut sheet = Sheet::default();
    sheet.grant("swordsmanship", xp_for_level(20));
    sheet.grant("defence", xp_for_level(12));
    commands.entity(npc).insert((Npc, Stats::preset(4, &build), sheet));

    // Civilians at work: a Smith by the anvil and a Farmer in the plots (team 0; nobody fights
    // them, they fight nobody).
    for (name, job, at, tint) in [
        ("Smith", "smith", Vec3::new(4.0, 0.0, 10.5), Color::srgb(0.55, 0.42, 0.32)),
        ("Farmer", "farmer", Vec3::new(-8.5, 0.0, -9.5), Color::srgb(0.42, 0.55, 0.30)),
    ] {
        let feet = at.with_y(ground_height(at.x, at.z));
        let e = spawn_character(
            commands,
            name,
            feet,
            0.0,
            Fighter::new(0, 100.0),
            Loadout {
                slots: vec!["unarmed".to_string()],
                current: 0,
            },
            Some(tint),
        );
        let holdings = vec![if job == "smith" { "forge" } else { "fields" }.to_string()];
        commands.entity(e).insert((
            Civilian,
            Occupation::new(job),
            super::contracts::Standing { holdings },
            super::combat::Revive {
                after: 8.0,
                home: Some((feet, 0.0)),
            },
        ));
    }
}

/// `GB_AUTOQUIT` / `GB_SCREENSHOT` state.
#[derive(Resource, Default)]
struct Smoke {
    quit_after: Option<f32>,
    shot: Option<String>,
    /// Game (virtual) seconds before the shot (`GB_SCREENSHOT_AT`, default `SCREENSHOT_AT`), so
    /// slow software rendering still catches timed events.
    shot_at: f32,
    shot_requested: bool,
    /// `GB_SPAWN=x,z,yaw_deg`: where the player is put once the world is up.
    spawn: Option<(Vec3, f32)>,
    spawned: bool,
    shot_done: bool,
    exit_sent: bool,
}

impl Smoke {
    fn from_env() -> Self {
        let quit_after = std::env::var("GB_AUTOQUIT")
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|s| s.is_finite() && *s >= 0.0);
        let shot = std::env::var("GB_SCREENSHOT").ok().filter(|s| !s.trim().is_empty());
        let shot_at = std::env::var("GB_SCREENSHOT_AT")
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .filter(|s| s.is_finite() && *s >= 0.0)
            .unwrap_or(SCREENSHOT_AT);
        let spawn = std::env::var("GB_SPAWN").ok().and_then(|s| {
            let v: Vec<f32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
            (v.len() == 3).then(|| (Vec3::new(v[0], ground_height(v[0], v[1]), v[1]), v[2].to_radians()))
        });
        Self {
            quit_after,
            shot,
            shot_at,
            spawn,
            ..default()
        }
    }
}

fn smoke_test(
    time: Res<Time<Real>>,
    game: Res<Time<Virtual>>,
    mut smoke: ResMut<Smoke>,
    mut commands: Commands,
    players: Query<Entity, With<Player>>,
    mut exit: MessageWriter<AppExit>,
    rig: Option<ResMut<super::camera::CameraRig>>,
) {
    let t = time.elapsed_secs();
    if let Some((feet, yaw)) = smoke.spawn
        && !smoke.spawned
        && game.elapsed_secs() > 0.5
    {
        smoke.spawned = true;
        for p in &players {
            commands.entity(p).try_insert(super::controller::Teleport { feet, yaw });
        }
        // The camera looks the way the player was put.
        if let Some(mut rig) = rig {
            rig.yaw = yaw;
        }
    }
    if let Some(path) = smoke.shot.clone()
        && !smoke.shot_requested
        && game.elapsed_secs() >= smoke.shot_at
    {
        smoke.shot_requested = true;
        info!("[world] GB_SCREENSHOT → {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path))
            .observe(|_: On<ScreenshotCaptured>, mut smoke: ResMut<Smoke>| smoke.shot_done = true);
    }
    if let Some(quit) = smoke.quit_after
        && t >= quit
        && !smoke.exit_sent
    {
        let waiting = smoke.shot.is_some() && !smoke.shot_done && t < quit.max(smoke.shot_at) + 5.0;
        if !waiting {
            info!("[world] GB_AUTOQUIT after {t:.1} s");
            smoke.exit_sent = true;
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guild_terrace_is_flat() {
        // Where the guild stands (x ≤ 41; past x 44 the map's outer rim starts to rise).
        for x in [15.0, 20.0, 30.0, 41.0] {
            for z in [-17.5, -9.0, 0.0, 13.5] {
                assert_eq!(ground_height(x, z), 0.0, "terrace not flat at {x},{z}");
            }
        }
        assert!(in_guild_grounds(32.0, -9.0) && !in_guild_grounds(-30.0, 30.0));
    }

    #[test]
    fn courtyard_is_flat_and_hills_rise_outside() {
        for x in [-14.5, -7.0, 0.0, 6.3, 14.5] {
            for z in [-14.5, -3.0, 0.0, 9.9, 14.5] {
                assert_eq!(ground_height(x, z), 0.0, "courtyard not flat at {x},{z}");
            }
        }
        assert_eq!(ground_height(SPAWN_FEET.x, SPAWN_FEET.z), 0.0);
        let mut relief = 0.0f32;
        for i in 0..40 {
            let a = i as f32 * 0.157;
            relief = relief.max(ground_height(a.cos() * 38.0, a.sin() * 38.0).abs());
        }
        assert!(relief > 0.3, "no hills outside the courtyard ({relief})");
        assert_eq!(ground_height(f32::NAN, 0.0), 0.0);
    }

    #[test]
    fn terrain_is_continuous_and_rim_is_high() {
        let mut worst = 0.0f32;
        let mut x = -TERRAIN_HALF;
        while x < TERRAIN_HALF {
            let mut z = -TERRAIN_HALF;
            while z < TERRAIN_HALF {
                let h = ground_height(x, z);
                assert!(h.is_finite());
                worst = worst
                    .max((ground_height(x + 0.1, z) - h).abs())
                    .max((ground_height(x, z + 0.1) - h).abs());
                z += 0.7;
            }
            x += 0.7;
        }
        assert!(worst < 0.3, "terrain jumps {worst} m over 10 cm");
        assert!(ground_height(0.0, TERRAIN_HALF - 0.5) > 5.0, "rim missing");
    }

    #[test]
    fn pond_holds_water() {
        let centre = ground_height(POND_CENTER.x, POND_CENTER.y);
        assert!(centre < WATER_LEVEL - 1.5, "pond too shallow: {centre}");
        // banks rise above the water
        assert!(ground_height(POND_CENTER.x + POND_RADIUS + 1.0, POND_CENTER.y) > WATER_LEVEL);
    }

    #[test]
    fn interactables_face_their_object() {
        let stand = Vec3::new(0.0, 0.0, -0.9);
        let yaw = face_toward(stand);
        let forward = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        assert!(forward.abs_diff_eq(Vec3::Z, 1e-6));
        let stand = Vec3::new(1.0, 0.0, 0.0);
        let yaw = face_toward(stand);
        assert!(Vec3::new(yaw.sin(), 0.0, yaw.cos()).abs_diff_eq(Vec3::NEG_X, 1e-6));
    }
}
