# Combat Motion Plan - Overgrowth as the reference

Status: **plan, nothing implemented.** Owner: boss (direction and scope). Implementation: Claude. Review: Aura, one pass, after Claude reports done.
Scope: combo structure, contact physics, animation. Sections 5 and 6 are the determinism contract and the debug/interaction protocol the boss asked for.

Evidence in this file was read from the project at the paths named, not carried in from memory. Where a claim is a design proposal it says so.

---

## 0. What this plan is, and what it is not

It is a build order for the three things MASTER §4 already specifies and the Unity tree has not built: a move table, a momentum-carrying contact model, and an inertialized pose pipeline with foot lock.

It is not a rewrite. The deterministic core in `Assets/Game/Combat/CombatWorld.cs` is worth keeping exactly as it is - fixed 1/120 s tick, index-ordered fighters, no randomness, no Rigidbody. Every phase below adds to it or layers above it; none of them replaces it.

The Sim step stays `Tuning.Step = 1f/120f`. Where MASTER §4 says `h = 1/60` (line 94), the Unity value is 1/120 and every formula that references h is evaluated at 1/120. Where §4 names Godot types (`Skeleton3D`, `PoseWriter`, `PhysicalBone3D`, `Jolt`), the Unity equivalent is named in §3 below.

---

## 1. Why Overgrowth

### 1.1 What the reference is used for

Overgrowth (Wolfire Games) is used here as an **idea and reading reference for grounded, momentum-carrying hand-to-hand**. Study is from public material and observation of how the fights read on screen. No Overgrowth file, asset or animation enters this project - see 1.4.

The reason it is the right reference and not, say, a generic AAA melee: Guildbound's stated target (`MASTER.md` §8, the D18/D20 decisions) is unarmed grounded combat at short reach, where the *reader of the screen* must be able to see commitment, impact and recovery. That is what Overgrowth's melee is built around, and its engine is public enough that we can reason about its structure rather than guess at it.

### 1.2 Five things worth taking

**1. Attack and reaction are authored as a pair, not independently.**
Overgrowth's fights read cleanly because a given attack and the defense that answers it interlock at a known contact frame - someone chose that pair, they were not blended. Guildbound today picks the reaction with one dot product:

```csharp
// Assets/Game/Anim/FighterAnimation.cs:87
rearHit = Vector3.Dot(source.Facing, contact.Normal) < 0;
```

That is a two-way guess (front/back only). The plan generalises it into an authored pair table: every move carries an explicit reaction per outcome (`Hit`, `Blocked`, `GuardBroken`, `Interrupted`) **and one for `Miss`**. Miss matters - see 1.3.

**2. The character is a body with a motor, and the pose is that body's output.**
Overgrowth keeps momentum across a hit; the animation follows the physics rather than driving it. Guildbound already declares this - MASTER L1 (motor is the sole translation authority) and L4 (ragdoll exits carry velocity), and `FighterAnimation.cs:196` enforces it by zeroing `transform.localPosition`. What is missing is the *middle*: there is no momentum to carry. See Phase 2.

**3. Procedural layers sit under authored clips.**
Foot planting, ground adaptation, look-at and reach are layered rather than authored per case. Guildbound has exactly one procedural layer, `Assets/Game/Anim/FighterProceduralPose.cs` (143 lines: lean, recoil, turn-lag, no foot IK), and MASTER §4.6 - the foot-lock phase machine - is specified and unbuilt. See Phase 3b.

**4. A symmetrical move is one move plus a side parameter.**
The project already does this once, deliberately: `Documentation/build/animation/ACTION_CLIPS.md:11` marks `Light2` as `Atk_P_2.fbx` with `Mirror: True`. Overgrowth's lesson is to make that systematic rather than a special case - mirrored moves keep identical timing, which is what makes a left/right string feel like one string. The move table in Phase 1 carries a side flag for this.

**5. Commitment is readable and the cancel is earned.**
Overgrowth's rush strings commit; you get out by connecting. Guildbound already has the mechanic - `f.landed && f.IsStrike && Phase == Recovery` allows a cancel into the next strike while a whiff or a block keeps full recovery. Overgrowth is the reference for *how tight that window is*, and for the fact that the window is per-move, not global. See `cancelAfterImpact` in Phase 1.

### 1.3 What not to take

Overgrowth's melee has a known failure mode: when a pair has no good entry, the fight reads as slippery flailing rather than impact. The cause is a reaction gap, not the procedural layer.

