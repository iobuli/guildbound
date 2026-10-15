# Alpha lessons, carried into the Bevy beta (2026-10-11)

Boss, 2026-10-11: the earlier builds were alphas, Bevy is the beta, and the beta is rebuilt with the context
from all of them.

| Build | Engine | Where it lives now |
|---|---|---|
| Alpha 1 | Godot 4 (GDScript, Jolt) | `_OldGodot/` zip (local, git-ignored); its defects are MASTER.md section 2, L1-L14 |
| Alpha 2 | Unity 6.3 LTS, two attempts | tag `unity-final`; this file, sections 1-6 |
| Beta | Bevy 0.20 (Rust), code only | branch `build/bevy` |

## 0. Alpha 1 (Godot) - the defects the beta must make impossible

MASTER.md section 2 is the full table; each row is a real defect and the rule that prevents it. In Bevy terms:

| # | Rule | Beta form |
|---|---|---|
| L1 | One translation authority | sim motor only; views copy sim state |
| L2 | One final pose writer per skeleton, fixed layer order | one ordered chain of pose systems after animation evaluation |
| L3 | Foot lock is a phase machine, anchors written only on PLANT | `FootLock` per foot, MASTER 4.6 |
| L4 | Every ragdoll/get-up exit inertializes from the captured pose | the inertialization system, MASTER 4.8 |
| L5 | Collision layers owned by state enter/exit | state transitions in the sim, one test each |
| L6 | Every controller state declares its animation contract | exhaustive `match` on the state enum: the compiler enforces it |
| L7 | Tests use public APIs, the runner times out | `cargo test` on public sim APIs |
| L8 | Hit-stop is per actor | per-actor clock in the sim, never `Time<Virtual>` |
| L9 | Swept limb capsules from the simulated pose + line of sight | sim hit detection, MASTER 4.7 |
| L10 | SI units; names carry the unit | `mass_kg`, `impulse_ns` |
| L11 | Rest lengths measured once, immutable | rig data loaded once into an immutable asset |
| L12 | IK pole from the previous knee plane, with fallback | two-bone IK, MASTER 4.5 |
| L13 | One design doc | MASTER GDD in `TANDEM_REASONING.log` + MASTER.md; this file only maps lessons |
| L14 | No god object, explicit dependency direction | sim crate depends on nothing; game crate depends on sim |

Also from alpha 1 (playtest measurements): the knee is a hinge; procedural kicks showed hip twist of 72-78 deg
against a human ~+-45; spatial combat rules are engine-independent: aim through the target, contact hold,
kicks step in before planting, conform to the ground.

---

Alpha 2: Unity was dropped on 2026-10-11 (boss: "we move to bevy... now we can only work in code").
The last Unity state is commit `0adfb47`, tag `unity-final` on `build/m0`. Read any old file with
`git show unity-final:<path>`, e.g. `git show unity-final:Assets/Game/Combat/CombatWorld.cs`.
Git-ignored Unity material (asset packs, Aura package, Library cache, stray "New Unity Project" folders)
was moved, not deleted, to `D:/New folder (4)/guildbound-unity-archive/bevy-move-2026-10-11/`.

Design of record is unchanged: the MASTER GDD block in `TANDEM_REASONING.log`, `Documentation/MASTER.md`
section 4 (engine-independent physics/animation math), `collab/DECISIONS.md`, and
`Documentation/COMBAT_MOTION_PLAN.md` (phases 0-5; the Unity type names in its section 3 need the
Bevy translation below).

## 1. Architecture that worked - keep it

- **Deterministic fixed-step core.** `CombatWorld` ticked at 1/120 s, fighters in index order, no randomness,
  no engine physics, no `Time.*` in the tick. 64/64 fixtures passed at 30/60/120 fps render rates.
  In Bevy: make the sim a plain Rust crate with no Bevy dependency, so `cargo test` drives it headless.
- **The motor is the only thing that moves a fighter** (MASTER L1). Root motion becomes desired velocity;
  it never moves the mesh. The view copies sim position and zeroes local offsets.
- **Rule-clock pose sampling.** Non-looping poses are sampled from `ActionState.Elapsed`, not render time.
  Payoff: per-actor hit-stop freezes the pose for free, and the pose can never run ahead of the rules.
  Source clip markers are presentation only; they never authorize damage.
- **Presentation reads the sim, never writes it.** Interfaces between them (`ICombatPresentation`,
  `IHudModel`, `IInventoryModel`) kept the sim testable and the HUD swappable.
- **Combat rules worth porting as-is:** hit-confirm cancel (cancel only on hit, in Recovery), late-recovery
  input buffer 0.25 s, one hit per target per action, world-blocked checked before target, bounded
  wind-up retarget, bounded turning, run speed carried into strikes.
- **Plan before code** (`COMBAT_MOTION_PLAN.md`): move table with all five reactions per move
  (`Hit/Blocked/GuardBroken/Interrupted/Miss`), momentum channel + per-actor hit-stop, then
  inertialization, foot lock, ClipMeta bake, distance-driven locomotion, optional ragdoll.
  The determinism contract (`StateHash()` replay) applies unchanged.

