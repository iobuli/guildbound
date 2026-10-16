# Ranger movement and combat tool

The player and opponent are the male and female Ranger models already in the user's asset collection. Native idle/walk tracks provide the starting poses. Run transitions, steering, braking, jump preparation, air tuck, landing, roll, punches and disarm are procedural layers using each skeleton's measured segment lengths and actual parent frames. There is no newly imported run or jump clip. `D:\gb_live` remains unchanged; its drag/turn controller, distance-driven phase and two-bone IK maths are the basis.

| Control | Action |
|---|---|
| WASD / left stick | Move relative to camera |
| Shift / RT | Run |
| Hold Space / A, then release | Charge jump power, compress, push off |
| H | New procedural head scratch with finger motion |
| Ctrl / B | Roll; pressing in air queues a landing roll |
| MB4 (back side button) / F / Y | Automatic disarm and equip |
| Hold MB5 (forward side button) / E / LB | Both hands grip one reachable body surface |
| Alternate A/D taps while holding | Pump spin in one direction, with circular footwork |
| W/S / forward stick while holding | Move the pivot and carry the opponent |
| Release MB5 / E / LB | Remove grip and throw with existing momentum |
| LMB / RB | Alternating punches; left fist when the right hand carries a sword |
| V / D-pad down | Drop equipped weapon with hand velocity |
| RMB drag / right stick | Orbit camera |
| Mouse wheel | Zoom |
| R / X | Restore both fighters and sword |
| Tab / Start | Straight walking demonstration |
| F5 / F9 | Save / load `profiles/feel.json` |

The right panel has thirteen pages with 65 values. In addition to body firmness, limited air response, jump charge, ground anchoring, finger controls and RPG reference influence, `PAPER RIG` and `PAPER MOTION` expose the paper-derived hierarchy system: topology attention, chain coherence, leg symmetry, pose limits, temporal/acceleration regularization, fine-detail preservation, source recovery and graph radius. Save a preferred setup with F5 to share its exact values. Resetting fighters preserves these values, and older profiles remain compatible.

Procedural offsets now start from clean bind transforms before native animation evaluation each frame; corrections cannot accumulate. Pelvis bob and extra arm/body sway are reduced. Flight uses gravity with midpoint vertical integration for a ballistic arc; limited damped angular responses add secondary limb motion without altering the root trajectory. This is light joint response, not a fully dynamic player ragdoll. The green ground anchor/ray and purple takeoff arc expose the connection and trajectory.

RPG reference measurements come from `hero/fighter.glb` and its `.motion.json`: running/strafe and straight-punch clips are sampled for knee/elbow ranges, stride, support fraction and strike timing. Only aggregate parameters reach the procedural generators; the source clips never enter playback. `profiles/rpg_reference.json` records the measurements and source names. This is kinematic parameter extraction, not neural training or identification of masses/friction from animation. H generates a new head-relative hand path, bends calibrated phalange chains and corrects using the actual fingertip. Finger sliders also work outside the gesture, with automatic grip closure during combat.

The supplied Puppeteer and Make-It-Animatable papers now contribute a separate real-time pose layer. It builds a compact joint/parent graph in hierarchical order, calculates the canonical hip frame, precomputes skeletal graph distances, distinguishes body and finger detail, and applies connectivity/symmetry priors plus a closed-form source/velocity/acceleration pose fit. The imported Ranger weights remain authoritative and Bevy still performs FK/LBS. The papers' unavailable trained networks and Puppeteer's minutes-per-clip differentiable renderer are not placed in the frame loop. `profiles/paper_rig.json` records the discovered graph; `PAPER_MOTION_IMPLEMENTATION.md` maps the paper equations and design choices to code.

The stone strip behind the fighters is a sparse 0.25 m voxel-column path. Rendering, box colliders, foot heights and roll clearance use the same column data. The orange guide previews available roll travel. The yellow head-to-pelvis line and blue support lines expose the body relationship. Guide visibility can be toggled.

The player remains a controlled character, with a capsule when upright and a compact sphere during the roll. Requested ground acceleration is projected onto the terrain plane and bounded by Coulomb friction, including tangential gravity. Speed drives gait phase and the run blend; transition and pose curves are continuous. A head/torso lead precedes hip turning, turn rate and speed drive inward bank, and braking acceleration adds backward lean and knee compression. Landing estimates time to contact, extends the legs, then plants and recovers or rolls along the available ground path. Roll tucking and a skeletal clearance envelope keep the model above the sampled ground. A running jump keeps its horizontal takeoff velocity; only gravity changes flight velocity until contact, apart from the workspace boundary.

The 65 kg opponent becomes an articulated dynamic body on a grab or punch. Its capsules follow actual bone geometry, with hinge knees, bounded other joints and swept collision detection. A two-handed body grab uses one spherical constraint at the nearest shared reachable collider surface, with both hand targets on that surface. Angular acceleration uses an estimate of loaded inertia and available foot traction; moving the pivot and circular steps deliver momentum through the constraint. Grip force and overstretch can break the hold. Release removes only that constraint and anchor; it adds no throw impulse or replacement velocity. The HUD reports solver force, commanded spin and centre-of-mass release speed.

Disarm stays separate from the body grab: a timed wrist control and weapon strip transfers the sword into the player's equipment. Movement stays available during the sequence. An equipped sword is controlled from the hand and stowed for two-handed carrying or a roll; dropping returns it to dynamic simulation. Punches sweep the fist path against opponent capsules and apply a single impulse at contact. These are assisted character mechanics coupled to a dynamic opponent, not a full inverse-dynamics simulation of both fighters.

Changes are uncommitted. The previous combat controller source is saved in `staging/checkpoints/combat_before_motion_20261015_150623`. No automated, headless or scripted movement checks were run. Compilation establishes buildability; the normal viewer is for the user's movement and timing feedback.
