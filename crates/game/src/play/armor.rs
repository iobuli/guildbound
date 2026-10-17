//! Plate armour: a light adventurer set (open bascinet, cuirass, three-lame pauldrons,
//! vambraces, greaves) built at startup from the character's own body and worn as rigid steel.
//! `armor` owns every plate entity, `ArmorSet`, the armour stand (its pose and availability) and
//! the wearer's `BodyPadding`.
//!
//! Building (on the async pool, one thread per plate, from `anim/ual2.glb`; bind pose = skeleton
//! rest to 1 µm, mesh in model space): most plates are idealised shells around a `Profile`, the
//! outermost skin radius per (height, angle) bin from rays cast out of an axis or centre against
//! the plate's body region, filled, dilated and blurred. The plate lies a clearance `gap` outside
//! that profile with a `thick`ness, both measured along the surface normal (the radial offset is
//! scaled by the local slope, so steep areas do not thin out). Its cut lines are designed curves
//! in the same frame (arm holes, a neck dip, the bascinet's face opening) with a rolled bead along
//! every edge, a ridge on the breastplate and the shins, a flared lower edge on the cuirass and
//! rivets. Pauldrons are different: three lames that are true spherical bands around the shoulder
//! joint with strictly increasing radii, each lower lame over the one above.
//!
//! The density is sampled on a `CELL` lattice and meshed with Surface Nets
//! (`voxel::surface_nets`), then given smooth normals, UVs (cylindrical with the seam split for
//! tubes, azimuthal about the pole for spheres, so neither has a singularity on steel), tangents
//! and vertex colours (tint, polished edges, darker inside). One material carries the detail:
//! ambientCG Metal049C (hammered steel colour, normal and metallic-roughness), with mip chains
//! generated at load.
//!
//! Wearing: plates never bend. Every frame each plate takes the least-squares rigid motion of the
//! skin under it (Kabsch over its sample points skinned from `motion::RigPose`, Müller et al.'s
//! rotation extraction, warm-started). Pauldron lames take only a rotation about the shoulder
//! joint's current position, so their concentric spheres can never cut into each other, however
//! differently they turn. The wearer gets `BodyPadding` derived from the built plates, so
//! `motion::body` keeps arms and legs clear of the steel.
//!
//! The stand: a dark wooden mannequin (the character model frozen in its idle pose) wears the set.
//! E there plays the reach clip; at its beat the plates leave the stand one after another, fly to
//! the player and lock into place with a clank and a small jolt. E again hangs them back.
use super::camera::CameraShake;
use super::fx::{Sfx, SfxKind};
use super::interact::{PropState, UseEffect};
use super::voxel::{Lattice, surface_nets};
use super::{Interactable, Phys, Player};
use crate::motion::body::{ARM_R, BodyPadding, CALF_R, HEAD_LIFT, HEAD_R, TORSO_R};
use crate::motion::{
    Bones, ContactRequest, Layer, Mask, ModelRequest, MotionLayers, MotionSet, Rig, RigPose, Skeleton, Source,
};
use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use gbperf::PerfApp;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};
use bevy::transform::TransformSystems;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

/// Lattice spacing (m) the plates are sampled at.
const CELL: f32 = 0.0048;
/// Plate thickness (m) along the normal: over 2 cell diagonals' worth so the two faces never
/// share a Surface Nets cell.
const THICK: f32 = 0.011;
/// The steel texture repeats every `TEX` metres of plate.
const TEX: f32 = 0.4;
const STEEL: Color = Color::srgb(0.66, 0.68, 0.72);
/// Rivet head radius (m).
const RIVET_R: f32 = 0.0065;
/// Skin points each plate is fitted to, and the most rotation-extraction iterations per frame
/// (warm-started frames stop after a few; a cold start needs about 30).
const FIT_SAMPLES: usize = 40;
const FIT_ITERATIONS: usize = 32;
/// Equip flight: seconds per plate, delay between plates leaving, arc height (m).
const FLY_TIME: f32 = 0.45;
const FLY_STAGGER: f32 = 0.08;
const FLY_ARC: f32 = 0.3;
/// Lock-in jolt (rad/s) on the plate's bone and camera shake for the player.
const JOLT: f32 = 1.6;
const LOCK_SHAKE: f32 = 0.04;
/// The stand's frozen pose: clip and time.
const STAND_CLIP: &str = "ual1/Idle_Loop";
const STAND_POSE_T: f32 = 0.6;
/// Extra clearance (m) on top of the plates' own extent in the derived `BodyPadding`.
const PAD_MARGIN: f32 = 0.006;
const TEXTURES: [&str; 3] = [
    "textures/armor/plate_color.png",
    "textures/armor/plate_normal.png",
    "textures/armor/plate_mr.png",
];

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, start_build)
        .add_systems(
            Update,
            (
                finish_build,
                make_material,
                dress_stands,
                stand_availability,
                pose_stands,
                equip.after(super::interact::drive),
                launch.after(equip),
            ),
        )
        .add_systems(
            PostUpdate,
            fit_plates
                .after(MotionSet::Compose)
                .before(TransformSystems::Propagate),
        )
        .perf_span(PostUpdate, "armor fit", fit_plates);
}

// ---------------------------------------------------------------------------------------------
// Geometry helpers.

fn smoothstep01(x: f32) -> f32 {
    let x = if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 };
    x * x * (3.0 - 2.0 * x)
}

/// Angle in (−π, π].
fn wrap(a: f32) -> f32 {
    let a = (a + PI).rem_euclid(TAU) - PI;
    if a <= -PI { a + TAU } else { a }
}

fn angle_dist(a: f32, b: f32) -> f32 {
    wrap(a - b).abs()
}

/// Möller–Trumbore ray / triangle distance (unit `d`), hits ahead of `o` only.
fn ray_triangle(o: Vec3, d: Vec3, t: &[Vec3; 3]) -> Option<f32> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-10 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let dist = e2.dot(q) * inv;
    (dist > 0.0).then_some(dist)
}

/// Coordinates a plate is designed in.
#[derive(Clone, Copy, Debug)]
pub enum Frame {
    /// `s` = height along `axis` from `origin`, `θ` = angle from `forward` toward
    /// `axis × forward`, radius from the axis.
    Cylinder { origin: Vec3, axis: Vec3, forward: Vec3 },
    /// `s` = polar angle from `pole`, `θ` = azimuth from `forward` toward `pole × forward`,
    /// radius from `origin`.
    Sphere { origin: Vec3, pole: Vec3, forward: Vec3 },
}

impl Frame {
    fn cylinder(origin: Vec3, axis: Vec3, forward: Vec3) -> Self {
        let axis = axis.normalize_or(Vec3::Y);
        let forward = (forward - axis * forward.dot(axis)).normalize_or(axis.any_orthonormal_vector());
        Frame::Cylinder { origin, axis, forward }
    }

    fn sphere(origin: Vec3, pole: Vec3, forward: Vec3) -> Self {
        let pole = pole.normalize_or(Vec3::Y);
        let forward = (forward - pole * forward.dot(pole)).normalize_or(pole.any_orthonormal_vector());
        Frame::Sphere { origin, pole, forward }
    }

    fn basis(&self) -> (Vec3, Vec3, Vec3, Vec3) {
        match *self {
            Frame::Cylinder { origin, axis, forward } => (origin, axis, forward, axis.cross(forward)),
            Frame::Sphere { origin, pole, forward } => (origin, pole, forward, pole.cross(forward)),
        }
    }

    /// (s, θ, radius) of a point.
    pub fn coords(&self, p: Vec3) -> (f32, f32, f32) {
        let (origin, up, forward, side) = self.basis();
        let v = p - origin;
        match self {
            Frame::Cylinder { .. } => {
                let s = v.dot(up);
                let r = v - up * s;
                (s, r.dot(side).atan2(r.dot(forward)), r.length())
            }
            Frame::Sphere { .. } => {
                let rho = v.length();
                let n = v / rho.max(1e-6);
                (n.dot(up).clamp(-1.0, 1.0).acos(), n.dot(side).atan2(n.dot(forward)), rho)
            }
        }
    }

    /// Unit direction away from the axis / centre at (s, θ).
    fn direction(&self, s: f32, theta: f32) -> Vec3 {
        let (_, up, forward, side) = self.basis();
        let around = forward * theta.cos() + side * theta.sin();
        match self {
            Frame::Cylinder { .. } => around,
            Frame::Sphere { .. } => up * s.cos() + around * s.sin(),
        }
    }

    /// Where a ray for bin (s, θ) starts.
    fn ray_origin(&self, s: f32) -> Vec3 {
        let (origin, up, _, _) = self.basis();
        match self {
            Frame::Cylinder { .. } => origin + up * s,
            Frame::Sphere { .. } => origin,
        }
    }

    /// The point at (s, θ, radius).
    pub fn point(&self, s: f32, theta: f32, rho: f32) -> Vec3 {
        self.ray_origin(s) + self.direction(s, theta) * rho
    }

    /// Metres per unit of `s` at radius `rho`.
    fn s_scale(&self, rho: f32) -> f32 {
        match self {
            Frame::Cylinder { .. } => 1.0,
            Frame::Sphere { .. } => rho,
        }
    }

    /// Metres per radian of `θ` at (s, radius).
    fn theta_scale(&self, s: f32, rho: f32) -> f32 {
        match self {
            Frame::Cylinder { .. } => rho,
            Frame::Sphere { .. } => rho * s.sin().abs(),
        }
    }
}

