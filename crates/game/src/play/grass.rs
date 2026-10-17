//! Grass: a knee-high, wind-swept meadow over the hills (BotW-style), trampled by characters.
//!
//! Owns the grass chunks, their material (`GrassMaterial` = `StandardMaterial` + `GrassExtension`
//! with the vertex shader `assets/shaders/grass.wgsl`) and the `GrassWind` resource.
//!
//! How it works:
//! - Placement (once, `PostStartup`, after transforms propagate): the terrain is resampled on its own
//!   0.5 m grid (`GroundGrid`) so blade roots sit exactly on the rendered triangles, and every node
//!   gets a cover weight from `grass_cover`, which follows `world::ground_color`'s rules (no grass on
//!   the courtyard dirt, the pond sand and mud, steep or high rock, near the rim or underwater).
//!   A jittered 0.16 m cell grid then hashes each cell (deterministic, no RNG state): a blade grows
//!   where `hash < cover × patchiness × obstacle clearance`. Blades lean away from the centre of
//!   their tuft (`CLUMP_SIZE`), and height and colour follow low-frequency noise patches, so the
//!   field reads as tufts in a patchy meadow. Static colliders standing on the ground (rocks, bank
//!   stones, tree trunks) clear an elliptical spot so blades do not poke through them.
//! - Meshes: one entity per 8 × 8 m chunk and level of detail. LOD0 has every blade with
//!   `SEGMENTS_NEAR` segments; LOD1 keeps a `LOD1_KEEP` subset, wider and with fewer segments.
//!   Bevy's `VisibilityRange` cross-fades them around `LOD0_END`; past `FADE_START` the shader
//!   shrinks blades into the ground so the far edge never pops. Each chunk carries an `Aabb`
//!   inflated by the sway reach (`NoAutoAabb` keeps Bevy from shrinking it back).
//! - Vertex data: POSITION / NORMAL are the rest blade, UV_0 = (t along the blade, root height),
//!   UV_1 = root x, z, COLOR = (linear rgb, per-blade random). The shader bends each vertex along
//!   an arc by `angle × t` (tip offset ∝ t²) from the sum of: rolling gust fronts and flutter
//!   (`GrassWind`), the magic gust slot (`GrassWind::gust`) and up to `MAX_PUSHERS` characters
//!   (`Mover`s nearest the camera). Normals tilt with the bend, then lean toward world up for soft
//!   meadow shading (both faces share one normal; culling is off).
//! - Per frame `update_wind` advances the wind clock and writes one small storage buffer
//!   (`GrassWindData`). The material asset itself is never touched, so chunks are not re-extracted
//!   or re-specialised every frame.
//! - Grass neither casts shadows nor joins the prepass (`enable_prepass`/`enable_shadows` are off,
//!   so only the forward pipeline with the custom vertex shader is ever built); it receives sun
//!   shadows and fog through the standard PBR fragment shader.
use super::camera::MainCamera;
use super::world::{
    COURTYARD_HALF, POND_CENTER, POND_RADIUS, TERRAIN_HALF, Terrain, WATER_LEVEL, courtyard_distance,
    ground_height, value_noise,
};
use super::{HALF_HEIGHT, Mover};
use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::{NoAutoAabb, VisibilityRange};
use bevy::light::NotShadowCaster;
use bevy::material::OpaqueRendererMethod;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use bevy::transform::TransformSystems;
use std::f32::consts::TAU;

/// Vertex shader (asset path).
const GRASS_SHADER: &str = "shaders/grass.wgsl";

/// Chunk edge (m); one entity per chunk and LOD.
pub const CHUNK_SIZE: f32 = 8.0;
/// Chunks per terrain side (`CHUNKS × CHUNK_SIZE = 2 × TERRAIN_HALF`).
const CHUNKS: usize = 15;
/// Placement cells per chunk side: 0.16 m cells, ≈ 39 candidate blades per m².
const CELLS_PER_CHUNK: usize = 50;
/// Terrain vertex spacing (m), shared with `world` so roots sit on the mesh.
const GROUND_GRID: f32 = super::world::GRID;

/// Blade height range (m) before patch / border scaling.
pub const BLADE_HEIGHT_MIN: f32 = 0.35;
pub const BLADE_HEIGHT_MAX: f32 = 0.8;
/// Blade width range (m) at the root.
pub const BLADE_WIDTH_MIN: f32 = 0.03;
pub const BLADE_WIDTH_MAX: f32 = 0.05;
/// Segments per blade near the camera (LOD0) and further out (LOD1).
const SEGMENTS_NEAR: usize = 4;
const SEGMENTS_FAR: usize = 3;
/// Blades lean away from the nearest tuft centre on this grid (m).
const CLUMP_SIZE: f32 = 0.7;
/// Roots start this far below the ground so slopes never show a gap under a blade.
const ROOT_SINK: f32 = 0.03;
/// How far the two edge normals of a blade turn sideways (fakes a rounded blade).
const EDGE_ROUNDNESS: f32 = 0.45;

/// Fraction of blades kept in the far LOD, and how much wider they are drawn to keep coverage.
pub const LOD1_KEEP: f32 = 0.4;
pub const LOD1_WIDEN: f32 = 1.5;
/// Camera distance (m) where the dense LOD hands over to the sparse one, and the cross-fade band.
pub const LOD0_END: f32 = 30.0;
pub const LOD_BLEND: f32 = 8.0;
/// Blades shrink into the ground between these camera distances (m) and are gone beyond.
pub const FADE_START: f32 = 52.0;
pub const FADE_END: f32 = 66.0;

