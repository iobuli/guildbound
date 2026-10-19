//! World surface detail: tileable albedo-detail and normal maps generated on the CPU at startup
//! (no image files), and world-scale UVs + tangents for the meshes that wear them.
//!
//! Each `Surface` is a pair of `SIZE`² RGBA8 images with full mip chains (`armor::add_mips`) and
//! a repeating anisotropic sampler:
//! - `detail` (sRGB): near-white variation (≈0.75–1.0) that multiplies the material's
//!   `base_color` and any vertex colour, so the palette and the terrain's vertex colours keep
//!   their hue and only gain texture;
//! - `normal` (linear, OpenGL convention like Bevy expects): from a tileable height field by
//!   central differences, scaled by the surface's `bump`.
//!
//! Heights come from periodic value noise (fBm) and periodic Worley cells, so every map tiles
//! seamlessly: `ground` (soil and pebbles), `stone` (laid stones with mortar cracks), `rock`
//! (rough fractured face), `wood` (warped grain), `metal` (hammer dents), `cloth` (weave),
//! `sand` (wind ripples).
//!
//! UVs: `world_uvs` box-projects a mesh's local positions (times its transform scale) on the axis
//! of each vertex normal, `1 / tile` UV per metre, then generates tangents; `world` applies it to
//! every static prop it spawns, and the terrain projects (x, z) directly. Texture size and look
//! are set by the constants and `Recipe`s below.
use super::armor::{Texel, add_mips};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Texture edge (texels).
pub const SIZE: usize = 256;
/// Metres one texture repeat covers on props and on the terrain.
pub const PROP_TILE: f32 = 1.5;
pub const TERRAIN_TILE: f32 = 3.0;

/// A generated detail + normal map pair.
#[derive(Clone, Debug)]
pub struct Surface {
    pub detail: Handle<Image>,
    pub normal: Handle<Image>,
}

impl Surface {
    /// A material using this surface over `base` (keeps the rest of `base`).
    pub fn material(&self, base: StandardMaterial) -> StandardMaterial {
        StandardMaterial {
            base_color_texture: Some(self.detail.clone()),
            normal_map_texture: Some(self.normal.clone()),
            ..base
        }
    }
}

/// Every world surface.
#[derive(Resource, Clone, Debug)]
pub struct Surfaces {
    pub ground: Surface,
    pub stone: Surface,
    pub rock: Surface,
    pub wood: Surface,
    pub metal: Surface,
    pub cloth: Surface,
    pub sand: Surface,
}

