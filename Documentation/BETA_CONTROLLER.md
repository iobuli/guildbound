# Bevy beta: RPG hero controller and presentation

The beta uses Bevy 0.19.1 and the RPG Character skeleton/model. The local hero
library combines 40 native RPG clips, 42 FightingMotions clips and seven Kevin
Iglesias mage clips retargeted
offline onto that same skeleton. Converted pack content remains ignored by git.

## Controller contract

- `crates/sim` owns fighter translation, facing, gravity, jump preparation,
  dash/roll, collision bounds, separation and combat permissions at 120 Hz.
- `sim_bridge.rs` collects local press edges once, retains them through local
  hit-stop, and accepts scripted intents through the local remote API.
- `controller.rs` maps actual velocity relative to facing to eight walk/run
  directions. A shared gait phase advances by distance / measured clip stride.
  Releasing input retains the braking gait until the body stops.
- `anim.rs` samples committed actions from their rule time. Locomotion uses
  distance; preview/block/jump poses use time actually advanced by the sim.
  Inertialization and procedural layers stop with the actor's clock.
- Root locks reconstruct current parent transforms instead of using previous
  frame globals. Committed poses anchor pelvis XZ to the motor in world space,
  retaining authored height/rotation. This fixes the nearly one-metre extra
  travel from a rotating roll root. Inertialization caches the corrected pose.
- During a jump, the motor supplies height and the clip's root height is removed.
  Grounded clip bob remains presentation. The physics prop proxy follows height.
- Menu/focus loss cancels held charge/guard without a projectile. Reset discards
  old contacts, queued input, scripts and removed targets, retaining live tuning.

This is a controller with mapped motion, not a claim of completed foot IK,
foot locking or a production character mesh.

## Mapping and audit framework

`Tools/blender/hero_mapping.json` is the explicit bone, role and eight-direction
clip map shared by the converter and runtime. The left hand is actually named
`B_L_Forearm.001` on this rig; that name is intentional and verified on disk.
Unknown mapped bones fail conversion; missing runtime roles emit a warning.

The retargeter preserves the target's world scale when solving a pose. The old
unit-scale solve inflated child transforms on a centimetre rig and made the
default mapped idle lie on the floor. Each bake now isolates the active action
from previously collected NLA export tracks and resets the target pose.

Rebuild and audit from the repository root:

```powershell
D:/blender.exe -b --factory-startup -P Tools/blender/build_hero.py -- --archive "D:/New folder (4)/guildbound-unity-archive/bevy-move-2026-10-11/Assets" --out assets/packs/hero/fighter.glb
D:/blender.exe -b --factory-startup -P Tools/blender/check_hero_mapping.py -- --glb assets/packs/hero/fighter.glb --out Documentation/checks/live/hero-mapping-audit.json
```

The audit samples nine poses in every exported animation, checks finite
transforms and unit bone scales, requires upright idles, and verifies every
semantic/locomotion/combat/spell role. It also samples 65 poses in each of the 37
mapped attack/cast clips, measuring the named fist/foot's forward extension relative
to the pelvis. Root travel is excluded. These are presentation landmarks, not
new damage rules. It writes measured root travel and contact landmarks to the ignored
`fighter.glb.motion.json` sidecar. Other packs without an audited sidecar use
the existing reference-speed fallback.

## Presentation framework

- `arena.rs`: a quiet practice courtyard, four entrances, boundary markings and
  paired guild marks, constructed from primitives and sized from arena tuning.
- `feedback.rs`: one outcome profile for each hit, block, counter hit, guard break
  and parry. Camera and contact marks share it. Camera shake can be reduced to
  zero in the dev panel and ignores unrelated mob exchanges.
- `hud.rs`: health/guard/energy, live spell affordability, charging, counter
  opportunity, contact outcome and a rematch action. It reads rules and returns
  input requests. It does not write fighter state.
- Menu: implemented Spar, Practice, Resume/New spar and Quit actions. Spar locks
  onto a partner who fights back; Practice keeps the partner idle. Tab opens the
  dev panel. The normal play view starts with debug overlays hidden.

