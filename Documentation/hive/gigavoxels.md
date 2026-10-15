# GigaVoxels TER (Ríos Pavía & Crassin), shell-map deformation — Animation
Page numbers are PDF pages.

## Usable by Fable
- Animate a coarse mesh and carry the voxel detail with it as a fixed volume texture; the voxels themselves are never edited [§2 p3–4, Fig.4 p5]. For us: the voxel character stays static in the store, and the skeleton drives a coarse shell.
- Keep the vertex count and order fixed per frame so the tetrahedra (and their indices) are built once, not rebuilt every frame [§4.4 p10].
- Non-bijective shell with a constant offset (self-intersections allowed) is fine for a prototype [§4.1 p8].
- Tetrahedra plus barycentric mapping beats per-step ray correction for real time, at the cost of seams [§4.1 p7–8].

## Where it plugs in [team]
- The coarse shell is skinned by our existing rig (`anim_rig` FK, the 65 skin joints). The shell is render-only, downstream of the pose, and never touches physics or the motor writer.
- The voxel store still changes only through `VoxelEdit`; a hit edits the texture-space voxels, and the shell carries them.
- Simpler first step [team]: rigid-per-bone voxel chunks (each voxel bound to one bone), with the shell only at joints.

## Not applicable
- The CUDA A-buffer / tetra rasteriser [§3.5, §4.2, §5.2]: Bevy/wgpu would need its own version.
- Harmonic-coordinate cages (Pixar) are only cited as inspiration [§3.1 p5], not used. Correction: my brainstorm row A5 said "cage skinning"; the paper's method is shell maps.
- The report is an unfinished prototype with no performance numbers; use the idea only.
