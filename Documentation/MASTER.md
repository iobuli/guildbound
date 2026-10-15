# FreeGuild (Guildbound) - Master Document

Single source of truth for the ground-up rebuild. Everything older lives in `_archive/docs/` and is history, not instruction.
Owners: Claude (sections 1-4, 6, 7), Codex (section 5). Change a section you do not own only through `collab/CHANNEL.md`.

Contents
1. Vision
2. Lessons from the old builds
3. Architecture and code standards
4. Physics and animation (graduate appendix)
5. Inventory, crafting and economy (Codex)
6. Asset pipeline
7. Roadmap and proof gates
8. Combat look and grounded combat
9. Elements (Fire, Electric)

---

## 1. Vision

FreeGuild is a third-person anime fantasy action sandbox that can grow toward an online RPG.

- **Institution:** the Hunter's Guild. Contracts, the Hunter's Card, ranks F, E, D, C, B, A, S, a guild market.
- **Peoples:** humanoid races only (humans, elves, dwarves, orcs, beastfolk). Growth is earned, not bought.
- **Combat feel:** Budokai Tenkaichi 3 / Sparking Zero rhythm on the ground: fast strings, directional finishers, chase, guard, parry, guard break, knockdown and recovery. Readable, weighty, never floaty.
- **Economy feel:** RuneScape-style: gather, craft, trade; materials come from the world and from hunting; gold has real sinks.
- **Elements:** Fire and Wind first. Elemental potions grant buffs and elemental XP; affinity unlocks Element Manipulation later.
- **World:** a large procedural region generated from a seed, streamed in chunks, tuned for an RTX 3060 at 1080p / 60 fps.
- **Platform:** Godot 4.7.2, Jolt physics, Forward+ renderer, Windows desktop first.

Out of scope until stated otherwise: networking, flying, races beyond the base humanoid, player trading.

## 2. Lessons from the old builds

Each line is a real defect from the archived code or its test logs. The new code must make each one structurally impossible or covered by a test.

| # | What went wrong | Root cause | Rule in the rebuild |
|---|-----------------|------------|---------------------|
| L1 | Visual model drifted away from the collider | Two sources moved the body (root motion and velocity) | One translation authority: the CharacterBody3D motor. Root motion is converted to a desired velocity, never applied to the mesh. |
| L2 | Late pose modifiers overwrote strikes | Several systems wrote bone poses in any order | One final pose writer per skeleton, fixed layer order (section 4.9). |
| L3 | Walking feet supported only 33% of samples, 11.6 cm gaps | Foot anchors overwritten every tick; planner returned early above 0.12 m/s | Foot lock is a phase machine with anchors written only on PLANT (section 4.6). |
| L4 | Get-up popped a joint 0.993 m in one tick | Blend started from the clip pose, not the current simulated pose | Get-up and every ragdoll exit inertialize from the captured pose (section 4.8). |
| L5 | `stun()` during KNOCKDOWN left collision layer 0, fighter intangible | State transitions edited collision ad hoc | Collision layers are owned by state enter/exit hooks; one test per transition. |
| L6 | CLIMB/MANTLE showed idle or run poses | Controller states had no animation contract | Every controller state declares its animation contract; a test asserts no state falls through. |
| L7 | Five of six tests referenced removed interfaces and a run hung instead of failing | Tests reached into internals, no runner timeout | Tests use public APIs only; runner has a global timeout and exits non-zero. |
| L8 | Hit pause via `Engine.time_scale` froze everything | Global time used for a local effect | Hit-stop is per actor. |
| L9 | Inflated hitboxes, hits through walls | Volumes not tied to limbs; no occlusion | Swept limb capsules from the simulated pose plus a line-of-sight check (section 4.7). |
| L10 | `shove()` passed a velocity change and called it an impulse | No units | SI units throughout: kg, m, s, N, N s. Names carry the unit when ambiguous. |
| L11 | Limb lengths remeasured every tick, proportions crept | Mutable rest data | Rest lengths measured once at load and are immutable. |
| L12 | Skin flipped on near-straight limbs | Pole from a fresh cross product of near-parallel vectors | Pole vectors come from the previous frame's knee/elbow plane with a fallback axis (section 4.5). |
| L13 | Docs, README and code disagreed | No single owner | This file is the only design doc; README points here. |
| L14 | God object scene root; fighter reached into its parent | No dependency direction | Systems talk through typed signals and injected references; nothing calls `get_parent()` for game logic. |

## 3. Architecture and code standards

### 3.1 Layout
```
src/core/     Events (signal bus), Settings, Save, GameClock         (autoloads)
src/world/    WorldGen, ChunkStreamer, TerrainChunk, Foliage
src/actor/    Actor (motor), Command, PlayerInput, Brain
src/combat/   MoveDef, ComboGraph, CombatState, HitSweep, Resolver, HitStop
src/anim/     PoseSampler, Inertializer, Locomotion, FootLock, ProceduralLayers, PoseWriter
src/items/    ItemDef, Inventory, RecipeDef, Market                   (Milestone 2)
src/ui/       HUD, menus
data/         .tres definitions (moves, items, recipes, ranks)
tests/        headless suites, one file per system
tools/        Python + Blender batch scripts
```

### 3.2 Dependency direction
core <- world, actor <- combat, anim <- ui. Lower layers never reference higher ones. Cross-layer notifications go through `Events` or signals on the emitting object.

### 3.3 GDScript rules
- Static typing everywhere (`var speed: float`, typed arrays, typed returns). Enable `untyped_declaration` warnings as errors in project settings.
- Definitions are `Resource` subclasses with `@export` fields, stored as `.tres` in `data/`. Runtime state is plain objects or Dictionaries that serialize to JSON with `schema_version`.
- One script, one responsibility. A script past ~300 lines is a design smell, not a rule.
- `_physics_process` for simulation at a fixed 60 Hz; `_process` only for presentation (interpolation, UI, VFX).
- No `get_parent()`, `get_node("../..")` or groups for game logic. Inject references in `setup()`.
- Use `class_name` for every reusable type. No magic strings for state: use enums.
- Units in names where ambiguous: `impulse_ns`, `mass_kg`, `angle_rad`.
- Performance: no allocations in per-frame hot loops (reuse `PackedFloat32Array`, preallocate). Use `WorkerThreadPool` for generation, never touch the scene tree from a worker thread.

