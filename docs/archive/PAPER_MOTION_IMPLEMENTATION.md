# Paper-derived rig and motion layer

This build integrates the runtime-reproducible parts of:

- `D:\2508.10898v1.pdf` — *Puppeteer: Rig and Animate Your 3D Models*.
- `D:\2411.18197v3.pdf` — *Make-It-Animatable: An Efficient Framework for Authoring Animation-Ready 3D Characters*.

The papers solve automatic authoring. Their neural rig generators cannot be recreated from the PDFs alone: Puppeteer trains a 350M-parameter skeleton transformer and a separate skin-weight network on Articulation-XL2.0 using eight A100 GPUs; Make-It-Animatable trains its particle autoencoder and structure-aware decoder on proprietary/prepared character-motion data. The PDFs do not contain trained weights. Puppeteer's video-guided animation stage is also explicitly offline: roughly 20 minutes for 50 frames on one A100, with longer runs for denser meshes. Placing either training or iterative differentiable rendering in the game loop would violate the real-time and RTX 3060 requirements.

The supplied Ranger assets are already rigged and skinned. The build therefore preserves those artist-authored assets and implements the papers' transferable mathematics as a deterministic runtime layer.

## Implemented correspondence

| Paper component | Runtime implementation |
|---|---|
| Joint token `(x, y, z, parent)` | Each imported bone is recorded once as an entity, local rest transform, parent index, hierarchy depth and fine/coarse flag. |
| Hierarchical ordering | Joints are sorted parent-first. Spatial ordering is not used, avoiding child-before-parent chains. |
| TAJA graph-distance term | All-pairs skeletal graph distances are precomputed. Runtime attention uses normalized `exp(-lambda * distance)` inside a configurable radius. |
| Bone/part context | Graph attention compares pose energy across nearby bones. Isolated changes receive stronger temporal regularization; coordinated chain motion remains responsive. Local axes are never incorrectly averaged between differently rolled bones. |
| Low-rank dynamics `D ~= Bt Ws` | The existing skeleton is the low-rank temporal basis and the imported skin weights are the spatial basis. Bevy evaluates FK and LBS on the GPU. |
| Pose-to-rest mapping | Every frame is expressed as a shortest rotation vector relative to that bone's imported rest rotation. The result is converted back to the same local bone frame. |
| Canonical transformation | A canonical hip frame is calculated from pelvis and upper-thigh positions: hip origin, right-to-left thigh X axis and hip-plane Z normal. It is recorded for later model import/retarget tools. |
| Coarse-to-fine representation | Major body joints receive the full stability solve. Finger chains are marked fine and preserve a configurable share of their authored/procedural detail. |
| Structure-aware next-child processing | The rig is traversed parent-first and deeper joints receive depth-aware coherence control. |
| Connectivity prior | Connectivity is guaranteed by the imported parent hierarchy; only rotations change, so bone translations and segment lengths remain intact. |
| Symmetry prior | Paired leg bones receive a configurable magnitude prior while preserving each leg's axis and opposing gait phase. Arms are excluded so punches, grabs and disarms stay intentionally asymmetric. |
| Global rigid representation | Existing Bevy global transforms provide the rigid hierarchy. Runtime corrections are applied in each bone's calibrated local rest frame, avoiding identity-rest assumptions. |
| Puppeteer animation loss | Native animation plus the existing procedural controller is the source/data term. The temporal and acceleration terms are solved in closed form per bone. |
| Temporal jitter regularization | For rotation vector `x`, source `d`, prior `x1` and second prior `x2`, the system minimizes `wd|x-d|^2 + wv|x-x1|^2 + wa|x-(2x1-x2)|^2`. The exact minimizer is a weighted sum, so no iterative solver is needed. |
| Visibility/tracking/render losses | These remain offline-only concepts. In this game the source pose is already known in 3D, so 2D RGB, mask, depth, optical-flow and tracker reconstruction would add cost and ambiguity without information. |

## Runtime cost and data flow

Calibration is performed once after the Ranger scene loads. For about 65 joints it builds a small graph-distance table and writes `profiles/paper_rig.json`. The per-frame path is linear in joints plus a small bounded graph neighborhood. It allocates only short pose arrays, performs no inference, rendering feedback, file scanning, ray batch, or iterative optimization, and adds no GPU model. This is appropriate for the existing 120 Hz physics / rendered-pose architecture and a 3060-class target.

The order is:

1. Restore the imported bind transforms.
2. Evaluate the Ranger's native idle/walk animation.
3. Apply terrain feet, balance, jump, roll and air response.
4. Apply combat arm IK and procedural finger gestures.
5. Apply the paper-derived topology and temporal pose solve.
6. Propagate transforms; Bevy performs normal skinned rendering.

## Settings

The right-side control panel now has two additional pages:

- `PAPER RIG`: system influence, topology falloff, chain coherence, leg symmetry prior and pose limit.
- `PAPER MOTION`: temporal fit, acceleration regularizer, fine-detail preservation, source-pose recovery and topology graph radius.

These values save with the existing `profiles/feel.json`. Older profiles remain compatible because absent values retain their defaults. The graph representation and canonical frame are written to `profiles/paper_rig.json` when the model loads.

The new module is `crates/game/src/paper_motion.rs`. It is kept separate from locomotion, combat and fingers so future imported models, reference clips, body priors or authoring tools can use the same graph without rewriting the controller.
