# Neutral-rig terrain and motion alignment test

Run play.cmd or `cargo run -p guildbound`. The default scene uses plain capsule
rigs, not the earlier voxel characters. It does not load a GLB or run a source
character's animation player. The older scene remains behind `--sandbox`.

## Offline animation alignment

tools/bake_motion_tracks.py reads the reference GLB and verified contact metadata
outside the viewer. Numeric joint rotations, foot paths normalized by leg reach,
and contact curves are exported into assets/motion/neutral_tracks.json.
The viewer embeds this 450 KB table. Re-bake: `python tools/bake_motion_tracks.py`.

Alignment measures the target rig's immutable leg lengths and hip width once at
setup. Normalized paths scale to those proportions. Bind-frame differences were
removed during the bake. No source character is instantiated or consulted during
the experiment. The rendered figures are untextured capsule-and-box test rigs.

## Maths from gb_live

D:/gb_live/crates/game/src/support.rs supplies the contact thresholds, fixed
floor-anchor logic, bounded 30-degree foot tilt and pelvis reach correction;
ik.rs supplies the analytic two-bone solve; motion_math.rs supplies the damped
spring. The underlying courtyard height function is shared with the overhaul's
existing sim terrain module. D:/gb_live is unchanged and has no runtime role.

The live area's selected floor is currently flat. This test uses a hillside
patch of its underlying courtyard terrain. Mesh, collision triangles and probes
use the same triangulation. The motor now accepts actual floor heights under
each foot; a missed probe removes support. Fall height is relative to the floor,
so a negative world elevation does not incorrectly latch stand_failed.

## Visible experiment

- Left: aligned tracks and terrain IK, no rigid bodies or colliders.
- Middle: passive physical rig, no motors or active foot damping.
- Right: physical rig follows joint targets with optional capped spring assist.

Planned roots travel in small loops against stationary terrain. Green markers
and trails show planned feet; amber marks swing; cyan shows actual physical feet
and their trails. Probe lines and surface normals expose floor alignment.

Motor strength scales torque and active damping. Tracking assist adds segment
spring forces and gravity compensation while traced foot support is available.
It is enabled by default and can be turned off. This is assisted physical
tracking, not evidence of unassisted muscle-only walking. Dynamic bodies are
never teleported onto the reference after initialization/reset.

Physics blend changes the displayed pose mixture, not mass or gravity. At 100%
the right rig displays measured physics. Pause freezes track targets while
physics continues. Idle/Walk/Run reset the experiment. Push applies the same
velocity change to both physical bodies. R resets.

## Verification

Tests check final IK foot positions and fixed lengths, probe agreement with
collision geometry, raised/lowered floor contact, missing support, zero-motor
collapse, slider endpoints, reset cleanup, and ten seconds of assisted physical
terrain motion with real world displacement and upright-height checks.

    cargo test -p guildbound_sim
    cargo test -p guildbound --bin guildbound -- --nocapture
    cargo build -p guildbound