### 3.4 Open-source rules
- New code is MIT. Each new file starts with no license header; the root LICENSE covers it.
- GPL sources in `reference/` are read for ideas only. Write from the math in section 4, not from their code. If an Apache-2.0 routine (Overgrowth) is adapted, keep its notice in the file and list it in LICENSES.md.
- Third-party assets require a LICENSES.md row before first use.
- Commits are small and describe behaviour. Every commit passes the headless test suite.

### 3.5 Tests
`Godot_v4.7.2-stable_win64_console.exe --headless --path . --fixed-fps 60 -- --test` runs every `tests/test_*.gd`, each a `RefCounted` with `func run(t: TestCtx)`. The runner enforces a per-suite frame budget and a global timeout, prints a table, exits 1 on any failure. Visual checks write contact sheets to `captures/`.

## 4. Physics and animation (graduate appendix)

Notation: positions x, velocities v, angular velocity w, mass m, inertia tensor I, time step h = 1/60 s, gravity g = 9.81 m/s^2. Quaternions q are unit, composed left-to-right in world space.

### 4.1 Time integration and interpolation
Physics runs at a fixed h. Rendering runs at a variable rate and draws the state at alpha = accumulator / h between the previous and current physics states: x_draw = lerp(x_prev, x_cur, alpha), q_draw = slerp(q_prev, q_cur, alpha). Godot's `physics_interpolation` does this for nodes; skeleton poses are not interpolated by Godot, so PoseWriter stores the previous and current pose arrays and interpolates bone transforms in `_process` (old bug: `presentation_point()` skipped this).

Integrator: semi-implicit (symplectic) Euler, v += a h; x += v h. Explicit Euler (x first) gains energy every step on oscillators and must not be used for springs. Symplectic Euler conserves a shadow Hamiltonian, so springs neither explode nor die artificially.

### 4.2 Rigid bodies
For a body with mass m and body-frame inertia I_b, world inertia is I = R I_b R^T. Newton-Euler: m dv/dt = F, I dw/dt + w x (I w) = tau. An impulse J at contact point p (lever arm r = p - x_com) changes velocities by dv = J / m and dw = I^-1 (r x J). The gyroscopic term w x (I w) matters for long thin limbs spinning fast; Jolt includes it.

Segment masses for a 70 kg humanoid (Winter, anthropometric tables, rounded): pelvis 10%, abdomen+chest 33%, head 8%, upper arm 2.8%, forearm 1.6%, hand 0.6%, thigh 10%, shank 4.6%, foot 1.4% each. Approximate each limb as a capsule; I about its long axis is (1/2) m r^2, across it (1/12) m (3r^2 + L^2).

### 4.3 Springs, dampers and PD control
A damped spring on error e = x_target - x gives a = k e - c v. Writing k = omega^2 and c = 2 zeta omega:
- zeta = 1 critically damped (fastest settle, no overshoot), zeta < 1 overshoots.
- Natural frequency omega in rad/s; settle time ~ 4 / (zeta omega).

Stability under semi-implicit Euler requires omega h < 2 (for zeta = 0) and in practice omega h < 1 with damping. At 60 Hz that caps omega near 60 rad/s. The old solver used gains up to 2600 per unit mass, i.e. omega ~ 51, and combined them with 4 position iterations, effectively multiplying stiffness: that is why the body jittered. Rule: specify springs by (omega, zeta), never by raw gain, and clamp omega h <= 0.8.

For unconditional stability use the exact-step damped spring (Ryan Juckett / Daniel Holden "spring-damper by half-life"): with y = x - x_target, j0 = y0, j1 = v0 + j0 zeta omega, for zeta = 1:
y(t) = (j0 + j1 t) e^(-omega t), v(t) = (v0 - j1 omega t) e^(-omega t).
This is used for every presentation spring (lean, flinch, camera) because it is exact for any h.

Angular PD for an active-ragdoll joint: tau = k_p * axis_angle(q_target * q^-1) - k_d (w - w_target), scaled by the effective inertia about the joint so the same (omega, zeta) works for a hand and a thigh: k_p = I_eff omega^2, k_d = 2 I_eff zeta omega. Use stable PD (Tan, Liu, Turk 2011): evaluate the error at the next step's predicted state, which permits higher gains without blowing up.

### 4.4 Constraints: PBD and XPBD
Position-based dynamics projects positions to satisfy constraints C(x) = 0 after prediction. XPBD (Macklin, Mueller, Chentanez 2016) adds compliance alpha~ = alpha / h^2:
dlambda = (-C - alpha~ lambda) / (sum_i w_i |grad_i C|^2 + alpha~), dx_i = w_i grad_i C dlambda, with w_i = 1 / m_i.
Unlike plain PBD, the stiffness of XPBD does not depend on iteration count or time step. The old Lugaru-style solver used equal weights and 4 iterations, so stiffness changed with every tuning pass. Any custom constraint (cloth, hair, secondary motion) uses XPBD with substeps (4-8 substeps of 1 iteration beat 1 step of 8 iterations).

### 4.5 Inverse kinematics
Two-bone analytic IK (thigh-shank, upper arm-forearm). Given lengths a, b and target distance d (clamped to [|a - b| + eps, a + b - eps]), the interior angle at the middle joint by the law of cosines is theta = acos((a^2 + b^2 - d^2) / (2ab)). The chain plane is fixed by a pole vector. Choose the pole as the previous frame's knee direction projected onto the plane perpendicular to the root-target axis, falling back to the character's forward axis when that projection is shorter than 1 cm. This removes the near-straight flip (L12).

Foot orientation: align the sole to the ground normal n with the minimal rotation from the animated sole normal to n, limited to 30 degrees, then restore the animated yaw about n. Toes use a second small rotation from the toe probe.