/// Upper bound on the bend angle (rad) from all influences together.
pub const MAX_BEND: f32 = 1.4;
/// 0 = true blade normals, 1 = all normals straight up.
pub const NORMAL_UP_BIAS: f32 = 0.55;
/// Per-blade flutter (rad at wind strength 1, scaled up inside gusts).
pub const FLUTTER: f32 = 0.06;
/// Distance (m) between rolling gust fronts.
pub const GUST_WAVELENGTH: f32 = 14.0;
/// Characters push blades within this radius (m) of their feet...
pub const TRAMPLE_RADIUS: f32 = 0.9;
/// ...by up to this bend angle (rad) at the centre.
pub const TRAMPLE_BEND: f32 = 1.25;
/// Characters that can push grass at once (nearest the camera); matches the shader array.
pub const MAX_PUSHERS: usize = 8;
/// Chunk bounds grow by this much sideways so swaying tips are never culled (m).
const SWAY_MARGIN: f32 = 0.9;

/// Obstacle footprints grow by this margin (m) and thin grass out to `EXCLUSION_FEATHER` × radius.
const EXCLUSION_MARGIN: f32 = 0.08;
const EXCLUSION_FEATHER: f32 = 1.35;
/// Colliders whose footprint is smaller than this (posts, apples) or wider than the max (walls,
/// boundaries, water volumes) do not clear grass.
const EXCLUSION_MIN_SIZE: f32 = 0.25;
const EXCLUSION_MAX_SIZE: f32 = 6.0;

/// Flutter clock wraps after this many seconds and the gust-noise scroll after this many metres
/// (keeps f32 precision). grass.wgsl has the same constants: it rounds flutter frequencies to
/// whole cycles per wrap and repeats its noise lattices over the scroll period, so neither wrap
/// shows.
const FLUTTER_WRAP: f32 = 3600.0;
const SCROLL_WRAP: f32 = 2000.0;

pub type GrassMaterial = ExtendedMaterial<StandardMaterial, GrassExtension>;

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<GrassMaterial>::default())
        .init_resource::<GrassWind>()
        .add_systems(PostStartup, spawn_grass.after(TransformSystems::Propagate))
        .add_systems(Update, update_wind);
}

/// Ambient wind over the meadow. Systems may change any field at runtime (weather, wind magic).
#[derive(Resource, Clone, Debug)]
pub struct GrassWind {
    /// Horizontal direction (world x, z) the wind blows toward; normalised on use.
    pub direction: Vec2,
    /// 0 = still air, ~0.6 = breezy meadow, 1.5+ = storm.
    pub strength: f32,
    /// Speed (m/s) at which gust fronts roll across the field.
    pub speed: f32,
    /// One local burst, e.g. a wind spell. Off while `radius` or `strength` is 0.
    pub gust: GrassGust,
}

impl Default for GrassWind {
    fn default() -> Self {
        Self {
            direction: Vec2::new(0.85, 0.35),
            strength: 0.6,
            speed: 3.5,
            gust: GrassGust::default(),
        }
    }
}

/// A radial push: blades within `radius` of `centre` bend away from it by up to `strength`
/// radians (≈ 1.3 flattens them; negative values pull toward the centre).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GrassGust {
    pub centre: Vec3,
    pub radius: f32,
    pub strength: f32,
}

/// Material extension: the custom vertex shader and its per-frame wind buffer.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct GrassExtension {
    /// `GrassWindData`, rewritten every frame by `update_wind`.
    #[storage(100, read_only)]
    pub wind: Handle<ShaderBuffer>,
}