/// Outermost body radius around a frame per (s, θ) bin, cleaned into a smooth profile.
#[derive(Clone, Debug)]
pub struct Profile {
    s0: f32,
    ds: f32,
    ns: usize,
    nt: usize,
    r: Vec<f32>,
}

impl Profile {
    /// Cast every bin's ray against `triangles` (outermost hit), fill misses from neighbours
    /// (`fallback` if nothing was hit at all), dilate by a bin and blur.
    pub fn build(frame: Frame, s_range: (f32, f32), ds: f32, nt: usize, triangles: &[[Vec3; 3]], fallback: f32) -> Self {
        let ds = ds.max(1e-3);
        let ns = (((s_range.1 - s_range.0) / ds).ceil() as usize + 1).max(2);
        let nt = nt.max(8);
        let theta = |j: usize| -PI + (j as f32 + 0.5) * TAU / nt as f32;
        let mut r = vec![f32::NAN; ns * nt];
        for i in 0..ns {
            let s = s_range.0 + i as f32 * ds;
            let o = frame.ray_origin(s);
            for j in 0..nt {
                let d = frame.direction(s, theta(j));
                let hit = triangles
                    .iter()
                    .filter_map(|t| ray_triangle(o, d, t))
                    .fold(f32::NAN, f32::max);
                r[i * nt + j] = hit;
            }
        }
        let at = |i: isize, j: isize| (i.clamp(0, ns as isize - 1) as usize) * nt + j.rem_euclid(nt as isize) as usize;
        // Fill misses from their neighbours, ring by ring.
        for _ in 0..(ns + nt) {
            if !r.iter().any(|v| v.is_nan()) {
                break;
            }
            let prev = r.clone();
            for i in 0..ns as isize {
                for j in 0..nt as isize {
                    if !prev[at(i, j)].is_nan() {
                        continue;
                    }
                    let near: Vec<f32> = [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)]
                        .iter()
                        .map(|&(a, b)| prev[at(a, b)])
                        .filter(|v| !v.is_nan())
                        .collect();
                    if !near.is_empty() {
                        r[at(i, j)] = near.iter().sum::<f32>() / near.len() as f32;
                    }
                }
            }
        }
        for v in &mut r {
            if v.is_nan() {
                *v = fallback;
            }
        }
        // Dilate one bin (bulge over gaps between skin facets), then blur.
        let prev = r.clone();
        for i in 0..ns as isize {
            for j in 0..nt as isize {
                let mut m = prev[at(i, j)];
                for a in -1..=1 {
                    for b in -1..=1 {
                        m = m.max(prev[at(i + a, j + b)]);
                    }
                }
                r[at(i, j)] = m;
            }
        }
        for (passes, along_s) in [(2, true), (3, false)] {
            for _ in 0..passes {
                let prev = r.clone();
                for i in 0..ns as isize {
                    for j in 0..nt as isize {
                        let (a, b) = if along_s { (at(i - 1, j), at(i + 1, j)) } else { (at(i, j - 1), at(i, j + 1)) };
                        r[at(i, j)] = 0.25 * prev[a] + 0.5 * prev[at(i, j)] + 0.25 * prev[b];
                    }
                }
            }
        }
        Self {
            s0: s_range.0,
            ds,
            ns,
            nt,
            r,
        }
    }

    fn s_max(&self) -> f32 {
        self.s0 + (self.ns - 1) as f32 * self.ds
    }

    /// Profile radius at (s, θ), bilinear, s clamped to the profile, θ wrapped.
    pub fn radius(&self, s: f32, theta: f32) -> f32 {
        let f = ((s - self.s0) / self.ds).clamp(0.0, (self.ns - 1) as f32);
        let g = (wrap(theta) + PI) / TAU * self.nt as f32 - 0.5;
        let (i0, j0) = (f.floor() as usize, g.floor());
        let (fi, fj) = (f - i0 as f32, g - j0);
        let i1 = (i0 + 1).min(self.ns - 1);
        let j0i = (j0 as isize).rem_euclid(self.nt as isize) as usize;
        let j1i = (j0i + 1) % self.nt;
        let v = |i: usize, j: usize| self.r[i * self.nt + j];
        let a = v(i0, j0i) + (v(i0, j1i) - v(i0, j0i)) * fj;
        let b = v(i1, j0i) + (v(i1, j1i) - v(i1, j0i)) * fj;
        a + (b - a) * fi
    }

    /// Largest radius over a band of the profile (sampled).
    fn max_over(&self, s: (f32, f32), arc: (f32, f32)) -> f32 {
        let mut m = 0.0_f32;
        for i in 0..=16 {
            let si = s.0 + (s.1 - s.0) * i as f32 / 16.0;
            for j in 0..=32 {
                let t = arc.0 - arc.1 + 2.0 * arc.1 * j as f32 / 32.0;
                m = m.max(self.radius(si, t));
            }
        }
        m
    }
}

/// The plate's s-range at an angle.
type Span = Box<dyn Fn(f32) -> (f32, f32) + Send + Sync>;

/// One plate's shape (see the module docs).
struct Plate {
    frame: Frame,
    /// The body profile the plate follows, or a constant inner radius (a true sphere / tube).
    shape: Shape,
    gap: f32,
    thick: f32,
    span: Span,
    /// Angular extent (centre θ, half-width rad); `None` = a full ring.
    arc: Option<(f32, f32)>,
    /// Extra clearance at the low / high end of the span (m, over s units).
    flare_low: (f32, f32),
    flare_high: (f32, f32),
    /// Ridges (centre θ, half-width rad, height m; negative = groove).
    ridges: Vec<(f32, f32, f32)>,
    /// Domes (direction from a sphere frame's centre, half-width rad, height m).
    bumps: Vec<(Vec3, f32, f32)>,
    /// Rolled-edge tube radius (m).
    bead: f32,
    rivets: Vec<Vec3>,
}

enum Shape {
    Profile(Arc<Profile>),
    Radius(f32),
}

/// What `Plate::eval` knows about a point.
#[derive(Clone, Copy, Debug)]
struct Eval {
    density: f32,
    s: f32,
    theta: f32,
    rho: f32,
    /// Distance (m) to the nearest cut edge, positive on the plate.
    edge: f32,
    r_in: f32,
    r_out: f32,
}

impl Plate {
    fn new(frame: Frame, shape: Shape, gap: f32, span: Span) -> Self {
        Plate {
            frame,
            shape,
            gap,
            thick: THICK,
            span,
            arc: None,
            flare_low: (0.0, 1.0),
            flare_high: (0.0, 1.0),
            ridges: Vec::new(),
            bumps: Vec::new(),
            bead: 0.0045,
            rivets: Vec::new(),
        }
    }

    /// Inner and outer radius at (s, θ). The gap and thickness are taken along the surface
    /// normal: on a profile the radial offsets grow with the local slope.
    fn radii(&self, s: f32, theta: f32) -> (f32, f32) {
        let (lo, hi) = (self.span)(theta);
        let (mut r_in, slope) = match &self.shape {
            Shape::Radius(r) => (*r, 1.0),
            Shape::Profile(p) => {
                let sc = s.clamp(lo.max(p.s0), hi.min(p.s_max()).max(p.s0));
                let r = p.radius(sc, theta);
                let (hs, ht) = (0.5 * p.ds, 0.03);
                let drds = (p.radius(sc + hs, theta) - p.radius(sc - hs, theta)) / (2.0 * hs);
                let drdt = (p.radius(sc, theta + ht) - p.radius(sc, theta - ht)) / (2.0 * ht);
                let ks = self.frame.s_scale(r).max(1e-3);
                let kt = self.frame.theta_scale(sc, r).max(1e-3);
                let slope = (1.0 + (drds / ks).powi(2) + (drdt / kt).powi(2)).sqrt().min(3.0);
                (r + self.gap * slope, slope)
            }
        };
        if self.flare_low.0 > 0.0 {
            r_in += self.flare_low.0 * smoothstep01(1.0 - (s - lo) / self.flare_low.1.max(1e-4));
        }
        if self.flare_high.0 > 0.0 {
            r_in += self.flare_high.0 * smoothstep01(1.0 - (hi - s) / self.flare_high.1.max(1e-4));
        }
        let mut r_out = r_in + self.thick * slope;
        for &(c, w, h) in &self.ridges {
            r_out += h * (-(angle_dist(theta, c) / w).powi(2)).exp();
        }
        if !self.bumps.is_empty() {
            let dir = self.frame.direction(s, theta);
            for &(b, w, h) in &self.bumps {
                let a = dir.dot(b.normalize_or(Vec3::Y)).clamp(-1.0, 1.0).acos();
                r_out += h * (-(a / w).powi(2)).exp();
            }
        }
        (r_in, r_out)
    }

    fn eval(&self, p: Vec3) -> Eval {
        let (s, theta, rho) = self.frame.coords(p);
        let (lo, hi) = (self.span)(theta);
        let k = self.frame.s_scale(rho);
        let mut edge = ((s - lo) * k).min((hi - s) * k);
        if let Some((c, w)) = self.arc {
            edge = edge.min((w - angle_dist(theta, c)) * self.frame.theta_scale(s, rho));
        }
        let (r_in, r_out) = self.radii(s, theta);
        let slab = (rho - r_in).min(r_out - rho);
        let mut density = slab.min(edge);
        if self.bead > 0.0 {
            // A tube just inside the nearest edge, standing proud of the outer surface.
            let (de, dr) = (edge - self.bead, rho - (r_out - 0.35 * self.bead));
            density = density.max(self.bead - (de * de + dr * dr).sqrt());
        }
        for c in &self.rivets {
            density = density.max(RIVET_R - p.distance(*c));
        }
        Eval {
            density,
            s,
            theta,
            rho,
            edge,
            r_in,
            r_out,
        }
    }