FABRIK for longer chains (spine, tail, hair): iterate forward and backward placing joints at fixed lengths, converge in 2-4 iterations, then convert positions to rotations with swing-only alignment to keep twist from the animation.

Joint limits: decompose q = q_swing * q_twist about the bone axis; clamp twist angle and the swing cone separately (ellipse cone for shoulders and hips).

### 4.6 Foot locking and ground contact
Each foot runs a phase machine driven by baked contact curves from the clip (section 6): SWING -> PLANT (contact rises above 0.5) -> PLANTED -> RELEASE (contact drops) -> SWING.
- On PLANT: raycast the predicted foot position, store the anchor in the local space of the collider it hit (so moving platforms carry it).
- While PLANTED: IK holds the foot at the anchor. If the anchor drifts more than 0.25 m from the animated foot (sharp turn, shove), release early.
- RELEASE: blend from the anchor back to the animated foot over 0.08-0.12 s with the exact spring (4.3).
- Pelvis offset: lower the pelvis by min(0, lowest required leg reach deficit), then spring it, so both legs can reach. Never raise above the animated height.
The capsule handles step-up and slopes; feet only adjust the presentation. Step height and the visual stride are independent (fixes stair cycle restart).

### 4.7 Hit detection
Each strike limb is a capsule (radius 4-6 cm) on the simulated pose, not the interpolated draw pose. Between ticks it sweeps from the previous to the current segment: test the swept volume (approximated by the capsule at 3 interpolated sub-positions when the tip moved more than its radius) against hurt capsules. This is continuous collision detection in practice; without it a 12 m/s kick tip moves 20 cm per tick and tunnels through a forearm. A ray from attacker chest to contact point rejects hits through walls. Each move hits a victim at most once per active window.

### 4.8 Animation blending
Crossfade lerps two playing clips, which needs both to keep running and causes foot sliding and averaged-out impacts. Inertialization (Bollo, GDC 2018; Holden 2021) switches to the target clip immediately and decays the offset between the old output pose and the new one:
offset_0 = pose_old - pose_new (per bone: position difference, and rotation as a quaternion difference in log space, with the shortest arc), velocity from the last two frames, then offset(t) decays with a critically damped spring or Bollo's quintic that matches value, velocity and acceleration at t = 0 and reaches zero at t_blend.
Only one clip is evaluated per layer, transitions can be interrupted at any frame without pops, and combo segments keep their exact impact poses. This is the default for every combat transition. Ragdoll exits (L4) inertialize from the captured physical pose with its velocity.

Locomotion uses a continuous phase p advanced by distance / stride_length, so changing between walk, jog and sprint keeps the feet in sync; clips are aligned by their contact markers and blended by speed.

Root motion: extract the root's horizontal displacement per tick from the clip, rotate it into the actor's facing and feed it to the motor as desired velocity d / h (L1). Vertical root motion only during authored jumps.

### 4.9 Pose pipeline (per physics tick)
1. Controller decides state and command.
2. Motor moves the capsule (only translation authority).
3. Sampler evaluates base locomotion and the action layer.
4. Inertializer applies decaying transition offsets.
5. Procedural layers: lean, look-at, guard IK, flinch springs, strike overshoot.
6. Foot lock and pelvis offset.
7. Physical layer (when active): active ragdoll drives toward this pose; result replaces it.
8. HitSweep reads this final simulated pose.
9. PoseWriter stores it; `_process` interpolates and writes the Skeleton3D once.

Leg ownership during actions (agreed Claude/Codex, 2026-10-10):
- Grounded and still: the action drives the upper body; its pelvis CHANGE since the strike began is layered on the stance pelvis (hips initiate); legs stay on the stance and FootLock pins the stance footprint, rear heel may pivot about the ball of the foot.
- Grounded and moving: locomotion owns the legs; the action drives the upper body only.
- Airborne: the jump pose owns the legs; FootLock is off.

### 4.10 Active ragdoll and handoff
States: ANIMATED (no bodies), POWERED (PhysicalBone3D bodies driven by stable PD toward the animated pose, weight per bone), LIMP (gains zero), RECOVER. On a heavy hit, switch the hit chain to POWERED with reduced gains for 0.2-0.4 s; on knockdown go LIMP with the body's linear and angular velocity carried over (L4, velocity was dropped before). Recovery: wait until the pelvis speed is below 0.3 m/s for 0.2 s, classify face-up or face-down from the pelvis forward axis, pick the matching get-up clip, align its first frame's root to the pelvis, then inertialize from the physical pose. The capsule teleports to the pelvis ground point at the moment of recovery, never earlier.

### 4.11 Jolt notes
Jolt uses sequential impulses with soft constraints and position correction via Baumgarte-free split impulses; it ignores some Godot soft-limit properties on Generic6DOF joints, so joint limits are hard and drives are set through motor targets. Use 2 velocity and 1 position iteration minimum for ragdolls; enable CCD (`continuous_cd`) on small fast bodies (hands, feet, projectiles). Contact friction is Coulomb with combined coefficient sqrt(mu_a mu_b).

### 4.12 World generation and performance
- Heightfield h(x, z) = fBm of `FastNoiseLite` (OpenSimplex2, 5 octaves, lacunarity 2, gain 0.5) plus a ridged layer for mountains, warped by a low-frequency domain warp. Same seed gives the same world.
- Chunks 64 m x 64 m, 65 x 65 vertices at LOD0, halved per LOD (3 LODs). Shared edges use skirts to hide cracks between LODs.
- Generation runs on `WorkerThreadPool`; meshes are built with `SurfaceTool`-free `ArrayMesh.add_surface_from_arrays` from packed arrays; nodes are added on the main thread with a budget of 4 ms per frame.
- Collision: `HeightMapShape3D` only for the 3 x 3 chunks around each actor.
- Foliage: one `MultiMeshInstance3D` per chunk per species, density from a noise mask and slope, `visibility_range_end` per LOD.
- Rendering on RTX 3060: Forward+, occlusion culling on, SDFGI off by default (option), shadows 2 cascades at 120 m, mesh LOD on.
Budget: 16.6 ms per frame total, physics under 4 ms, world streaming under 4 ms.