## 2. Animation lessons

- **Chain-blend teleport (found by measurement, not by eye).** Holding the outgoing clip at a fixed
  `exitPhase` on handover rewinds it when the cancel lands later than that phase: one-tick jump out and back.
  Fix was `hold = max(exitPhase, currentTime)`: handover accel 0.0583 -> 0.0339 m/tick^2. The remaining
  gap (limit 0.0174) is the cross-clip pose difference.
- **Blending on the rule clock fixes WHEN a blend ends, not WHERE the pose is.** Two time-to-pose
  mappings meeting at a join will disagree. The real fix is inertialization (MASTER 4.8: capture
  `pose_old - pose_new` per bone, decay with a quintic). Do it in each bone's **parent space**;
  log-space rotation offsets are not additive across a hierarchy.
- **Locomotion phase must be distance-driven** (`phase += distance / stride`), not time-driven, or feet slide.
- **Joints:** the knee is a hinge; the visible fault in procedural kicks was hip twist 72-78 deg against a
  human ~+-45. IK pole = last knee direction projected off the root->target axis, fallback to facing.
- **Mirrored moves are one move plus a side flag**, so left/right strings keep identical timing.
- **Dead fields mislead.** `FighterChain.entryPhase` and `FighterClip.entry` were documented and never read.
  Implement a field or delete it in the same change.

## 3. Verification lessons

- A check must exercise the exact path it claims. Per-state "PASS on 13 states" did not cover the chained
  blend, and the snap was still there.
- Measure a snap as an acceleration spike against neighbouring ticks, not as speed across clips.
- Measure pose in the rig's local space, so root motion does not pollute a pose metric.
- State a falsifiable prediction before the re-run; never tune a threshold to pass. If the criterion changes,
  record why.
- Make sure a measurement has samples: "max seam gap 0" over 0 clusters measured nothing.
- Verify claims against disk, not against someone's summary. Several Aura/Claude claims were retracted
  this way (base-layer weight, variable timestep, MonoBehaviour vs plain C# assignment).

## 4. Why Unity cost us time (what Bevy removes)

- Editor state outside the code: dirty scenes, 393 prefab overrides, TMP write-back noise, builders
  refusing on a dirty scene, PLAY/FREE handoffs between agents sharing one editor.
- Import settings as hidden behaviour: humanoid "Bake Into Pose", loop-blend flags, and `unpack.py`
  overwriting our edited pack `.meta` files.
- Asset Store ownership: Magic Arsenal effects referenced materials from a pack we did not own (magenta).
- Two engines' worth of vocabulary in one doc. Keep one translation table (below) and nowhere else.

## 5. New risks in Bevy (Claude's list; settle before porting art)

- **No humanoid retargeting.** Unity let any Humanoid clip play on any Humanoid rig. Bevy clips are bound
  to a skeleton's bone names/hierarchy. Pick one hero rig and retarget clips offline (Blender) before import.
- **Formats.** Bevy loads glTF/GLB. Pack FBX content has to go through Blender.
- **Licence of pack assets outside Unity: UNKNOWN.** Check the Asset Store EULA terms for each pack before
  any converted mesh or clip enters the Bevy repo. Until checked, treat pack assets as reference only.
- **API churn.** Bevy breaks APIs between minor versions. Pin one version in `Cargo.toml` and verify names
  (system sets, animation types) against that version's docs, not memory.
- **Toolchain.** Rust is not installed on this machine yet (`cargo` not found, 2026-10-11).

## 6. Unity -> Bevy translation (the only place the two vocabularies meet)

| Unity (tag `unity-final`) | Bevy |
|---|---|
| `Tuning.Step = 1/120`, manual tick | `FixedUpdate` with `Time<Fixed>` at 120 Hz, or the sim crate ticked directly in tests |
| `CombatWorld` / `FighterSim` (plain C#) | plain Rust sim crate, no Bevy types; Bevy wraps it in a resource |
| `PlayableGraph` + mixer, `SetTime` from the rule clock | `AnimationGraph` clip/blend nodes, weights set per node, active animation seeked to the rule time |
| `AnimationScriptPlayable` inertialization | a system ordered after Bevy's animation evaluation that adds a decaying per-bone offset to `Transform`s |
| `MonoBehaviour.LateUpdate` ordering | explicit system ordering in `PostUpdate` |
| EditMode fixtures, `Tools/Check*.cs` probes | `cargo test` on the sim crate; checks as tests or small bins writing `Documentation/checks/*` |
| uGUI HUD (`Assets/Game/UI/Kit`) | `bevy_ui`; the HUD layout spec in `LAYOUT.md` and colours (AMEND-UI-01) still apply |
| `Physics.Raycast` for foot lock | own `HeightField` queries (already engine-free in `Assets/Game/World/HeightField.cs`) |
| `Rigidbody` + `ConfigurableJoint` ragdoll | physics crate choice deferred to Phase 4 |
