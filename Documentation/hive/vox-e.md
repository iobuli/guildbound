# Vox-E (Sella et al., arXiv:2303.12048v3, 2023): what Fable can use
Pages = PDF pages (main text p1-9, supplement p12-17). Source text: papers/txt/vox-e-text-guided-voxel-editing-2023.txt. Our own ideas are tagged [team].
Licence: none stated in the PDF; code only "through our project page" [s1 p1, footnote 1]. Ideas only - reuse no code or weights.

## 1. What Fable can use
- Masked merge of two grids: G_r = M*G_e + (1-M)*G_i, M a binary per-voxel mask [s3.3 p5, Eq.4; Fig.3 p4]. Exact and integer-friendly for us: "chunk after edit = mask ? edited : original" [team].
- Graph-cut split of a voxel region into two labels: a unary term from per-voxel label probabilities plus a pairwise smoothness w_pq = exp(-(c_p - c_q)^2 / (2 sigma^2)) between same-labelled 6-neighbours, sigma = 0.1, lambda = 5, solved by max-flow/min-cut [s3.3 p5, Eq.5]. Graph details: one node per non-empty voxel, top 300 "edit" seeds to source and top 200 "object" seeds to sink, 6-neighbourhood edges with capacity w_pq [supp p12-13]. For us: split a damaged chunk into debris vs intact, seeds from the hit point and from ground-connected cells, w_pq from material-id difference instead of RGB [team].
- Correlation score between two grids: L_reg3D = 1 - Cov(f_i, f_e) / sqrt(Var(f_i) Var(f_e)) over density features [s3.2 p4, Eq.3]. As a deterministic diff metric between chunk states (replay/regression checks, "how much did this edit change") [team]. Supp shows correlation beat plain L1/L2 as a regulariser [supp p14, B.1, Tab.3] - for us that only says it is scale-invariant, not that it is needed [team].
- Grid layout reference: 1 density channel (ReLU) + 3 colour channels (sigmoid), no view-dependent appearance [s3.1 p3]; grid 160^3 in their runs [supp p12]. Shows a small fixed per-voxel feature vector is enough for appearance [team].

## 2. Where it plugs in [team]
- `sim` voxel store: `voxel_mask_merge(a, b, mask)` emitted as one logged `VoxelEdit` (only writer rule kept).
- Destruction: `split_region(chunk, seeds) -> mask` on the tick boundary, then debris pieces from mask components, mass/inertia via the shared phys_mass path (brainstorm idea 5/P3). Max-flow must be a fixed-order integer solver (e.g. push-relabel with fixed node order) to stay replay-exact; PyMaxflow is the paper's choice [supp p13] and is not usable.
- Tests/tooling: correlation diff as a replay check between two runs' chunk states.

## 3. What doesn't apply
- The editing itself: SDS loss with a latent diffusion model [s3.2 p4, Eq.1-2]; Stable Diffusion 2.1 for editing and 1.4 for attention maps [supp p12]; 8000 + 1500 optimisation iterations [supp p12]. GPU ML, non-deterministic - offline authoring at most.
- 3D cross-attention grids supervised from 2D attention maps [s3.3 p5; Fig.4 p5]: needs the diffusion model.
- Needs posed multiview images of the object [abstract p1; s3.1 p3-4].
- Dense grid, not sparse/chunked [s3.1 p3].
- Stated failures: edits placed inconsistently across views, wrong attribute binding [s4.7 p9, Fig.11].
- Nothing on physics, contact, collision or articulated bodies [checked p1-17].