## 5. Inventory, crafting and economy

Status: Milestone 2 design; runtime implementation pending. Based on the accepted task 3 outline and Claude's five requested changes in CHANNEL.md (2026-10-10 13:35). Milestone 1 contains schema/stub boundaries only. Numbers and new field names below are rebuild proposals; this document describes no existing runtime implementation.

### 5.1 Player loop and scope

Hunt or gather -> collect materials -> return to guild -> craft consumables -> keep supplies or sell surplus -> complete contracts and earn standing -> access harder work and better services. Gathering/crafting proficiency is dependable progression. Elemental affinity/rarity remains its own system; this proposal adds no affinity rolls or unlock thresholds. Guild rank measures institutional standing, not combat power.

First Milestone 2 slice: one pack, equipment pane, four consumable shortcuts, one guild vendor, one alchemy station, eight item definitions and three recipes. Contracts can supply rewards through the same inventory transaction boundary. Banking, player trading, auction house, durability/repair, random affixes and offline market simulation are deferred. Equipment uses authored item definitions and the combat modifier boundary; it does not change animation timing directly.

### 5.2 Inventory and UI

- Open Pack with I, close with I/Escape. Left: equipment silhouette. Centre: 6 x 4 grid (24 slots), category filters and search. Right: selected item's name, icon, quality text, quantity, effects and actions. Footer: gold, occupied slots, four quick slots. Keep every action usable by click or keyboard/controller selection.
- Equipment slots: head, torso, hands, legs, feet, main_hand, off_hand, trinket. Empty main_hand selects the unarmed moveset. Weapon definitions carry moveset_id and stat_mods; the committed loadout selects its combat moveset only while action-legal. Two-handed items occupy both hand slots with one actual item instance; the second slot shows a linked reservation. Races share these logical slots initially.
- Materials stack to 99, consumables to 10, equipment to 1. Every item occupies one grid cell; no rotated shapes or carry-weight penalty in this slice. Stack only identical definition, quality, binding and instance modifiers. Unique equipment never merges.
- Drag/drop moves, merges or swaps; equip validates the destination and space for displaced equipment. Secondary action menu offers Use/Equip, Split, Assign shortcut and Discard as applicable. Discard asks for confirmation; protected contract items cannot be discarded or sold. Sorting is explicit and stable, never triggered by pickup.
- Quick slots select an item definition and quality, not a fragile grid position. They consume the first matching eligible stack by slot order. Show total eligible count; an empty selection remains assigned and greyed out after consumption.
- Loot fills matching stacks then empty slots. Accept only what fits, show the accepted amount and leave the remainder in the loot source. Buying/crafting either completes the requested quantity or fails with the exact reason and no losses.
- Shop view shows owned amount, stock, buy/sell price and quantity 1/5/Max. Craft view shows available/required ingredients, station, fee, output quality and capacity. Disabled actions explain missing materials, gold, rank or space.
- Opening a modal releases mouse capture and suppresses gameplay inputs. On close, require held combat buttons to be released before a fresh press. Pause only in the local solo mode; eventual multiplayer keeps the world running. Item use and equipment changes follow shared action legality, so a modal cannot cancel an attack or bypass recovery.

### 5.3 Items, recipes and quality

Starting values below are gold per item, before quality, stock and rank adjustments. Gold is an integer wallet value, not a pack item.

| Proposed item ID | Type / stack | Source | Base value |
|---|---|---|---:|
| herb | material / 99 | gathering, guild supply | 8 |
| spring_water | material / 99 | water source, guild supply | 2 |
| glass_vial | material / 99 | guild supply | 4 |
| ember_fragment | material / 99 | Fire creature loot | 20 |
| gale_fragment | material / 99 | Wind creature loot | 20 |
| healing_draught | consumable / 10 | alchemy, guild supply | 36 |
| fire_infusion | consumable / 10 | alchemy | 80 |
| wind_infusion | consumable / 10 | alchemy | 80 |

| Proposed recipe ID | Inputs for one batch | Output | Station / gold fee |
|---|---|---|---|
| brew_healing | herb x1, spring_water x1, glass_vial x1 | healing_draught x1 | alchemy / 2 |
| brew_fire | herb x1, spring_water x1, glass_vial x1, ember_fragment x1 | fire_infusion x1 | alchemy / 2 |
| brew_wind | herb x1, spring_water x1, glass_vial x1, gale_fragment x1 | wind_infusion x1 | alchemy / 2 |

Initial vendor sells herbs, water, vials and healing draughts; fragments are earned through hunting. Vendor accepts ordinary surplus and crafted potions subject to its stock/buyback limits.

Initial potion behaviour proposal: healing restores 30% maximum health over 3 seconds, with a shared 20-second healing cooldown. Infusion duration is a 60-second placeholder owned by the elemental section. Proposed one-infusion replacement shows confirmation before starting item use. Replacement policy, magnitude, cooldown and unlock progression are pending elemental design. Attempting to heal at full health or use an item in an illegal action state consumes nothing. All consumption follows section 5.4.

Quality tiers: Standard, Refined, Masterwork. Start with Standard only. Later Refined/Masterwork recipes are explicit variants unlocked by crafting proficiency, requiring extra graded inputs, with guaranteed previewed output rather than a random roll. Proposed price multipliers: 1.00 / 1.15 / 1.30. Consumable magnitude cap: 1.00 / 1.10 / 1.20; duration/cooldowns unchanged. Equipment quality uses per-definition bounded modifiers, not a universal stat multiplier. Element rarity (Common through Epic) is separate and does not increase merely because an infusion is well crafted.

Crafting is immediate and atomic at the correct nearby station while action-legal. Select exact input stacks before execution. Simulate ingredient removal and output insertion together, so consuming the last ingredient in a full pack can free a valid output slot. Reject insufficient space without charging fees. No chance failure, timed queue or cancelled-craft losses in this slice.

### 5.4 Item use is a combat action

Item use joins CombatState (section 3.1) and the action layer of the pose pipeline (section 4.9). Its gameplay clocks advance on the fixed simulation tick (section 4.1); rendered interpolation never triggers consumption or advances cooldowns.

