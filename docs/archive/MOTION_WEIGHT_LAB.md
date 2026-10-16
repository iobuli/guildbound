# Isolated motion / weight lab

Historical first version. The current neutral-rig terrain test is documented in
TERRAIN_ALIGNMENT.md; the GLB-character setup below is no longer the default.

Run play.cmd or `cargo run -p guildbound` for the three-lane comparison.
The earlier controllable sandbox is `cargo run -p guildbound -- --sandbox`.
D:/gb_live is a read-only mathematical reference, not a runtime dependency.
Its reduced-inertia and Stable PD functions agree with the overhaul's current
motor formula; no live-game systems were imported for this experiment.

All lanes use the same master GLB reference, fitted segment geometry and clip
clock. They are shown as voxels. Their floor ovals are visual reference marks.

- Animation: source clip posed directly on a visual rig. No rigid body, collider,
  gravity, forces, simulated velocity or root travel.
- Ragdoll: separate dynamic body, gravity and floor contact, zero muscle torque
  and zero active foot damping. It is expected to collapse.
- Combined: another independent dynamic body follows source joint targets through
  bounded paired motor torques. Its displayed rig mixes the reference and measured
  local joint rotations, preserving segment lengths. Root position and orientation
  also blend. The dynamic segments are hidden behind that visual rig.

Physics blend changes the visible mixture: zero displays only animation, one
displays the physical result. It does not change gravity or physical body mass.
Motor strength scales the combined body's motor torque and active foot damping.
At zero, the combined physical body is passive. It still has physical mass and
momentum even when its visible contribution is zero. Animation alone does not
generate forward propulsion; this lab exposes the controller's current response.
Walking/running balance is not claimed to be solved.

Playback speed changes the clip clock and reference derivatives, not the physics
step. Pause freezes the clip while physical simulation continues. Idle/Walk/Run
changes the reference and resets the bodies. Push both ragdolls applies the same
1.5 m/s lateral velocity increment to each body, independent of segment mass.
Reset (or R) recreates only the physical bodies and resets the shared clip clock.
The models do not collide with each other, and no grab links connect the lanes.

Tests check drag endpoints/release, blend endpoints, the absence of physics on
visual rigs, passive collapse at zero motor strength, and cleanup of old sim
bodies on reset. Existing real-rig mapping and physics-bridge tests remain.

    cargo test -p guildbound --bin guildbound
    cargo test -p guildbound_sim
    cargo build -p guildbound
