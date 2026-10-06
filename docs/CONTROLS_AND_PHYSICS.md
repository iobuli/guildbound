# GUILDBOUND — Controls & Character Controller (v0.1)

Target feel: **Breath of the Wild's controller in its purest form** on PC. Tight, readable,
forgiving, with the world as the toy. Plus GTA's casual on-foot flow and a MOBA-style ability layer on top.

Companion to [MAGIC_AND_STATS.md](MAGIC_AND_STATS.md). Every number marked *(stat)* comes from the
stat engine, so buffs and gear change how the character moves.

---

## 1. What "BotW-pure" means (the design rules)

1. **Almost nothing is locked behind a button prompt.** You can climb nearly any steep surface, jump off
   anything, glide, swim, push things. Stamina is the limiter, not invisible walls.
2. **The controller is tight, not floaty.** Instant response, no momentum fighting. Physics belongs to the
   *world*, not to the player's feet.
3. **The world is a toy.** Fire spreads, metal conducts, crates slide, water freezes. The player solves problems with
   verbs, not scripted keys.
4. **Multiple solutions.** Climb it, buff your jump, glide to it, build a bridge, or find the door.
5. **The camera never fights you.** Collision-safe, auto-recentering while running, a clean lock-on.
6. **Moving while acting.** Casting, drinking and aiming never root you. (Attacks commit; casting doesn't.)

---

## 2. Default PC bindings

### 2.1 On foot

| Input | Action | Notes |
|---|---|---|
| **W A S D** | Move | Camera-relative. Analogue speed on a gamepad |
| **Mouse** | Camera | Over-the-shoulder orbit; auto-collision |
| **Shift** (hold) | Sprint | Uses stamina |
| **Space** | Jump | In air with glider: deploy glide. Climbing: jump up |
| **C** | Crouch / Sneak (toggle) | Cuts noise and stealth radius (Veil/Rogue play) |
| **E** (tap) | Interact | Context prompt: talk, open, pick up, loot, harvest |
| **E** (hold) | Grab & carry | Lift physics objects up to your Carry limit, LMB to throw |
| **X** | Draw / sheathe weapon | Sheathed = social mode, quieter, NPCs relax |
| **V** (hold) | **Sense** | Shows clues, traces, magic residue, scents (see §5) |
| **Left Alt** | Dodge / roll | Direction-aware. Perfect dodge = Flurry window |
| **Tab** | Main menu | Inventory, Character, Notebook, Map, Skills |
| **I / J / M / K** | Direct to Inventory / Notebook / Map / Skills | |
| **Esc** | Pause / settings | |
| **H** | Call mount | Later milestone |

### 2.2 Combat

| Input | Weapon stance | Focus (spell) stance |
|---|---|---|
| **LMB** | Light attack combo (3 hits). Hold = charged attack | Cast selected **Bolt** |
| **RMB** (hold) | Guard (shield/buckler/weapon). Tap just before a hit = **parry** | **Aim** (precision, zoomed shoulder cam) |
| **MMB** (click) | Lock-on toggle. **Mouse wheel** while locked cycles targets | same |
| **R** | Swap stance: Weapon ⇄ Focus | same |
| **Left Alt** | Dodge | Dodge |

Modeless rule: **bows and magic staves** use the Focus stance; **melee weapons** use the Weapon stance. Equipping decides
your default stance.

### 2.3 Abilities (the important part)

Magic is mostly buffs, so the buff bar has to be fast and always reachable.

| Input | Action |
|---|---|
| **1 2 3 4** | Hotbar slots. Default loadout: your four **Boons** |
| **Ctrl + 1/2/3/4** | Hotbar page 2 (Bane, utility, second Boon set) |
| **Hold Q** | **Quick Wheel**: every learned spell and consumable by school. Release on the choice to assign/cast. World slows to 25% while open (single-player). |
| **F** | Use the selected **quick consumable** (food/potion). Mouse wheel + F cycles in menu, or pick from the wheel |
| **Hold 1-4** (on an ally) | Cast the Boon on the **targeted ally** instead of self |
| **Double-tap** the same key | Dismiss a sustained buff early |
| **Mouse side buttons (M4/M5)** | Rebindable: default = your two most-used Boons |

How casting behaves:
- **Tap** = cast on self (or toggle off if already active).
- **Boons are instant** (0.2 s Light, 0.5 s Dark with a visible flare). **You keep moving.**
- **Bolts** use the Focus stance + aim; you move at **70% speed** while aiming.
- **Imbue**: hold the Boon key while your weapon is drawn to put it on the weapon instead of yourself.
- Failed casts **tell you why** in one line: *no mana · no attunement slot · cooldown 3.2s*.

### 2.4 HUD for abilities
- **Buff bar** (MOBA-style) near the HP bar: icon, timer, and a gold (Light) or violet (Dark) frame.
- **Stamina wheel** around the reticle/character (BotW), fades when full.
- **Hotbar** bottom centre: slot, key, cooldown sweep, mana cost, slot-usage pips (e.g., ●●○○ attunement).
- Hover or hold **Alt+click** on a buff to see its source breakdown (the same `explain()` data as the stat sheet).

### 2.5 Gamepad mapping (for later, same actions)

| Pad | Action |
|---|---|
| Left stick / right stick | Move / camera |
| A / B / X / Y | Jump / dodge / attack / interact |
| LT / RT | Guard-Aim / Cast bolt |
| LB / RB | Sprint / Lock-on |
| D-pad ↑ ↓ ← → | Hotbar 1-4 |
| Hold LB+RB | Quick Wheel |
| R3 | Sense |

All actions are named Godot `InputMap` actions (`move_forward`, `jump`, `cast_1`...). Nothing reads raw keys.

---

## 3. Character controller

### 3.1 Architecture
- **Kinematic**: `CharacterBody3D` with a capsule. We do **not** make the player a rigid body. The world's physics
  objects respond to the player through deliberate impulses and carry rules.
- **State machine**, one state at a time, explicit transitions:
  `Grounded · Airborne · Climbing · Gliding · Swimming · Crouch · Dodging · Attacking · Guarding · Carrying · Staggered · Downed`
- **Fixed 60 Hz physics step.** Input is sampled into an `InputIntent` struct each tick:
  `{ move_vec, look, jump, sprint, crouch, dodge, interact, ability_slot, ... }`.
  The controller is a pure function `step(state, intent, dt, world_queries) → new_state`.
  This is what makes later **client-side prediction / server reconciliation** for co-op possible.
- The 10 Hz world sim (economy, NPC schedules) is separate and never touches the controller step.

### 3.2 Capsule and ground

| Parameter | Value |
|---|---|
| Capsule | radius 0.35 m, height 1.80 m (crouch 1.1 m) |
| Step height | 0.40 m (auto-step up stairs and kerbs) |
| Max walkable slope | 50° (steeper = slide, or climb if the surface allows) |
| Ground snap | 0.3 m (stay glued going downhill) |
| Moving platforms | Inherit platform velocity |

### 3.3 Movement speeds *(all feed from the stat engine)*

| Mode | Speed | Source |
|---|---|---|
| Walk (hold toggle) | 0.4 × Move Speed | Social, sneaking past |
| **Run (default)** | **1.0 × Move Speed** *(stat, base 6 m/s)* | WASD |
| Crouch | 0.5 × Move Speed | Stealth reduced noise |
| **Sprint** | **1.35 × Move Speed** | Costs stamina |
| Swim | 0.5 × Move Speed (sprint-swim 0.8×) | Costs stamina |
| Glide forward | 1.3 × Move Speed (fall rate 2 m/s) | Glider item required |
| Climb | Climb Speed *(stat, base 1.5 m/s)* | Costs stamina |

Move Speed already has a soft cap (+60%) and hard cap (+100%), so sprint tops out near **16 m/s**. We also clamp
sprint at **12 m/s** to protect world streaming and camera.

Acceleration is snappy: ground accel 45 m/s², decel 60 m/s², turn rate 720°/s. No ice-skating, except on actual ice.

### 3.4 Jump and air

| Parameter | Value |
|---|---|
| Jump height *(stat, base 1.2 m)* | initial velocity = √(2·g·h) = √(2·20·1.2) ≈ **6.9 m/s** |
| Gravity | **20 m/s²** (snappier than real, BotW-like) |
| Variable jump | Releasing Space early cuts upward velocity by 50% |
| Coyote time | 0.12 s |
| Jump buffer | 0.12 s |
| Air control | 60% of ground acceleration |
| Terminal velocity | 40 m/s |

**Fall damage:** safe up to **8 m**. Above that, 6% of Max HP per extra metre. **Water ≥ 2 m deep is safe.**
Reduced by Ward buffs, Wisp passive, and armor with the right affix; instant-KO is *never* a thing.

### 3.5 Stamina (the BotW limiter)
Stamina *(stat, base 100)* in a wheel.

| Action | Cost |
|---|---|
| Sprint | 10 /s |
| Climb | 6 /s (×2 when wet, ice = can't climb) |
| Fast climb (Shift) | 10 /s |
| Jump | 8 |
| Glide | 5 /s |
| Swim | 4 /s |
| Dodge | 15 |
| Charged attack | 20 |
| Guard (per blocked hit) | scales with damage |
| **Regen** | **18 /s**, after a 0.6 s pause |

- At 0 → **Exhausted**: can't sprint, climb or glide until 30% refilled. Dropping while climbing makes you slip and fall.
- **Vigor** buffs add max stamina and regen; **food and boots** add more. This is the number players will upgrade first.

### 3.6 Climbing
- You can climb any surface **steeper than the 50° walkable limit** and whose material is **climbable**
  (rock, wood, brick, ivy), not **ice, wet-glass, polished metal, lava** (material tags).
- Free climbing: WASD relative to the wall, Space = hop up (cost 8), Shift = fast climb, release = hang.
- Ledge grab at the top with an auto-mantle. Over-the-shoulder camera pulls back.
- **Rain makes rock slippery** (×2 cost), like BotW. Swiftness/Vigor/gloves counter it.

### 3.7 Gliding
Space in the air with a **glider item** equipped (cheap, early quest item). 5 stamina/s. Forward 1.3× Move Speed, sink 2 m/s,
steer with WASD. Updrafts (fire, hot springs, wind vents) are **physics fields** you can create with Bolts.

### 3.8 Vaulting, mantling, steps
- Running into a ≤1.2 m obstacle **auto-vaults** (GTA flow).
- Jumping into a ledge at ≤2.0 m auto-mantles.
- Tall steps and slopes need no button.

### 3.9 Swimming
Surface swim, dive with Ctrl+down (Crouch while swimming), no oxygen meter for the prototype. Heavy loads slow you (carry penalty).
Frozen water becomes ground; swim controller exits on ice.

### 3.10 Carry and push
- Hold **E** on a rigid body whose mass ≤ your **Carry stat** (kg). Heavier objects can be **pushed** (impulse scaled by STR).
- **Might** buffs raise Carry, so a Might-buffed character moves objects an ordinary one can't, a *physical* solution
  to a puzzle that other routes (a lever, a bridge) also solve.
- Objects have **mass in kg** that matches the item weight system, so one unit works for inventory *and* physics.

---

## 4. Combat feel (physics side)

- **Melee**: animation-driven swept hit volumes (capsule sweep along the weapon arc), not trigger boxes.
- **Hit-stop**: 60-90 ms freeze on both attacker and target (see the game-feel skill).
- **Knockback** = impact force ÷ target mass, reduced by **Tenacity** and stance.
- **Guard**: holding RMB blocks frontal arcs; cost scales with damage; **parry window 150 ms** staggers the target.
- **Dodge**: 0.35 s with 0.2 s invulnerability (i-frames); **perfect dodge** (within 0.2 s of a hit) triggers
  **Flurry**: 1.5 s of slowed enemies and +attack speed (via a temporary `more` modifier, so it shows in the stat breakdown).
- **Lock-on**: camera frames player + target, strafe-circle movement; flip/side-hop on Space while locked.
- **Ranged**: Bolts are physical projectiles (fast, small gravity arc). Aim cam is a tighter shoulder view.
- **Ragdoll** on KO/death (cosmetic, quickly sleeps).

---

## 5. Sense (the investigation control)

**Hold V.** The world desaturates slightly and highlights (colour-coded, with fade-in range):

| Channel | Shown to |
|---|---|
| Footprints, scents, blood | Anyone with Tracking; Beastkin brighter |
| Magic residue (Light thin, Dark thick) | Sylph; anyone with Insight (Light) up |
| Hidden marks, doors | Insight (Light) |
| Documents, tampered locks, loose bricks | Literacy / Rogue skills |
| Echoes (recent events) | Wisp, limited |

Sense never gives answers. It shows *where to look*; the player still reads, tails, and links claims in the Notebook (`J`).
Costs a little mana per second (hence it's a hold, not a toggle). Using it in front of guards looks suspicious.

---

## 6. The world as physics playground (chemistry-lite)

Every material has the **same properties as crafting** (flammability, conductivity, hardness, weight) plus tags
(`wood`, `metal`, `water`, `grass`, `ice`). Interactions are table-driven:

| Source | Target | Result |
|---|---|---|
| Fire bolt | `grass`, `wood`, rope | Ignites, spreads with wind |
| Fire | air column | Updraft (glide) |
| Frost bolt | `water` surface | Ice platform, lasts 30 s |
| Storm bolt | `metal`, `water` | Conducts, stuns standing targets |
| Stone bolt | ground | Raises platform / cover |
| Might buff | heavy rigid body | Pushable / carriable |
| Veil / Umbral | light sources | Snuffs lights (stealth) |

Environmental zones apply stat debuffs (cold, heat, poison) that Ward and gear counter, as in BotW's temperature.

---

## 7. Physics engine and performance

- Godot 4 with **Jolt Physics** (available as an option from 4.4) for rigid bodies; character stays on `CharacterBody3D`.
- **Budget**: ≤ 150 active rigid bodies near the player, aggressive sleeping, props beyond 60 m become static.
- Wind, heat, water currents are **fields (area queries)**, not simulated fluids.
- Only gameplay-relevant props are replicated in co-op. Cosmetic debris is client-local.
- Fixed 60 Hz physics; render interpolated. Target 60 fps on RTX 3060 at 1080p / 1440p.

---

## 8. Input quality

- **Buffering**: 120 ms buffer for jump, dodge, attack, and cast.
- **Everything rebindable**; toggle-or-hold options for sprint, crouch, Sense, guard.
- **Mouse**: sensitivity, invert-Y, raw input, optional smoothing off by default.
- **Accessibility**: remappable, hold-to-toggle, reduced camera shake, colour-blind Light/Dark frames (shape + colour).

---

## 9. Prototype (controller-only) scope

A **greybox test arena** to answer: *does moving, climbing, gliding and buffing feel good before any content exists?*

- Capsule controller with the state machine above, camera, lock-on.
- Terrain with slopes (30°, 50°, 70°), a 20 m cliff (climb and rain slippery toggle), water, stairs, ledges, a gap needing a buffed jump.
- Stamina wheel, glider, fall damage.
- A few rigid bodies (crate 20 kg, boulder 200 kg), a Might buff that makes a 90 kg object liftable.
- Hotbar 1-4 wired to the existing stat engine: Gale Step, Bright Valor, Aegis, Shadowrush, with the buff bar.
- Debug overlay showing the stats and their sources live.

Kill criteria: if people prefer walking around the cliff over climbing it, stamina costs or climb speed are wrong. If the buff bar
feels like a chore, the cast is too slow or the keys too awkward.

## 10. Open questions

1. Is **dodge on Left Alt** comfortable for you, or would you rather put it on a mouse side button or Space (with jump on another key)?
2. **Quick Wheel slow-mo** is single-player only. Is that fine for the co-op plan, where it would stay real-time?
3. Do you want a **glider early** (like BotW's tutorial gift) or later as a reward?
4. Is **auto-vault/mantle** okay, or do you want every climb to be deliberate?
5. Aim-assist for mouse bolts (soft magnetism) or fully manual?

---

## 11. Animation system (built, v0.1)

Code: `game/character_visual.gd`, `game/procedural_pose.gd`. Clips: Quaternius Universal Animation Library 1 + 2 (CC0, same 65-bone rig as the base characters, so no retargeting).

| Layer | What | Why |
|---|---|---|
| **mode** (Transition) | ground blend space (Idle / Walk / Jog / Sprint), crouch, air, swim, push | One base state at a time, 0.2 s crossfades |
| **loco_speed** (TimeScale) | playback = real speed / the clip's measured speed | Feet don't slide; gear and buffs speed the legs up |
| **full** (OneShot) | roll, ledge pull-up, hard landing | Whole-body actions |
| **upper** (OneShot, spine and up) | cast, pick up, flinch | Casting never roots you: legs keep running |
| **ProceduralPose** (SkeletonModifier3D) | free climb, glide, carry-arms | Moves no pack covers; fades via `influence` |

Rules:
- **Gait, not raw speed, picks the clip**: 0 idle, 1 walk (2 m/s), 2 run (your Move Speed), 3 sprint. A Gale Step character still *runs*, just faster.
- **No input reads in visuals.** Everything comes from player state (state, speed, flags, hp, distance climbed), so a remote co-op player animates identically.
- **Climb cycle advances by distance climbed** (one cycle per 1.2 m), so hands match movement.
- No jump wind-up: physics leaves the ground instantly, so the anticipation clip was dropped.
- Not yet: lock-on strafing (needs 8-way clips from the paid Pro pack, or procedural), foot IK on slopes, lean into turns, combat combos (UAL2 sword clips are already loaded).