impl MaterialExtension for GrassExtension {
    fn vertex_shader() -> ShaderRef {
        GRASS_SHADER.into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

/// GPU mirror of `GrassWind` + clock + pushers; layout must match `GrassWind` in grass.wgsl.
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
struct GrassWindData {
    /// xy: unit direction, z: strength, w: gust-front phase (rad).
    wind: Vec4,
    /// xy: noise scroll (m), z: flutter clock (s), w: pushers in use.
    motion: Vec4,
    /// xyz: magic gust centre, w: radius.
    gust: Vec4,
    /// x: magic gust bend, y: `FADE_START`, z: `FADE_END`, w: `TRAMPLE_BEND`.
    params: Vec4,
    /// x: gust wave number, y: `MAX_BEND`, z: `NORMAL_UP_BIAS`, w: `FLUTTER`.
    shape: Vec4,
    /// xyz: feet, w: push radius.
    pushers: [Vec4; MAX_PUSHERS],
}

/// Marker on grass chunk entities.
#[derive(Component)]
pub struct GrassChunk;

/// The wind buffer shared by the grass material.
#[derive(Resource)]
struct GrassBuffers {
    wind: Handle<ShaderBuffer>,
}

/// Wind state integrated over time, so changing `GrassWind` never makes the field jump.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct WindClock {
    /// Gust-front phase (rad, wrapped to 0..TAU).
    phase: f32,
    /// How far the gust noise has drifted downwind (m, wrapped to 0..SCROLL_WRAP).
    scroll: Vec2,
    /// Flutter clock (s); runs faster in stronger wind.
    time: f32,
}

impl WindClock {
    fn advance(self, wind: &GrassWind, dt: f32) -> Self {
        let dt = dt.max(0.0);
        let dir = wind_direction(wind.direction);
        let speed = wind.speed.max(0.0);
        Self {
            phase: (self.phase + speed * TAU / GUST_WAVELENGTH * dt).rem_euclid(TAU),
            scroll: (self.scroll + dir * speed * dt).rem_euclid(Vec2::splat(SCROLL_WRAP)),
            time: (self.time + dt * (0.7 + 0.5 * wind.strength.clamp(0.0, 3.0))).rem_euclid(FLUTTER_WRAP),
        }
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn wind_direction(direction: Vec2) -> Vec2 {
    direction.try_normalize().unwrap_or(Vec2::X)
}

/// Integer hash → 0..1 for cell (x, z) and a per-property salt.
fn hash(x: i32, z: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (z as u32).wrapping_mul(0xD816_3841)
        ^ salt.wrapping_mul(0xCB1A_B31F)
        ^ 0x9E37_79B9;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// How much grass the ground at (x, z) with height `y` and normal `n` supports, 0..1. Mirrors the
/// grass / dirt / sand / mud / rock rules of `world::ground_color`, with soft borders.
pub fn grass_cover(x: f32, z: f32, y: f32, n: Vec3) -> f32 {
    // zero on the courtyard dirt, full 2.5 m outside it
    let yard = smoothstep(0.0, 2.5, courtyard_distance(x, z));
    // the pond's sand ring reaches POND_RADIUS + 2 m
    let pond = smoothstep(POND_RADIUS + 1.6, POND_RADIUS + 3.2, Vec2::new(x, z).distance(POND_CENTER));
    let dry = smoothstep(WATER_LEVEL + 0.1, WATER_LEVEL + 0.5, y);
    // rock on slopes steeper than ~25° and on the high ground
    let flat = smoothstep(0.8, 0.93, n.y);
    let low = 1.0 - smoothstep(5.5, 8.5, y);
    let edge = x.abs().max(z.abs());
    let rim = 1.0 - smoothstep(TERRAIN_HALF - 9.0, TERRAIN_HALF - 5.0, edge);
    (yard * pond * dry * flat * low * rim).clamp(0.0, 1.0)
}

/// Density patches: lush stretches, thinner patches and the odd clearing (0..1).
fn patchiness(x: f32, z: f32) -> f32 {
    let n = 0.65 * value_noise(x * 0.07 + 13.1, z * 0.07 - 7.7) + 0.35 * value_noise(x * 0.23 - 3.3, z * 0.23 + 5.9);
    smoothstep(0.22, 0.52, n)
}

/// Terrain heights and grass cover on the terrain mesh's own grid.
struct GroundGrid {
    /// Cells per side.
    n: usize,
    heights: Vec<f32>,
    cover: Vec<f32>,
}

impl GroundGrid {
    fn sample() -> Self {
        let n = (2.0 * TERRAIN_HALF / GROUND_GRID).round() as usize;
        let stride = n + 1;
        let coord = |i: usize| -TERRAIN_HALF + i as f32 * GROUND_GRID;
        let mut heights = Vec::with_capacity(stride * stride);
        for iz in 0..=n {
            for ix in 0..=n {
                heights.push(ground_height(coord(ix), coord(iz)));
            }
        }
        let at = |ix: usize, iz: usize| heights[iz * stride + ix];
        let mut cover = Vec::with_capacity(stride * stride);
        for iz in 0..=n {
            for ix in 0..=n {
                let (x0, x1) = (ix.saturating_sub(1), (ix + 1).min(n));
                let (z0, z1) = (iz.saturating_sub(1), (iz + 1).min(n));
                let dx = (at(x1, iz) - at(x0, iz)) / ((x1 - x0).max(1) as f32 * GROUND_GRID);
                let dz = (at(ix, z1) - at(ix, z0)) / ((z1 - z0).max(1) as f32 * GROUND_GRID);
                let normal = Vec3::new(-dx, 1.0, -dz).normalize_or(Vec3::Y);
                cover.push(grass_cover(coord(ix), coord(iz), at(ix, iz), normal));
            }
        }
        Self { n, heights, cover }
    }

    /// Cell and fractions for (x, z), clamped to the terrain.
    fn locate(&self, x: f32, z: f32) -> (usize, usize, f32, f32) {
        let fx = ((x + TERRAIN_HALF) / GROUND_GRID).clamp(0.0, self.n as f32);
        let fz = ((z + TERRAIN_HALF) / GROUND_GRID).clamp(0.0, self.n as f32);
        let ix = (fx.floor() as usize).min(self.n - 1);
        let iz = (fz.floor() as usize).min(self.n - 1);
        (ix, iz, fx - ix as f32, fz - iz as f32)
    }

    fn corners(values: &[f32], stride: usize, ix: usize, iz: usize) -> [f32; 4] {
        let a = iz * stride + ix;
        [values[a], values[a + 1], values[a + stride], values[a + stride + 1]]
    }

    /// Height of the rendered terrain: same triangles as `world::build_terrain` (a-c-b, b-c-d).
    fn height_at(&self, x: f32, z: f32) -> f32 {
        let (ix, iz, fx, fz) = self.locate(x, z);
        let [a, b, c, d] = Self::corners(&self.heights, self.n + 1, ix, iz);
        if fx + fz <= 1.0 {
            a + (b - a) * fx + (c - a) * fz
        } else {
            d + (c - d) * (1.0 - fx) + (b - d) * (1.0 - fz)
        }
    }

    /// Grass cover, bilinear.
    fn cover_at(&self, x: f32, z: f32) -> f32 {
        let (ix, iz, fx, fz) = self.locate(x, z);
        let [a, b, c, d] = Self::corners(&self.cover, self.n + 1, ix, iz);
        let near = a + (b - a) * fx;
        let far = c + (d - c) * fx;
        near + (far - near) * fz
    }
}

/// Elliptical ground footprint of an obstacle that grass grows around.
#[derive(Clone, Copy, Debug)]
struct Exclusion {
    centre: Vec2,
    half: Vec2,
}

impl Exclusion {
    /// 0 inside the footprint, rising to 1 at `EXCLUSION_FEATHER` × its size.
    fn clearance(&self, p: Vec2) -> f32 {
        let q = ((p - self.centre) / self.half).length();
        smoothstep(1.0, EXCLUSION_FEATHER, q)
    }

    fn overlaps(&self, min: Vec2, max: Vec2) -> bool {
        let reach = self.half * EXCLUSION_FEATHER;
        self.centre.x + reach.x >= min.x
            && self.centre.x - reach.x <= max.x
            && self.centre.y + reach.y >= min.y
            && self.centre.y - reach.y <= max.y
    }
}

/// Grass colours (linear) for the lush and the sun-dried ends of the meadow.
struct Palette {
    base: Vec3,
    mid_lush: Vec3,
    mid_warm: Vec3,
    tip_lush: Vec3,
    tip_warm: Vec3,
}

impl Palette {
    fn new() -> Self {
        let lin = |r: f32, g: f32, b: f32| {
            let c = Color::srgb(r, g, b).to_linear();
            Vec3::new(c.red, c.green, c.blue)
        };
        Self {
            base: lin(0.10, 0.20, 0.05),
            mid_lush: lin(0.20, 0.40, 0.09),
            mid_warm: lin(0.36, 0.47, 0.13),
            tip_lush: lin(0.55, 0.74, 0.25),
            tip_warm: lin(0.76, 0.78, 0.36),
        }
    }
}

/// One blade, world space.
#[derive(Clone, Copy, Debug)]
struct Blade {
    root: Vec3,
    height: f32,
    width: f32,
    /// Direction the blade leans and curls toward (rad; forward = (sin, 0, cos)).
    yaw: f32,
    /// Lean from vertical at the root (rad).
    lean: f32,
    /// Extra lean gained toward the tip (rad).
    curve: f32,
    /// Per-blade random 0..1 (shader flutter phase).
    rand: f32,
    /// 0..1; the blade is in the far LOD when below `LOD1_KEEP`.
    keep: f32,
    /// Root, middle and tip colour (linear).
    colours: [Vec3; 3],
}

impl Blade {
    fn colour(&self, s: f32) -> Vec3 {
        let [base, mid, tip] = self.colours;
        if s < 0.45 {
            base.lerp(mid, smoothstep(0.0, 0.45, s))
        } else {
            mid.lerp(tip, smoothstep(0.45, 1.0, s))
        }
    }
}

/// Placement cell edge (m).
fn cell_size() -> f32 {
    CHUNK_SIZE / CELLS_PER_CHUNK as f32
}

/// Nearest tuft centre to `p` (3 × 3 cell search) and that tuft's random.
fn nearest_clump(p: Vec2) -> (Vec2, f32) {
    let cx = (p.x / CLUMP_SIZE).floor() as i32;
    let cz = (p.y / CLUMP_SIZE).floor() as i32;
    let mut best = (Vec2::ZERO, 0.0, f32::INFINITY);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (x, z) = (cx + dx, cz + dz);
            let centre = Vec2::new(
                (x as f32 + 0.15 + 0.7 * hash(x, z, 101)) * CLUMP_SIZE,
                (z as f32 + 0.15 + 0.7 * hash(x, z, 102)) * CLUMP_SIZE,
            );
            let d = centre.distance_squared(p);
            if d < best.2 {
                best = (centre, hash(x, z, 103), d);
            }
        }
    }
    (best.0, best.1)
}

/// Whether a blade grows in global cell (gx, gz) and, if so, its shape and colour.
fn plan_blade(gx: i32, gz: i32, ground: &GroundGrid, exclusions: &[Exclusion], palette: &Palette) -> Option<Blade> {
    let cell = cell_size();
    let x = -TERRAIN_HALF + (gx as f32 + 0.05 + 0.9 * hash(gx, gz, 1)) * cell;
    let z = -TERRAIN_HALF + (gz as f32 + 0.05 + 0.9 * hash(gx, gz, 2)) * cell;
    let p = Vec2::new(x, z);
    let cover = ground.cover_at(x, z);
    if cover <= 0.0 {
        return None;
    }
    let clear: f32 = exclusions.iter().map(|e| e.clearance(p)).product();
    if hash(gx, gz, 3) >= cover * patchiness(x, z) * clear {
        return None;
    }
    let (clump, clump_rand) = nearest_clump(p);
    let away = p - clump;
    let spread = (away.length() / (0.5 * CLUMP_SIZE)).min(1.0);
    let rand = hash(gx, gz, 4);
    let tuft_yaw = if away.length_squared() > 1e-4 { away.x.atan2(away.y) } else { rand * TAU };
    let yaw = tuft_yaw + (hash(gx, gz, 5) - 0.5) * 1.4;
    let lean = 0.06 + 0.32 * spread + 0.1 * hash(gx, gz, 6);
    let curve = 0.15 + 0.55 * hash(gx, gz, 7);

    let size = 0.55 * hash(gx, gz, 8) + 0.45 * clump_rand;
    let tall = value_noise(x * 0.09 + 5.0, z * 0.09 - 3.0);
    let height = (BLADE_HEIGHT_MIN + (BLADE_HEIGHT_MAX - BLADE_HEIGHT_MIN) * size)
        * (0.8 + 0.32 * tall)
        * (0.55 + 0.45 * cover.sqrt());
    let width = BLADE_WIDTH_MIN + (BLADE_WIDTH_MAX - BLADE_WIDTH_MIN) * hash(gx, gz, 9);

    let warm = smoothstep(0.35, 0.75, value_noise(x * 0.045 - 21.0, z * 0.045 + 8.0));
    let shade = (0.88 + 0.2 * value_noise(x * 0.3 + 3.0, z * 0.3 - 11.0)) * (0.86 + 0.24 * hash(gx, gz, 10));
    let tip_warm = (warm + (hash(gx, gz, 11) - 0.5) * 0.35).clamp(0.0, 1.0);
    let colours = [
        palette.base * shade,
        palette.mid_lush.lerp(palette.mid_warm, warm) * shade,
        palette.tip_lush.lerp(palette.tip_warm, tip_warm) * shade,
    ];
    Some(Blade {
        root: Vec3::new(x, ground.height_at(x, z) - ROOT_SINK, z),
        height: height.clamp(0.2, 0.9),
        width,
        yaw,
        lean,
        curve,
        rand,
        keep: hash(gx, gz, 12),
        colours,
    })
}

/// Vertex streams for one chunk mesh (chunk-local positions).
#[derive(Default)]
struct BladeMesh {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uv0: Vec<[f32; 2]>,
    uv1: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
    blades: usize,
    min: Vec3,
    max: Vec3,
}

impl BladeMesh {
    fn is_empty(&self) -> bool {
        self.blades == 0
    }