impl Surfaces {
    pub fn build(images: &mut Assets<Image>) -> Self {
        let mut make = |r: Recipe| {
            let (detail, normal) = r.bake();
            Surface {
                detail: images.add(detail),
                normal: images.add(normal),
            }
        };
        Self {
            ground: make(Recipe::Ground),
            stone: make(Recipe::Stone),
            rock: make(Recipe::Rock),
            wood: make(Recipe::Wood),
            metal: make(Recipe::Metal),
            cloth: make(Recipe::Cloth),
            sand: make(Recipe::Sand),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipe {
    Ground,
    Stone,
    Rock,
    Wood,
    Metal,
    Cloth,
    Sand,
}

impl Recipe {
    /// Normal strength (height units per texel → slope).
    fn bump(self) -> f32 {
        match self {
            Recipe::Ground => 2.4,
            Recipe::Stone => 5.0,
            Recipe::Rock => 6.0,
            Recipe::Wood => 2.5,
            Recipe::Metal => 1.5,
            Recipe::Cloth => 2.0,
            Recipe::Sand => 2.0,
        }
    }

    /// Height (0..1) and detail brightness (≈0.75..1) at tile coordinates `u, v` in 0..1.
    pub fn sample(self, u: f32, v: f32) -> (f32, f32) {
        match self {
            Recipe::Ground => {
                let soil = fbm(u, v, 8, 5, 0.5, 11);
                let (f1, _, cell) = worley(u, v, 14, 23);
                // A pebble in one cell in seven, small: denser read as speckle noise from eye height.
                let pebble = (1.0 - (f1 / 0.28).min(1.0)).powi(2) * step(0.86, cell);
                let h = soil * 0.7 + pebble * 0.6;
                (h, 0.8 + 0.2 * soil - 0.08 * pebble)
            }
            Recipe::Stone => {
                // Laid stones: big cells, mortar where the two nearest seeds are equidistant.
                let (f1, f2, cell) = worley(u, v, 5, 7);
                let mortar = smooth(0.0, 0.07, f2 - f1);
                let grain = fbm(u, v, 16, 4, 0.5, 3);
                let dome = (1.0 - f1).clamp(0.0, 1.0) * 0.25;
                let h = mortar * (0.55 + dome + grain * 0.25);
                (h, 0.72 + 0.12 * cell + 0.12 * grain - 0.15 * (1.0 - mortar))
            }
            Recipe::Rock => {
                let (f1, f2, _) = worley(u, v, 6, 31);
                let crack = smooth(0.0, 0.04, f2 - f1);
                let rough = fbm(u, v, 6, 6, 0.55, 17);
                let h = rough * 0.8 + crack * 0.3;
                (h, 0.78 + 0.2 * rough - 0.12 * (1.0 - crack))
            }
            Recipe::Wood => {
                // Grain runs along V (props are mostly boards along their long side).
                let warp = fbm(u, v, 3, 3, 0.5, 41) * 2.0;
                let rings = (u * 18.0 + warp).fract();
                let ring = smooth(0.0, 0.15, rings) * smooth(1.0, 0.6, rings);
                // 4 repeats across × 8 cells: fibres stretched along V, still tiling.
                let fibre = fbm(u * 4.0, v, 8, 3, 0.5, 5);
                let h = ring * 0.6 + fibre * 0.4;
                (h, 0.78 + 0.16 * ring + 0.06 * fibre)
            }
            Recipe::Metal => {
                let (f1, _, _) = worley(u, v, 9, 53);
                let dent = 1.0 - (f1 * 1.4).min(1.0).powi(2);
                let fine = fbm(u, v, 32, 3, 0.5, 9);
                (dent * 0.5 + fine * 0.15, 0.9 + 0.1 * fine)
            }
            Recipe::Cloth => {
                let tau = std::f32::consts::TAU;
                let n = 48.0;
                let warp = (u * n * tau).sin() * 0.5 + 0.5;
                let weft = (v * n * tau).sin() * 0.5 + 0.5;
                let over = if ((u * n).floor() + (v * n).floor()) as i32 % 2 == 0 { warp } else { weft };
                let fuzz = fbm(u, v, 32, 2, 0.5, 13);
                (over * 0.8 + fuzz * 0.2, 0.84 + 0.12 * over + 0.04 * fuzz)
            }
            Recipe::Sand => {
                let tau = std::f32::consts::TAU;
                let warp = fbm(u, v, 3, 3, 0.5, 61);
                let ripple = ((v * 9.0 + warp * 1.5) * tau).sin() * 0.5 + 0.5;
                let grain = fbm(u, v, 64, 2, 0.5, 19);
                (ripple * 0.6 + grain * 0.4, 0.86 + 0.1 * grain + 0.04 * ripple)
            }
        }
    }

    /// The (detail, normal) images.
    pub fn bake(self) -> (Image, Image) {
        let n = SIZE;
        let mut height = vec![0.0f32; n * n];
        let mut detail = vec![0u8; n * n * 4];
        for y in 0..n {
            for x in 0..n {
                let (h, d) = self.sample((x as f32 + 0.5) / n as f32, (y as f32 + 0.5) / n as f32);
                height[y * n + x] = h;
                let g = (d.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                detail[(y * n + x) * 4..][..4].copy_from_slice(&[g, g, g, 255]);
            }
        }
        let normal = normals_from_height(&height, n, self.bump());
        let detail = image(detail, TextureFormat::Rgba8UnormSrgb, Texel::Srgb);
        let normal = image(normal, TextureFormat::Rgba8Unorm, Texel::Normal);
        (detail, normal)
    }
}

/// RGBA8 tangent-space normals (OpenGL: +Y up the V axis) from a wrapping height field.
pub fn normals_from_height(height: &[f32], n: usize, bump: f32) -> Vec<u8> {
    let at = |x: isize, y: isize| {
        let (x, y) = (x.rem_euclid(n as isize) as usize, y.rem_euclid(n as isize) as usize);
        height[y * n + x]
    };
    let mut out = vec![0u8; n * n * 4];
    for y in 0..n as isize {
        for x in 0..n as isize {
            // Heights are per texture; a texel step is 1/n of a tile, so scale slopes by n/64.
            let k = bump * n as f32 / 64.0;
            let dx = (at(x + 1, y) - at(x - 1, y)) * 0.5 * k;
            // Image rows run down V; flip so +Y points up the texture (OpenGL normal maps).
            let dy = (at(x, y - 1) - at(x, y + 1)) * 0.5 * k;
            let v = Vec3::new(-dx, -dy, 1.0).normalize();
            let enc = |c: f32| ((c * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            let i = (y as usize * n + x as usize) * 4;
            out[i..i + 4].copy_from_slice(&[enc(v.x), enc(v.y), enc(v.z), 255]);
        }
    }
    out
}

fn image(data: Vec<u8>, format: TextureFormat, texel: Texel) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        anisotropy_clamp: 8,
        ..ImageSamplerDescriptor::linear()
    });
    add_mips(&mut image, texel);
    image
}

// ---------------------------------------------------------------------------------------------
// UVs.

/// Box-project world-scale UVs onto `mesh` (local positions times `scale`, `1 / tile` per metre,
/// projected along each vertex normal's dominant axis) and generate tangents. Meshes without
/// positions or normals are left alone.
pub fn world_uvs(mesh: &mut Mesh, scale: Vec3, tile: f32) {
    let (Some(VertexAttributeValues::Float32x3(positions)), Some(VertexAttributeValues::Float32x3(normals))) =
        (mesh.attribute(Mesh::ATTRIBUTE_POSITION), mesh.attribute(Mesh::ATTRIBUTE_NORMAL))
    else {
        return;
    };
    let tile = if tile.is_finite() && tile > 1e-3 { tile } else { 1.0 };
    let uvs: Vec<[f32; 2]> = positions
        .iter()
        .zip(normals)
        .map(|(p, n)| box_uv(Vec3::from(*p) * scale, Vec3::from(*n), tile))
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    if mesh.indices().is_some() {
        let _ = mesh.generate_tangents();
    }
}

/// UV of point `p` with normal `n`: the two axes across the normal's dominant one, per `tile` m.
/// V runs along Y on walls so wood grain and stone courses stand upright.
pub fn box_uv(p: Vec3, n: Vec3, tile: f32) -> [f32; 2] {
    let a = n.abs();
    let (u, v) = if a.y >= a.x && a.y >= a.z {
        (p.x, p.z)
    } else if a.x >= a.z {
        (p.z * n.x.signum(), p.y)
    } else {
        (-p.x * n.z.signum(), p.y)
    };
    [u / tile, -v / tile]
}

// ---------------------------------------------------------------------------------------------
// Periodic noise (every function tiles over u, v in 0..1).

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn step(edge: f32, x: f32) -> f32 {
    if x >= edge { 1.0 } else { 0.0 }
}

/// Value noise on a `period`-cell lattice that wraps.
fn value(u: f32, v: f32, period: i32, seed: u32) -> f32 {
    let (x, y) = (u * period as f32, v * period as f32);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let w = |i: i32| i.rem_euclid(period);
    let (xi, yi) = (x0 as i32, y0 as i32);
    let corner = |dx: i32, dy: i32| hash(w(xi + dx), w(yi + dy), seed);
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    top + (bottom - top) * sy
}

/// Fractal sum of `octaves` periodic value noises from `period` cells, each octave doubling the
/// period (so it still tiles) and scaling by `gain`. Normalised to 0..1.
fn fbm(u: f32, v: f32, period: i32, octaves: u32, gain: f32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut p) = (0.0, 1.0, 0.0, period.max(1));
    for o in 0..octaves {
        sum += value(u, v, p, seed.wrapping_add(o * 101)) * amp;
        norm += amp;
        amp *= gain;
        p *= 2;
    }
    sum / norm.max(1e-6)
}

/// Periodic Worley noise over `period`² jittered seeds: (nearest distance, second nearest, a
/// per-cell random 0..1 of the nearest), distances in cell units.
fn worley(u: f32, v: f32, period: i32, seed: u32) -> (f32, f32, f32) {
    let (x, y) = (u * period as f32, v * period as f32);
    let (cx, cy) = (x.floor() as i32, y.floor() as i32);
    let (mut f1, mut f2, mut id) = (f32::MAX, f32::MAX, 0.0);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (gx, gy) = (cx + dx, cy + dy);
            let (wx, wy) = (gx.rem_euclid(period), gy.rem_euclid(period));
            let px = gx as f32 + hash(wx, wy, seed);
            let py = gy as f32 + hash(wx, wy, seed ^ 0x51ED);
            let d = Vec2::new(px - x, py - y).length();
            if d < f1 {
                f2 = f1;
                f1 = d;
                id = hash(wx, wy, seed ^ 0xA5A5);
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    (f1, f2, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Recipe; 7] = [
        Recipe::Ground,
        Recipe::Stone,
        Recipe::Rock,
        Recipe::Wood,
        Recipe::Metal,
        Recipe::Cloth,
        Recipe::Sand,
    ];

    /// Opposite edges of every height field match, so the maps tile without a seam.
    #[test]
    fn heights_tile() {
        for r in ALL {
            for i in 0..32 {
                let t = i as f32 / 32.0;
                let (a, _) = r.sample(0.0, t);
                let (b, _) = r.sample(1.0, t);
                let (c, _) = r.sample(t, 0.0);
                let (d, _) = r.sample(t, 1.0);
                assert!((a - b).abs() < 0.02 && (c - d).abs() < 0.02, "{r:?} at {t}: {a} {b} {c} {d}");
            }
        }
    }

    #[test]
    fn bakes_mipped_maps_in_range() {
        for r in ALL {
            let (detail, normal) = r.bake();
            assert!(detail.texture_descriptor.mip_level_count > 1);
            assert!(normal.texture_descriptor.mip_level_count > 1);
            let base = &normal.data.as_ref().unwrap()[..SIZE * SIZE * 4];
            // Normals face out of the surface (blue channel high).
            assert!(base.chunks(4).all(|t| t[2] >= 128), "{r:?}");
            let lum = &detail.data.as_ref().unwrap()[..SIZE * SIZE * 4];
            assert!(lum.chunks(4).all(|t| t[0] >= 150), "{r:?} detail too dark");
        }
    }

    #[test]
    fn box_uvs_scale_with_size() {
        let a = box_uv(Vec3::new(3.0, 0.0, 1.5), Vec3::Y, 1.5);
        assert_eq!(a, [2.0, -1.0]);
        let wall = box_uv(Vec3::new(0.0, 3.0, 0.0), Vec3::X, 1.5);
        assert_eq!(wall[1], -2.0);
    }
}