    /// A rivet head on the outer surface at (s, θ).
    fn rivet_at(&self, s: f32, theta: f32) -> Vec3 {
        let (_, r_out) = self.radii(s, theta);
        self.frame.point(s, theta, r_out - 0.0015)
    }

    /// World bounds of the plate (with room for beads and rivets).
    fn bounds(&self) -> (Vec3, Vec3) {
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let (c, w) = self.arc.unwrap_or((0.0, PI));
        let sphere = matches!(self.frame, Frame::Sphere { .. });
        for j in 0..=48 {
            let theta = c - w + 2.0 * w * j as f32 / 48.0;
            let (lo, hi) = (self.span)(theta);
            let (lo, hi) = if sphere { (lo.max(0.0), hi.min(PI)) } else { (lo, hi) };
            for i in 0..=24 {
                let s = lo + (hi - lo) * i as f32 / 24.0;
                let (r_in, r_out) = self.radii(s, theta);
                for rho in [(r_in - 0.01).max(0.0), r_out + self.bead + 2.0 * RIVET_R + 0.01] {
                    let p = self.frame.point(s, theta, rho);
                    min = min.min(p);
                    max = max.max(p);
                }
            }
        }
        (min, max)
    }
}

/// Inner radii of nested spherical lames: each at least its natural radius (`natural[k]`, the
/// body under it plus clearance) and `clear` beyond the previous lame's inner radius, so every
/// lower lame sits over the one above whatever their relative rotation.
pub fn nest_radii(natural: &[f32], clear: f32) -> Vec<f32> {
    let mut out: Vec<f32> = Vec::with_capacity(natural.len());
    for &r in natural {
        let r = match out.last() {
            Some(&prev) => r.max(prev + clear),
            None => r,
        };
        out.push(r);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// The body and the set.

/// The character's bind-pose surface with skin weights (model space).
#[derive(Clone, Debug, Default)]
struct BodyMesh {
    positions: Vec<Vec3>,
    joints: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
    triangles: Vec<[u32; 3]>,
}

impl BodyMesh {
    fn load(path: &std::path::Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let gltf = gltf::Gltf::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let blob = gltf.blob.as_deref();
        let buffers = |buffer: gltf::Buffer| match buffer.source() {
            gltf::buffer::Source::Bin => blob,
            gltf::buffer::Source::Uri(_) => None,
        };
        let mesh = gltf.meshes().next().ok_or("no mesh")?;
        let mut body = BodyMesh::default();
        for primitive in mesh.primitives() {
            let reader = primitive.reader(buffers);
            let base = body.positions.len() as u32;
            let positions: Vec<Vec3> = reader.read_positions().ok_or("no positions")?.map(Vec3::from_array).collect();
            let n = positions.len();
            let joints: Vec<[u16; 4]> = reader.read_joints(0).map(|j| j.into_u16().collect()).unwrap_or_default();
            let weights: Vec<[f32; 4]> = reader.read_weights(0).map(|w| w.into_f32().collect()).unwrap_or_default();
            if joints.len() != n || weights.len() != n {
                return Err("primitive without skin weights".into());
            }
            let indices: Vec<u32> = reader
                .read_indices()
                .map(|i| i.into_u32().collect())
                .unwrap_or_else(|| (0..n as u32).collect());
            body.positions.extend(positions);
            body.joints.extend(joints);
            body.weights.extend(weights);
            body.triangles
                .extend(indices.chunks_exact(3).map(|t| [t[0] + base, t[1] + base, t[2] + base]));
        }
        if body.triangles.is_empty() {
            return Err("empty body mesh".into());
        }
        Ok(body)
    }

    /// Skin weight of vertex `v` on the bones in `set`.
    fn weight(&self, v: usize, set: &[usize]) -> f32 {
        self.joints[v]
            .iter()
            .zip(self.weights[v])
            .filter(|(j, _)| set.contains(&(**j as usize)))
            .map(|(_, w)| w)
            .sum()
    }

    /// Triangles with a vertex mostly on `set`.
    fn region(&self, set: &[usize]) -> Vec<[Vec3; 3]> {
        self.triangles
            .iter()
            .filter(|t| t.iter().any(|&v| self.weight(v as usize, set) >= 0.5))
            .map(|t| t.map(|v| self.positions[v as usize]))
            .collect()
    }
}

/// A skin point a plate is fitted to: bind position and up to four (bone, weight) pairs.
#[derive(Clone, Copy, Debug)]
struct Sample {
    rest: Vec3,
    joints: [u16; 4],
    weights: [f32; 4],
}

/// How a plate follows the body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fit {
    /// The least-squares rigid motion of the skin under it.
    Free,
    /// Only a rotation about this bone's joint (the plate stays centred on it).
    Pivot(usize),
}

/// Which body capsule a plate pads (`motion::body`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Head,
    Torso,
    UpperArm(usize),
    Forearm(usize),
    Calf(usize),
}

struct PlateDef {
    name: &'static str,
    plate: Plate,
    /// Bones whose skin the plate is fitted to.
    bones: Vec<usize>,
    fit: Fit,
    /// Bone the plate follows if it has too few samples, and the one jolted when it locks.
    anchor: usize,
    tint: Color,
    part: Part,
}

/// A plate ready to wear.
struct Built {
    name: &'static str,
    mesh: Mesh,
    samples: Vec<Sample>,
    centroid: Vec3,
    anchor: usize,
    fit: Fit,
}

struct BuiltSet {
    plates: Vec<Built>,
    padding: BodyPadding,
}

/// Rest (bind) landmarks.
struct Marks<'a> {
    rest: &'a [Transform],
    bones: &'a Bones,
    skeleton: &'a Skeleton,
}

impl Marks<'_> {
    fn at(&self, bone: usize) -> Vec3 {
        self.rest[bone].translation
    }
    fn named(&self, name: &str) -> Option<usize> {
        self.skeleton.find(name)
    }
}

