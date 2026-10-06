# Handoff: finish movement fixes + build hand-to-hand combat and lock-on

Read this first in a new session. It is the full context of where the project stands and what is next.

## The job ("all or nothing")

1. **Complete fix** of the physical character:
   - get-up works from any landing (flat, slopes, stairs, against walls) with no snaps and no clipping
   - climbing legs press the wall (feet as contacts), natural falling pose
   - stairs per the user's model (below), foot sliding cut, locomotion pops cut
2. **New: hand-to-hand melee combat + lock-on fighting system**, built on the same physical body.

The design workflow script is in `tools/workflows/guildbound-design.js` (diagnose 4 defect areas in worktrees, three
independent combat designs, three judges, a synthesis, and a completeness critic). Run it with the Workflow tool, read the
results, then run an implementation workflow (parallel tracks in worktrees, then integrate and verify with the self-test).

## User's non-negotiable direction

- The whole character is a solid physical object: nothing clips through anything (world, props, its own limbs).
- Prefer maths-driven / procedural motion over a canned clip firing every time. Clips may be a style layer.
- Joints are fixed pivots (rotation only). Feet have no transform except pivoting at the heel / ball contact.
- **Stairs**: walk toward a step, the leg lifts and goes ON TOP of the step. If the player stops, that foot stays there.
  Walking back puts it back on the floor. Body height comes from the planted feet, not a capsule teleport.
- Ragdoll keeps real momentum. The get-up starts from wherever the body lies, contacts grounded, and is universal
  (any landing angle, slopes, stairs, walls).
- Climbing: wide arm spread, palms are the contact points. Gliding is removed.
- Efficient colliders. Work quietly, verify with numbers, report results not every step.
- Never buy packs or create accounts. Free downloads are fine.

## Running the game and tests

The project is Godot **4.7.2** (Jolt physics, set in `project.godot`).

Windows (the user's PC): `D:\Godot\Godot_v4.7.2-stable_win64_console.exe`.

Linux / cloud:

```bash
curl -L -o godot.zip https://github.com/godotengine/godot/releases/download/4.7.2-stable/Godot_v4.7.2-stable_linux.x86_64.zip
unzip -o godot.zip && export GODOT="$PWD/Godot_v4.7.2-stable_linux.x86_64"
"$GODOT" --headless --path . --import          # builds the .godot cache (gitignored); also needed after adding a class_name script
```

- Self-test (headless, 6-10 min for everything): `"$GODOT" --headless --path . -- --selftest [--only=a,b]`
  Test names: parity, run, slopes, cliff, gap, swim, carry, fall, getup, hotbar, slide, pops, roll, climbanim,
  stairsanim (gdbg and debug only when asked). Lines are `[PASS]/[FAIL] name - detail`; diagnostics start with three
  spaces. Filter the harmless Jolt "Cone twist" warnings with `grep -v`.
- Visual contact sheets need a display: `-- --getupshots` (docs/getup_sheet.png), `-- --animshots` (docs/anim_sheet.png).
  On Linux try `xvfb-run "$GODOT" --rendering-driver opengl3 --path . -- --getupshots`.

GDScript rules that bite: tabs; typed code; a variable inferred from a Dictionary / Array / Variant element needs an
explicit type (`var x: float = d["k"]`) or the script will not parse; SkeletonModifier3D already has a property named
`active`.

## Architecture (game/)

| File | Role |
|---|---|
| player.gd | CharacterBody3D kinematic capsule state machine GROUND/AIR/CLIMB/SWIM/RAGDOLL/RECOVER; stamina, step-up teleport + visual easing, dodge, climb/mantle, swim, slide, carry/push, ragdoll, get-up hand-off. Reads only its `intent` (intent.gd: `Intent.from_input(cam)` for the local player, an `ai` child or outside code for others); `Player.fighters` registry; works without camera (`is_ai`). |
| character_visual.gd | AnimationTree from Quaternius UAL1/UAL2 + CMU clips; drives MotionIK; owns BodyRig, GetUpPose, RagdollPose; `clip_pose()`. |
| motion_ik.gd | Foot IK + foot locking/stepping, pelvis shift, landing / hit springs, torso lean, push hands, climbing palms. |
| get_up_pose.gd | Procedural get-up: `plan(face_down, floor_pt, fwd)` builds contact stages (face down: palms, all fours, kneel up, half kneel, stand; face up: sit up on palms, squat, stand); phase one blends from the exact ragdoll pose; limbs are two-bone IK on contacts; terrain fit; knee floor guard; also bends the spine while ragdolled. |
| body_rig.gd | The physical body: 15 rigid parts (box pelvis/chest/hands/feet, ball head, capsule limbs), pins, velocity drive toward targets with strength 0..1, ragdoll with ConeTwist limits + self collision, `recover(self_collide)`, grandparent collision exceptions, target extrapolation between drawn frames (get-ups only). Layers WORLD 1, PLAYER 2, BODY 4, PROPS 8. |
| ragdoll_pose.gd | Last skeleton modifier: records the pose as body targets, then draws the skeleton FROM the physics bodies. |
| camera_rig.gd, hud.gd, input_setup.gd, arena.gd | Orbit camera, HUD, named input actions, test arena (ramps, stairs at x -8, 20 m cliff, pit, pond, props). |
| stats.gd, char_model.gd | MOBA-style stat engine (flat, add_pct, more, caps, slow): max_hp, attack_damage, attack_speed, armor (taken = 100/(100+armor)), crit_chance, lifesteal, tenacity, carry, stamina... shared with the Python sim in `sim/`. |
| selftest.gd, anim_probe.gd, anim_shots.gd | Scripted checks, pop/slide metrics, contact sheets. |

Combat controls already specified in `docs/CONTROLS_AND_PHYSICS.md` §2.2 and §4: LMB light combo (hold = charged), RMB guard
+ 150 ms parry, MMB lock-on + wheel cycles targets, Alt dodge + perfect-dodge Flurry, 60-90 ms hit-stop, knockback reduced
by Tenacity, strafe-circle while locked, Space side-hop while locked.

Free clips available for combat: Punch_Jab, Punch_Cross, Melee_Hook(+_Rec), Hit_Chest, Hit_Head, Hit_Knockback,
Idle_Shield, Sword_Block, Shield_Dash, Death01. No kicks in the free packs (CMU mocap can be imported with
tools/bvh_import.gd).

## Status at handoff

- Full self-test: 105 checks, 94 pass (combat foundation added the `selfcol` checks). All 11 failures are get-up checks.
- Get-up problems left: phase-one "snaps" when a tangled ragdoll untangles (varies run to run); shin catching step edges
  on stairs; cliff-wall landings can plan contacts into the wall (and the --getupshots camera ends inside the rock);
  occasional late foot-landing snaps.
- Locomotion diagnostics: dodge end pop 0.45-0.78 m, push start 0.6-0.72 m, jump onto wall 0.62 m, swim 0.38-0.52 m,
  after throw 0.43-0.48 m; planted-foot sliding: slow walk 0.76 m/s, crouch walk 1.0-1.3, push 2.9-6.5, stairs down
  3.4-4.1, turn in place 1.7-2.6 m over 4 turns.
- Play-test: climbing legs hang limp; the falling pose is a stiff wide frog.