**The rule this yields for Guildbound: every move in the Phase 1 table must have a reaction entry for all five outcomes, `Miss` included.** A move with a missing branch is a bug the same way an unhandled enum case is a bug, and it should be caught by a check, not by playing it.

The second thing not to take is the temptation to make the procedural layer carry the read. Foot lock and inertia make a *correct* move look better; they do not make an authorless move look correct.

### 1.4 IP boundary - hard rule

`MASTER.md:85` already governs this and it is quoted here so it cannot drift:

> GPL sources in `reference/` are read for ideas only. Write from the math in section 4, not from their code. If an Apache-2.0 routine (Overgrowth) is adapted, keep its notice in the file and list it in LICENSES.md.

Consequences, stated bluntly because this is the one place where a mistake is expensive:

- **Overgrowth engine code may be adapted.** It is Apache-2.0. The condition is mechanical: the routine keeps its notice in the file it lands in, **and** a row goes in `LICENSES.md`.
- **`LICENSES.md` does not exist in this project.** `MASTER.md:85` and `:86` both require it. Creating it is Phase 0, before any adaptation, not after.
- **Overgrowth's game assets and animations are not Apache-2.0 and must never be copied.** The engine licence does not reach them. Same for anything from the Wolfire content packs.
- **Lugaru stays dropped.** `DECISIONS.md:76` (Decision 18, boss, 2026-10-11): "Lugaru animations DROPPED entirely. Lugaru is only an idea reference... never its GPL code or its files." This plan does not reopen that.
- **No Lugaru or Overgrowth file may be imported into `Assets/`.** Reading is not importing, and `Documentation/checks/design_check.py` already enforces an external-IP cleanliness pass (`C03`/`C06` in `Documentation/checks/last_report.txt`). Anything this plan produces has to keep those two checks green.

---

## 2. Where Guildbound stands

Read from the files. This is the gap the plan closes.

| Area | State today | Evidence |
|---|---|---|
| Combo structure | One line. Light presses alternate L/R. | `CombatWorld.TryStart`: `var kind = f.LastKind == ActionKind.LightL && (f.hasBuffer \|\| chain) ? ActionKind.LightR : ActionKind.LightL;` |
| Move metadata | None. No per-move cancel window, reaction class, knockback or mana gain. | grep `MoveDef\|ComboGraph` across `Assets/Game` -> no matches |
| Directional finishers | None. Decision 15's W/S + F rising/sweep does not exist in the core. | grep `rising\|sweep` in `Assets/Game/Combat` -> no matches |
| Contact physics | Kinematic teleport. Knockback is a fixed displacement over a fixed time; separation is a circle push. No mass, no impulse, no momentum. | `CombatWorld.Travel(f, dir, distance, time)`; `Separate()` at `2 * BodyRadius = 0.6 m` |
| Hit-stop | None per actor. | grep `hitstop\|hitStop` across `Assets/Game` -> no matches |
| Inertialization | None. `controller.CrossFadeInFixedTime` for normal transitions plus a hand-rolled 2-input chain blend. | `FighterAnimation.cs:143` and `:165-183` |
| Foot lock / foot IK | None. | grep `footLock\|FootIK` across `Assets/Game` -> no matches |
| Active ragdoll | None. No Rigidbody anywhere in the fighter path. | grep `Rigidbody\|ragdoll` across `Assets/Game` -> no matches |
| Clip metadata sidecar | None. Contact/follow-through are hand-typed floats per clip. | `FighterAnimationProfile.FighterClip { contact, followThrough, entry }` |
| Locomotion phase | Time-driven, so the feet slide. | `FighterAnimation.cs:157-160`: `loopTime += Time.deltaTime * (Velocity.magnitude / walkReferenceSpeed)` |

**What is already good and must not be regressed:** the fixed-step deterministic core, the rule-clock pose sampling (`FighterAnimation.Sample` fits source markers to rule-owned phases, and comments correctly that these do not authorize damage), hit-confirm cancel, the late-recovery input buffer (0.25 s), the one-hit-per-target-per-action dedup, the world-blocked-before-target hit order, and root-authority enforcement.

---

## 3. Target - MASTER §4 translated to Unity

MASTER §4 is the platform-independent source of truth and is written against the Godot tree. This table is the translation, and it is the only place the two vocabularies are allowed to meet.