    /// Append `blade` (`segments` ≥ 1, width × `widen`) relative to `origin`: two vertices per
    /// level, one at the tip → 2·segments + 1 vertices, 2·segments − 1 triangles.
    fn push_blade(&mut self, blade: &Blade, origin: Vec3, segments: usize, widen: f32) {
        let segments = segments.max(1);
        let first = self.positions.len() as u32;
        let forward = Vec3::new(blade.yaw.sin(), 0.0, blade.yaw.cos());
        let side = Vec3::new(forward.z, 0.0, -forward.x);
        let root = blade.root - origin;
        let step = blade.height / segments as f32;
        let tangent_angle = |s: f32| blade.lean + blade.curve * s * s;
        let mut offset = Vec3::ZERO;
        for k in 0..=segments {
            let s = k as f32 / segments as f32;
            let a = tangent_angle(s);
            // upper face of a blade curling toward `forward`
            let normal = Vec3::Y * a.sin() - forward * a.cos();
            let c = blade.colour(s);
            let colour = [c.x, c.y, c.z, blade.rand];
            let uv0 = [s, root.y];
            let uv1 = [root.x, root.z];
            if k == segments {
                self.push_vertex(root + offset, normal, uv0, uv1, colour);
            } else {
                let half = 0.5 * blade.width * widen * (1.0 - s).powf(0.6);
                for sign in [-1.0, 1.0] {
                    let n = (normal + side * (sign * EDGE_ROUNDNESS)).normalize_or(normal);
                    self.push_vertex(root + offset + side * (sign * half), n, uv0, uv1, colour);
                }
            }
            let am = tangent_angle((k as f32 + 0.5) / segments as f32);
            offset += (Vec3::Y * am.cos() + forward * am.sin()) * step;
        }
        for k in 0..segments as u32 {
            let (l0, r0) = (first + 2 * k, first + 2 * k + 1);
            if k + 1 == segments as u32 {
                self.indices.extend([l0, r0, first + 2 * k + 2]);
            } else {
                let (l1, r1) = (l0 + 2, r0 + 2);
                self.indices.extend([l0, r0, l1, r0, r1, l1]);
            }
        }
        self.blades += 1;
    }

