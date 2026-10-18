# Third-party assets

| Asset | Source | License | Notes |
| --- | --- | --- | --- |
| `assets/anim/cmu/*.bvh` (67 takes) | CMU Graphics Lab Motion Capture Database, http://mocap.cs.cmu.edu/, through the cgspeed "Motionbuilder-friendly" BVH conversion (mirror: github.com/Shriinivas/cmubvh) | CMU places no restrictions on use; the conversion adds none ("CMU places no restrictions on the use of the original dataset, and I (Bruce) place no additional restrictions on the use of this particular BVH conversion.") | Picked and downsampled to 30 fps by `tools/cmu/subset.py`. The data was obtained from mocap.cs.cmu.edu; the database was created with funding from NSF EIA-0196217. |
| `assets/anim/ual1.glb`, `assets/anim/ual2.glb` | Quaternius, Universal Animation Library 1 and 2 | CC0 1.0 | |
| `assets/vfx/kenney/*`, `assets/audio/kenney/*` | Kenney (kenney.nl) | CC0 1.0 | See the license files beside them. |

Not listed here (licence terms not recorded in this repository): `assets/anim/sekiro.glb`, `assets/anim/rokoko.glb` (converted from `Rokoko_Free_Mocap_FBX_263`, per `motion/mod.rs`) and `assets/packs/weapons/*`.