/// The adventurer set, designed on the bind (T) pose: character faces +Z, its left is +X.
fn design(body: &BodyMesh, m: &Marks) -> Vec<PlateDef> {
    let b = m.bones;
    let mut defs = Vec::new();

    // Bascinet: a dome with a slight point toward the back, an arched face opening, cheek
    // guards to the jaw and down to the nape behind; a row of rivets along the lower edge.
    let neck = m.named("neck_01").unwrap_or(b.neck);
    let head_centre = m.at(b.head) + Vec3::new(0.0, 0.09, -0.005);
    let head_frame = Frame::sphere(head_centre, Vec3::Y, Vec3::Z);
    let head_profile = Arc::new(Profile::build(head_frame, (0.0, 2.3), 0.05, 72, &body.region(&[b.head, neck]), 0.1));
    let lower_edge = |theta: f32| {
        let a = theta.abs();
        let face = smoothstep01((a - 0.5) / 0.55);
        let back = smoothstep01((a - 1.7) / 1.1);
        let cheek = 1.05 + (1.95 - 1.05) * face;
        (-10.0, cheek + (2.08 - cheek) * back)
    };
    let mut helmet = Plate::new(head_frame, Shape::Profile(head_profile), 0.016, Box::new(lower_edge));
    helmet.bumps.push((Vec3::new(0.0, 1.0, -0.35), 0.45, 0.018));
    helmet.rivets = (0..11)
        .map(|k| wrap(1.25 + (TAU - 2.5) * k as f32 / 10.0))
        .map(|t| helmet.rivet_at(lower_edge(t).1 - 0.07, t))
        .collect();
    defs.push(PlateDef {
        name: "Bascinet",
        plate: helmet,
        bones: vec![b.head],
        fit: Fit::Free,
        anchor: b.head,
        tint: Color::srgb(0.72, 0.74, 0.78),
        part: Part::Head,
    });

    // Cuirass: the ribcage only, with real arm holes (the sides drop below the hanging arms'
    // armpits), a neck dip, a medial ridge and a flared rolled bottom the belly folds under.
    let clav = b.clavicle;
    let torso_bones = vec![b.pelvis, b.spine[0], b.spine[1], b.spine[2], clav[0], clav[1]];
    let torso_frame = Frame::cylinder(Vec3::new(0.0, 0.0, -0.008), Vec3::Y, Vec3::Z);
    let torso_profile = Arc::new(Profile::build(torso_frame, (0.98, 1.5), 0.01, 72, &body.region(&torso_bones), 0.14));
    let cuirass_top = |theta: f32| {
        let a = theta.abs();
        let front = 1.445 - 0.03 * (-(a / 0.35).powi(2)).exp();
        let back = 1.465 - 0.018 * (-((PI - a) / 0.35).powi(2)).exp();
        let side = 1.24;
        let to_front = 1.0 - smoothstep01((a - 0.75) / 0.55);
        let to_back = smoothstep01((a - 2.2) / 0.45);
        side + (front - side) * to_front + (back - side) * to_back
    };
    let mut cuirass = Plate::new(torso_frame, Shape::Profile(torso_profile), 0.026, Box::new(move |t| (1.18, cuirass_top(t))));
    cuirass.flare_low = (0.01, 0.03);
    cuirass.ridges.push((0.0, 0.13, 0.006));
    cuirass.rivets = [0.6f32, -0.6, PI - 0.5, -(PI - 0.5)]
        .iter()
        .map(|&t| cuirass.rivet_at(cuirass_top(t) - 0.022, t))
        .collect();
    defs.push(PlateDef {
        name: "Cuirass",
        plate: cuirass,
        bones: vec![b.spine[1], b.spine[2]],
        fit: Fit::Free,
        anchor: b.spine[2],
        tint: STEEL,
        part: Part::Torso,
    });

    for side in 0..2 {
        let names = if side == 0 {
            ["Pauldron L", "Pauldron L lame", "Pauldron L lower lame", "Vambrace L", "Greave L"]
        } else {
            ["Pauldron R", "Pauldron R lame", "Pauldron R lower lame", "Vambrace R", "Greave R"]
        };
        // Pauldron: three spherical bands around the shoulder joint (pole down the arm, θ = 0
        // up), from the cap over the shoulder to the lower lame on the upper arm, top half only.
        let shoulder = m.at(b.upperarm[side]);
        let elbow = m.at(b.lowerarm[side]);
        let cap_frame = Frame::sphere(shoulder, elbow - shoulder, Vec3::Y);
        let shoulder_bones = [clav[side], b.upperarm[side], b.spine[2]];
        let cap_profile = Profile::build(cap_frame, (0.05, 2.2), 0.04, 72, &body.region(&shoulder_bones), 0.08);
        // (polar band, half arc, clearance to the body)
        let bands = [((1.0, 1.95), 1.75, 0.022), ((0.62, 1.07), 1.65, 0.016), ((0.36, 0.68), 1.55, 0.012)];
        let natural: Vec<f32> = bands
            .iter()
            .map(|&(s, half, gap)| cap_profile.max_over(s, (0.0, half)) + gap)
            .collect();
        let radii = nest_radii(&natural, THICK + 0.0045 + 0.005);
        for (k, ((s, half, _), radius)) in bands.into_iter().zip(radii).enumerate() {
            let mut lame = Plate::new(cap_frame, Shape::Radius(radius), 0.0, Box::new(move |_| s));
            lame.arc = Some((0.0, half));
            lame.bead = 0.004;
            let mid = 0.5 * (s.0 + s.1);
            lame.rivets = [half - 0.14, -(half - 0.14)].iter().map(|&t| lame.rivet_at(mid, t)).collect();
            let bones = if k == 0 {
                vec![clav[side], b.upperarm[side]]
            } else {
                vec![b.upperarm[side]]
            };
            defs.push(PlateDef {
                name: names[k],
                plate: lame,
                bones,
                fit: Fit::Pivot(b.upperarm[side]),
                anchor: b.upperarm[side],
                tint: if k == 0 { STEEL } else { Color::srgb(0.62, 0.64, 0.68) },
                part: Part::UpperArm(side),
            });
        }
        // Vambrace: a closed tube from below the elbow to above the wrist, flared at the wrist,
        // with a groove along the inner seam.
        let wrist = m.at(b.hand[side]);
        let fore_frame = Frame::cylinder(elbow, wrist - elbow, Vec3::Y);
        let fore_profile = Arc::new(Profile::build(fore_frame, (0.0, 0.28), 0.008, 72, &body.region(&[b.lowerarm[side]]), 0.04));
        let mut vambrace = Plate::new(fore_frame, Shape::Profile(fore_profile), 0.012, Box::new(|_| (0.05, 0.235)));
        vambrace.flare_high = (0.006, 0.035);
        vambrace.bead = 0.004;
        vambrace.ridges.push((PI, 0.07, -0.0025));
        vambrace.rivets = [0.6f32, -0.6].iter().map(|&t| vambrace.rivet_at(0.07, t)).collect();
        defs.push(PlateDef {
            name: names[3],
            plate: vambrace,
            bones: vec![b.lowerarm[side]],
            fit: Fit::Free,
            anchor: b.lowerarm[side],
            tint: STEEL,
            part: Part::Forearm(side),
        });
        // Greave: a closed tube down the shin with a keel at the front and a flare over the
        // ankle, cut lower at the back so a bent knee clears the top edge.
        let knee = m.at(b.calf[side]);
        let ankle = m.at(b.foot[side]);
        let shin_frame = Frame::cylinder(knee, ankle - knee, Vec3::Z);
        let shin_profile = Arc::new(Profile::build(shin_frame, (0.0, 0.42), 0.01, 72, &body.region(&[b.calf[side]]), 0.05));
        let shin_span = |t: f32| (0.055 + 0.05 * smoothstep01((t.abs() - 1.2) / 1.2), 0.36);
        let mut greave = Plate::new(shin_frame, Shape::Profile(shin_profile), 0.013, Box::new(shin_span));
        greave.flare_high = (0.008, 0.04);
        greave.ridges.push((0.0, 0.25, 0.006));
        greave.rivets = [(0.15, PI - 0.25), (0.15, -(PI - 0.25)), (0.3, PI - 0.25), (0.3, -(PI - 0.25))]
            .iter()
            .map(|&(s, t)| greave.rivet_at(s, t))
            .collect();
        defs.push(PlateDef {
            name: names[4],
            plate: greave,
            bones: vec![b.calf[side]],
            fit: Fit::Free,
            anchor: b.calf[side],
            tint: STEEL,
            part: Part::Calf(side),
        });
    }
    defs
}

/// Skin points under a plate (on its bones, inside its footprint), spread over the plate.
fn samples_for(def: &PlateDef, body: &BodyMesh) -> Vec<Sample> {
    let under = skin_under(def, body);
    let step = (under.len() as f32 / FIT_SAMPLES as f32).max(1.0);
    (0..FIT_SAMPLES)
        .map(|k| (k as f32 * step) as usize)
        .filter(|&k| k < under.len())
        .map(|k| {
            let v = under[k];
            Sample {
                rest: body.positions[v],
                joints: body.joints[v],
                weights: body.weights[v],
            }
        })
        .collect()
}

/// Body vertices under a plate (weighted to its bones, inside its span), sorted.
fn skin_under(def: &PlateDef, body: &BodyMesh) -> Vec<usize> {
    let plate = &def.plate;
    let frame = &plate.frame;
    let mut under: Vec<usize> = (0..body.positions.len())
        .filter(|&v| body.weight(v, &def.bones) >= 0.4)
        .filter(|&v| {
            let (s, theta, rho) = frame.coords(body.positions[v]);
            let (lo, hi) = (plate.span)(theta);
            let margin = 0.01 / frame.s_scale(rho.max(0.05));
            let in_arc = plate.arc.is_none_or(|(c, w)| angle_dist(theta, c) <= w + 0.1);
            s >= lo - margin && s <= hi + margin && in_arc
        })
        .collect();
    under.sort_unstable();
    under
}

/// Mesh a plate: Surface Nets, shared smooth normals, UVs from the frame, vertex colours (tint,
/// polished edges, darker inside), tangents.
fn mesh_plate(def: &PlateDef) -> Mesh {
    let plate = &def.plate;
    let (min, max) = plate.bounds();
    let lattice = Lattice::sample(min, max, CELL, |p| plate.eval(p).density);
    let nets = surface_nets(&lattice);
    if nets.indices.is_empty() {
        return Mesh::from(Cuboid::new(0.01, 0.01, 0.01));
    }
    let positions: Vec<[f32; 3]> = nets.positions.iter().map(|p| p.to_array()).collect();
    // Smooth normals on the unsplit mesh, so the UV seam leaves no crease.
    let mut shaded = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(nets.indices.clone()));
    shaded.compute_smooth_normals();
    let normals: Vec<[f32; 3]> = match shaded.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(n)) => n.clone(),
        _ => vec![[0.0, 1.0, 0.0]; nets.positions.len()],
    };
    let evals: Vec<Eval> = nets.positions.iter().map(|p| plate.eval(*p)).collect();
    let mut thetas: Vec<f32> = evals.iter().map(|e| e.theta).collect();
    let mut source: Vec<usize> = (0..nets.positions.len()).collect();
    let mut indices = nets.indices.clone();
    let tube = matches!(plate.frame, Frame::Cylinder { .. });
    if tube {
        // Split the θ seam: a triangle straddling ±π gets copies of its negative-θ vertices at +2π.
        let mut copies: HashMap<u32, u32> = HashMap::new();
        for tri in indices.chunks_exact_mut(3) {
            let t = [tri[0], tri[1], tri[2]].map(|i| thetas[i as usize]);
            let spread = t.iter().cloned().fold(f32::MIN, f32::max) - t.iter().cloned().fold(f32::MAX, f32::min);
            if spread <= PI {
                continue;
            }
            for i in tri.iter_mut() {
                if thetas[*i as usize] < 0.0 {
                    let original = *i;
                    *i = *copies.entry(original).or_insert_with(|| {
                        thetas.push(thetas[original as usize] + TAU);
                        source.push(original as usize);
                        (thetas.len() - 1) as u32
                    });
                }
            }
        }
    }
    let tint = def.tint.to_linear();
    let mut out_pos = Vec::with_capacity(source.len());
    let mut out_nor = Vec::with_capacity(source.len());
    let mut uvs = Vec::with_capacity(source.len());
    let mut colors = Vec::with_capacity(source.len());
    for (k, &v) in source.iter().enumerate() {
        let e = &evals[v];
        let theta = thetas[k];
        let uv = if tube {
            [theta * e.rho / TEX, e.s / TEX]
        } else if plate.arc.is_some_and(|(_, w)| w < PI) {
            // A band off the pole that never reaches ±π: plain (θ, s) keeps the steel unstretched.
            [theta * e.rho * e.s.sin() / TEX, e.s * e.rho / TEX]
        } else {
            // Azimuthal about the pole: no singularity anywhere on the plate.
            let arc = e.s * e.rho / TEX;
            [arc * theta.cos(), arc * theta.sin()]
        };
        let polish = 1.0 + 0.22 * smoothstep01(1.0 - e.edge / 0.009);
        let inside = if e.rho < 0.5 * (e.r_in + e.r_out) && e.edge > 0.004 { 0.5 } else { 1.0 };
        let k = polish * inside;
        out_pos.push(nets.positions[v].to_array());
        out_nor.push(normals[v]);
        uvs.push(uv);
        colors.push([tint.red * k, tint.green * k, tint.blue * k, 1.0]);
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, out_pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, out_nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices));
    if let Err(e) = mesh.generate_tangents() {
        warn!("[armor] {}: no tangents ({e})", def.name);
    }
    mesh
}