    fn push_vertex(&mut self, p: Vec3, n: Vec3, uv0: [f32; 2], uv1: [f32; 2], colour: [f32; 4]) {
        if self.positions.is_empty() {
            (self.min, self.max) = (p, p);
        } else {
            self.min = self.min.min(p);
            self.max = self.max.max(p);
        }
        self.positions.push(p.to_array());
        self.normals.push(n.to_array());
        self.uv0.push(uv0);
        self.uv1.push(uv1);
        self.colors.push(colour);
    }

    /// Bounds that contain every swaying, bent or trampled pose (bending only lowers vertices).
    fn aabb(&self) -> Aabb {
        let grow = Vec3::new(SWAY_MARGIN, 0.05, SWAY_MARGIN);
        Aabb::from_min_max(self.min - grow, self.max + grow)
    }

    fn into_mesh(self) -> Mesh {
        let indices = if self.positions.len() <= u16::MAX as usize + 1 {
            Indices::U16(self.indices.iter().map(|&i| i as u16).collect())
        } else {
            Indices::U32(self.indices)
        };
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv0)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.uv1)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(indices)
    }
}

/// Both LOD meshes of one chunk; `origin` is the entity translation (chunk centre on the ground,
/// which is also where `VisibilityRange` measures from).
struct ChunkGrass {
    origin: Vec3,
    near: BladeMesh,
    far: BladeMesh,
}

/// Grow chunk (ci, cj), counted from the terrain's −x, −z corner.
fn build_chunk(ci: usize, cj: usize, ground: &GroundGrid, exclusions: &[Exclusion], palette: &Palette) -> ChunkGrass {
    let min = Vec2::new(-TERRAIN_HALF + ci as f32 * CHUNK_SIZE, -TERRAIN_HALF + cj as f32 * CHUNK_SIZE);
    let max = min + Vec2::splat(CHUNK_SIZE);
    let centre = (min + max) * 0.5;
    let origin = Vec3::new(centre.x, ground.height_at(centre.x, centre.y), centre.y);
    let mut chunk = ChunkGrass {
        origin,
        near: BladeMesh::default(),
        far: BladeMesh::default(),
    };
    // the courtyard never grows grass: skip chunks entirely inside it
    if max.x.abs().max(min.x.abs()) <= COURTYARD_HALF && max.y.abs().max(min.y.abs()) <= COURTYARD_HALF {
        return chunk;
    }
    let local: Vec<Exclusion> = exclusions.iter().copied().filter(|e| e.overlaps(min, max)).collect();
    let (gx0, gz0) = ((ci * CELLS_PER_CHUNK) as i32, (cj * CELLS_PER_CHUNK) as i32);
    for b in 0..CELLS_PER_CHUNK as i32 {
        for a in 0..CELLS_PER_CHUNK as i32 {
            let Some(blade) = plan_blade(gx0 + a, gz0 + b, ground, &local, palette) else {
                continue;
            };
            chunk.near.push_blade(&blade, origin, SEGMENTS_NEAR, 1.0);
            if blade.keep < LOD1_KEEP {
                chunk.far.push_blade(&blade, origin, SEGMENTS_FAR, LOD1_WIDEN);
            }
        }
    }
    chunk
}

