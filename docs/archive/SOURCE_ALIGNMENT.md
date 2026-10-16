# Source-driven ragdoll alignment

The sandbox now reads real GLB skeletons and motion instead of giving every
model the same hard-coded geometry. The peasant is the first model and the
dummy uses the same source. The visible body is still voxel geometry fitted
to the source joints; this change does not render the original skinned mesh.

Sources are assets/packs/npc/male_peasant.glb and the master, fighter and
mannequin GLBs under assets/packs/hero/. Missing files report a geometry
fallback. There is no network or LLM call in the controller.

## Conversion

source.rs reads the full node hierarchy, including intermediary bones and
object transforms. The first idle frame defines the fitted physical rest
geometry, with foot soles placed on the floor. Mannequin has no animation and
uses its bind pose. Left hand is B_L_Forearm.001 in these files.

For source bone i, its physical world rotation delta is
D_i(t) = Q_i(t) * inverse(Q_i(0)). For physical parent p, the motor target is
inverse(D_p(t)) * D_i(t). Using global source rotations before collapsing the
chain preserves the intermediary spine, neck and clavicle contributions.
Bone-frame rotations are never copied directly into physics-joint targets.

Capsule endpoints, voxel bounds and inertia tensors follow the fitted segment
axis. Avian and the pure motor receive the same rotated inertia tensor.
Angular rates are derived from adjacent fixed-tick target orientations.
The existing paired torque writer remains responsible for physical movement.

## Controls and limits

The model picker starts with peasant, followed by master, mannequin and hero.
Idle is the source idle clip. Movement requests select available forward walk
or run clips; peasant has walk but no run and falls back to walk. A model with
no clips holds its fitted bind pose.

Source root translation is not applied to the ragdoll. Segment lengths remain
fixed and the body is not teleported onto animation frames. Directional
retargeting, phase-aware clip transitions and contact-event resynchronization
remain unfinished. Playing a forward walking reference does not establish
working physical locomotion. Existing procedural jump/get-up paths still need
validation against the fitted geometry.

## Verification

Game tests load all four actual GLBs, check source units and segment geometry,
check zero bind twist at the calibration frame, and reconstruct source world
rotations through the collapsed physical chain. A headless game test exercises
the same spawn, measurements, motor substeps, Avian joints and floor as the
viewer for 4800 updates (approximately 40 simulated seconds).

During development, both source-driven peasants finished that run Grounded
without a latched stand failure, COM heights 0.986 and 0.984 m. The player's
final XZ offset was approximately 0.55 m from its spawn origin. This is evidence
of standing with remaining drift, not a claim that drift or walking is fixed.

Run:

    cargo test -p guildbound --bin guildbound -- --nocapture
    cargo test -p guildbound_sim
    cargo build -p guildbound

The pasted history references 56dae75 and 1e5ee74. Neither object was available
in D:/gb_live during this work. Their reported pelvis-guide, getup-lock and
landing changes have not been assumed to exist in this checkout.