| MASTER §4 | Godot-era terms | Unity translation here |
|---|---|---|
| §4.1 integration | `h = 1/60`, `physics_interpolation` | `Tuning.Step = 1/120f`; render interpolation is per-bone in the pose playable, not on the transform |
| §4.3 springs | exact-step damped spring, `(omega, zeta)` | already implemented as `FighterProceduralPose.StepSpring(position, velocity, frequency, dt)` - reuse it, do not write a second one |
| §4.4 XPBD | `WorkerThreadPool` substeps | not needed for Phase 1-3; revisit only if Phase 4 needs cloth/secondary |
| §4.5 two-bone IK | `Skeleton3D`, pole from last frame | analytic two-bone on hip->knee->ankle and shoulder->elbow->wrist, driven **after** the graph evaluates |
| §4.6 foot lock | SWING/PLANT/PLANTED/RELEASE, 0.25 m drift release | `Physics.Raycast` on the predicted foot, anchor in the hit collider's local space |
| §4.7 hit detection | swept capsule on the simulated pose, 3 sub-samples | extend the existing `SweepStrike`; today it sweeps `FistRadius 0.10 + BodyRadius 0.30` once per tick |
| §4.8 inertialization | Bollo quintic, log-space rotation offset | `AnimationScriptPlayable` offset buffer, see 3a |
| §4.9 pose pipeline | `PoseWriter`, `_process` | one `PlayableGraph` evaluated in `LateUpdate`, then procedural, then IK, then ragdoll replace |
| §4.10 active ragdoll | `PhysicalBone3D`, POWERED/LIMP | per-bone `Rigidbody` + `ConfigurableJoint`, stable PD toward the animated pose |
| §4.8 locomotion | phase `p += distance / stride_length` | replace the `loopTime` accumulator at `FighterAnimation.cs:159` |
| L8 | per-actor hit-stop | freeze that actor's `ActionState.Elapsed` advance - the pose freezes for free |
| L10 | SI units, `kg*m/s` | name new fields `massKg`, `impulseNs` per `MASTER.md:80` |

---

## 4. Phases

Each phase is independently landable and independently checkable. Nothing in Phase *n+1* is a precondition for Phase *n*'s check passing.

### Phase 0 - unblock (no design, do first)

1. **Create `LICENSES.md` at the repo root.** Required by `MASTER.md:85` and `:86` and absent today. Header row: name, source, licence, files it covers, what was adapted. Empty table is fine; it must exist before Phase 3a/3b land anything, because those are the two places an Apache-2.0 routine is plausibly adapted.
2. **Fix `AGENTS.md:11`.** It states Unity 6000.5.1f1 / URP 17.5. `ProjectSettings/ProjectVersion.txt` says **6000.3.26f1** and `Packages/manifest.json` says **render-pipelines.universal 17.3.0**. `CLAUDE.md:27` is correct. Until this is fixed, `AGENTS.md` is not an input to any shared context file.
3. **Save the open scene before running any builder.** `Assets/Game/Editor/WorldSceneBuilder.cs` lines 23-28 refuses when *any* open scene is dirty, and `Main.unity` is currently dirty immediately after its own save with 0 prefab overrides and no prefab instances. Saving is the fix; the flag is not an authored change.

### Phase 1 - the move table (`MoveDef` + branch resolution)

**Files:** new `Assets/Game/Combat/MoveDef.cs`, new `Assets/Game/Combat/MoveTable.cs`, edits to `Assets/Game/Combat/CombatWorld.cs` (`TryStart`), `Assets/Game/Combat/FighterSim.cs` (`LastMove` replacing `LastKind` as the branch key), `Assets/Game/Combat/Tuning.cs`.

**The table.** One static, immutable array. Deterministic by construction - it is read-only data, so the sim cannot branch on anything but its inputs.

```csharp
public enum ReactionClass { None, StaggerLight, StaggerHeavy, Launch, Knockdown }

public struct MoveDef
{
    public MoveId     id;
    public ActionKind kind;          // the contract kind the core still speaks
    public string     clipState;     // presentation key: "Light1", "Heavy1", ...
    public bool       mirrored;      // Decision: side is a parameter, not a second clip
    public float      startup, active, recovery;
    public float      reach;         // metres, fist tip sweep radius
    public float      damageScale;   // multiplies Stat, replaces the heavy ? 2 : 1
    public float      cancelAfterImpact; // seconds after impact a cancel is legal
    public ReactionClass reaction;
    public float      knockbackDistance, knockbackSeconds;
    public float      hitStopSeconds;
    public float      manaGain;

    public MoveId[]   lightBranch;   // resolved by chain depth
    public MoveId[]   heavyBranch;   // [neutral, up, down] per Decision 17
}
```

