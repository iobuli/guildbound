//! Physics: sparse voxel store + physics queries for destructible terrain/props.
//! Pure Rust, no deps, deterministic: BTreeMap/BTreeSet only, fixed loop orders, f32 out, f64 accumulators.
//! Cites: XCube [§3 p3, §3.1 p3, Fig. 3] for the coarse-to-fine nesting; everything else is [team]
//! (brainstorm ideas 4, 5, 6, 9, 10 and rows P1-P6). Vox-E's graph-cut [§3.3 p5, Eq. 5] is replaced by a plain flood fill [team].

use crate::phys_math::V3;
use std::collections::{BTreeMap, BTreeSet};

/// Fine cells per chunk edge.
pub const CS: i32 = 16;
/// Coarse cells per chunk edge (each coarse cell covers 4^3 fine cells).
pub const CC: i32 = 4;
pub const CELLS: usize = (CS * CS * CS) as usize;

/// Material ids. 0 = empty.
pub const MAT_EMPTY: u8 = 0;
pub const MAT_STONE: u8 = 1;
pub const MAT_DIRT: u8 = 2;
pub const MAT_WOOD: u8 = 3;
pub const MAT_ICE: u8 = 4;
pub const MAT_METAL: u8 = 5;
pub const MAT_COUNT: usize = 6;
/// Density kg/m^3 per material id (P4, idea 5) [team].
pub const DENSITY: [f32; MAT_COUNT] = [0.0, 2600.0, 1500.0, 700.0, 917.0, 7800.0];
/// Coulomb friction mu per material id; slip when |t| > mu*n at the sole (P4) [team].
pub const FRICTION: [f32; MAT_COUNT] = [0.0, 0.8, 0.6, 0.5, 0.05, 0.4];

pub fn density(m: u8) -> f32 { DENSITY.get(m as usize).copied().unwrap_or(0.0) }
pub fn friction(m: u8) -> f32 { FRICTION.get(m as usize).copied().unwrap_or(0.0) }

pub type CKey = [i32; 3];
pub type WPos = [i32; 3];

/// Local index of (x, y, z) in 0..16: x fastest, then z, then y.
pub fn lidx(x: i32, y: i32, z: i32) -> u16 { (x + CS * (z + CS * y)) as u16 }
pub fn lpos(i: u16) -> [i32; 3] { let i = i as i32; [i % CS, i / (CS * CS), (i / CS) % CS] }
/// World voxel -> (chunk key, local idx). Floor division, so negatives work.
pub fn split(w: WPos) -> (CKey, u16) {
    let c = [w[0].div_euclid(CS), w[1].div_euclid(CS), w[2].div_euclid(CS)];
    (c, lidx(w[0].rem_euclid(CS), w[1].rem_euclid(CS), w[2].rem_euclid(CS)))
}

/// One 16^3 chunk: fine u8 materials + coarse 4^3 occupancy bitmask (bit = x + 4z + 16y).
/// The coarse level strictly contains the fine one: a coarse bit is set iff any fine cell inside is solid
/// [XCube §3 p3 nesting, §3.1 p3 / Fig. 3 coarse-to-fine].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Chunk { pub cells: Vec<u8>, pub coarse: u64 }
impl Chunk {
    pub fn new() -> Self { Self { cells: vec![0; CELLS], coarse: 0 } }
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 { self.cells[lidx(x, y, z) as usize] }
    pub fn rebuild_coarse(&mut self) {
        let mut m = 0u64;
        for i in 0..CELLS as u16 {
            if self.cells[i as usize] != 0 { let p = lpos(i); m |= 1u64 << coarse_bit(p[0] / 4, p[1] / 4, p[2] / 4); }
        }
        self.coarse = m;
    }
    pub fn is_empty(&self) -> bool { self.coarse == 0 }
}
impl Default for Chunk { fn default() -> Self { Self::new() } }
pub fn coarse_bit(cx: i32, cy: i32, cz: i32) -> u32 { (cx + CC * cz + CC * CC * cy) as u32 }

/// Sparse store: absent chunk = all empty [XCube §1 p2 sparsity; BTreeMap is [team]].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Store { pub chunks: BTreeMap<CKey, Chunk>, pub last_tick: u64 }

/// The only write path. Applied on whole ticks before PhysicsSystems::First (P1).
#[derive(Clone, Debug)]
pub struct VoxelEdit { pub chunk: CKey, pub cells: Vec<(u16, u8)>, pub tick: u64 }

impl Store {
    pub fn get(&self, w: WPos) -> u8 { let (c, i) = split(w); self.chunks.get(&c).map_or(0, |ch| ch.cells[i as usize]) }
    pub fn solid(&self, w: WPos) -> bool { self.get(w) != 0 }
}

