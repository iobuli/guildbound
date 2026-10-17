//! Voxels and smooth surfaces through them. `VoxelGrid` is a dense box of cells (0 = empty, else
//! a 1-based palette index) that `trees` builds, carves and splits; it meshes either as exposed
//! cube faces (`mesh`) or as a smooth surface (`smooth_mesh`). `Lattice` + `surface_nets` mesh
//! any sampled density field (> 0 inside) the same way; `armor` builds its plates with them. Pure
//! data, no ECS.
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub struct VoxelGrid {
    pub dims: UVec3,
    /// Cell edge (m).
    pub cell: f32,
    /// Local position of cell (0, 0, 0)'s min corner.
    pub origin: Vec3,
    pub cells: Vec<u8>,
}

/// Corner offsets (unit cube) of each face, counter-clockwise seen from outside, and its normal.
const FACES: [([[f32; 3]; 4], [i32; 3]); 6] = [
    ([[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1.0, 0.0, 1.0]], [1, 0, 0]),
    ([[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0]], [-1, 0, 0]),
    ([[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0]], [0, 1, 0]),
    ([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]], [0, -1, 0]),
    ([[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0]], [0, 0, 1]),
    ([[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 0.0]], [0, 0, -1]),
];

/// Per-voxel brightness jitter (±).
const JITTER: f32 = 0.08;

/// Hash of integer coordinates to 0..1.
pub fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27D4_EB2D)
        ^ (y as u32).wrapping_mul(0x1656_67B1)
        ^ (z as u32).wrapping_mul(0x9E37_79B9)
        ^ seed.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
}

impl VoxelGrid {
    pub fn new(dims: UVec3, cell: f32, origin: Vec3) -> Self {
        let n = (dims.x * dims.y * dims.z) as usize;
        Self {
            dims,
            cell: if cell.is_finite() && cell > 0.0 { cell } else { 0.1 },
            origin,
            cells: vec![0; n],
        }
    }

    fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let d = self.dims.as_ivec3();
        if x < 0 || y < 0 || z < 0 || x >= d.x || y >= d.y || z >= d.z {
            return None;
        }
        Some((x + d.x * (z + d.z * y)) as usize)
    }

    /// Cell value, 0 outside the grid.
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        self.index(x, y, z).map_or(0, |i| self.cells[i])
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, value: u8) {
        if let Some(i) = self.index(x, y, z) {
            self.cells[i] = value;
        }
    }

    /// Local centre of a cell.
    pub fn center(&self, x: i32, y: i32, z: i32) -> Vec3 {
        self.origin + (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * self.cell
    }

    /// Layer index containing local height `y` (may be outside the grid).
    pub fn layer_of(&self, y: f32) -> i32 {
        ((y - self.origin.y) / self.cell).floor() as i32
    }

    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.cells.iter().filter(|c| **c != 0).count()
    }

    /// Every filled cell as (x, y, z, value).
    pub fn filled(&self) -> impl Iterator<Item = (i32, i32, i32, u8)> + '_ {
        let d = self.dims.as_ivec3();
        self.cells.iter().enumerate().filter(|(_, v)| **v != 0).map(move |(i, v)| {
            let i = i as i32;
            let x = i % d.x;
            let z = (i / d.x) % d.z;
            let y = i / (d.x * d.z);
            (x, y, z, *v)
        })
    }

    /// Fill each cell whose centre passes `shape` with the value it returns (0 = leave).
    pub fn fill(&mut self, shape: impl Fn(Vec3, IVec3) -> u8) {
        let d = self.dims.as_ivec3();
        for y in 0..d.y {
            for z in 0..d.z {
                for x in 0..d.x {
                    let v = shape(self.center(x, y, z), IVec3::new(x, y, z));
                    if v != 0 {
                        self.set(x, y, z, v);
                    }
                }
            }
        }
    }

    /// Empty every filled cell whose centre satisfies `cut`; returns how many and their mean
    /// local centre.
    pub fn carve(&mut self, cut: impl Fn(Vec3) -> bool) -> (usize, Vec3) {
        let hits: Vec<(i32, i32, i32)> = self
            .filled()
            .filter(|&(x, y, z, _)| cut(self.center(x, y, z)))
            .map(|(x, y, z, _)| (x, y, z))
            .collect();
        let mut sum = Vec3::ZERO;
        for &(x, y, z) in &hits {
            sum += self.center(x, y, z);
            self.set(x, y, z, 0);
        }
        let n = hits.len();
        (n, if n > 0 { sum / n as f32 } else { Vec3::ZERO })
    }

    /// Move every cell in layer `layer` and above into a new grid with the same frame; this
    /// grid keeps the layers below.
    pub fn split_above(&mut self, layer: i32) -> VoxelGrid {
        let mut top = VoxelGrid::new(self.dims, self.cell, self.origin);
        let layer = layer.max(0);
        let cut: Vec<(i32, i32, i32, u8)> = self.filled().filter(|c| c.1 >= layer).collect();
        for (x, y, z, v) in cut {
            top.set(x, y, z, v);
            self.set(x, y, z, 0);
        }
        top
    }

    /// Exposed faces only, flat normals, vertex colours from `palette` (sRGB, 1-based by cell
    /// value) with per-voxel brightness jitter. An empty grid gives one degenerate triangle so
    /// the GPU buffers are never empty.
    pub fn mesh(&self, palette: &[Color], seed: u32) -> Mesh {
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut colors: Vec<[f32; 4]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let linear: Vec<LinearRgba> = palette.iter().map(|c| c.to_linear()).collect();
        for (x, y, z, v) in self.filled() {
            let Some(base) = linear.get(v as usize - 1).or(linear.last()) else {
                continue;
            };
            let k = 1.0 + JITTER * (2.0 * hash3(x, y, z, seed) - 1.0);
            let color = [base.red * k, base.green * k, base.blue * k, 1.0];
            let corner = self.origin + Vec3::new(x as f32, y as f32, z as f32) * self.cell;
            for (quad, n) in FACES {
                if self.get(x + n[0], y + n[1], z + n[2]) != 0 {
                    continue;
                }
                let start = positions.len() as u32;
                for c in quad {
                    positions.push((corner + Vec3::from_array(c) * self.cell).to_array());
                    normals.push([n[0] as f32, n[1] as f32, n[2] as f32]);
                    colors.push(color);
                }
                indices.extend([start, start + 1, start + 2, start, start + 2, start + 3]);
            }
        }
        if indices.is_empty() {
            positions = vec![[0.0; 3]; 3];
            normals = vec![[0.0, 1.0, 0.0]; 3];
            colors = vec![[0.0, 0.0, 0.0, 1.0]; 3];
            indices = vec![0, 1, 2];
        }
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
            .with_inserted_indices(Indices::U32(indices))
    }

    /// Smoothed density at voxel centre (x, y, z), > 0 inside: the cell counts twice, its six
    /// face neighbours once. Lone voxels vanish, one-voxel rods survive, flat faces sit halfway
    /// between the last filled and first empty voxel.
    fn density(&self, x: i32, y: i32, z: i32) -> f32 {
        let filled = |x, y, z| if self.get(x, y, z) != 0 { 1.0 } else { 0.0 };
        let sum = 2.0 * filled(x, y, z)
            + filled(x - 1, y, z)
            + filled(x + 1, y, z)
            + filled(x, y - 1, z)
            + filled(x, y + 1, z)
            + filled(x, y, z - 1)
            + filled(x, y, z + 1);
        sum / 8.0 - DENSITY_BIAS
    }

    /// Smooth surface through the voxels (`surface_nets` over `density`, with a one-voxel empty
    /// border so it always closes). Colours come from the nearest filled voxel (with the same
    /// jitter as `mesh`), so carved wood still shows. `flat` = faceted normals (low-poly), else
    /// smooth.
    pub fn smooth_mesh(&self, palette: &[Color], seed: u32, flat: bool) -> Mesh {
        let d = self.dims.as_ivec3();
        let linear: Vec<LinearRgba> = palette.iter().map(|c| c.to_linear()).collect();
        // Lattice sample (x, y, z) is voxel centre (x - 1, y - 1, z - 1).
        let dims = d + 2;
        let mut density = Vec::with_capacity((dims.x * dims.y * dims.z) as usize);
        for y in 0..dims.y {
            for z in 0..dims.z {
                for x in 0..dims.x {
                    density.push(self.density(x - 1, y - 1, z - 1));
                }
            }
        }
        let lattice = Lattice {
            dims,
            origin: self.center(-1, -1, -1),
            cell: self.cell,
            density,
        };
        let nets = surface_nets(&lattice);
        if nets.indices.is_empty() {
            return self.mesh(palette, seed);
        }
        // Colour of the most interior filled corner of each vertex's cell (or its neighbour).
        let colors: Vec<[f32; 4]> = nets
            .cells
            .iter()
            .map(|cell| {
                let deepest = CORNERS
                    .iter()
                    .map(|c| *cell + *c)
                    .max_by(|a, b| lattice.at(*a).total_cmp(&lattice.at(*b)))
                    .unwrap_or(*cell)
                    - IVec3::ONE;
                let source = std::iter::once(deepest)
                    .chain([IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z].map(|o| deepest + o))
                    .find(|q| self.get(q.x, q.y, q.z) != 0);
                match source {
                    Some(q) => {
                        let v = self.get(q.x, q.y, q.z);
                        let c = linear.get(v as usize - 1).or(linear.last()).copied().unwrap_or(LinearRgba::WHITE);
                        let k = 1.0 + JITTER * (2.0 * hash3(q.x, q.y, q.z, seed) - 1.0);
                        [c.red * k, c.green * k, c.blue * k, 1.0]
                    }
                    None => linear.last().map_or([1.0; 4], |c| [c.red, c.green, c.blue, 1.0]),
                }
            })
            .collect();
        let positions: Vec<[f32; 3]> = nets.positions.iter().map(|p| p.to_array()).collect();
        let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
            .with_inserted_indices(Indices::U32(nets.indices));
        if flat {
            mesh.duplicate_vertices();
            mesh.compute_flat_normals();
        } else {
            mesh.compute_smooth_normals();
        }
        mesh
    }
}