**Branch resolution**, per Decision 16 (dir captured at the heavy press) and Decision 17 (neutral finishers at N=0..4 = front/front/low/round/round; up = rising, down = sweep; light strings cap at four):

```csharp
// replaces the one-line alternation in TryStart
var next = f.hasBuffer || chain
    ? moves.Next(f.LastMove, press.Kind, press.Direction, f.ChainDepth)
    : moves.Entry(press.Kind);
```

`Next` is a pure function of `(MoveId, pressKind, direction, chainDepth)`. No randomness, no time, no allocation in the tick.

**Push-button changes:** `CombatWorld.Tick` keeps its exact signature. `Intent` gains a `Direction` field (neutral/up/down) captured at the press edge, per Decision 16 - the existing boolean press flags stay edges.

**Check (Phase 1):** a new `Tools/CheckComboTable.cs` (read-only, `public static string Execute()`, restores state in `finally`, same convention as `Tools/CheckFighterAnimationRuntime.cs`). It drives the deterministic sim headlessly and prints the resolved `MoveId` sequence for: (a) a four-light string, (b) each of the five heavy finisher entries N=0..4 at neutral, up and down, (c) a whiff into a buffered light. Writes `Documentation/checks/motion/C-M01.json`. Passes only if every move in the table is reachable and every `MoveDef` has all five reaction entries (1.3).

### Phase 2 - contact physics and hit-stop

**Files:** `Assets/Game/Combat/FighterSim.cs`, `Assets/Game/Combat/CombatWorld.cs`, `Assets/Game/Combat/Tuning.cs`.

**2a. A momentum channel.** Add to `FighterSim` a vector that is *separate from the motor's velocity*:

```csharp
public Vector3 momentumNs;   // kg*m/s, contact-driven only
public float   massKg = 70f; // Tuning default; MASTER.md:104 segment table is the later refinement
```

Contact writes an impulse, it does not teleport:

```csharp
// was: Travel(f, dir, distance, time)
var v0 = 2f * def.knockbackDistance / def.knockbackSeconds;   // constant deceleration over the window
f.momentumNs += dir * f.massKg * v0;
```

Each tick, before the motor integrates, the channel contributes displacement through the **existing** world constraint so it can never enter geometry - this reuses the constraint `Separate()` already sweeps from:

```csharp
var stepDisplacement = f.momentumNs / f.massKg * Tuning.Step;
f.position = Constrain(f.moveFrom, f.position + stepDisplacement);
f.momentumNs *= Tuning.MomentumDecay;   // per-tick, tune so the window matches def.knockbackSeconds
```

This is the change that makes hits feel like mass rather than a slide. It is also the change most likely to break determinism if written carelessly, hence §5.

**2b. Per-actor hit-stop (MASTER L8).** Add `public float hitStopRemaining;` to `FighterSim`. In the action-state stage, the *actor's own* rule clock does not advance while it is frozen:

```csharp
if (f.hitStopRemaining > 0f) f.hitStopRemaining -= Tuning.Step;
else f.action = Advance(f.action, Tuning.Step);   // unchanged otherwise
```

The animation needs no change at all from this: `FighterAnimation` samples non-looping states off `ActionState.Elapsed` (`FighterAnimation.Sample`), so the pose freezes exactly as long as the rules freeze. That is the whole reason the rule-clock design was worth keeping.

**2c. Reaction class drives the scripted branches.** `ReactionClass.Knockdown` still selects the existing `Knockdown -> GetUp` scripted path (`KnockdownTime 1.0`, `GetUpTime 0.6`). Phase 4 replaces *that* with the ragdoll; until then, nothing about it changes.

**Check (Phase 2):** `Tools/CheckContactPhysics.cs`. Asserts, deterministically: (a) per-tick momentum and cumulative displacement over a knockback window match the analytic `v0 = 2d/t` curve within 1e-4; (b) eight directions x four walls produce zero interpenetration and zero pass-through, tested by asserting the swept constraint never returns a position outside `ArenaHalfSize`; (c) a frozen actor's `Elapsed` does not advance while its attacker's does. Writes `Documentation/checks/motion/C-M02.json`.