/// Closest distance from `p` to segment `a–b` and the segment parameter there.
fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> (f32, f32) {
    let ab = b - a;
    let t = if ab.length_squared() > 1e-10 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    (p.distance(a + ab * t), t)
}

/// `BodyPadding` that keeps limbs clear of the built plates: for each `motion::body` capsule,
/// how far its plates stand off the body (rest pose), plus a margin. The body there is the
/// wider of the capsule and the skin under the plate: where the skin reaches past the capsule
/// (chest sides, shoulders) the bare body already lets the arms hang there, so only the plate's
/// standoff from the skin counts. Limb capsules only count points along their middle (not the
/// ends, where joints bend).
fn derive_padding(plates: &[(Part, &Mesh, &[Vec3])], rest: &[Transform], b: &Bones) -> BodyPadding {
    let at = |i: usize| rest[i].translation;
    let crown = at(b.head) + (at(b.head) - at(b.neck)).normalize_or(Vec3::Y) * HEAD_LIFT;
    let mut pad = BodyPadding::default();
    for (part, mesh, skin) in plates {
        let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|p| p.as_float3()) else {
            continue;
        };
        let reach = |points: &mut dyn Iterator<Item = Vec3>, a: Vec3, bb: Vec3, interior: bool| {
            points
                .map(|p| segment_distance(p, a, bb))
                .filter(|(_, t)| !interior || (0.05..=0.95).contains(t))
                .map(|(d, _)| d)
                .fold(0.0_f32, f32::max)
        };
        let standoff = |a: Vec3, bb: Vec3, interior: bool, capsule: f32| {
            let plate = reach(&mut positions.iter().map(|p| Vec3::from_array(*p)), a, bb, interior);
            let body = reach(&mut skin.iter().copied(), a, bb, interior).max(capsule);
            plate - body + PAD_MARGIN
        };
        match *part {
            Part::Head => pad.head = pad.head.max(standoff(crown, crown, false, HEAD_R)),
            Part::Torso => pad.torso = pad.torso.max(standoff(at(b.pelvis), at(b.spine[2]), false, TORSO_R)),
            Part::UpperArm(s) => {
                let (shoulder, elbow) = (at(b.upperarm[s]), at(b.lowerarm[s]));
                pad.upper_arm = pad.upper_arm.max(standoff(shoulder.lerp(elbow, 0.5), elbow, true, ARM_R));
            }
            Part::Forearm(s) => pad.forearm = pad.forearm.max(standoff(at(b.lowerarm[s]), at(b.hand[s]), true, ARM_R)),
            Part::Calf(s) => pad.calf = pad.calf.max(standoff(at(b.calf[s]), at(b.foot[s]), true, CALF_R)),
        }
    }
    for v in [&mut pad.torso, &mut pad.head, &mut pad.upper_arm, &mut pad.forearm, &mut pad.calf] {
        *v = v.clamp(0.0, 0.2);
    }
    pad
}

/// The set's `BodyPadding`. Pivot plates (the pauldron lames: spheres about the shoulder joint)
/// don't count: turning the arm turns them in place, so they never close on the body.
fn set_padding(defs: &[PlateDef], meshes: &[Mesh], body: &BodyMesh, rest: &[Transform], b: &Bones) -> BodyPadding {
    let skins: Vec<Vec<Vec3>> = defs.iter().map(|d| skin_under(d, body).into_iter().map(|v| body.positions[v]).collect()).collect();
    let plates: Vec<_> = defs
        .iter()
        .zip(meshes)
        .zip(&skins)
        .filter(|((d, _), _)| !matches!(d.fit, Fit::Pivot(_)))
        .map(|((d, m), s)| (d.part, m, s.as_slice()))
        .collect();
    derive_padding(&plates, rest, b)
}

/// Build the whole set (runs on the async compute pool; plates mesh in parallel).
fn build_set(skeleton: Skeleton, bones: Bones) -> Result<BuiltSet, String> {
    let path = std::path::Path::new(crate::ASSETS).join(crate::motion::MODEL_GLB);
    let body = BodyMesh::load(&path)?;
    let rest = skeleton.fk(&skeleton.rest_pose(), bones.pelvis);
    let marks = Marks {
        rest: &rest,
        bones: &bones,
        skeleton: &skeleton,
    };
    let defs = design(&body, &marks);
    let meshes: Vec<Mesh> = std::thread::scope(|scope| {
        let jobs: Vec<_> = defs.iter().map(|def| scope.spawn(move || mesh_plate(def))).collect();
        jobs.into_iter()
            .map(|job| job.join().unwrap_or_else(|_| Mesh::from(Cuboid::new(0.01, 0.01, 0.01))))
            .collect()
    });
    let padding = set_padding(&defs, &meshes, &body, &rest, &bones);
    let plates = defs
        .iter()
        .zip(meshes)
        .map(|(def, mesh)| {
            let samples = samples_for(def, &body);
            let centroid = if samples.is_empty() {
                Vec3::ZERO
            } else {
                samples.iter().map(|s| s.rest).sum::<Vec3>() / samples.len() as f32
            };
            Built {
                name: def.name,
                mesh,
                samples,
                centroid,
                anchor: def.anchor,
                fit: def.fit,
            }
        })
        .collect();
    Ok(BuiltSet { plates, padding })
}

// ---------------------------------------------------------------------------------------------
// Fitting.

/// Rotation of `a` (Müller, Bender, Chentanez, Macklin 2016: "A Robust Method to Extract the
/// Rotational Part of Deformations"), starting from `q`.
pub fn extract_rotation(a: Mat3, mut q: Quat, iterations: usize) -> Quat {
    for _ in 0..iterations {
        let r = Mat3::from_quat(q);
        let omega = r.x_axis.cross(a.x_axis) + r.y_axis.cross(a.y_axis) + r.z_axis.cross(a.z_axis);
        let denom = (r.x_axis.dot(a.x_axis) + r.y_axis.dot(a.y_axis) + r.z_axis.dot(a.z_axis)).abs() + 1e-9;
        let omega = omega / denom;
        let w = omega.length();
        if !w.is_finite() || w < 1e-9 {
            break;
        }
        q = (Quat::from_axis_angle(omega / w, w) * q).normalize();
        if w < 1e-6 {
            break;
        }
    }
    q
}

/// Least-squares rigid motion carrying `rest` points onto `now`, with rest centre `pc` going to
/// `qc` (the centroids for a free fit; a joint's rest and current positions for a pivot fit).
pub fn rigid_fit(rest: &[Vec3], pc: Vec3, now: &[Vec3], qc: Vec3, warm: Quat) -> Option<Transform> {
    if rest.len() != now.len() || rest.len() < 3 {
        return None;
    }
    let mut a = Mat3::ZERO;
    for (p, q) in rest.iter().zip(now) {
        let (p, q) = (*p - pc, *q - qc);
        a += Mat3::from_cols(q * p.x, q * p.y, q * p.z);
    }
    let rotation = extract_rotation(a, warm.normalize(), FIT_ITERATIONS);
    let translation = qc - rotation * pc;
    (rotation.is_finite() && translation.is_finite()).then_some(Transform {
        translation,
        rotation,
        scale: Vec3::ONE,
    })
}

// ---------------------------------------------------------------------------------------------
// Mip chains.

/// Which kind of data a texture holds, for averaging its mip levels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Texel {
    Srgb,
    Linear,
    Normal,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// Append a full box-filtered mip chain to an RGBA8 image (no-op for other formats or if it
/// already has mips).
pub(crate) fn add_mips(image: &mut Image, texel: Texel) {
    let format = image.texture_descriptor.format;
    if !matches!(format, TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb)
        || image.texture_descriptor.mip_level_count > 1
    {
        return;
    }
    let (w, h) = (image.width() as usize, image.height() as usize);
    let Some(base) = image.data.as_ref().filter(|d| d.len() == w * h * 4) else { return };
    let decode = |v: u8, ch: usize| {
        let x = v as f32 / 255.0;
        match texel {
            Texel::Srgb if ch < 3 => srgb_to_linear(x),
            Texel::Normal if ch < 3 => 2.0 * x - 1.0,
            _ => x,
        }
    };
    let encode = |x: f32, ch: usize| {
        let y = match texel {
            Texel::Srgb if ch < 3 => linear_to_srgb(x),
            Texel::Normal if ch < 3 => 0.5 * x + 0.5,
            _ => x,
        };
        (y.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
    };
    let mut levels: Vec<Vec<u8>> = vec![base.clone()];
    let (mut lw, mut lh) = (w, h);
    while lw > 1 || lh > 1 {
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let prev = levels.last().map(Vec::as_slice).unwrap_or(&[]);
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                let mut acc = [0.0f32; 4];
                let mut n = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(lw - 1), (2 * y + dy).min(lh - 1));
                    let i = (sy * lw + sx) * 4;
                    for (ch, a) in acc.iter_mut().enumerate() {
                        *a += decode(prev[i + ch], ch);
                    }
                    n += 1.0;
                }
                for a in &mut acc {
                    *a /= n;
                }
                if texel == Texel::Normal {
                    let v = Vec3::new(acc[0], acc[1], acc[2]).normalize_or(Vec3::Z);
                    acc[0] = v.x;
                    acc[1] = v.y;
                    acc[2] = v.z;
                }
                let o = (y * nw + x) * 4;
                for (ch, a) in acc.iter().enumerate() {
                    next[o + ch] = encode(*a, ch);
                }
            }
        }
        levels.push(next);
        lw = nw;
        lh = nh;
    }
    image.texture_descriptor.mip_level_count = levels.len() as u32;
    image.data = Some(levels.concat());
}