/// Grow every chunk on all cores; the result is in chunk order whatever the thread count.
fn build_chunks(ground: &GroundGrid, exclusions: &[Exclusion], palette: &Palette) -> Vec<ChunkGrass> {
    let jobs: Vec<(usize, usize)> = (0..CHUNKS).flat_map(|cj| (0..CHUNKS).map(move |ci| (ci, cj))).collect();
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
    let mut slots: Vec<Option<ChunkGrass>> = jobs.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                let jobs = &jobs;
                scope.spawn(move || {
                    jobs.iter()
                        .enumerate()
                        .skip(w)
                        .step_by(workers)
                        .map(|(k, &(ci, cj))| (k, build_chunk(ci, cj, ground, exclusions, palette)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for handle in handles {
            for (k, chunk) in handle.join().expect("grass worker panicked") {
                slots[k] = Some(chunk);
            }
        }
    });
    slots.into_iter().flatten().collect()
}

/// Ground footprint of a static collider that stands on the ground, if it should clear grass.
fn exclusion_from_aabb(min: Vec3, max: Vec3) -> Option<Exclusion> {
    let size = max - min;
    if !size.is_finite()
        || size.x.max(size.z) > EXCLUSION_MAX_SIZE
        || size.x.max(size.z) < EXCLUSION_MIN_SIZE
    {
        return None;
    }
    let centre = Vec2::new(min.x + max.x, min.z + max.z) * 0.5;
    // floating shapes (canopies, lintels) leave the grass under them alone
    if min.y > ground_height(centre.x, centre.y) + 0.4 {
        return None;
    }
    Some(Exclusion {
        centre,
        half: Vec2::new(size.x, size.z) * 0.5 + Vec2::splat(EXCLUSION_MARGIN),
    })
}

fn spawn_grass(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    wind: Res<GrassWind>,
    colliders: Query<(&Collider, &GlobalTransform, Option<&RigidBody>), (Without<Sensor>, Without<Terrain>, Without<Mover>)>,
) {
    let started = std::time::Instant::now();
    let exclusions: Vec<Exclusion> = colliders
        .iter()
        .filter(|(_, _, body)| matches!(body, None | Some(RigidBody::Static)))
        .filter_map(|(collider, transform, _)| {
            let (_, rotation, translation) = transform.to_scale_rotation_translation();
            let aabb = collider.aabb(translation, rotation);
            exclusion_from_aabb(aabb.min, aabb.max)
        })
        .collect();
    let ground = GroundGrid::sample();
    let chunks = build_chunks(&ground, &exclusions, &Palette::new());

    let data = wind_data(&wind, &WindClock::default(), &[]);
    let wind_buffer = buffers.add(ShaderBuffer::from(data));
    let material = materials.add(GrassMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.7,
            reflectance: 0.3,
            // one shared up-biased normal for both faces: no back-face flip, no culling
            double_sided: false,
            cull_mode: None,
            opaque_render_method: OpaqueRendererMethod::Forward,
            ..default()
        },
        extension: GrassExtension {
            wind: wind_buffer.clone(),
        },
    });
    commands.insert_resource(GrassBuffers { wind: wind_buffer });

    let (mut blades, mut near_verts, mut far_verts, mut entities) = (0, 0, 0, 0);
    let near_range = VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: (LOD0_END - 0.5 * LOD_BLEND)..(LOD0_END + 0.5 * LOD_BLEND),
        use_aabb: false,
    };
    // past FADE_END every blade of a chunk has shrunk away even at the chunk's near corner
    let chunk_reach = CHUNK_SIZE * std::f32::consts::FRAC_1_SQRT_2;
    let far_range = VisibilityRange {
        start_margin: near_range.end_margin.clone(),
        end_margin: (FADE_END + chunk_reach)..(FADE_END + chunk_reach + 2.0),
        use_aabb: false,
    };
    for chunk in chunks {
        blades += chunk.near.blades;
        for (mesh, range, lod) in [(chunk.near, &near_range, "near"), (chunk.far, &far_range, "far")] {
            if mesh.is_empty() {
                continue;
            }
            if lod == "near" {
                near_verts += mesh.positions.len();
            } else {
                far_verts += mesh.positions.len();
            }
            let aabb = mesh.aabb();
            commands.spawn((
                Name::new(format!("Grass chunk ({lod})")),
                GrassChunk,
                Mesh3d(meshes.add(mesh.into_mesh())),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(chunk.origin),
                aabb,
                NoAutoAabb,
                NotShadowCaster,
                range.clone(),
            ));
            entities += 1;
        }
    }
    info!(
        "[grass] {blades} blades, {entities} chunk meshes ({near_verts} near + {far_verts} far vertices) in {:.0} ms",
        started.elapsed().as_secs_f32() * 1000.0
    );
}

