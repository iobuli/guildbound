# XCube (Ren et al., NVIDIA, CVPR 2024): what Fable can use
Pages = PDF pages 1–8 (proceedings pp. 4209–4216). Our own ideas are tagged [team].

## 1. What Fable can use
- Strict nesting: the hierarchy is L coarse-to-fine sparse grids, and each finer grid G(l+1) sits strictly inside the coarser G(l) [XCube §3 p3]. Our store can keep this invariant, so a coarse cell exists whenever any fine cell inside it exists.
- Subdivide/prune: finer levels come from splitting existing voxels into octants and pruning the extras [§3.1 p3, Fig. 3]. Doing it in steps (progressive pruning) beats one single prune: IoU 92.88% vs 89.68%, and the single prune used 3× the GPU memory [§4.4 p8]. This is how levels are built, not a collision method.
- Sparsity: nothing is stored for empty regions, so dense detail sits only near surfaces [§1 p2, §2 p2]. Storage is VDB: 11 MB for 3.4M voxels, with fast lookup [§3.4 p5]. A sparse chunked map keyed by integer coords fits this idea [team].
- Level depth: 2–3 levels perform about the same, and a coarsest level of 16³ is enough for the overall shape [§4.4 p8, Tab. 2]. We can use that as a starting LOD depth guess [team].
- Per-voxel attributes (normals, semantics, TSDF) are stored on each level, and a mesh is extracted from the TSDF [§3 p3, §3.3 p4, Fig. 1]. We could store a material id or semantic tag per cell the same way and derive meshes from the store [team].
- Coarse-level editing: add or remove coarse voxels and regenerate the finer ones. They did it in Goxel, a Minecraft-style editor [§4.1 p6, Fig. 6]. Good fit for authoring workflow.
- Offline generation: objects from text or a category in about 30 s for geometry plus about 30 s for texture [§4.2 p6, Fig. 7, 9]. Outdoor scenes in 102.4 m chunks with 10 cm voxels [§4.3 p6–7, Fig. 10, 12]. Scan completion [§4.3 p7, Fig. 11]. Authoring only.

## 2. Where it plugs into our code [team unless cited]
- `sim` voxel store [team]: chunk = fine level and a per-chunk coarse occupancy level, kept in sync on each VoxelEdit at the tick boundary. Nesting rule from [§3 p3].
- Broad-phase [team, Physics' idea]: query the coarse occupancy first for clearance and debris, then go to fine cells only where coarse is occupied. XCube does not cover this.
- LOD and meshing [team]: far LOD reads the coarse levels and near LOD reads the fine levels. Meshes are rebuilt on whole ticks from the per-cell attributes (TSDF-to-mesh idea from [§3.3 p4]).
- Offline tool, outside the game [team]: generator output → voxelize → import as asset chunks. Any edits are recorded as VoxelEdit only. No ML in sim.

## 3. What doesn't apply
- The ML itself: the VAE, latent diffusion and the losses [§3.1–3.3, Eq. 1–7 p3–4], DDIM sampling [§3.3 p4], and CLIP conditioning [§3.2 p4]. These run on the GPU and aren't deterministic, so they're offline at most.
- Their sparse CUDA/VDB framework [§3.4 p5]. We use Rust with our own store [team].
- Early dilation and the refinement network [§3.4 p4–5] are details of how the network is built.
- Collision and physics: the paper has none on either (Physics' correction confirmed).
- Licence: the PDF states no licence and has no code or weights link, only a "project webpage" with no URL given [Abstract p1]. Use the ideas only and none of their code or models until a licence is checked [team].
- Their training sets (ShapeNet, Objaverse, Waymo, Karton City [§4 p5–7]) each have their own terms, so we shouldn't ship anything trained on them without checking.