/// Apply one edit; returns the dirty chunk keys (sorted). Coarse mask rebuilt only for those.
/// Out-of-range indices are ignored; chunks left fully empty are removed (sparsity).
pub fn apply_edit(store: &mut Store, e: &VoxelEdit) -> Vec<CKey> {
    let ch = store.chunks.entry(e.chunk).or_default();
    let mut changed = false;
    for &(i, m) in &e.cells {
        if (i as usize) < CELLS && ch.cells[i as usize] != m { ch.cells[i as usize] = m; changed = true; }
    }
    store.last_tick = store.last_tick.max(e.tick);
    if !changed { if ch.is_empty() { store.chunks.remove(&e.chunk); } return Vec::new(); }
    ch.rebuild_coarse();
    if ch.is_empty() { store.chunks.remove(&e.chunk); }
    vec![e.chunk]
}

/// Apply a tick's edits in order; returns sorted, deduplicated dirty chunks.
pub fn apply_edits(store: &mut Store, edits: &[VoxelEdit]) -> Vec<CKey> {
    let mut d = BTreeSet::new();
    for e in edits { for k in apply_edit(store, e) { d.insert(k); } }
    d.into_iter().collect()
}

/// Axis-aligned box in chunk-local cells: min inclusive, max exclusive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VBox { pub min: [i32; 3], pub max: [i32; 3], pub mat: u8 }

/// Greedy box merge of one chunk (idea 4, P2) [team]. Seeds scanned y, z, x; each box grows in x, then z,
/// then y. Growing x and z first gives the widest flat top faces, so a floor is one box, not a seam grid.
/// Boxes are per material so friction stays per box.
pub fn merge_boxes(ch: &Chunk) -> Vec<VBox> {
    let mut used = vec![false; CELLS];
    let mut out = Vec::new();
    let free = |used: &Vec<bool>, x: i32, y: i32, z: i32, m: u8| { let i = lidx(x, y, z) as usize; !used[i] && ch.cells[i] == m };
    for y in 0..CS { for z in 0..CS { for x in 0..CS {
        let m = ch.get(x, y, z);
        if m == 0 || used[lidx(x, y, z) as usize] { continue; }
        let mut x1 = x + 1;
        while x1 < CS && free(&used, x1, y, z, m) { x1 += 1; }
        let mut z1 = z + 1;
        while z1 < CS && (x..x1).all(|xx| free(&used, xx, y, z1, m)) { z1 += 1; }
        let mut y1 = y + 1;
        while y1 < CS && (z..z1).all(|zz| (x..x1).all(|xx| free(&used, xx, y1, zz, m))) { y1 += 1; }
        for yy in y..y1 { for zz in z..z1 { for xx in x..x1 { used[lidx(xx, yy, zz) as usize] = true; } } }
        out.push(VBox { min: [x, y, z], max: [x1, y1, z1], mat: m });
    } } }
    out
}

/// Mass, centre of mass and inertia tensor about the COM (row-major 3x3).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MassProps { pub mass: f32, pub com: V3, pub inertia: [[f32; 3]; 3] }

/// Mass properties of world voxels of edge `s` metres (idea 5, P3) [team].
/// Per cube: m*s^2/6 on the diagonal, plus parallel-axis m(|r|^2 I - r r^T); then shifted to the COM.
/// Accumulated in f64 in the given order, returned f32. Cell centre = (w + 0.5) * s.
pub fn mass_props(cells: &[(WPos, u8)], s: f32) -> MassProps {
    let s = s as f64; let vol = s * s * s;
    let (mut mt, mut mc, mut i0) = (0.0f64, [0.0f64; 3], [[0.0f64; 3]; 3]);
    for &(w, mat) in cells {
        let m = density(mat) as f64 * vol;
        if m <= 0.0 { continue; }
        let r = [(w[0] as f64 + 0.5) * s, (w[1] as f64 + 0.5) * s, (w[2] as f64 + 0.5) * s];
        let r2 = r[0] * r[0] + r[1] * r[1] + r[2] * r[2];
        mt += m;
        for a in 0..3 {
            mc[a] += m * r[a];
            for b in 0..3 { i0[a][b] += m * ((if a == b { r2 + s * s / 6.0 } else { 0.0 }) - r[a] * r[b]); }
        }
    }
    if mt <= 0.0 { return MassProps { mass: 0.0, com: [0.0; 3], inertia: [[0.0; 3]; 3] }; }
    let c = [mc[0] / mt, mc[1] / mt, mc[2] / mt];
    let c2 = c[0] * c[0] + c[1] * c[1] + c[2] * c[2];
    let mut inertia = [[0.0f32; 3]; 3];
    for a in 0..3 { for b in 0..3 {
        inertia[a][b] = (i0[a][b] - mt * ((if a == b { c2 } else { 0.0 }) - c[a] * c[b])) as f32;
    } }
    MassProps { mass: mt as f32, com: [c[0] as f32, c[1] as f32, c[2] as f32], inertia }
}