/// Up to `MAX_PUSHERS` feet positions nearest `camera`, as (feet, radius) vec4s, and the count.
fn nearest_pushers(camera: Vec3, feet: impl Iterator<Item = Vec3>, scratch: &mut Vec<(f32, Vec3)>) -> ([Vec4; MAX_PUSHERS], usize) {
    scratch.clear();
    scratch.extend(feet.filter(|f| f.is_finite()).map(|f| (f.distance_squared(camera), f)));
    scratch.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = [Vec4::ZERO; MAX_PUSHERS];
    let count = scratch.len().min(MAX_PUSHERS);
    for (slot, (_, f)) in out.iter_mut().zip(scratch.iter()) {
        *slot = f.extend(TRAMPLE_RADIUS);
    }
    (out, count)
}

/// Pack wind, clock and pushers for the shader.
fn wind_data(wind: &GrassWind, clock: &WindClock, pushers: &[Vec4]) -> GrassWindData {
    let dir = wind_direction(wind.direction);
    let mut slots = [Vec4::ZERO; MAX_PUSHERS];
    let count = pushers.len().min(MAX_PUSHERS);
    slots[..count].copy_from_slice(&pushers[..count]);
    let gust = wind.gust;
    GrassWindData {
        wind: Vec4::new(dir.x, dir.y, wind.strength.max(0.0), clock.phase),
        motion: Vec4::new(clock.scroll.x, clock.scroll.y, clock.time, count as f32),
        gust: gust.centre.extend(gust.radius.max(0.0)),
        params: Vec4::new(gust.strength, FADE_START, FADE_END, TRAMPLE_BEND),
        shape: Vec4::new(TAU / GUST_WAVELENGTH, MAX_BEND, NORMAL_UP_BIAS, FLUTTER),
        pushers: slots,
    }
}