EffectDef owns use timing, voluntary cancellation, movement scale and cooldown. Proposed first-slice action tuning: healing use time 1.2 s, infusion use time 1.0 s; both allow voluntary cancellation during the first 0.3 s and allow movement at 0.35 of normal speed. These new timings require play review. A null cancel_window_s disables voluntary cancellation; it does not prevent a hit from interrupting use.

1. Start only when the shared action rules permit item use and an eligible item exists. Reserve one unit in its exact stack without removing it. Block split, sell, discard, transfer or crafting operations involving that reserved stack while use is pending.
2. Enter the use action. No healing, infusion, consumption or cooldown occurs on entry. Movement uses the action's scale; attack, dodge, boost and equipment changes cannot bypass its action rules.
3. Voluntary cancel is legal from action start through cancel_window_s, inclusive. Hit interruption, defeat and actor removal cancel at any point before completion. Cancellation releases the reservation and consumes nothing.
4. At use_time_s completion, process incoming hit interruptions first on that simulation tick. Revalidate the reservation, ownership, action and cooldown, then atomically consume one item, apply the effect, start the cooldown and increment affected revisions. A duplicate completion request has no effect.
5. Healing revalidates that health is below maximum at commit; if it is full, release the reservation with no item loss or cooldown. Infusion replacement consent is recorded before starting use; if the existing infusion changes during use, cancel rather than silently replace a new effect.

Healing uses a healing cooldown group of 20 s and restores 30% maximum health over 3 s. Quality can scale the healing magnitude within the previously stated cap. Fire/Wind effect magnitude, cooldown and elemental progression remain owned by the elemental section. Their 60 s duration is a placeholder owned by that section; historical data had 75 s and unlock_xp 64, which the elemental design must reconcile. Historical values are not authoritative rebuild defaults.

A solo inventory modal may pause the simulation, but a started use action runs after closing that modal. Quick-slot use starts the same action. Save/reload cancels pending uses with no consumption and persists already committed effects and cooldowns. Cooldowns and effect expiration use the saved gameplay clock.

### 5.5 Economy, supply and guild rank

Item sources: resource nodes, creature drops, vendor supply and contract rewards. Item sinks: consumable use, crafting inputs/vials, explicit disposal and NPC surplus removal. Gold sources: completed contract rewards and capped NPC buyback payments. Gold sinks: NPC purchases and station fees. Selling an item is not itself an item sink until the NPC removes it from circulation. NPC treasury replenishment is a controlled currency source and must be counted.

Stock responds to purchases and accepted sales. Each stock entry has a positive target_stock and a stock_cap of 2 x target_stock; accepted items not initially stocked for purchase use target_stock 50 and cap 100. Let target_stock be positive and stock range from 0 to 2 x target_stock. The proposed supply multiplier is:
s = clamp(1 + 0.10 x (1 - stock / target_stock), 0.90, 1.10).
This gives a small scarcity signal without making ordinary consumables unaffordable. Evaluate from the quoted pre-transaction stock; freeze the unit price for that quantity. Restock on persisted gameplay-time intervals, never on menu open, reload or chunk regeneration.

Suggested starting vendor settings: target stock 50 for each supply material, 10 for healing draughts; buyback cap 100 per other accepted item; buyback gold budget 250 per 10 gameplay minutes. Reset remaining budget to 250 at an interval, without accumulating unused budget. Only vendor-supplied items replenish to target; hunting fragments and crafted-only infusions are never spawned by restocking. Accepted surplus above target is removed toward target at the interval; below-target buyback-only stock remains unchanged. These are balancing knobs. Loot sources have persistent claim state; sparring/reset/debug actions give no economy rewards.

The Hunter's Card uses letter ranks F, E, D, C, B, A, S. Guild progression owns promotion requirements; standing comes from completed guild work, never repeated buying/selling. Access labels below are proposed unlock mapping.

| Rank ID | Order | Buy discount bps | Sell bonus bps | Access |
|---|---:|---:|---:|---|
| F | 0 | 0 | 0 | basic supplies and all three basic recipes |
| E | 1 | 100 | 100 | introductory harder contracts |
| D | 2 | 200 | 200 | established hunter contracts |
| C | 3 | 300 | 300 | specialist stock |
| B | 4 | 400 | 400 | refined recipe access |
| A | 5 | 500 | 500 | advanced contracts |
| S | 6 | 600 | 600 | masterwork recipe access |

Recipe proficiency requirements still apply; guild access alone does not grant crafting mastery. Essential healing remains accessible at F rank.

For base item value v, quality price factor q and rank discount/bonus d/b:

- Player buys for ceil(v x q x s x (1 - d)) gold per unit.
- Player sells for floor(v x q x s x 0.35 x (1 + b)) gold per unit.
- Craft station fees receive no rank discount in the first slice.

Use integer fixed-point factors (10,000 basis points = 1.00), round only the final per-unit price, then multiply by quantity. A quote carries stock/rank/inventory revisions; changed state requires a new displayed quote before spending.

Example at target stock: an F-rank hunter pays 8 + 2 + 4 + 2 fee = 16 to brew a healing draught, whose sell value is 12. At S rank and the most favourable stock extremes, bought inputs cost at least 7 + 2 + 4 + 2 = 15; potion sell value is at most 14. Thus the basic merchant -> craft -> merchant route loses gold. Hunting materials can create positive income; that is intended.

Audit every recipe/rank/quality against minimum input purchase cost and maximum output resale value before adding it. Also audit currency rounding, recipe cycles and buy/resell across shops. Future quality variants need new inputs and this audit, rather than blindly multiplying the existing output value. Contracts require one-time claim IDs and cannot reward the same completion twice.

### 5.6 Data definitions and mutable state (schema v1)

These are new design contracts, not existing runtime interfaces. Definition field names below are proposed for the rebuild. Defs are authored Godot Resources (.tres), treated as immutable at runtime. Agreed directories: data/items, data/recipes, data/ranks and data/qualities. EffectDef is also a Resource; its directory is UNKNOWN until the project skeleton chooses it. Do not create runtime Resources during this design-only task.