### Phase 3 - animation

This is the phase where Overgrowth is doing the most work as a reference, and the largest phase by far. Land it as four separate deliveries so a failure in one does not block the others.

**3a. Inertialization replaces crossfade and the hand-rolled chain blend.**

MASTER §4.8 is explicit: "This is the default for every combat transition."

Replace both transition paths in `FighterAnimation` - `controller.CrossFadeInFixedTime` at line 143 and the `AnimationMixerPlayable(2)` chain overlay at lines 165-183 - with a single persistent offset buffer:

1. Evaluate exactly one clip per layer on the rule clock.
2. On a transition, capture `offset_0 = pose_old - pose_new` per bone (position delta; rotation as a quaternion difference in **log space**, shortest arc).
3. Decay with Bollo's quintic to zero at `t_blend`, matching value, velocity and acceleration at t = 0.

**The Unity nuance that will bite, stated here so it is not discovered in play mode:** Unity writes **local** bone rotations. Log-space quaternion differences are not additive on local rotations composed under a parent. The offset must be captured and decayed in each bone's **parent space**, and applied as a parent-space composition - not added to the world pose. `AnimationScriptPlayable` with a job body is the place for it; the offset buffer is a `NativeArray<AffineTransform>` sized to the humanoid bone count and allocated once.

**Payoff, and it is the reason to do this at all:** the `FighterChain` contract (`exitPhase` / `entryPhase` / `blendSeconds`) survives untouched as *timing data*, but the transition becomes interruptible at any frame with no pop, which removes the "a new change interrupts a live chain: settle on the current pose" fallback at `FighterAnimation.cs:106-110`. That fallback is where combos currently lose their impact pose.

**3b. Foot lock (`FootLock`), per MASTER §4.6.**

New `Assets/Game/Anim/FootLock.cs`. Four-state machine per foot: **SWING -> PLANT -> PLANTED -> RELEASE**.

- **PLANT** when the baked contact flag rises above 0.5 (from 3c, not a heuristic). Raycast from the **predicted** foot position - the sim pose forward-integrated by `Velocity * Tuning.Step`, not the render pose - and store the anchor in the local space of the collider it hit, so moving geometry carries the foot.
- **PLANTED**: hold the anchor with the §4.5 two-bone analytic IK. Pole vector = last frame's knee direction projected onto the plane perpendicular to the root->target axis; fall back to `source.Facing` when that projection is under 1 cm. This removes the near-straight knee flip.
- Drift above **0.25 m** releases early.
- **RELEASE**: blend anchor -> animated foot over **0.08-0.12 s** using the existing exact-step spring (`FighterProceduralPose.StepSpring`) - reuse it, do not write a second spring.
- **Pelvis offset**: lower by `min(0, lowest required leg reach deficit)`, spring it, never raise above the animated height.
- **Leg ownership** follows MASTER §4.9 lines 164-167, which already agree with Decision 12: grounded and still -> the action drives the upper body, stance owns the legs, FootLock pins the footprint; grounded and moving -> locomotion owns the legs; airborne -> the jump pose owns the legs and FootLock is off.

Ordering inside the frame: evaluate the graph, restore the authored pose, run procedural, **then** FootLock, then the pelvis offset. `FighterAnimation.LateUpdate:164` already restores the authored pose before procedural for exactly this reason.

**3c. `ClipMeta` sidecar - the bake (Decision 19 schema).**

Schema is already agreed and this plan does not change it: `clip, length, verified, travels, contacts.l/r (seconds), strikes (limb/start/impact/end in seconds), handover, notes`.

The bake tool samples each source clip and derives `contacts.l/r` from the foot bones' height and the rig root's relative motion, and `strikes` from the striking hand's forward reach - the same measurement `Documentation/build/animation/ACTION_CLIPS.md:25` already describes as hand-typed ("measured maximum forward reach of the striking hand, then first sample with 10% retraction... presentation candidates, not verified collision points"). The bake replaces the hand-typing and sets `verified: true`; anything unverified keeps the existing analysis fallback, per Decision 19.

This feeds 3b's phase machine and 3a's clip alignment. It does not authorize damage - `FighterClip.Sample` already carries the correct comment that source pose markers "do not authorize damage", and that stays true.

**3d. Locomotion phase - distance-driven, not time-driven.**

Replace the accumulator at `FighterAnimation.cs:159`:

```csharp
// was: loopTime += Time.deltaTime * (Velocity.magnitude / profile.walkReferenceSpeed);
phase += Vector3.Project(source.Velocity, source.Facing).magnitude * Tuning.Step / strideLength;
seconds = Mathf.Repeat(phase * clip.length / cycleDistance, clip.length);
```

Walk, jog and sprint clips are aligned by their `contacts` markers (3c) and blended by speed. This is MASTER §4.8's "changing between walk, jog and sprint keeps the feet in sync", and it is the direct fix for the foot slide.

**Check (Phase 3):** this one is a **capture, not a print**.
- `Tools/CheckFootLock.cs` (read-only) reports, for a 10 s driven run: frames PLANTED, max sole displacement while PLANTED, foot-anchor release count, and count of early releases. Writes `Documentation/checks/motion/C-M03.json`.
- `Tools/CheckChainBlend.cs` - **the dedup Claude already owes** (channel 22:05: "it runs each state on its own, does not exercise a chained blend, and so does not prove the Light1 -> Light2 snap"). Start `Light1`, reach `exitPhase`, press `Light2` inside the cancel window, and compare the pose at blend end against the same frame played unchained. Prints per-bone max delta. This is the check that closes the open snap claim, and it should land with 3a, not after.
- One Sparring capture at a fixed camera, 10 s, attached to the phase entry.

### Phase 4 - active ragdoll (POWERED / LIMP)

**File:** new `Assets/Game/Anim/FighterRagdoll.cs`, plus the handoff in `CombatWorld`'s reaction path.

MASTER §4.10, translated:

- States **ANIMATED** (no rigidbodies, the current behaviour) / **POWERED** (per-bone `Rigidbody` + `ConfigurableJoint`, stable PD toward the animated pose, per-bone weight) / **LIMP** (gains zero) / **RECOVER**.
- Heavy hit -> the hit chain goes POWERED at reduced gains for **0.2-0.4 s**.
- Knockdown -> LIMP with the capsule's linear and angular velocity carried over. This is L4, and it is why Phase 2's momentum channel should exist first: without it there is no velocity to carry.
- **Recovery**: pelvis speed below **0.3 m/s** for **0.2 s** -> classify face-up / face-down from the pelvis forward axis -> pick the matching get-up clip -> align that clip's first frame root to the pelvis -> **inertialize from the physical pose** (3a again, with the physical pose as the source). The capsule teleports to the pelvis ground point at the moment of recovery, never earlier.
- The existing `knockdown_A.fbx` / `getup_A.fbx` clips (`ACTION_CLIPS.md:22-23`) stay. The ragdoll replaces their traversed path, not their data.
- Unity specifics: `ConfigurableJoint` limits are hard under PhysX and drives are set through motor targets (MASTER §4.11 is the analogue); use stable PD per §4.3 line 117 so one `(omega, zeta)` works for a hand and a thigh; note Unity 6 renamed `Rigidbody.velocity` to `linearVelocity`.

This phase is the least urgent and the most likely to be cut. It is listed because §4.10 specifies it and the knockdown path is currently scripted, not physical.

**Check (Phase 4):** capture at a fixed camera plus `Documentation/checks/motion/C-M04.json` reporting velocity continuity across the ANIMATED->LIMP handoff (no discontinuity above a stated threshold) and no tunnel-through on the arena floor/walls.

### Phase 5 - the practice space (Decision 20)

Immutable pre-stream practice clearing blended into natural terrain, tree exclusion, static training landmarks. Built by a **new** builder in the `WorldSceneBuilder` pattern (`Assets/Game/Editor/`), not by editing `WorldSceneBuilder` itself - `Main.unity`'s Stage 1 output currently matches `WorldSceneBuilder.cs` lines 17-52 exactly and should not be disturbed by this work.

This is where the debug surface lives: a fixed camera rig, fixed spawn positions, and a known set of walls and steps so the Phase 2 and Phase 3 checks have a stable world to run against.

---

## 5. Determinism contract

The core's value is that the same inputs produce the same state. Phases 2 and 3 both push on that, so the contract is stated before the work and enforced as the first thing each check does.

**Rule:** any change to `CombatWorld` or `FighterSim` must be proven by deterministic replay, not by observation.

Add one method, `CombatWorld.StateHash()`, computing a documented hash over every fighter field in index order and every projectile, in order. Then:

- `Tools/CheckDeterminism.cs` replays a fixed input script of N ticks twice from a fresh world and asserts the hashes match at every 64th tick.
- Every Phase 1, 2 and 4 check calls `StateHash()` at the end and records it in its JSON. A check that changes the hash of an unchanged input script has changed behaviour, and the entry says so.

What must stay out of the tick: `Time.deltaTime`, `Time.time`, `UnityEngine.Random`, any `Dictionary` iteration order, and any float op whose operand order depends on scene order. Note that `FighterAnimation` currently reads `Time.time` for the guard-hit window (`FighterAnimation.cs:88`, `:219`) and `Time.deltaTime` for loop/defeat advance - that is legitimate *presentation* and it must stay on the presentation side of the boundary. Nothing in `CombatWorld` may ever read it.

---

## 6. Debug and interaction protocol

This is the second half of the boss's ask: a protocol where Claude opens a new tab carrying a message to be sent to Aura, and Aura knows what to do with it.

### 6.1 The four surfaces

| Surface | Path | Who writes | What travels |
|---|---|---|---|
| Coordination log | `collab/CHANNEL.md` | both agents | Decisions, file claims, Done/Review/Needs/Next |
| Decisions | `collab/DECISIONS.md` | both agents | Settled rulings with a number and a date |
| Machine-readable results | `Documentation/checks/motion/C-M0n.json` | whoever runs the check | Numbers, pass/fail, `StateHash` |
| Probes | `Tools/Check*.cs`, `Tools/*Probe.cs` | either agent | Read-only, `public static string Execute()` |

The split is deliberate and matches what already exists: `collab/` holds the agent-facing coordination state (`CHANNEL.md`, `TASKS.md`, `DECISIONS.md`), `Documentation/` holds the human-facing set plus the checks. Claude logs to `TANDEM_REASONING.log`, Aura logs to `collab/CHANNEL.md`; both are read cross-wise, and this plan does not change that.

### 6.2 Rules

1. **Announce before you touch the editor.** `PLAY <agent>` before Play, `PLAY FREE` after. Announce any scene change before making it, naming the scene and the one you are leaving. No parallel scene edits. No scene save during another agent's source import or Play window.
2. **A dirty scene is reported, not resolved.** Aura does not save or discard another client's unsaved work. The one documented exception is write-back noise, which has a test: memory overrides equal disk overrides by count and by `propertyPath` set, and the console shows the writer (the HudPreview case - TMP's `m_overflowMode` / `m_TextStyleHashCode` / `m_fontColor32.rgba` rewrite out of `TMPro.TextMeshProUGUI.Rebuild`). Anything else is reported and waited on.
3. **Probes are read-only and restore what they touched.** Every `Tools/Check*.cs` returns a string, changes nothing, and restores any profile or driver it swapped, inside a `finally` - exactly the shape `Tools/CheckFighterAnimationRuntime.cs` already has. A probe that writes is not a probe, it is an edit, and it goes in the channel as one.
4. **Every check writes its JSON and posts one channel entry** whose `Done` line contains the pass/fail verdict and the check id.
5. **Aura's review pass is the last step of each phase, and it verifies against disk, not against prose.** The method is the 22:05 pass: read the code, run the probes, compare live values to the file on disk, and report which claims held and which did not. Where a claim cannot be checked with an available tool, the entry says so rather than restating the claim.
6. **A claimed check that does not exist is not a passing check.** Prior example: `CheckFighterAnimationRuntime` "PASS on 13 states" was correctly reported as *not* proving the chained snap, because it runs each state in isolation. That is the standard.

### 6.3 Probe conventions

- Location: `Tools/`. Name: `Check<Subject>.cs` for a pass/fail check, `<Subject>Probe.cs` for a fact dump.
- Entry point: `public static string Execute()`. Run via `execute_script` with the file path.
- Output: one fact per line, `KEY value`, plus a final `PASS:`/`FAIL:` summary line and the `StateHash` where the sim was involved.
- Also write the same content to `Documentation/checks/motion/C-M0n.txt` so the result survives the console buffer.
- Current inventory that this extends: `Tools/CheckFighterAnimationRuntime.cs`, `Tools/CheckFighterProceduralPose.cs`, `Tools/HudDirtyProbe.cs`, `Tools/SceneStateProbe.cs` (the last two are read-only state probes and remain useful).

### 6.4 Hand-off block - to be pasted into a fresh Claude tab

The boss asked for this to be literal. Copy everything between the rules, including the header line.

---