/// Density threshold of `VoxelGrid::density` (a bit under half so one-voxel rods survive).
const DENSITY_BIAS: f32 = 0.45;

/// The 8 corners of a cell (min-corner offsets) and its 12 edges between them.
const CORNERS: [IVec3; 8] = [
    IVec3::new(0, 0, 0),
    IVec3::new(1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(1, 1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(1, 0, 1),
    IVec3::new(0, 1, 1),
    IVec3::new(1, 1, 1),
];
const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

/// A density field (> 0 inside) sampled on a regular lattice: `dims` samples per axis, sample
/// (0, 0, 0) at `origin`, `cell` apart, stored x fastest, then z, then y. Keep the outermost
/// samples outside (≤ 0) so the surface closes.
#[derive(Clone, Debug)]
pub struct Lattice {
    pub dims: IVec3,
    pub origin: Vec3,
    pub cell: f32,
    pub density: Vec<f32>,
}

impl Lattice {
    /// Sample `field` over `min..max` with a two-sample margin on every side. Non-finite values
    /// count as outside.
    pub fn sample(min: Vec3, max: Vec3, cell: f32, field: impl Fn(Vec3) -> f32) -> Self {
        let cell = if cell.is_finite() && cell > 1e-4 { cell } else { 0.01 };
        let origin = min - Vec3::splat(2.0 * cell);
        let span = ((max - min) / cell).max(Vec3::ZERO);
        let dims = span.ceil().as_ivec3() + 5;
        let mut density = Vec::with_capacity((dims.x * dims.y * dims.z) as usize);
        for y in 0..dims.y {
            for z in 0..dims.z {
                for x in 0..dims.x {
                    let v = field(origin + IVec3::new(x, y, z).as_vec3() * cell);
                    density.push(if v.is_finite() { v } else { -1.0 });
                }
            }
        }
        Self {
            dims,
            origin,
            cell,
            density,
        }
    }

    /// Density at sample `p` (must be inside the lattice).
    pub fn at(&self, p: IVec3) -> f32 {
        self.density[(p.x + self.dims.x * (p.z + self.dims.z * p.y)) as usize]
    }

    pub fn position(&self, p: IVec3) -> Vec3 {
        self.origin + p.as_vec3() * self.cell
    }
}

/// Output of `surface_nets`.
#[derive(Clone, Debug, Default)]
pub struct Nets {
    pub positions: Vec<Vec3>,
    /// Triangles, wound so their normals point from inside out.
    pub indices: Vec<u32>,
    /// Each vertex's cell (the lattice sample at its min corner).
    pub cells: Vec<IVec3>,
}

/// The two triangles of a wound quad, split along the diagonal that keeps them closest to
/// coplanar (Surface Nets quads are rarely flat; the other diagonal can fold one triangle over).
fn split_quad(q: [u32; 4], pos: impl Fn(u32) -> Vec3) -> [u32; 6] {
    let normal = |a: u32, b: u32, c: u32| (pos(b) - pos(a)).cross(pos(c) - pos(a)).normalize_or_zero();
    let along_02 = normal(q[0], q[1], q[2]).dot(normal(q[0], q[2], q[3]));
    let along_13 = normal(q[0], q[1], q[3]).dot(normal(q[1], q[2], q[3]));
    if along_02 >= along_13 {
        [q[0], q[1], q[2], q[0], q[2], q[3]]
    } else {
        [q[0], q[1], q[3], q[1], q[2], q[3]]
    }
}

/// Naive Surface Nets: one vertex per cell the surface crosses, at the mean of its interpolated
/// edge crossings, and one quad per crossed sample edge, from the four cells around it (in order
/// round the edge), wound so it faces from inside out.
pub fn surface_nets(l: &Lattice) -> Nets {
    let d = l.dims;
    let mut out = Nets::default();
    if d.min_element() < 2 || l.density.len() != (d.x * d.y * d.z) as usize {
        return out;
    }
    let cells = d - 1;
    let cell_at = |c: IVec3| (c.x + cells.x * (c.z + cells.z * c.y)) as usize;
    let mut vertex = vec![u32::MAX; (cells.x * cells.y * cells.z) as usize];
    for cy in 0..cells.y {
        for cz in 0..cells.z {
            for cx in 0..cells.x {
                let base = IVec3::new(cx, cy, cz);
                let ds = CORNERS.map(|c| l.at(base + c));
                let inside = ds.iter().filter(|v| **v > 0.0).count();
                if inside == 0 || inside == 8 {
                    continue;
                }
                let mut sum = Vec3::ZERO;
                let mut n = 0.0_f32;
                for (a, b) in EDGES {
                    if (ds[a] > 0.0) == (ds[b] > 0.0) {
                        continue;
                    }
                    let t = ds[a] / (ds[a] - ds[b]);
                    let (pa, pb) = (l.position(base + CORNERS[a]), l.position(base + CORNERS[b]));
                    sum += pa + (pb - pa) * t.clamp(0.0, 1.0);
                    n += 1.0;
                }
                vertex[cell_at(base)] = out.positions.len() as u32;
                out.positions.push(sum / n.max(1.0));
                out.cells.push(base);
            }
        }
    }
    let axes = [IVec3::X, IVec3::Y, IVec3::Z];
    for y in 0..d.y {
        for z in 0..d.z {
            for x in 0..d.x {
                let p = IVec3::new(x, y, z);
                let inside = l.at(p) > 0.0;
                for (k, axis) in axes.iter().enumerate() {
                    let q = p + *axis;
                    if q.x >= d.x || q.y >= d.y || q.z >= d.z || inside == (l.at(q) > 0.0) {
                        continue;
                    }
                    // The other two axes, in cyclic order.
                    let (u, v) = (axes[(k + 1) % 3], axes[(k + 2) % 3]);
                    let ring = [p - u - v, p - v, p, p - u];
                    let ids: Vec<u32> = ring
                        .iter()
                        .filter(|c| c.min_element() >= 0 && c.x < cells.x && c.y < cells.y && c.z < cells.z)
                        .map(|c| vertex[cell_at(*c)])
                        .filter(|i| *i != u32::MAX)
                        .collect();
                    if ids.len() != 4 {
                        continue;
                    }
                    // The ring runs counter-clockwise seen from +axis, so it faces +axis: keep it
                    // when the inside is at `p`, reverse it otherwise. The whole quad is wound
                    // one way (never per triangle, which can fold a sliver inside out).
                    let quad = if inside { [ids[0], ids[1], ids[2], ids[3]] } else { [ids[0], ids[3], ids[2], ids[1]] };
                    out.indices.extend(split_quad(quad, |i| out.positions[i as usize]));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn faces(mesh: &Mesh) -> usize {
        match mesh.indices() {
            Some(Indices::U32(i)) => i.len() / 6,
            _ => 0,
        }
    }

    #[test]
    fn meshes_only_exposed_faces_facing_out() {
        let mut g = VoxelGrid::new(UVec3::new(3, 3, 3), 0.1, Vec3::ZERO);
        g.set(1, 1, 1, 1);
        let palette = [Color::WHITE];
        let one = g.mesh(&palette, 0);
        assert_eq!(faces(&one), 6);
        g.set(2, 1, 1, 1);
        assert_eq!(faces(&g.mesh(&palette, 0)), 10, "the shared face is hidden twice");
        // Every triangle winds counter-clockwise around its outward normal.
        let pos = one.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        let nor = match one.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap() {
            bevy::mesh::VertexAttributeValues::Float32x3(v) => v.clone(),
            _ => panic!(),
        };
        let Some(Indices::U32(idx)) = one.indices() else { panic!() };
        for t in idx.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from_array(pos[i as usize]));
            let n = (b - a).cross(c - a);
            assert!(n.normalize().dot(Vec3::from_array(nor[t[0] as usize])) > 0.99);
        }
        // An empty grid still yields a valid (degenerate) mesh.
        let empty = VoxelGrid::new(UVec3::new(2, 2, 2), 0.1, Vec3::ZERO);
        assert_eq!(empty.mesh(&palette, 0).indices().unwrap().len(), 3);
    }

    #[test]
    fn surface_nets_of_an_analytic_sphere() {
        let r = 0.3;
        let lattice = Lattice::sample(Vec3::splat(-r), Vec3::splat(r), 0.02, |p| r - p.length());
        assert!(lattice.at(IVec3::ZERO) < 0.0, "the margin is outside");
        let nets = surface_nets(&lattice);
        assert!(nets.positions.len() > 200 && nets.indices.len() % 3 == 0);
        assert_eq!(nets.positions.len(), nets.cells.len());
        for p in &nets.positions {
            assert!((p.length() - r).abs() < 0.004, "{p} not on the sphere");
        }
        for t in nets.indices.chunks(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| nets.positions[i as usize]);
            let n = (b - a).cross(c - a);
            assert!(n.dot((a + b + c) / 3.0) > 0.0, "triangle at {a} faces inward");
        }
        // A field that is negative everywhere gives nothing.
        let empty = Lattice::sample(Vec3::ZERO, Vec3::ONE * 0.1, 0.02, |_| -1.0);
        assert!(surface_nets(&empty).indices.is_empty());
    }

    fn ball(radius: f32, cell: f32) -> VoxelGrid {
        let n = (2.0 * radius / cell).ceil() as u32 + 2;
        let half = n as f32 * cell / 2.0;
        let mut g = VoxelGrid::new(UVec3::splat(n), cell, Vec3::splat(-half));
        g.fill(|p, _| if p.length() <= radius { 1 } else { 0 });
        g
    }

    #[test]
    fn smooth_mesh_wraps_a_ball_facing_out() {
        let (radius, cell) = (0.5, 0.08);
        let g = ball(radius, cell);
        for flat in [false, true] {
            let mesh = g.smooth_mesh(&[Color::WHITE], 3, flat);
            let pos = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap().to_vec();
            let nor = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap() {
                bevy::mesh::VertexAttributeValues::Float32x3(v) => v.clone(),
                _ => panic!(),
            };
            assert!(pos.len() > 100);
            for (p, n) in pos.iter().zip(&nor) {
                let (p, n) = (Vec3::from_array(*p), Vec3::from_array(*n));
                assert!((p.length() - radius).abs() < 1.5 * cell, "vertex {p} off the ball");
                assert!(n.is_finite() && n.dot(p.normalize()) > 0.3, "normal {n} at {p} points in");
            }
        }
        // Carving changes it; an empty grid still meshes.
        let mut carved = g.clone();
        carved.carve(|p| p.x > 0.2);
        let count = |m: &Mesh| m.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().len();
        assert_ne!(count(&carved.smooth_mesh(&[Color::WHITE], 3, false)), count(&g.smooth_mesh(&[Color::WHITE], 3, false)));
        let empty = VoxelGrid::new(UVec3::splat(3), 0.1, Vec3::ZERO);
        assert!(empty.smooth_mesh(&[Color::WHITE], 0, true).attribute(Mesh::ATTRIBUTE_POSITION).is_some());
    }

    #[test]
    fn carve_and_split_conserve_cells() {
        let mut g = VoxelGrid::new(UVec3::new(4, 6, 4), 0.1, Vec3::new(-0.2, 0.0, -0.2));
        g.fill(|_, _| 2);
        let total = g.count();
        assert_eq!(total, 96);
        let (removed, mean) = g.carve(|p| p.x > 0.0 && (0.2..0.3).contains(&p.y));
        assert_eq!(removed, 8);
        assert!((mean.y - 0.25).abs() < 1e-5 && mean.x > 0.0);
        assert_eq!(g.count(), total - removed);
        let before = g.count();
        let top = g.split_above(g.layer_of(0.3));
        assert_eq!(top.count() + g.count(), before);
        assert!(top.filled().all(|c| c.1 >= 3) && g.filled().all(|c| c.1 < 3));
        assert_eq!(g.get(-1, 0, 0), 0);
        assert_eq!(g.layer_of(0.25), 2);
    }
}