Mutable state serializes to JSON with schema_version = 1 at the document root. IDs are stable strings; item/recipe/effect IDs use lowercase names, while rank IDs are uppercase F through S. References must resolve before loading. Optional fields are explicitly null. Mutable state stores definition IDs, never live Nodes or paths to UI controls.

#### Immutable definition contracts

| Record | Fields and constraints |
|---|---|
| ItemDef | id: unique string; display_name: string; category: material/consumable/equipment/contract; icon: asset reference or null until sourced; max_stack: integer >=1; base_value_gold: integer >=0; sellable: bool; discardable: bool; allowed_qualities: nonempty list of quality IDs; equip_occupies: equipment slot list; effect_id: EffectDef ID or null; moveset_id: authored moveset ID or null; stat_mods: list of authored modifier references, empty initially. |
| QualityDef | id: standard/refined/masterwork; display_name: string; price_bps: positive integer; consumable_magnitude_bps: positive integer. Starting pairs: 10000/10000, 11500/11000, 13000/12000. |
| RecipeDef | id: unique string; station_id: string; min_rank_id: RankDef ID; proficiency_requirement: {skill_id, level>=0} or null; inputs: nonempty list of {item_id, quantity>=1, minimum_quality_id}; outputs: nonempty list of {item_id, quantity>=1, quality_id}; fee_gold: integer >=0. |
| RankDef | id: F/E/D/C/B/A/S; order: unique integer 0..6; buy_discount_bps: 0/100/200/300/400/500/600 by order; sell_bonus_bps: same; access_ids: list of authored access references. Promotion gates belong to guild progression data. |
| EffectDef | id: unique string; kind: heal_over_time/elemental_infusion; use_time_s: number >=0; cancel_window_s: number 0..use_time_s or null; movement_scale: number 0..1; cooldown_group: string; cooldown_s: number >=0; duration_s: number >=0; magnitude: kind-specific record; element_id: authored element ID or null. |

Item constraints:

- Equipment has max_stack = 1 and nonempty equip_occupies. Non-equipment has an empty equip_occupies list, null moveset_id and empty stat_mods.
- For main_hand/off_hand equipment, moveset_id selects an authored combat moveset where applicable. Shield-only off_hand can have null while an authored pairing rule chooses sword_shield. Greatsword occupies both hands with one record and selects greatsword. Combat must validate the final loadout as a pair.
- Empty main_hand selects unarmed. Unsupported combinations fail equip validation instead of falling back silently. Combat reads moveset on a committed equip change only while action-legal, never midway through an action. stat_mods use authored modifiers; they do not write animation timing directly.
- Consumables require a valid effect_id. Contract items have sellable = false and discardable = false. Equipment and material effect_id is null in this slice.
- The first three recipes accept at least standard-quality inputs, produce standard output, require rank F and have no proficiency gate. Prefer the lowest eligible quality unless the player chooses exact higher-quality ingredients. Preview consumed qualities.
- Healing magnitude record: {max_health_fraction: 0.30}, constrained to 0..1. Healing element_id is null, duration_s = 3 and cooldown_s = 20.
- Elemental magnitude fields and elemental cooldown_s are UNKNOWN until the elemental section specifies them. Document examples may use null to show this unresolved decision, but incomplete EffectDefs must not become enabled runtime assets. The elemental section must also resolve replacement behaviour and 60 s placeholder duration before implementation.

#### Mutable state contracts

| Record | Fields and constraints |
|---|---|
| ItemRecord | instance_id: unique string; item_id: ItemDef ID; quantity: integer 1..max_stack; quality_id: allowed QualityDef ID; binding: none/character; bound_owner_id: string or null; modifier_ids: ordered list of authored instance modifier IDs, empty initially. Equipment quantity =1. |
| InventoryState | owner_id: string; revision: integer >=0; capacity: 24; records: ItemRecord list; pack_slots: array of 24 instance IDs or null; equipment: map of the eight slot IDs to instance ID or null; quick_slots: array of four {item_id, quality_id} selections or null. |
| WalletGuildState | owner_id: string; revision: integer >=0; gold: integer >=0; standing: integer >=0; rank_id: RankDef ID; claimed_contract_ids: unique string list. Gold and standing are independent. |
| VendorState | vendor_id: string; revision: integer >=0; stock: list of {item_id, quality_id, quantity>=0, target_stock>=1, stock_cap>=target_stock}; buyback_gold_remaining: integer >=0; next_restock_game_time: nonnegative gameplay seconds. |
| EffectCooldownState | owner_id: string; revision: integer >=0; active_effects: list of {effect_id, source_quality_id, magnitude, applied_game_time, expires_game_time, healing_applied_hp}; cooldowns: list of {cooldown_group, ready_game_time}. Magnitudes are resolved at commit; healing_applied_hp is a number >=0 for healing and null for other effects, preventing healing replay on reload. |
| SaveState | schema_version: 1; game_time_s: number >=0; inventory, wallet_guild, vendor_states, effect_cooldowns; claimed loot/resource state; recent committed request IDs/results sufficient to reject a replay of the last saved transaction. Save these together, not as unrelated writes. |

Each item record has one owning location. A two-handed equipped record reserves main_hand and off_hand, which reference the same instance ID without duplicating quantity. Moving either reservation moves/unequips that one item. Pack slots never repeat a record ID. Quick slots select items and own no records.

Stack comparison includes item_id, quality_id, binding, bound_owner_id and modifier_ids. Bound_owner_id is null for unbound items and equals the owning character for character-bound items. Unique equipment never stacks. stat_mods are immutable definition modifiers; modifier_ids are instance modifiers and are not a duplicate of stat_mods.

Vendor stock limits follow section 5.5. Its definition policy specifies which items can be purchased versus only sold to it; no inferred shop access from quantity alone. Loot/resource claim state uses the world's persistent source identity, whose concrete schema belongs to the world/save section.

Example proposed data content below illustrates Resource fields and a JSON item record. It is not a combined runtime JSON definition format and is not a .tres file.

