# MDR (Liu & Liu, NeurIPS 2021): what Fable can use
Pages = PDF pages = printed pages (main text p1-10). Source text: papers/txt/liu-voxel-3d-detection-reconstruction-single-image-neurips2021.txt. Our own ideas are tagged [team].
Licence: none stated; "Code is available at http://cvlab.cse.msu.edu/project-mdr.html" [abstract p1]. Ideas only - reuse no code or weights.

## 1. What Fable can use
- Coarse/fine split: coarse binary occupancy (inside or surface-crossing voxels are occupied), fine shape only for occupied voxels [s3.3 p5; Fig.3 p5].
- Local PCA-SDF decode: per occupied voxel a k^3 SDF lattice; S_i = S_B z_i, with S_B the l_B PCA bases (l_B << k^3) and z_i the voxel's coefficients [s3.3 p6]. Without a network the coefficients are a least-squares fit of the bases to the true SDF [s4.4 p10]. Decode is a fixed-order dot product.
- Sizes: k = 11, l_B = 64, bases from ~200,000 voxels of 200 surfaces [s4.1 p7]; l_B = 32 gives nearly the same error (mean CD 0.029 vs 0.025) at half the storage [s4.4 p10, Tab.4]. Bases transfer to unseen categories [s4.4 p10].
- Seams: 1.5x overlapping footprint per voxel, average overlapping SDF values at boundaries [s3.3 p6].
- LOD: trilinear resampling of S_B decodes at higher resolution without new coefficients [s3.3 p6].
- Smaller voxels make local shapes more alike (bases explain more) [s4.3 p9; Fig.5(b) p8].
- Compact box format: centre + offset, log-size vs class mean, sin/cos yaw; corners B = R_th [+-l/2, +-h/2, +-w/2] + c3d [s3.2 p5, Eq.1].

## 2. Where it plugs in [team]
- Render only: optional smooth-surface mode; per-surface-voxel coefficients baked offline, decoded at mesh build or in a wgpu shader; seam averaging applied across chunk borders.
- `sim` keeps integer occupancy + material id; SDF never feeds sim state, colliders, or the hash.
- Asset bake tool: compute S_B from our own asset library, fit coefficients per asset.

## 3. What doesn't apply
- All the learned parts: ResNet-34 features, 2D-to-3D lifting, 3D U-Net, CenterNet-3D, the coefficient MLP [s3.1 p4; s3.2 p4-5; s3.4 p6-7].
- Needs a camera calibration matrix (their stated limitation) [s5 p10].
- Image-to-scene reconstruction is offline block-out at most [abstract p1].
- Nothing on physics, contact, or articulated bodies [checked p1-10].