// ---------------------------------------------------------------------------------------------
// ECS.

#[derive(Resource)]
struct ArmorBuild(Task<Result<BuiltSet, String>>);

/// The built meshes waiting for their textures (mipped before the material sees them).
#[derive(Resource)]
struct PendingMaterial {
    textures: [Handle<Image>; 3],
    set: Option<BuiltSet>,
}

struct PlateAsset {
    name: &'static str,
    mesh: Handle<Mesh>,
    samples: Vec<Sample>,
    rest: Vec<Vec3>,
    centroid: Vec3,
    anchor: usize,
    fit: Fit,
}

#[derive(Resource)]
pub struct ArmorAssets {
    plates: Vec<PlateAsset>,
    material: Handle<StandardMaterial>,
    /// Inverse bind (rest model-space) transform and rest joint position per bone.
    rest_inverse: Vec<Mat4>,
    rest_joint: Vec<Vec3>,
    /// What a wearer gets so limbs clear the plates.
    pub padding: BodyPadding,
}

/// The plates someone wears (stand or character root).
#[derive(Component, Clone, Debug, Default)]
pub struct ArmorSet {
    pub plates: Vec<Entity>,
}

/// Marker on armour stands.
#[derive(Component)]
pub struct ArmorStand;

/// One plate entity (a child of its wearer's model).
#[derive(Component, Clone, Copy, Debug)]
pub struct ArmorPlate {
    index: usize,
    wearer: Entity,
    /// Last fitted rotation (warm start).
    rotation: Quat,
    /// In flight to `wearer`: start transform under the new wearer's model and progress 0..1.
    flight: Option<(Transform, f32)>,
    /// Waiting its turn: the new wearer and seconds until it leaves (it stays on its current
    /// wearer until then).
    pending: Option<(Entity, f32)>,
}

fn start_build(mut commands: Commands, bank: Option<Res<crate::motion::bank::PoseBank>>) {
    let Some(bank) = bank else {
        warn!("[armor] no pose bank; no armour");
        return;
    };
    let (skeleton, bones) = (bank.skeleton.clone(), bank.bones);
    let task = AsyncComputeTaskPool::get().spawn(async move { build_set(skeleton, bones) });
    commands.insert_resource(ArmorBuild(task));
}

fn repeat_linear(settings: &mut ImageLoaderSettings, srgb: bool) {
    settings.is_srgb = srgb;
    settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        anisotropy_clamp: 8,
        ..ImageSamplerDescriptor::linear()
    });
}

/// The set is built: start loading the steel maps.
fn finish_build(mut commands: Commands, build: Option<ResMut<ArmorBuild>>, assets: Res<AssetServer>) {
    let Some(mut build) = build else { return };
    let Some(result) = block_on(poll_once(&mut build.0)) else { return };
    commands.remove_resource::<ArmorBuild>();
    let set = match result {
        Ok(set) => set,
        Err(e) => {
            error!("[armor] could not build the set: {e}");
            return;
        }
    };
    let srgb = [true, false, false];
    let textures = std::array::from_fn(|i| {
        let is_srgb = srgb[i];
        assets
            .load_builder()
            .with_settings(move |s: &mut ImageLoaderSettings| repeat_linear(s, is_srgb))
            .load(TEXTURES[i])
    });
    commands.insert_resource(PendingMaterial { textures, set: Some(set) });
}

/// Once the steel maps are loaded: give them mip chains, make the material, publish the set.
fn make_material(
    mut commands: Commands,
    pending: Option<ResMut<PendingMaterial>>,
    bank: Option<Res<crate::motion::bank::PoseBank>>,
    server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(mut pending), Some(bank)) = (pending, bank) else { return };
    let failed = pending
        .textures
        .iter()
        .any(|t| matches!(server.load_state(t), bevy::asset::LoadState::Failed(_)));
    if !failed && !pending.textures.iter().all(|t| server.is_loaded_with_dependencies(t)) {
        return;
    }
    let Some(set) = pending.set.take() else { return };
    let textures = pending.textures.clone();
    commands.remove_resource::<PendingMaterial>();
    for (handle, texel) in textures.iter().zip([Texel::Srgb, Texel::Normal, Texel::Linear]) {
        if let Some(mut image) = images.get_mut(handle) {
            add_mips(&mut image, texel);
        }
    }
    let [color, normal, mr] = textures;
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: (!failed).then_some(color),
        normal_map_texture: (!failed).then_some(normal),
        metallic_roughness_texture: (!failed).then_some(mr),
        metallic: 0.7,
        perceptual_roughness: if failed { 0.45 } else { 0.9 },
        ..default()
    });
    if failed {
        warn!("[armor] steel textures failed to load; plain steel");
    }
    let rest = bank.skeleton.fk(&bank.skeleton.rest_pose(), bank.bones.pelvis);
    let vertices: usize = set.plates.iter().map(|b| b.mesh.count_vertices()).sum();
    info!("[armor] built {} plates ({vertices} vertices), padding {:?}", set.plates.len(), set.padding);
    let plates = set
        .plates
        .into_iter()
        .map(|b| PlateAsset {
            name: b.name,
            mesh: meshes.add(b.mesh),
            rest: b.samples.iter().map(|s| s.rest).collect(),
            samples: b.samples,
            centroid: b.centroid,
            anchor: b.anchor,
            fit: b.fit,
        })
        .collect();
    commands.insert_resource(ArmorAssets {
        plates,
        material,
        rest_inverse: rest.iter().map(|t| t.to_matrix().inverse()).collect(),
        rest_joint: rest.iter().map(|t| t.translation).collect(),
        padding: set.padding,
    });
}

/// Spawn an armour stand (feet at `at`, facing `yaw`) that will wear the set.
pub fn spawn_stand(commands: &mut Commands, meshes: &mut Assets<Mesh>, wood: &Handle<StandardMaterial>, at: Vec3, yaw: f32) -> Entity {
    let base_height = 0.06;
    let stand = commands
        .spawn((
            Name::new("Armor stand"),
            ArmorStand,
            ArmorSet::default(),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            RigidBody::Static,
            Interactable {
                kind: "armor_stand".into(),
                prompt: "Put on armor".into(),
                track: "ual1/Interact".into(),
                duration: 1.2,
                looping: false,
                // In front of the mannequin (it faces its local +Z), facing it.
                stand: Vec3::new(0.0, 0.0, 0.95),
                face_yaw: PI,
                hands: [None, None],
                grip: None,
                range: 1.8,
                requires: None,
                beat: None,
                reach: None,
                radial: None,
            },
            ModelRequest {
                path: crate::motion::MODEL_GLB.to_string(),
                offset: Vec3::Y * base_height,
                tint: Some(Color::srgb(0.32, 0.23, 0.15)),
            },
        ))
        .id();
    commands.spawn((
        Name::new("Armor stand base"),
        Mesh3d(meshes.add(Cylinder::new(0.34, base_height))),
        MeshMaterial3d(wood.clone()),
        Transform::from_xyz(0.0, base_height * 0.5, 0.0),
        ChildOf(stand),
    ));
    commands.spawn((
        Name::new("Armor stand collider"),
        Collider::cylinder(0.3, 1.8),
        CollisionLayers::new(Phys::World, LayerMask::ALL),
        Transform::from_xyz(0.0, 0.9, 0.0),
        ChildOf(stand),
    ));
    stand
}

/// Once the set is built and a stand's model is bound, hang the plates on it (only on a stand
/// that has not lent them out: `PropState.on` = someone is wearing them).
fn dress_stands(
    mut commands: Commands,
    assets: Option<Res<ArmorAssets>>,
    mut stands: Query<(Entity, &Rig, &mut ArmorSet, &PropState), With<ArmorStand>>,
) {
    let Some(assets) = assets else { return };
    for (stand, rig, mut set, prop) in &mut stands {
        if !set.plates.is_empty() || prop.on {
            continue;
        }
        set.plates = assets
            .plates
            .iter()
            .enumerate()
            .map(|(index, p)| {
                commands
                    .spawn((
                        Name::new(p.name),
                        ArmorPlate {
                            index,
                            wearer: stand,
                            rotation: Quat::IDENTITY,
                            flight: None,
                            pending: None,
                        },
                        Mesh3d(p.mesh.clone()),
                        MeshMaterial3d(assets.material.clone()),
                        Transform::IDENTITY,
                        ChildOf(rig.model),
                    ))
                    .id()
            })
            .collect();
        commands.entity(stand).try_insert(assets.padding);
    }
}

/// A stand offers nothing while it holds no plates and nobody wears them (the set still
/// building, or the build failed).
fn stand_availability(mut stands: Query<(&ArmorSet, &mut PropState), With<ArmorStand>>) {
    for (set, mut prop) in &mut stands {
        let disabled = set.plates.is_empty() && !prop.on;
        if prop.disabled != disabled {
            prop.disabled = disabled;
        }
    }
}