**GUILDBOUND - MOTION PLAN HANDOFF (for Claude, new tab)**

Read, in this order, before doing anything else:
1. `Documentation/COMBAT_MOTION_PLAN.md` - this plan. Sections 1.4 (IP) and 5 (determinism) are constraints, not advice.
2. `Documentation/MASTER.md` §3.4 (open-source rules) and §4 (physics and animation).
3. `collab/DECISIONS.md` - Decisions 15, 16, 17, 18, 19, 20.
4. `collab/CHANNEL.md` - the last entry, currently Aura 2026-10-11 22:05.
5. `TANDEM_REASONING.log` tail - your own last entry.

Then state, in `collab/CHANNEL.md`, in this exact format:

```
## [claude] <YYYY-MM-DD HH:MM> - motion plan accepted, starting Phase <n>
Done: <the phase you are starting and the first file you will touch>
Review: <anything in the plan you disagree with, or "none">
Needs: <from the boss, or "nothing">
Next: <the check you will run and the surface it writes to>
```

Rules that apply from the moment you post it:

- `AGENTS.md:11` is wrong (it says Unity 6000.5.1f1 / URP 17.5). The project is **6000.3.26f1 / URP 17.3.0**. Do not use `AGENTS.md` as a source for engine or pipeline versions.
- **Overgrowth engine code only**, and only with its notice kept in the file **and** a row in `LICENSES.md`. Overgrowth and Lugaru **assets, animations and files are never copied**. `LICENSES.md` does not exist yet - Phase 0 item 1 creates it.
- `CombatWorld` stays deterministic. No `Time.*`, no `UnityEngine.Random`, no dictionary iteration in the tick. Prove every Phase 1/2/4 change with `StateHash()` replay.
- `Tuning.Step = 1f/120f` is fixed. Non-looping poses and blends advance on `ActionState.Elapsed`, never on render delta.
- The motor is the sole translator (`MASTER` L1). Root motion becomes desired velocity; it never moves the mesh.
- Announce `PLAY claude` before Play and `PLAY FREE` after. Announce any scene change before making it, naming the scene you are leaving.
- `Assets/Game/Editor/WorldSceneBuilder.cs` refuses when **any** open scene is dirty. `Assets/Game/Scenes/Main.unity` is currently dirty - save it before running any builder, or the run throws on its own output.
- Land one phase, then one channel entry, then stop. Do not start Phase *n+1* in the same pass as Phase *n*; Aura's review of Phase *n* is the gate.

When you post the entry, notify Aura. Aura's job from that point: read the code, run the probes in `Tools/`, verify every claim against disk, and post one `[aura]` entry with Done / Review / Needs / Next.

---

## 7. Open decisions for the boss

1. **Overgrowth as reference only, or adapt an Apache-2.0 routine?** This plan assumes *reference only* unless a specific routine proves worth the licence bookkeeping. If we do adapt one, say so up front so `LICENSES.md` is written before the code, not after.
2. **Does the four-light cap (Decision 16) still hold** once the table exists and finishers are per-N? A five- or six-entry string is a one-line data change.
3. **Jump chase and the launcher** are still marked adjustable (Decision 16). Phase 1's `heavyBranch[up] = rising` is the natural hook for a launcher; it stays grounded unless the boss opens it.
4. **Phase 4 (ragdoll) - in or out of this pass?** It is the largest phase and the least urgent; the knockdown path works today, just scripted. Cutting it does not affect Phases 1-3.

## 8. Order and gating

```
Phase 0  unblock (LICENSES.md, AGENTS.md, save the scene)     no gate
Phase 1  MoveDef + branch resolution         -> C-M01         gate: all five reaction entries exist
Phase 2  momentum channel + per-actor hit-stop -> C-M02       gate: StateHash replay identical
Phase 3a inertialization                     -> C-M03         gate: chain-blend pose delta at blend end
Phase 3b foot lock                           -> C-M03         gate: max sole drift while PLANTED
Phase 3c ClipMeta bake                       (feeds 3a/3b)
Phase 3d locomotion phase                    -> C-M03
Phase 4  active ragdoll (optional)           -> C-M04         gate: velocity continuity at handoff
Phase 5  practice space
```

Phase 3a should land before 3b: inertialization is what makes the foot lock's release blend land without a pop, and it is also the check that closes the open snap claim.

Phase 3c has no check of its own and no gate - it is data that 3a and 3b consume, and it is verified by their checks.