```json
{
  "schema_version": 1,
  "item": {
    "id": "fire_infusion",
    "display_name": "Fire Infusion",
    "category": "consumable",
    "icon": null,
    "max_stack": 10,
    "base_value_gold": 80,
    "sellable": true,
    "discardable": true,
    "allowed_qualities": [
      "standard"
    ],
    "equip_occupies": [],
    "effect_id": "fire_infusion",
    "moveset_id": null,
    "stat_mods": []
  },
  "recipe": {
    "id": "brew_fire",
    "station_id": "alchemy",
    "min_rank_id": "F",
    "proficiency_requirement": null,
    "inputs": [
      {
        "item_id": "herb",
        "quantity": 1,
        "minimum_quality_id": "standard"
      },
      {
        "item_id": "spring_water",
        "quantity": 1,
        "minimum_quality_id": "standard"
      },
      {
        "item_id": "glass_vial",
        "quantity": 1,
        "minimum_quality_id": "standard"
      },
      {
        "item_id": "ember_fragment",
        "quantity": 1,
        "minimum_quality_id": "standard"
      }
    ],
    "outputs": [
      {
        "item_id": "fire_infusion",
        "quantity": 1,
        "quality_id": "standard"
      }
    ],
    "fee_gold": 2
  },
  "record": {
    "instance_id": "example_stack_01",
    "item_id": "fire_infusion",
    "quantity": 3,
    "quality_id": "standard",
    "binding": "none",
    "bound_owner_id": null,
    "modifier_ids": []
  }
}
```

### 5.7 Transactions, saving and integration

One gameplay authority validates and commits transactions. UI submits intentions and renders the resulting state; it never edits wallet, pack, equipment or effects independently. These boundaries can be local in Milestone 2 and are not a networking implementation.

Proposed TransactionRequest fields: request_id, actor_id, operation (loot/use/equip/craft/buy/sell), source_id, selected instance IDs and quantities, recipe_id or null, and expected inventory/wallet/vendor/effect revisions as applicable. Proposed TransactionResult fields: same request_id, committed bool, reason code, accepted quantities and resulting revisions. Equip also identifies destination slots; buy/loot identifies source item definition/quality and quantity; use follows the reserved-stack action lifecycle in section 5.4.

Validate ownership, quantities, source/range/station, action legality, rank/proficiency, cooldown, stock, buyback budget, funds and final capacity before mutation. For buy/sell, the quote includes item, quality, quantity, unit price and relevant revisions. Requote changed state before spending. Reject a repeated request without double spending; after reload replay the saved result for a retained committed ID.

Craft/equip/buy/sell commit all requested changes together or none. Loot may accept only capacity-fitting quantity, with the remainder staying in its source. Equip publishes the new loadout/moveset only after successful commit. Use commits item consumption and effect/cooldown together at action completion.

Bind crafted outputs to the same character when any consumed input is character-bound. Reject foreign-owner inputs and unsupported binding combinations. Protected contract items cannot be crafted or sold through an alternate path.

Persist inventory, wallet/standing, claimed contracts and loot, vendor stocks/budget, clock, effects/cooldowns and recent transaction receipts coherently. Reload must not duplicate a reward, restart healing from zero, reset a cooldown or replenish vendor supplies. Pending use reservations are transient and cancel on load; no item has been consumed for them.

The same legality rules apply to player and AI item use. Definition lookup and equipment modifiers are separate from input and presentation. No UI node reference belongs in gameplay state.

### 5.8 Milestone 2 proof gates

- Conservation under split, merge, move, sort, equip and unequip; bound ownership included in stack matching.
- Full-pack crafting that frees an input slot versus crafting with no final output capacity; failed actions lose no materials or gold.
- Two-handed equip with both hands occupied, insufficient displacement space and linked-slot unequip; exactly one weapon record remains.
- Unarmed, sword_shield and greatsword moveset selection from committed loadouts; illegal pairing and mid-action swaps leave combat unchanged.
- Shortcut selection after sorting and after the last stack is consumed.
- Start use, voluntary cancel inside/outside the window, hit interrupt, completion and same-tick hit/completion ordering. Only completed legal use consumes one item and starts effects/cooldown once.
- Pending-use stack cannot be sold, split or consumed by crafting. Full-health healing and expired replacement consent consume nothing.
- Quantity price rounding across all seven ranks and stock limits; no basic merchant/craft/resell or cross-shop buy/resell profit loop.
- Stock capacity and buyback budget exhaustion; restock tied to saved gameplay time.
- Stale revisions and repeated request IDs; contract/loot duplicate claims and reload after a committed transaction.
- Save/reload preserves healing progress, effects/cooldowns, standing, equipment and vendor schedules; pending uses cancel without loss.
- Rendered 1080p layout, drag/drop, keyboard/controller navigation, mouse capture and suppression of held combat input on modal close.

These are future acceptance checks. This document adds no runtime code, data Resources or implementation test claims.

## 6. Asset pipeline

- Library layout and licenses: `/LICENSES.md`. Catalog: `assets/animations/CATALOG.json` (529 clips: 109 attack, 172 locomotion, 58 idle, 39 react, 36 air, 25 defend, 18 death, 15 evade). Regenerate with `python tools/catalog_anims.py`.
- Target skeleton: the Quaternius UAL fighter (`assets/models/characters/ual/fighter.glb`). Every clip is retargeted to it.
- Retarget pass (Blender 5.2.2, `D:\blender.exe -b -P tools/blender/retarget.py`): load source, map bones by a per-pack bone map, match rest poses by aligning each bone's world rest direction, transfer world-space rotation deltas, separate root motion onto the root bone, resample at 60 fps, export one GLB per pack to `assets/animations/baked/`.
- Annotation pass in the same script: foot contact curves (foot height below 3 cm and speed below 0.3 m/s), strike tip speed curve, hit frame = peak tip speed inside the authored window, written into a sidecar JSON next to the GLB and copied into CATALOG.json.
- Shipping builds use only `ship` rows in LICENSES.md.

## 7. Roadmap and proof gates

