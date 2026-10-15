# Overhaul context review - 2026-10-15

This is the initial context audit. Subsequent source-GLB fitting, animation
retargeting and game-bridge test results are recorded in SOURCE_ALIGNMENT.md.

## Direction

Keep the overhaul as the small physics sandbox. Use D:/gb_live as reference for
gameplay and motion intent. Its working tree has local modifications and is at
detached commit edc7988; the historical branch recommendation in the supplied
conversation is not evidence that another checkout should replace this one.
No files in gb_live were changed during this review.

The supplied conversation and gb_live/Documentation/CLAUDE_POSSESSION_CONCEPT.md
agree on a deterministic local controller with no runtime LLM requirement.
Reference motion supplies intent; measured physical support determines whether
that motion is possible. Fixed segment lengths, bounded torque and recovery
steps should take precedence over forcing a pose through missing support.

## Existing boundaries

| Responsibility | Current code |
| --- | --- |
| Bodies, state machine, targets, motor outputs | crates/sim/src/api/ |
| Balance, contact filtering, gait decisions, grabs, recovery maths | crates/sim/src/phys_*.rs |
| Procedural reference poses and gait targets | crates/sim/src/anim/ |
| Avian entities, measurements, paired torque application | crates/game/src/body.rs |
| Input wishes and actions | crates/game/src/input.rs |
| Rig geometry and segment inertia | crates/game/src/rig.rs |
| Segment-attached voxel appearance | crates/game/src/voxel.rs |
| Experimental voxel store, modal sway, stand lab | tools/gb_lab/src/ |

The game already has a 1 m box floor, voxel appearance, F3 overlay, and a model
picker, without the old F2 studio. The sparse voxel store and modal sway remain
lab implementations; the rendered character is rigid segment-attached voxel
geometry, not a demonstrated GigaVoxels shell-deformation implementation.
PoseNet is a separate workspace crate, not a dependency of the game crate.

## Evidence and attribution

The supplied correction attributes modal sway to DynamicTree section 4,
equations 6-7, and sparse storage to section 3.2. The lab's phys_modal.rs header
agrees with the modal attribution. Treat 16-cubed chunks and the 4-cubed mask as
team design, and GigaVoxels as the supplied shell-deformation reference.
These are supplied attributions, not independently verified paper citations.

The named physics papers, PHYSICS_MATH.md, ANIMATION_MATH.md and REPO_SURVEY.md
were not found in the searched overhaul tree or gb_live documentation, collab
and staging directories. The /workspace/hive paths in the conversation refer
to another machine. Do not claim to have read those PDFs here.

The full reduced-inertia matrix and modified Stable PD law are implemented in
crates/sim/src/phys_balance.rs. Omitting the extra Tan numerator term is an
explicit local variant, not proof of what the original paper recommends.

tools/gb_lab/rerun/kb5_ss12.json records a historical 40 s stand run: minimum COM
0.9806 m, final COM 0.9935 m, final drift 0.0342 m, maximum drift 0.3855 m,
paired torque sum zero, no reported floor tunnelling. Its kp=4000, kd=126,
foot_damp=0 and k_com=0 differ from current game defaults (1500, 55, 40, 0.11).
That saved run does not validate today's game controller or moving gait.

## Confirmed gaps

- The game spawn path lacked swept CCD on shins/feet and an angular speed cap.
  Added SweptCcd::NON_LINEAR on Knee/Ankle segments and MaxAngularSpeed using
  sim.tuning.w_cap (currently 50 rad/s). Avian 0.7.0 APIs checked in local source.
- The previous model-swap fix clears cached state, movement, boost and action
  state so replacement bodies do not inherit old launch/recovery inputs.
- All body segments collide only with the floor. The player and dummy therefore
  do not physically collide with each other. This also disables self-collision.
- Knees use spherical joints with a swing cone, not anatomical hinge constraints.
- sim's 50 rad/s fallback cap applies to its internal integrator; that alone did
  not cap Avian bodies. Torque finite checks do not establish recovery from an
  already non-finite rigid-body position or velocity.

## Next verification gates

Measure the actual game bridge with the current rig and gains: 40/80 s stand,
single support, walking, reversal, jump/landing, launch/recover, and model swaps
during launch/grab. Record peak and final drift, final physical sole heights,
contact changes, joint errors and finite state. A historical lab summary or a
passing pure-math suite is insufficient for these gates.

After those measurements, choose anatomical joint constraints and body collision
policy explicitly. Promote voxel or sway functions from the lab only when a
game feature needs them; retain one motor writer and one Avian dynamics solver.
