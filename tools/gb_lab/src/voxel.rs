//! Scene "clearance": sparse chunked voxel store keyed by IVec3, with a coarse level per chunk
//! (XCube-style 2-level hierarchy: a coarse cell is solid-ish if any fine voxel inside is solid).
//! Coarse-then-fine capsule clearance query, and greedy box merge of solids into Avian colliders.
use crate::{Args, ragdoll::make_app};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::BTreeMap;

pub const CHUNK: i32 = 16; // fine voxels per chunk edge
pub const COARSE: i32 = 4; // fine voxels per coarse cell edge
pub const VOX: f32 = 0.25; // metres per voxel

#[derive(Clone)]
pub struct Chunk {
    pub fine: Vec<bool>,  // CHUNK^3
    pub coarse: Vec<u16>, // (CHUNK/COARSE)^3, count of solid fine voxels
}

impl Chunk {
    fn new() -> Self {
        let n = (CHUNK / COARSE) as usize;
        Self {
            fine: vec![false; (CHUNK * CHUNK * CHUNK) as usize],
            coarse: vec![0; n * n * n],
        }
    }
}

fn fidx(l: IVec3) -> usize {
    (l.x + CHUNK * (l.y + CHUNK * l.z)) as usize
}
fn cidx(c: IVec3) -> usize {
    let n = CHUNK / COARSE;
    (c.x + n * (c.y + n * c.z)) as usize
}

/// BTreeMap keeps iteration order deterministic (HashMap order would not be).
#[derive(Default)]
pub struct VoxelStore {
    pub chunks: BTreeMap<(i32, i32, i32), Chunk>,
}

impl VoxelStore {
    fn split(p: IVec3) -> (IVec3, IVec3) {
        let c = p.div_euclid(IVec3::splat(CHUNK));
        (c, p - c * CHUNK)
    }
    /// The single writer: keeps the coarse level in sync on every edit.
    pub fn set(&mut self, p: IVec3, solid: bool) {
        let (c, l) = Self::split(p);
        let ch = self
            .chunks
            .entry((c.x, c.y, c.z))
            .or_insert_with(Chunk::new);
        let i = fidx(l);
        if ch.fine[i] == solid {
            return;
        }
        ch.fine[i] = solid;
        let ci = cidx(l / COARSE);
        if solid {
            ch.coarse[ci] += 1
        } else {
            ch.coarse[ci] -= 1
        }
    }
    pub fn get(&self, p: IVec3) -> bool {
        let (c, l) = Self::split(p);
        self.chunks
            .get(&(c.x, c.y, c.z))
            .map(|ch| ch.fine[fidx(l)])
            .unwrap_or(false)
    }
    /// Coarse cell occupancy for a world coarse coordinate.
    pub fn coarse(&self, cc: IVec3) -> bool {
        let p = cc * COARSE;
        let (c, l) = Self::split(p);
        self.chunks
            .get(&(c.x, c.y, c.z))
            .map(|ch| ch.coarse[cidx(l / COARSE)] > 0)
            .unwrap_or(false)
    }
}

fn seg_point_dist2(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
    (a + ab * t - p).length_squared()
}

/// Distance from segment to an AABB, approximated by sampling the segment (exact enough for a lab).
fn seg_aabb_dist(a: Vec3, b: Vec3, min: Vec3, max: Vec3) -> f32 {
    let mut best = f32::MAX;
    for i in 0..=16 {
        let p = a.lerp(b, i as f32 / 16.0);
        best = best.min(p.clamp(min, max).distance(p));
    }
    // also closest segment point to box centre
    let c = (min + max) * 0.5;
    let _ = seg_point_dist2(a, b, c);
    best
}

pub struct Query {
    pub clear: bool,
    pub coarse_tested: u32,
    pub fine_tested: u32,
}

/// Capsule clearance: is the capsule (segment a-b, radius r) free of solid voxels?
pub fn capsule_clear(s: &VoxelStore, a: Vec3, b: Vec3, r: f32, use_coarse: bool) -> Query {
    let lo = (a.min(b) - Vec3::splat(r)) / VOX;
    let hi = (a.max(b) + Vec3::splat(r)) / VOX;
    let (lo, hi) = (lo.floor().as_ivec3(), hi.floor().as_ivec3());
    let mut q = Query {
        clear: true,
        coarse_tested: 0,
        fine_tested: 0,
    };
    let test_fine = |p: IVec3, q: &mut Query| {
        q.fine_tested += 1;
        if s.get(p) {
            let mn = p.as_vec3() * VOX;
            if seg_aabb_dist(a, b, mn, mn + Vec3::splat(VOX)) < r {
                q.clear = false;
            }
        }
    };
    if use_coarse {
        let (clo, chi) = (
            lo.div_euclid(IVec3::splat(COARSE)),
            hi.div_euclid(IVec3::splat(COARSE)),
        );
        for z in clo.z..=chi.z {
            for y in clo.y..=chi.y {
                for x in clo.x..=chi.x {
                    let cc = IVec3::new(x, y, z);
                    q.coarse_tested += 1;
                    if !s.coarse(cc) {
                        continue;
                    }
                    let mn = (cc * COARSE).as_vec3() * VOX;
                    if seg_aabb_dist(a, b, mn, mn + Vec3::splat(VOX * COARSE as f32)) >= r {
                        continue;
                    }
                    for fz in 0..COARSE {
                        for fy in 0..COARSE {
                            for fx in 0..COARSE {
                                let p = cc * COARSE + IVec3::new(fx, fy, fz);
                                if p.cmplt(lo).any() || p.cmpgt(hi).any() {
                                    continue;
                                }
                                test_fine(p, &mut q);
                            }
                        }
                    }
                }
            }
        }
    } else {
        for z in lo.z..=hi.z {
            for y in lo.y..=hi.y {
                for x in lo.x..=hi.x {
                    test_fine(IVec3::new(x, y, z), &mut q);
                }
            }
        }
    }
    q
}