/// Stands hold their idle pose (they have no `Mover`, so `select` leaves them alone).
fn pose_stands(mut stands: Query<&mut MotionLayers, With<ArmorStand>>) {
    for mut layers in &mut stands {
        layers.layers = vec![Layer {
            source: Source::Track {
                name: STAND_CLIP.to_string(),
                time: STAND_POSE_T,
                looping: true,
            },
            weight: 1.0,
            mask: Mask::Full,
            style: None,
        }];
        // It stands on its base; its own collider would only confuse the foot probes.
        layers.contact = ContactRequest {
            foot_ik: 0.0,
            ..default()
        };
    }
}

/// Local transform under `parent` that keeps `item` where it is in the world.
fn kept(item: Option<&GlobalTransform>, parent: Option<&GlobalTransform>) -> Transform {
    let (Some(item), Some(parent)) = (item, parent) else { return Transform::IDENTITY };
    let t = Transform::from_matrix((parent.affine().inverse() * item.affine()).into());
    if t.translation.is_finite() && t.rotation.is_finite() {
        Transform { scale: Vec3::ONE, ..t }
    } else {
        Transform::IDENTITY
    }
}

/// E at the stand: queue the plates to leave the stand for the user one after another, or to go
/// back. The set and the padding change hands at once; each plate leaves on its turn (`launch`).
#[allow(clippy::type_complexity)]
fn equip(
    mut commands: Commands,
    assets: Option<Res<ArmorAssets>>,
    mut effects: MessageReader<UseEffect>,
    mut stands: Query<(&Interactable, &mut PropState, &mut ArmorSet), With<ArmorStand>>,
    mut wearers: Query<Option<&mut ArmorSet>, Without<ArmorStand>>,
    mut plates: Query<&mut ArmorPlate>,
) {
    let Some(assets) = assets else {
        effects.clear();
        return;
    };
    for effect in effects.read() {
        let Ok((it, mut prop, mut stand_set)) = stands.get_mut(effect.target) else {
            continue;
        };
        if it.kind != "armor_stand" {
            continue;
        }
        let Ok(user_set) = wearers.get_mut(effect.user) else {
            continue;
        };
        let (moving, to) = if !prop.on {
            (std::mem::take(&mut stand_set.plates), effect.user)
        } else {
            let Some(mut user_set) = user_set else { continue };
            (std::mem::take(&mut user_set.plates), effect.target)
        };
        if moving.is_empty() {
            continue;
        }
        for (k, &entity) in moving.iter().enumerate() {
            if let Ok(mut plate) = plates.get_mut(entity) {
                plate.pending = Some((to, k as f32 * FLY_STAGGER));
            }
        }
        if !prop.on {
            commands.entity(effect.user).try_insert((ArmorSet { plates: moving }, assets.padding));
            commands.entity(effect.target).try_remove::<BodyPadding>();
            prop.on = true;
        } else {
            stand_set.plates = moving;
            commands.entity(effect.user).try_remove::<BodyPadding>();
            commands.entity(effect.target).try_insert(assets.padding);
            prop.on = false;
        }
    }
}

/// Plates whose turn has come leave their wearer: re-parented to the new wearer's model where
/// they are in the world, then flown into place by `fit_plates`.
fn launch(
    mut commands: Commands,
    time: Res<Time>,
    rigs: Query<&Rig>,
    globals: Query<&GlobalTransform>,
    mut plates: Query<(Entity, &mut ArmorPlate)>,
) {
    let dt = time.delta_secs().clamp(0.0, 0.1);
    for (entity, mut plate) in &mut plates {
        let Some((to, delay)) = plate.pending else { continue };
        if delay > 0.0 {
            plate.pending = Some((to, delay - dt));
            continue;
        }
        plate.pending = None;
        let Ok(rig) = rigs.get(to) else { continue };
        let from = kept(globals.get(entity).ok(), globals.get(rig.model).ok());
        plate.wearer = to;
        plate.flight = Some((from, 0.0));
        commands.entity(entity).insert((ChildOf(rig.model), from));
    }
}