fn update_wind(
    time: Res<Time>,
    wind: Res<GrassWind>,
    grass: Option<Res<GrassBuffers>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    camera: Query<&Transform, With<MainCamera>>,
    movers: Query<&Transform, (With<Mover>, Without<MainCamera>)>,
    mut clock: Local<WindClock>,
    mut scratch: Local<Vec<(f32, Vec3)>>,
) {
    let Some(grass) = grass else {
        return;
    };
    *clock = clock.advance(&wind, time.delta_secs());
    let eye = camera.iter().next().map_or(Vec3::ZERO, |t| t.translation);
    let feet = movers.iter().map(|t| t.translation - Vec3::Y * HALF_HEIGHT);
    let (pushers, count) = nearest_pushers(eye, feet, &mut scratch);
    let data = wind_data(&wind, &clock, &pushers[..count]);
    if let Some(mut buffer) = buffers.get_mut(&grass.wind) {
        buffer.set_data(data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blade() -> Blade {
        Blade {
            root: Vec3::new(30.0, 1.0, 20.0),
            height: 0.6,
            width: 0.04,
            yaw: 0.7,
            lean: 0.2,
            curve: 0.5,
            rand: 0.3,
            keep: 0.1,
            colours: [Vec3::splat(0.05), Vec3::splat(0.2), Vec3::splat(0.6)],
        }
    }

    #[test]
    fn chunk_grid_tiles_the_terrain() {
        assert_eq!(CHUNKS as f32 * CHUNK_SIZE, 2.0 * TERRAIN_HALF);
        // the densest LOD0 chunk still fits 16-bit indices
        assert!(CELLS_PER_CHUNK * CELLS_PER_CHUNK * (2 * SEGMENTS_NEAR + 1) <= u16::MAX as usize);
        assert!(LOD0_END + LOD_BLEND < FADE_START && FADE_START < FADE_END);
    }

    #[test]
    fn no_grass_on_courtyard_pond_rim_rock_or_water() {
        let up = Vec3::Y;
        assert_eq!(grass_cover(0.0, 0.0, 0.0, up), 0.0);
        assert_eq!(grass_cover(COURTYARD_HALF - 0.1, 3.0, 0.0, up), 0.0);
        assert_eq!(grass_cover(POND_CENTER.x, POND_CENTER.y, -1.5, up), 0.0);
        let bank = POND_CENTER + Vec2::new(POND_RADIUS + 1.0, 0.0);
        assert_eq!(grass_cover(bank.x, bank.y, 0.0, up), 0.0);
        assert_eq!(grass_cover(30.0, 20.0, WATER_LEVEL - 0.1, up), 0.0);
        assert_eq!(grass_cover(TERRAIN_HALF - 2.0, 10.0, 1.0, up), 0.0);
        let steep = Vec3::new(0.7, 0.7, 0.0).normalize();
        assert_eq!(grass_cover(30.0, 20.0, 1.0, steep), 0.0);
        assert_eq!(grass_cover(30.0, 20.0, 10.0, up), 0.0);
        // open, flat, low meadow is fully grassy
        assert!(grass_cover(30.0, 20.0, 1.0, up) > 0.99);
        // borders fade instead of cutting off
        let fringe = grass_cover(COURTYARD_HALF + 1.2, 0.0, 0.0, up);
        assert!(fringe > 0.05 && fringe < 0.95, "{fringe}");
    }

    #[test]
    fn ground_grid_matches_the_terrain_mesh() {
        let ground = GroundGrid::sample();
        for (x, z) in [(0.0, 0.0), (20.5, -31.0), (-40.0, 12.5), (55.0, 55.0)] {
            assert!((ground.height_at(x, z) - ground_height(x, z)).abs() < 1e-4, "{x},{z}");
        }
        // between nodes the height lies within its cell's corners
        let (x, z) = (20.3, -30.85);
        let h = ground.height_at(x, z);
        let corners = [(20.0, -31.0), (20.5, -31.0), (20.0, -30.5), (20.5, -30.5)].map(|(x, z)| ground_height(x, z));
        let (lo, hi) = corners.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &c| (lo.min(c), hi.max(c)));
        assert!(h >= lo - 1e-4 && h <= hi + 1e-4);
        assert_eq!(ground.cover_at(0.0, 0.0), 0.0);
    }

    #[test]
    fn blade_mesh_has_expected_counts_and_finite_values() {
        for (segments, verts, tris) in [(SEGMENTS_NEAR, 9, 7), (SEGMENTS_FAR, 7, 5)] {
            let mut mesh = BladeMesh::default();
            let b = blade();
            mesh.push_blade(&b, Vec3::new(32.0, 0.5, 20.0), segments, 1.0);
            mesh.push_blade(&b, Vec3::new(32.0, 0.5, 20.0), segments, LOD1_WIDEN);
            assert_eq!(mesh.positions.len(), 2 * verts);
            assert_eq!(mesh.indices.len(), 2 * 3 * tris);
            assert_eq!(mesh.blades, 2);
            assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.positions.len()));
            assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(mesh.normals.iter().all(|n| (Vec3::from_array(*n).length() - 1.0).abs() < 1e-3));
            let root = b.root - Vec3::new(32.0, 0.5, 20.0);
            assert!(mesh.uv0.iter().all(|uv| (0.0..=1.0).contains(&uv[0]) && uv[1] == root.y));
            assert!(mesh.colors.iter().all(|c| c.iter().all(|v| v.is_finite()) && c[3] == b.rand));
            // the tip is the last vertex, at t = 1, above the root and no further than the blade length
            let tip = Vec3::from_array(mesh.positions[verts - 1]);
            assert_eq!(mesh.uv0[verts - 1][0], 1.0);
            assert!(tip.y > root.y + 0.3 && tip.distance(root) <= b.height + 1e-4);
            assert_eq!(mesh.uv1[0], [root.x, root.z]);
            let aabb = mesh.aabb();
            assert!(aabb.half_extents.x >= SWAY_MARGIN);
        }
    }

    #[test]
    fn chunks_are_deterministic_finite_and_respect_the_courtyard() {
        let ground = GroundGrid::sample();
        let palette = Palette::new();
        let centre = build_chunk(CHUNKS / 2, CHUNKS / 2, &ground, &[], &palette);
        assert!(centre.near.is_empty() && centre.far.is_empty());
        let mut total = 0;
        for (ci, cj) in [(10, 10), (3, 11), (11, 2), (12, 7)] {
            let a = build_chunk(ci, cj, &ground, &[], &palette);
            let b = build_chunk(ci, cj, &ground, &[], &palette);
            assert_eq!(a.near.positions, b.near.positions);
            assert_eq!(a.far.indices, b.far.indices);
            assert!(a.near.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(a.far.blades <= a.near.blades);
            total += a.near.blades;
        }
        assert!(total > 1000, "only {total} blades on the hills");
    }

    #[test]
    fn obstacles_clear_their_footprint() {
        let e = exclusion_from_aabb(Vec3::new(29.5, -0.2, 19.5), Vec3::new(30.5, 1.0, 20.5)).expect("rock clears grass");
        assert_eq!(e.clearance(Vec2::new(30.0, 20.0)), 0.0);
        assert_eq!(e.clearance(Vec2::new(32.0, 20.0)), 1.0);
        // walls, posts and floating canopies leave the grass alone
        assert!(exclusion_from_aabb(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 60.0, 120.0)).is_none());
        assert!(exclusion_from_aabb(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.1, 1.0, 0.1)).is_none());
        assert!(exclusion_from_aabb(Vec3::new(30.0, 30.0, 20.0), Vec3::new(31.0, 32.0, 21.0)).is_none());
    }

    #[test]
    fn nearest_pushers_keep_the_closest_eight() {
        let feet: Vec<Vec3> = (0..12).map(|i| Vec3::new(i as f32, 0.0, 0.0)).collect();
        let mut scratch = Vec::new();
        let (slots, count) = nearest_pushers(Vec3::new(20.0, 0.0, 0.0), feet.into_iter(), &mut scratch);
        assert_eq!(count, MAX_PUSHERS);
        assert_eq!(slots[0], Vec4::new(11.0, 0.0, 0.0, TRAMPLE_RADIUS));
        assert!(slots.iter().all(|s| s.x >= 4.0));
        let (_, none) = nearest_pushers(Vec3::ZERO, std::iter::empty(), &mut scratch);
        assert_eq!(none, 0);
    }

    #[test]
    fn wind_clock_and_buffer_stay_finite() {
        let wind = GrassWind {
            direction: Vec2::ZERO,
            ..default()
        };
        let mut clock = WindClock::default();
        for _ in 0..100_000 {
            clock = clock.advance(&wind, 0.05);
        }
        assert!((0.0..TAU).contains(&clock.phase));
        assert!((0.0..FLUTTER_WRAP).contains(&clock.time));
        assert!(clock.scroll.cmpge(Vec2::ZERO).all() && clock.scroll.cmplt(Vec2::splat(SCROLL_WRAP)).all());
        let data = wind_data(&wind, &clock, &[Vec4::ONE; 10]);
        assert_eq!(data.motion.w, MAX_PUSHERS as f32);
        assert_eq!(Vec2::new(data.wind.x, data.wind.y), Vec2::X);
        // 5 vec4 headers + MAX_PUSHERS vec4s, as grass.wgsl declares
        assert_eq!(GrassWindData::min_size().get(), 16 * (5 + MAX_PUSHERS as u64));
    }
}