Milestone 1 (now)
- T4 skeleton + procedural world: deterministic seed test, chunk streaming under budget, walkable terrain.
- I1 locomotion, camera, foot lock on terrain. Gate: feet planted >= 90% of contact frames, slide < 2 cm.
- I2 retarget + annotation pipeline. Gate: every catalog clip either bakes or is listed with a reason.
- I3 MoveDef and ComboGraph, >= 40 moves (unarmed + sword). Gate: every move reachable, every hit frame lands in its active window.
- I4 inertialized combat transitions, guard IK, flinch springs. Gate: no bone jumps > 5 cm per tick across any transition.
- I5 HitSweep, Resolver, hit-stop, knockdown and get-up. Gate: collision restored after every path out of knockdown; no root drift > 2 cm.
- I6 sparring AI on the shared Command interface.

Milestone 2
- Inventory, crafting, economy per section 5. Codex builds, Claude reviews.
- Active ragdoll POWERED/LIMP states.

Milestone 3
- Guild hall, contracts, elements, climbing.

## 8. Combat look and grounded combat

Source: boss spec v2 (2026-10-10) plus the Claude/Codex artistic brief. Reference games are Budokai Tenkaichi 3 and Dragon Ball: Sparking Zero, scaled down: lower power and fully grounded. Neither agent has played them; details come from the boss's descriptions and general knowledge, so treat specifics as approximate and tune by playtest.

### 8.1 Rules
- Everything stays on the ground. No flight, no aerial combat. Jump exists for traversal and grants no aerial attacks.
- Readable, weighty exchanges: punches, kicks, dashes, guards, counters.
- Animations are visual variants only. Damage, stun and outcomes come from CombatStats; no clip-specific damage multipliers.
- Results are explicit (miss, hit, block, parry, guard break) and available to presentation and tests. No random crits hiding whether a move landed.

### 8.2 Look (art brief, Codex creative lead)
1. Home stance: calm guard, staggered feet, soft knees, rear leg loaded, breath in chest and shoulders. Every string returns here.
2. Grounded preparation, one exaggerated accent at contact (impact hold + small overextension + one-frame trail; limb lengths are never stretched), restrained recovery. The bare mannequin must look convincing with no effects.
3. Hips and torso start every strike, fist arrives, shoulder follows, body absorbs momentum. Support foot anchored, rear heel may pivot, the other foot may release. No glued soles, no skating.
4. Strings are phrases: each follow-through becomes the next preparation. Starter phrase jab -> cross -> hook. Avoid uppercut -> jab.
5. Attack speed: compact arcs win at high speed; shorten the reset between strikes first, then the wind-up; the moment of commitment and the impact hold always stay readable.

### 8.3 Mechanics (carried from the archived design, to be tuned)
- Strings: presses buffer (about 0.25-0.35 s) and chain after the current strike's impact. Punch-led and kick-led routes; the chooser prefers the other limb and never repeats a clip back to back.
- Guard: hold RMB for frontal block. A FRESH press opens a parry window of roughly 100-150 ms; holding never renews it. A parry interrupts the attacker and opens a short counter opportunity. Side and rear attacks bypass frontal guard.
- Heavy / guard pressure: committed heavies have a clear tell and build guard pressure; a full guard shows a visible guard break and a clear recovery.
- Dash: Ctrl, a short grounded burst (dodge/close distance) with explicit invulnerability frames to be tuned (archived baseline: 0.4 s dash, i-frames 0.035-0.23 s). Chase dash toward a locked target is a later candidate.
- Hit-stun is bounded with short protection afterwards; no infinite stun loops. Ordinary hits do not knock down; knockdowns are rare, earned, and always have a reliable get-up (MASTER 4.10).
- Hit pause is local to the fighters involved (never Engine.time_scale) and still accepts buffered input.
- Later candidates from the references: directional finishers (BT3 style), lock-on with target cycling.

### 8.4 Controls
LMB punch, F kick, RMB guard (fresh press = parry), Ctrl dash, Space jump, Shift sprint, Alt walk, E hold/release Fire, Q awaken Electric, R fire Electric while awake, Esc free mouse.

## 9. Elements (Fire, Electric)

Magic is the grounded, elemental equivalent of a ki blast. Owner split: Codex owns the elemental caster, projectile behaviour and visuals; Claude owns animation, cast permission, stats, mana and hurt logic (DECISIONS 13).

### 9.1 Shared rules
- Cast only while grounded and allowed (`Actor.can_start_action()`); casting never floats or slides the body: arm poses are layered on the stance (section 4.9 leg ownership applies).
- Elements change visuals, damage type and projectile behaviour only. They never interfere with the punch/combo flow.
- Damage = CombatStats.magic_power scaled by the cast (charge), type &"fire" or &"electric", delivered through Vitals.take_hit(HitInfo).

### 9.2 Fire: charging fireball
- Hold E to charge while mana allows; strength grows with hold time (charge 0..1), mana drains while charging.
- Release fires a HOMING fireball (turn-rate-limited steering toward the target).
- Charging has clear visual feedback (orb growing at the casting hand) while the stance stays stable.

### 9.3 Electric: awakened lightning
Reference: Thor's Awakened state in Marvel Rivals, as described by the boss (our knowledge of the specifics is approximate; Codex adds its own reading here).
- Look: crackling blue/white arcs running along the arms and hands, hands and forearms glowing with contained lightning, a small sparking aura while active.
- Play: Q enters a temporary empowered window; R fires while awake. Projectiles are thick arcing bolts or electric waves, NOT homing: direct or slightly arcing paths. Feels more aggressive than Fire.

Codex visual interpretation of the boss's description: keep the energy contained in the forearms and palms, with a bright white core and a narrow blue halo. Short branching arcs trace elbow to wrist and jump between the knuckles; a few sparks around the shoulders establish the temporary awakened state without hiding the guard silhouette. The projectile is a broad, jagged ribbon with a bright leading edge and a short fading tail. Fire builds slowly as a warm orb at the casting hand; Electric snaps out in a sharp pulse. Neither effect lifts the body, stretches a limb, or replaces the authored attack silhouette. This is our art direction from the supplied description, not a claim of exact reference-game behaviour.

