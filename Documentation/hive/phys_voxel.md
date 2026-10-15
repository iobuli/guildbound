# phys_voxel: voxel physics from the hive research (Physics)

File: `/workspace/phys/phys_voxel.rs` (pure Rust, no deps, BTreeMap/BTreeSet only, fixed loop order, f64 sums -> f32 out).
Registered in `/workspace/phys_check/src/lib.rs`. 3 unit tests in-module, all pass; the existing 6 still pass.

## What it does
Sparse store of 16^3 chunks (u8 material, 0 = empty) with a 4^3 coarse occupancy mask per chunk, one write path (VoxelEdit), and the physics queries debris/terrain need: collider boxes, mass/inertia, debris split, ground height/slope, clearance.

## API
- Consts: `CS=16`, `CC=4`, `MAT_*` (empty, stone, dirt, wood, ice, metal), `DENSITY[]` kg/m^3, `FRICTION[]` mu; `density(m)`, `friction(m)` (P4) [team].
- Index: `lidx(x,y,z)` (x fastest, then z, then y), `lpos(i)`, `split(world) -> (chunk, idx)` (floor div, negatives OK).
- `Chunk { cells, coarse: u64 }`, `rebuild_coarse()`; `Store { chunks: BTreeMap<[i32;3],Chunk>, last_tick }`, `get`, `solid`.
- `apply_edit(&mut Store, &VoxelEdit) -> Vec<CKey>` / `apply_edits(..)` — only writer; coarse rebuilt for dirty chunks only; empty chunks dropped (P1) [team].
- `merge_boxes(&Chunk) -> Vec<VBox{min,max(excl),mat}>` — greedy, grows x then z then y, seeds in y,z,x order: full chunk = 1 box, flat floor = 1 wide box (idea 4, P2) [team].
- `mass_props(&[(WPos,u8)], s) -> MassProps{mass, com, inertia 3x3 about COM}` — per-cube m s^2/6 + parallel axis (idea 5, P3) [team].
- `split_debris(&Store, anchors, ground_y, s) -> Vec<Piece{cells, props}>` — 6-neighbour flood fill; unanchored components become bodies (idea 6, P5) [team], replacing Vox-E's graph-cut [Vox-E §3.3 p5, Eq. 5].
- `column_top`, `height_slope(store,x,z,ymin,ymax) -> (top-face h, [dh/dx, dh/dz])` central differences (idea 9, P6, A1) [team].
- `clear_aabb(store, min, max) -> bool` — absent chunk / clear coarse bit early-out, then fine cells (idea 10, A3) [team; coarse level from XCube §3.1 p3, Fig. 3].

## Plugging into Avian 0.7
1. Collect a tick's `VoxelEdit`s; apply them on the whole-tick boundary in a system ordered before `PhysicsSystems::First` (P1). Never mid-step.
2. For each dirty chunk: despawn its old static colliders, spawn one `Collider::cuboid` per `VBox` (half-extents = (max-min)*s/2, centre = chunk origin + (min+max)*s/2), `Friction::new(friction(mat))`.
3. Run `split_debris`; for each piece, remove its cells via a logged VoxelEdit, spawn a `RigidBody::Dynamic` with `Mass(props.mass)`, `CenterOfMass(props.com)`, `AngularInertia` from `props.inertia` (diagonalise or pass tensor), and colliders from merged boxes of the piece. Small pieces sleep/despawn after N ticks (P5).
4. Foot placement / capture point read `height_slope`; acro gate reads `clear_aabb`.

## Citations
- XCube: strict coarse-to-fine nesting [§3 p3], subdivision levels [§3.1 p3, Fig. 3], sparsity [§1 p2]. XCube says nothing about collision; all physics uses here are [team].
- Vox-E: graph-cut split [§3.3 p5, Eq. 5] — not implemented; flood fill instead [team].
- Box merge, mass sums, height cache, clearance, friction table: [team].

## Limits
- Box merge is per chunk, so a floor spanning chunks has a seam at every 16-cell edge (P2 cross-chunk merge not done yet).
- `split_debris` scans the whole store (fine for tests; production should flood only from dirty chunks with a bounded region).
- `height_slope` returns top-face height in voxels (multiply by s); scans downward per column, no cache yet.
- Inertia tensor is full 3x3; Avian needs it diagonalised for off-axis pieces.
- Edits use local indices; cross-chunk edits must be split by the caller (`split`).