/// All solid cells in the store, in BTreeMap/local-index order (deterministic).
pub fn solid_cells(store: &Store) -> Vec<(WPos, u8)> {
    let mut v = Vec::new();
    for (k, ch) in &store.chunks {
        if ch.is_empty() { continue; }
        for i in 0..CELLS as u16 {
            let m = ch.cells[i as usize];
            if m != 0 { let p = lpos(i); v.push(([k[0] * CS + p[0], k[1] * CS + p[1], k[2] * CS + p[2]], m)); }
        }
    }
    v
}

/// A loose piece: its cells (sorted) and mass properties.
#[derive(Clone, PartialEq, Debug)]
pub struct Piece { pub cells: Vec<(WPos, u8)>, pub props: MassProps }

const N6: [[i32; 3]; 6] = [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]];

/// Debris split after an edit (idea 6, P5) [team]: 6-neighbour flood fill, a plain replacement for
/// Vox-E's graph-cut [Vox-E §3.3 p5, Eq. 5]. Cells with y <= ground_y, or listed in `anchors`, are anchored.
/// Returns every connected component touching no anchor, ordered by its smallest cell, as a new body.
pub fn split_debris(store: &Store, anchors: &[WPos], ground_y: i32, s: f32) -> Vec<Piece> {
    let solid: BTreeMap<WPos, u8> = solid_cells(store).into_iter().collect();
    let anchor: BTreeSet<WPos> = anchors.iter().copied().collect();
    let mut seen: BTreeSet<WPos> = BTreeSet::new();
    let mut out = Vec::new();
    for (&start, _) in &solid {
        if seen.contains(&start) { continue; }
        seen.insert(start);
        let (mut comp, mut stack, mut anchored) = (Vec::new(), vec![start], false);
        while let Some(p) = stack.pop() {
            let m = solid[&p];
            if p[1] <= ground_y || anchor.contains(&p) { anchored = true; }
            comp.push((p, m));
            for d in N6 {
                let q = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
                if solid.contains_key(&q) && seen.insert(q) { stack.push(q); }
            }
        }
        if !anchored { comp.sort(); let props = mass_props(&comp, s); out.push(Piece { cells: comp, props }); }
    }
    out
}

/// Top solid y at column (x, z), scanning y_max down to y_min; None if the column is empty (idea 9, P6) [team].
pub fn column_top(store: &Store, x: i32, z: i32, y_min: i32, y_max: i32) -> Option<i32> {
    let mut y = y_max;
    while y >= y_min { if store.solid([x, y, z]) { return Some(y); } y -= 1; }
    None
}

/// Ground height (top face, in voxels) and slope dh/dx, dh/dz by central differences (idea 9, P6) [team].
/// Missing neighbours fall back to the centre height (one-sided -> zero contribution).
pub fn height_slope(store: &Store, x: i32, z: i32, y_min: i32, y_max: i32) -> Option<(f32, [f32; 2])> {
    let h0 = column_top(store, x, z, y_min, y_max)? + 1;
    let h = |xx, zz| column_top(store, xx, zz, y_min, y_max).map_or(h0, |v| v + 1) as f32;
    Some((h0 as f32, [(h(x + 1, z) - h(x - 1, z)) * 0.5, (h(x, z + 1) - h(x, z - 1)) * 0.5]))
}