Both the world and overlay camera use HDR. Local before/after captures confirmed
that this fixes the black world behind the menu. A related mixed-camera failure
is documented in [Bevy's issue tracker](https://github.com/bevyengine/bevy/issues/18901).

## Verification

```powershell
cargo test --workspace
cargo build -p guildbound
python -I Tools/check_beta.py
python -I Tools/check_combat.py
python -I Tools/check_spells.py
```

Run the last command with the beta open. It exercises every walk/run direction,
paused pose stability, jump/fall/landing, roll, strike contact and menu charge
cancellation. It writes screenshots and a JSON result under
`Documentation/checks/live/`, then returns to the menu.

`gb.presentation` is a read-only pose probe with clip, seek time, gait phase,
bone count, finite-transform flag and pose hash. `gb.state` includes height,
vertical speed, action, guard, energy and the active screen. These are observations
for verification, not a second simulation.

## Windows liveness check

A main-thread stack overflow invalidated the earlier "left running" status.
Increasing the original 1 MiB reserve was insufficient: the boss narrowed the
trigger to MB1, and inspection found an infinite hover-event dispatch loop in
bevy_lunex 0.7.0's `observer_event_duplicator`. Its child event retains the
parent entity as its target, so it dispatches the parent again indefinitely.

Our menu now applies `UiHover.enable` directly to the button's finite subtree
and never emits the problematic `UiHoverSet`. A headless regression runs 100
over/out pairs with the library's state plugin loaded. Decorative panels ignore
picking. No library cache or dependency source was patched.

`crates/game/build.rs` retains an 8 MiB reserve for the Windows MSVC game
executable, using the documented
[MSVC /STACK option](https://learn.microsoft.com/en-us/cpp/build/reference/stack-stack-allocations).
That budget is not a workaround for recursive event dispatch; the event loop
itself is removed from our menu. No simulation or animation tuning changed.

With the beta open, run `python -I Tools/check_beta_liveness.py --seconds 90 --cycles 30`.
This verifies the built executable's actual reserve, sends the actual
`Pointer<Over/Out/Click>` events via `gb.menu_pointer`, cycles menu/play, then
polls the API through the observation period. Failure to respond fails the
check. Evidence is saved to `Documentation/checks/live/beta-liveness.json`.

Latest corrected build: 30 cycles / 300 pointer events and 90.39 seconds of
responsive API polling passed. Workspace tests: 51. Controller/combat/mage live
checks: 20 + 9 + 3. Captures include world-space roll correction, physical
knockdown and mage launch poses. These checks cover the current slice; completed
foot locking and full anatomical ragdoll tuning are not claimed.

Controls: WASD movement, Shift sprint, Ctrl step, Shift+Ctrl homing dash (8 ki),
Space jump, C roll, LMB rush, RMB guard/parry, F/Mouse4 smash (hold to charge),
W/S+smash rising/sweep, Mouse5 lock, 1-4 spells, hold Q energy, Esc menu,
Tab dev panel, P pause, period single-step, I inventory placeholder.

## Rush, smash and contact matching

Light rush links jab -> cross -> hook -> uppercut. Recovery can cancel on a
landed contact; an early queued press waits until that gate opens. Whiffs remain
committed. A heavy ends the chain: neutral chooses front kick / low kick /
roundhouse by depth, forward chooses rising, backward chooses sweep. Hold the
heavy for up to 0.7 seconds to increase damage and push. A full charge breaks
settled guard and knocks down; a timely parry still wins. A parry opportunity
selects the separate counter blow. Homing dash needs a living lock target,
distance and 8 ki; its queued attack waits until evasion ends. Without enough
energy, Ctrl still gives the ordinary free step.

Ten mapped combat clusters rotate 32 native/retargeted attack clips. The same
prepared clip is retained across held-smash release. `motion_timing.rs` maps
anticipation, extension, peak, withdrawal and settle onto the rule's startup,
active midpoint, active end and recovery. A spell's punch uses those landmarks
at projectile release too. `gb.presentation` reports the active contact markers.
`motion_math.rs` supplies a closed-form critically damped spring for aim and
lean; tests compare 30/60/120 fps and long-frame stability. Short attack blends
finish before contact. Contact marks use head/body/low height.

These are our timings and mechanics, inspired by BT3 rather than a recreation
of its frame data. The publisher manual's energy-cost Dragon Dash is a reference:
[BT3 instruction manual](https://manuals.plus/m/7862bb23b21864250a275bc624669a32dd3a5b07690c142f66cd4613247f8df6).

Fireball uses a two-handed mage cast with a separate charge/load and charged
release. Lightning alternates left/right one-handed mage casts. Burst has its
own mage clip, and energy charge has casting idle. `hero_mapping.json` includes
the verified Kevin bone map and exact source FBXs. Kevin's animated B-root
contains a 100x unit conversion and a quarter-turn absent from its bind bones;
the converter removes this before mapping anatomical rest axes (head/hips and
foot/toe), independent of bone roll. This prevents horizontal bodies, reversed
casts and huge hip travel. The extended hero has 89 clips and 37 measured
attack/cast landmarks.

Charge -> cast previously reused its action sequence, which could leave the
presentation sampling the charge pose. The driver now observes action kind as
well as sequence. Fireball release commits its 0.22-second cast preparation and
launches once at that point; it no longer skips the cast pose on a quick tap.
Projectile clips map measured peak extension to the launch instant. Charge
orbs attach to the final animated hand positions after pose propagation.
`Tools/check_spells.py` verifies charged/tap fireball, both lightning hands,
no early projectile, exact launch sampling and damage, with captures.

## Physical knockdowns and procedural pipeline

`ragdoll.rs` creates 11 dynamic Avian capsules and 10 limited ball joints from
the RPG skeleton's actual bone positions. Mass distribution, friction, damping
and capped incoming momentum drive the collapse. Capsules collide with ground
and props; they exclude self-collision and fighter proxies. Each physical body
reconstructs the corresponding bone in current parent space, preserving scale.
Recovery eases the last physical pose into the authored get-up over 0.28 seconds.
Reset and model changes remove transient bodies/joints. Unsupported rigs keep
their authored reactions.

Pipeline: motor/rules -> phase-matched clips -> inertialization -> world root lock
-> bounded procedural aim/lean -> physical knockdown override -> transform
propagation. Avian advances by the actual sim time with substeps at most 1/120 s,
so menu pause, speed and explicit stepping apply to physics as well. `gb.physics`
reports active bodies/joints, finite poses, joint error, speed and frozen hashes.
The live combat check covers four-hit rush, three charged finishers, guard
break, dash rush, physical knockdown stability/pause/get-up and charge cancellation.

This follows the blend of animation and physical collapse described in
[Wolfire's ragdoll-knockout notes](https://www.wolfire.com/blog/2010/10/Ragdoll-knockouts/),
implemented with our own code and [Avian joints](https://docs.rs/avian3d/0.7.0/avian3d/dynamics/joints/struct.SphericalJoint.html).
Ragdolls are a physical presentation layer: damage, targeting and gameplay
position stay in the deterministic sim. Anatomical elbow/knee hinges, self
collision, terrain foot IK and physical locomotion are not complete.