/// Greedy 3D box merge per chunk: grow x, then y, then z. Returns (min, size) in voxel coords.
pub fn greedy_boxes(s: &VoxelStore) -> Vec<(IVec3, IVec3)> {
    let mut out = Vec::new();
    for (&(cx, cy, cz), ch) in &s.chunks {
        let mut used = vec![false; ch.fine.len()];
        let solid = |l: IVec3, used: &Vec<bool>| ch.fine[fidx(l)] && !used[fidx(l)];
        for z in 0..CHUNK {
            for y in 0..CHUNK {
                for x in 0..CHUNK {
                    let o = IVec3::new(x, y, z);
                    if !solid(o, &used) {
                        continue;
                    }
                    let mut sx = 1;
                    while x + sx < CHUNK && solid(o + IVec3::X * sx, &used) {
                        sx += 1;
                    }
                    let mut sy = 1;
                    'y: while y + sy < CHUNK {
                        for i in 0..sx {
                            if !solid(o + IVec3::new(i, sy, 0), &used) {
                                break 'y;
                            }
                        }
                        sy += 1;
                    }
                    let mut sz = 1;
                    'z: while z + sz < CHUNK {
                        for j in 0..sy {
                            for i in 0..sx {
                                if !solid(o + IVec3::new(i, j, sz), &used) {
                                    break 'z;
                                }
                            }
                        }
                        sz += 1;
                    }
                    for k in 0..sz {
                        for j in 0..sy {
                            for i in 0..sx {
                                used[fidx(o + IVec3::new(i, j, k))] = true;
                            }
                        }
                    }
                    out.push((IVec3::new(cx, cy, cz) * CHUNK + o, IVec3::new(sx, sy, sz)));
                }
            }
        }
    }
    out
}

fn build_world() -> VoxelStore {
    let mut s = VoxelStore::default();
    // floor 32x32 voxels (8 m), 2 voxels thick, top at y = 0
    for z in -16..16 {
        for x in -16..16 {
            for y in -2..0 {
                s.set(IVec3::new(x, y, z), true);
            }
        }
    }
    // pillar and a low wall
    for y in 0..12 {
        for z in 2..4 {
            for x in 2..4 {
                s.set(IVec3::new(x, y, z), true);
            }
        }
    }
    for y in 0..3 {
        for x in -10..-2 {
            s.set(IVec3::new(x, y, -6), true);
        }
    }
    // a VoxelEdit-style hole punched in the wall
    s.set(IVec3::new(-6, 1, -6), false);
    s
}

pub fn run(args: &Args) -> String {
    let s = build_world();
    let solids: usize = s
        .chunks
        .values()
        .map(|c| c.fine.iter().filter(|b| **b).count())
        .sum();
    // clearance queries: capsule r 0.3, height 1.8, at several spots
    let spots = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.6, 0.0, 0.6),
        Vec3::new(-1.5, 0.0, -1.4),
        Vec3::new(-1.5, 0.0, -2.5),
        Vec3::new(3.0, 0.0, 3.0),
    ];
    let mut rows = Vec::new();
    let mut agree = true;
    let (mut ct, mut ft, mut bt) = (0u32, 0u32, 0u32);
    for p in spots {
        let (a, b) = (p + Vec3::Y * 0.31, p + Vec3::Y * 1.5);
        let h = capsule_clear(&s, a, b, 0.3, true);
        let f = capsule_clear(&s, a, b, 0.3, false);
        agree &= h.clear == f.clear;
        ct += h.coarse_tested;
        ft += h.fine_tested;
        bt += f.fine_tested;
        rows.push(format!("{{\"at\":[{},{},{}],\"clear\":{},\"coarse_cells\":{},\"fine_hier\":{},\"fine_brute\":{}}}", p.x, p.y, p.z, h.clear, h.coarse_tested, h.fine_tested, f.fine_tested));
    }
    let boxes = greedy_boxes(&s);
    // Avian check: merged boxes as static colliders, drop a capsule onto the floor near the pillar
    let mut app = make_app();
    for (mn, sz) in &boxes {
        let size = sz.as_vec3() * VOX;
        let c = mn.as_vec3() * VOX + size * 0.5;
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            Transform::from_translation(c),
        ));
    }
    let cap = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::capsule(0.3, 1.2),
            Transform::from_xyz(0.0, 2.0, 0.0),
            LockedAxes::ROTATION_LOCKED,
        ))
        .id();
    for _ in 0..args.ticks.max(240) {
        app.update();
    }
    let y = app
        .world()
        .get::<Position>(cap)
        .map(|p| p.0.y)
        .unwrap_or(f32::NAN);
    let expected = 0.9; // 0.6 half-length + 0.3 radius
    format!(
        "{{\"scene\":\"clearance\",\"solid_voxels\":{solids},\"chunks\":{},\"greedy_boxes\":{},\"merge_ratio\":{:.1},\"queries\":[{}],\"hier_matches_brute\":{agree},\"coarse_cells_tested\":{ct},\"fine_tested_hier\":{ft},\"fine_tested_brute\":{bt},\"capsule_rest_y\":{y:.4},\"capsule_rest_err\":{:.4},\"pass\":{}}}",
        s.chunks.len(),
        boxes.len(),
        solids as f32 / boxes.len().max(1) as f32,
        rows.join(","),
        (y - expected).abs(),
        agree && (y - expected).abs() < 0.03
    )
}