/// True if no solid cell lies in the world-voxel AABB [min, max] (inclusive) (idea 10, A3).
/// Absent chunks and clear coarse bits exit early; only set coarse cells are checked fine
/// [team; coarse level from XCube §3.1 p3, Fig. 3].
pub fn clear_aabb(store: &Store, min: WPos, max: WPos) -> bool {
    let (c0, _) = split(min); let (c1, _) = split(max);
    for cy in c0[1]..=c1[1] { for cz in c0[2]..=c1[2] { for cx in c0[0]..=c1[0] {
        let Some(ch) = store.chunks.get(&[cx, cy, cz]) else { continue };
        if ch.is_empty() { continue; }
        let o = [cx * CS, cy * CS, cz * CS];
        let lo = [0, 1, 2].map(|a| (min[a] - o[a]).max(0));
        let hi = [0, 1, 2].map(|a| (max[a] - o[a]).min(CS - 1));
        for qy in lo[1] / 4..=hi[1] / 4 { for qz in lo[2] / 4..=hi[2] / 4 { for qx in lo[0] / 4..=hi[0] / 4 {
            if ch.coarse & (1u64 << coarse_bit(qx, qy, qz)) == 0 { continue; }
            for y in (qy * 4).max(lo[1])..=(qy * 4 + 3).min(hi[1]) {
                for z in (qz * 4).max(lo[2])..=(qz * 4 + 3).min(hi[2]) {
                    for x in (qx * 4).max(lo[0])..=(qx * 4 + 3).min(hi[0]) {
                        if ch.get(x, y, z) != 0 { return false; }
                    }
                }
            }
        } } }
    } } }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn near(a: f32, b: f32) -> bool { (a - b).abs() <= 1e-3 * b.abs().max(1.0) }

    #[test]
    fn inertia_cube_and_bar() {
        let m = DENSITY[MAT_STONE as usize];
        let p = mass_props(&[([0, 0, 0], MAT_STONE)], 1.0);
        assert!(near(p.mass, m) && near(p.com[0], 0.5));
        for a in 0..3 { assert!(near(p.inertia[a][a], m / 6.0)); }
        let b = mass_props(&[([0, 0, 0], MAT_STONE), ([1, 0, 0], MAT_STONE)], 1.0);
        let mt = 2.0 * m; // box 2x1x1: Ixx = M(1+1)/12, Iyy = Izz = M(4+1)/12
        assert!(near(b.com[0], 1.0));
        assert!(near(b.inertia[0][0], mt * 2.0 / 12.0));
        assert!(near(b.inertia[1][1], mt * 5.0 / 12.0) && near(b.inertia[2][2], mt * 5.0 / 12.0));
        assert!(b.inertia[0][1].abs() < 1e-2);
    }

    #[test]
    fn full_chunk_one_box() {
        let mut s = Store::default();
        let e = VoxelEdit { chunk: [0, 0, 0], cells: (0..CELLS as u16).map(|i| (i, MAT_STONE)).collect(), tick: 1 };
        assert_eq!(apply_edit(&mut s, &e), vec![[0, 0, 0]]);
        assert_eq!(s.chunks[&[0, 0, 0]].coarse, u64::MAX);
        assert_eq!(merge_boxes(&s.chunks[&[0, 0, 0]]), vec![VBox { min: [0; 3], max: [16; 3], mat: MAT_STONE }]);
        assert!(!clear_aabb(&s, [3, 3, 3], [3, 3, 3]) && clear_aabb(&s, [0, 16, 0], [15, 40, 15]));
        assert_eq!(height_slope(&s, 2, 2, -8, 40), Some((16.0, [0.0, 0.0])));
    }

    fn scene() -> (Store, Vec<VBox>, Vec<Piece>) {
        let mut s = Store::default();
        // floor y=0, two pillars, a bridge on top at y=3, then remove the bridge's middle cell.
        let mut c = Vec::new();
        for z in 0..8 { for x in 0..8 { c.push((lidx(x, 0, z), MAT_DIRT)); } }
        for y in 1..3 { c.push((lidx(1, y, 1), MAT_STONE)); c.push((lidx(5, y, 1), MAT_STONE)); }
        for x in 1..6 { c.push((lidx(x, 3, 1), MAT_WOOD)); }
        apply_edits(&mut s, &[VoxelEdit { chunk: [0, 0, 0], cells: c, tick: 1 },
            VoxelEdit { chunk: [0, 0, 0], cells: vec![(lidx(1, 2, 1), 0), (lidx(3, 3, 1), 0)], tick: 2 }]);
        let b = merge_boxes(&s.chunks[&[0, 0, 0]]);
        let d = split_debris(&s, &[], 0, 0.1);
        (s, b, d)
    }

    #[test]
    fn deterministic_and_debris() {
        let (s1, b1, d1) = scene();
        let (s2, b2, d2) = scene();
        assert_eq!(s1, s2); assert_eq!(b1, b2); assert_eq!(d1, d2);
        // pillar at x=1 lost y=2, so its bridge half (x=1..2,y=3) is loose; x=4..5 half still on pillar x=5.
        assert_eq!(d1.len(), 1);
        assert_eq!(d1[0].cells, vec![([1, 3, 1], MAT_WOOD), ([2, 3, 1], MAT_WOOD)]);
        assert!(near(d1[0].props.mass, 2.0 * 700.0 * 0.001));
        assert!(near(d1[0].props.com[0], 0.2));
    }
}