/// Place every plate: the rigid fit to the skin under it, eased in from its flight.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn fit_plates(
    time: Res<Time>,
    assets: Option<Res<ArmorAssets>>,
    bank: Option<Res<crate::motion::bank::PoseBank>>,
    mut wearers: Query<(&Rig, &RigPose, Option<&mut MotionLayers>, Has<Player>), Without<ArmorPlate>>,
    models: Query<&GlobalTransform>,
    mut plates: Query<(&mut ArmorPlate, &mut Transform)>,
    mut sounds: MessageWriter<Sfx>,
    mut shake: ResMut<CameraShake>,
) {
    let (Some(assets), Some(bank)) = (assets, bank) else { return };
    let dt = time.delta_secs().clamp(0.0, 0.1);
    let n = bank.skeleton.len();
    for (mut plate, mut tf) in &mut plates {
        let Some(asset) = assets.plates.get(plate.index) else { continue };
        let Ok((rig, pose, layers, player)) = wearers.get_mut(plate.wearer) else { continue };
        if pose.0.len() != n || assets.rest_inverse.len() != n {
            continue;
        }
        let skin = |j: usize| pose.0[j].to_matrix() * assets.rest_inverse[j];
        let fitted = if asset.samples.len() >= 3 {
            let now: Vec<Vec3> = asset
                .samples
                .iter()
                .map(|s| {
                    s.joints
                        .iter()
                        .zip(s.weights)
                        .filter(|(j, w)| *w > 0.0 && (**j as usize) < n)
                        .map(|(j, w)| skin(*j as usize).transform_point3(s.rest) * w)
                        .sum::<Vec3>()
                })
                .collect();
            let (pc, qc) = match asset.fit {
                Fit::Free => (asset.centroid, now.iter().sum::<Vec3>() / now.len() as f32),
                Fit::Pivot(joint) => (assets.rest_joint[joint.min(n - 1)], pose.0[joint.min(n - 1)].translation),
            };
            rigid_fit(&asset.rest, pc, &now, qc, plate.rotation)
        } else {
            None
        };
        let target = fitted
            .or_else(|| Some(Transform::from_matrix(skin(asset.anchor.min(n - 1)))))
            .filter(|t| t.is_finite());
        let Some(target) = target else { continue };
        plate.rotation = target.rotation;
        *tf = match plate.flight {
            None => target,
            Some((from, t)) => {
                let t = t + dt / FLY_TIME;
                if t >= 1.0 {
                    plate.flight = None;
                    let at = models
                        .get(rig.model)
                        .map_or(target.translation, |m| m.transform_point(target.translation));
                    sounds.write(Sfx {
                        kind: SfxKind::Clank,
                        at,
                        volume: 0.55,
                    });
                    if let Some(mut layers) = layers {
                        let left = models.get(rig.model).map_or(Vec3::X, |m| m.rotation() * Vec3::X);
                        layers.impulses.push((asset.anchor, -left * JOLT));
                    }
                    if player {
                        shake.add(LOCK_SHAKE);
                    }
                    target
                } else {
                    plate.flight = Some((from, t));
                    let u = smoothstep01(t);
                    Transform {
                        translation: from.translation.lerp(target.translation, u) + Vec3::Y * (FLY_ARC * (PI * u).sin()),
                        rotation: from.rotation.slerp(target.rotation, u),
                        scale: Vec3::ONE,
                    }
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let frames = [
            Frame::cylinder(Vec3::new(0.1, 0.2, 0.3), Vec3::new(1.0, 0.2, 0.0), Vec3::Y),
            Frame::sphere(Vec3::new(0.0, 1.6, 0.0), Vec3::Y, Vec3::Z),
        ];
        for frame in frames {
            for (s, t, r) in [(0.4, 0.3, 0.2), (1.1, -2.5, 0.15), (0.05, 3.0, 0.3)] {
                let p = frame.point(s, t, r);
                let (s2, t2, r2) = frame.coords(p);
                assert!((s - s2).abs() < 1e-4 && angle_dist(t, t2) < 1e-4 && (r - r2).abs() < 1e-4, "{frame:?} {s} {t} {r}");
            }
        }
        assert!(angle_dist(PI - 0.01, -PI + 0.01) < 0.03);
    }

    /// A closed hexagonal prism around the Y axis (radius `r`, height 0..1).
    fn prism(r: f32) -> Vec<[Vec3; 3]> {
        let ring = |y: f32| (0..6).map(|k| Vec3::new(r * (k as f32 * TAU / 6.0).cos(), y, r * (k as f32 * TAU / 6.0).sin())).collect::<Vec<_>>();
        let (a, b) = (ring(0.0), ring(1.0));
        let mut out = Vec::new();
        for k in 0..6 {
            let k1 = (k + 1) % 6;
            out.push([a[k], a[k1], b[k1]]);
            out.push([a[k], b[k1], b[k]]);
        }
        out
    }

    #[test]
    fn profile_measures_the_outside_of_a_prism() {
        let frame = Frame::cylinder(Vec3::ZERO, Vec3::Y, Vec3::Z);
        let p = Profile::build(frame, (0.1, 0.9), 0.05, 36, &prism(0.2), 0.05);
        for (s, t) in [(0.5, 0.0), (0.3, 1.0), (0.8, -2.0)] {
            let r = p.radius(s, t);
            // Between the hexagon's apothem and circumradius, nudged out by the dilation.
            assert!((0.17..=0.21).contains(&r), "radius {r} at {s} {t}");
        }
        let empty = Profile::build(frame, (0.0, 0.2), 0.05, 12, &[], 0.07);
        assert!((empty.radius(0.1, 0.5) - 0.07).abs() < 1e-5);
        assert!((empty.max_over((0.0, 0.2), (0.0, PI)) - 0.07).abs() < 1e-5);
    }

    fn def(plate: Plate) -> PlateDef {
        PlateDef {
            name: "test",
            plate,
            bones: vec![],
            fit: Fit::Free,
            anchor: 0,
            tint: STEEL,
            part: Part::Torso,
        }
    }

    #[test]
    fn tube_plates_are_closed_shells_with_uvs_and_tangents() {
        let frame = Frame::cylinder(Vec3::ZERO, Vec3::Y, Vec3::Z);
        let profile = Arc::new(Profile::build(frame, (0.0, 1.0), 0.05, 36, &prism(0.12), 0.05));
        let mut shell = Plate::new(frame, Shape::Profile(profile.clone()), 0.02, Box::new(|_| (0.3, 0.6)));
        shell.flare_low = (0.006, 0.03);
        shell.ridges.push((0.0, 0.2, 0.005));
        shell.rivets.push(shell.rivet_at(0.45, 1.0));
        let d = def(shell);
        let (r_in, r_out) = d.plate.radii(0.45, 0.5);
        assert!(r_out - r_in >= THICK - 1e-6, "thickness along the normal is never less than THICK");
        let e = d.plate.eval(frame.point(0.45, 0.5, 0.5 * (r_in + r_out)));
        assert!(e.density > 0.0, "inside the shell wall");
        assert!(d.plate.eval(Vec3::new(0.0, 0.45, 0.0)).density < 0.0, "hollow");
        assert!(d.plate.eval(frame.point(0.8, 0.0, 0.16)).density < 0.0, "above the plate");
        let mesh = mesh_plate(&d);
        assert!(mesh.count_vertices() > 500);
        for attr in [Mesh::ATTRIBUTE_NORMAL, Mesh::ATTRIBUTE_UV_0, Mesh::ATTRIBUTE_COLOR, Mesh::ATTRIBUTE_TANGENT] {
            assert!(mesh.attribute(attr).is_some(), "{attr:?}");
        }
        // The seam split leaves no triangle stretched across half a turn of UV.
        let Some(VertexAttributeValues::Float32x2(uv)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!() };
        let Some(Indices::U32(idx)) = mesh.indices() else { panic!() };
        let turn = TAU * 0.15 / TEX;
        for t in idx.chunks(3) {
            let u = t.iter().map(|&i| uv[i as usize][0]).collect::<Vec<_>>();
            let spread = u.iter().cloned().fold(f32::MIN, f32::max) - u.iter().cloned().fold(f32::MAX, f32::min);
            assert!(spread < 0.5 * turn, "triangle stretched across the seam: {spread}");
        }
    }

    #[test]
    fn spherical_plates_have_no_uv_singularity() {
        let frame = Frame::sphere(Vec3::ZERO, Vec3::Y, Vec3::Z);
        let dome = Plate::new(frame, Shape::Radius(0.12), 0.0, Box::new(|_| (-10.0, 1.2)));
        let mesh = mesh_plate(&def(dome));
        let Some(VertexAttributeValues::Float32x2(uv)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!() };
        let Some(Indices::U32(idx)) = mesh.indices() else { panic!() };
        // Neighbouring vertices stay neighbours in UV, crown included.
        for t in idx.chunks(3) {
            let p = t.iter().map(|&i| Vec2::from_array(uv[i as usize])).collect::<Vec<_>>();
            let span = p[0].distance(p[1]).max(p[1].distance(p[2])).max(p[2].distance(p[0]));
            assert!(span < 0.1, "stretched triangle in UV: {span}");
        }
    }

    #[test]
    fn nested_lames_keep_their_clearance() {
        let r = nest_radii(&[0.12, 0.11, 0.2], 0.02);
        assert_eq!(r[0], 0.12);
        assert!((r[1] - 0.14).abs() < 1e-6, "pushed out over the lame above");
        assert_eq!(r[2], 0.2, "already clear");
        for w in r.windows(2) {
            assert!(w[1] - w[0] >= 0.02 - 1e-6);
        }
        // Concentric spheres stay apart under any rotations about their centre.
        let (a, b) = (Quat::from_euler(EulerRot::XYZ, 0.4, -1.2, 2.0), Quat::from_euler(EulerRot::XYZ, -0.9, 0.3, 0.1));
        let p = a * (Vec3::new(0.3, 0.5, -0.2).normalize() * r[0]);
        let q = b * (Vec3::new(-0.7, 0.1, 0.4).normalize() * r[1]);
        assert!((q.length() - p.length()).abs() >= 0.02 - 1e-5);
    }

    #[test]
    fn padding_covers_a_plate_around_the_forearm() {
        use crate::motion::contact::tests::skeleton;
        let (skeleton, bones) = skeleton();
        let rest = skeleton.fk(&skeleton.rest_pose(), bones.pelvis);
        let (elbow, wrist) = (rest[bones.lowerarm[0]].translation, rest[bones.hand[0]].translation);
        let frame = Frame::cylinder(elbow, wrist - elbow, Vec3::Y);
        let tube = Plate::new(frame, Shape::Radius(0.07), 0.0, Box::new(|_| (0.05, 0.2)));
        let mesh = mesh_plate(&def(tube));
        let pad = derive_padding(&[(Part::Forearm(0), &mesh, &[])], &rest, &bones);
        assert!(pad.forearm >= 0.07 + THICK - ARM_R - 0.003, "forearm {}", pad.forearm);
        assert_eq!(pad.torso, 0.0);
    }

    #[test]
    fn rigid_fit_recovers_a_rotation_and_translation() {
        let rest = vec![
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::new(0.0, 0.2, 0.0),
            Vec3::new(0.0, 0.0, 0.3),
            Vec3::new(-0.1, 0.1, 0.05),
            Vec3::new(0.05, -0.1, 0.2),
        ];
        let pc = rest.iter().sum::<Vec3>() / rest.len() as f32;
        let truth = Transform::from_xyz(0.3, -0.2, 1.0).with_rotation(Quat::from_euler(EulerRot::XYZ, 0.7, -1.1, 2.0));
        let now: Vec<Vec3> = rest.iter().map(|p| truth.transform_point(*p)).collect();
        let qc = now.iter().sum::<Vec3>() / now.len() as f32;
        for warm in [Quat::IDENTITY, truth.rotation] {
            let fit = rigid_fit(&rest, pc, &now, qc, warm).unwrap();
            assert!(fit.rotation.angle_between(truth.rotation) < 2e-3, "{warm}");
            for (p, q) in rest.iter().zip(&now) {
                assert!(fit.transform_point(*p).distance(*q) < 2e-3);
            }
        }
        // A pivot fit keeps the pivot on its target and recovers the rotation.
        let pivot = Vec3::new(0.05, 0.05, 0.05);
        let fit = rigid_fit(&rest, pivot, &now, truth.transform_point(pivot), Quat::IDENTITY).unwrap();
        assert!(fit.transform_point(pivot).distance(truth.transform_point(pivot)) < 1e-4);
        // Repeated warm-started frames converge even from a far start.
        let mut q = Quat::from_rotation_y(2.5);
        for _ in 0..10 {
            q = rigid_fit(&rest, pc, &now, qc, q).unwrap().rotation;
        }
        assert!(q.angle_between(truth.rotation) < 2e-3);
        assert!(rigid_fit(&rest[..2], pc, &now[..2], qc, Quat::IDENTITY).is_none());
    }

    #[test]
    fn mip_chain_halves_down_to_one_texel() {
        use bevy::render::render_resource::{Extent3d, TextureDimension};
        let mut image = Image::new_fill(
            Extent3d {
                width: 8,
                height: 4,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[128, 128, 255, 255],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        add_mips(&mut image, Texel::Normal);
        assert_eq!(image.texture_descriptor.mip_level_count, 4);
        assert_eq!(image.data.as_ref().unwrap().len(), 4 * (32 + 8 + 2 + 1));
        // A flat normal stays flat.
        let last = &image.data.as_ref().unwrap()[4 * 42..];
        assert!(last[2] > 250 && (last[0] as i32 - 128).abs() <= 1);
    }

    #[test]
    fn the_real_set_builds_nested_and_padded() {
        let lib = crate::motion::donor::tests::lib("ual2");
        let skeleton = crate::motion::donor::skeleton(lib);
        let bones = Bones::new(&skeleton).expect("bones");
        let body = BodyMesh::load(&std::path::Path::new(crate::ASSETS).join(crate::motion::MODEL_GLB)).expect("body");
        let rest = skeleton.fk(&skeleton.rest_pose(), bones.pelvis);
        let marks = Marks {
            rest: &rest,
            bones: &bones,
            skeleton: &skeleton,
        };
        let defs = design(&body, &marks);
        let meshes: Vec<Mesh> = std::thread::scope(|scope| {
            let jobs: Vec<_> = defs.iter().map(|def| scope.spawn(move || mesh_plate(def))).collect();
            jobs.into_iter().map(|job| job.join().expect("mesh")).collect()
        });
        let padding = set_padding(&defs, &meshes, &body, &rest, &bones);
        println!("padding {padding:?}");
        let mut radii = Vec::new();
        for (def, mesh) in defs.iter().zip(&meshes) {
            let samples = samples_for(def, &body).len();
            if let Shape::Radius(r) = def.plate.shape {
                radii.push(r);
            }
            println!("{:22} vertices {:6} samples {samples}", def.name, mesh.count_vertices());
            assert!(samples >= 3, "{} has {samples} fit samples", def.name);
        }
        println!("lame radii {radii:?}");
        for side in radii.chunks(3) {
            assert!(side.windows(2).all(|w| w[1] > w[0]), "lames nested {side:?}");
        }
    }
}
