# Guildbound overhaul: handoff

State as of 2026-10-16, `master`. The machine-readable copy is `docs/handoff.json`; keep both in step.
Read this file first, then the module header (`//!`) of any file before you edit it.

## Build and run

- Toolchain: `CARGO_HOME=D:\dev\cargo`, `RUSTUP_HOME=D:\dev\rustup` (moved off C:). If a shell predates that, set both and put `D:\dev\cargo\bin` on PATH.
- `cargo build -p guildbound` is the verification bar. The user runs `play.cmd` and checks visuals at milestones; don't run the game or take screenshots yourself.
- Pause for a user playtest at every playable milestone.
- Workspace: `crates/game` (the game, bin `guildbound`), `crates/sim` (game uses only `sim::terrain::ground`), `tools/gb_lab` (old physics lab), `crates/posenet` (outside the workspace). Old docs are in `docs/archive/`, and unused asset dumps are in `D:\gb_assets_archive\`.

## Module map (`crates/game/src`)

| File | Responsibility | Status |
|---|---|---|
| `main.rs` | App, 120 Hz fixed tick, Avian (8 substeps), plugins | done |
| `motion/donor.rs` | GLB parse (gltf crate): hierarchy, skin joints, curves | done |
| `motion/retarget.rs` | Donor clips → canonical UAL2 skeleton (name copy / Mixamo bind-delta) | done |
| `motion/extract.rs` | Dense clip → tagged extreme keys | done |
| `motion/timing.rs` | Stylized timing: holds, snaps, stepping on twos, exaggeration | done |
| `motion/bank.rs` | `PoseBank`: tracks, masks, `assets/anim/poses.json` overrides (hot reload) | done |
| `motion/rig.rs` | Spawn model on `ModelRequest`, bind bones → `Rig` | done |
| `motion/secondary.rs` | Per-bone angular springs, arm world inertia + acceleration trail (`Arm swing` knob), chest/neck/head nod against vertical acceleration, impulses, squash | done |
| `motion/body.rs` | Body capsules (+ the root's `BodyPadding` for worn armour); swings arms out of torso/head/opposite thigh and thighs apart (after springs, before contact) | done |
| `motion/contact.rs` | Foot IK (heel + ball), pelvis drop only as far as a planted foot can't reach by straightening its leg (swing feet ignored), step-bounce dip (`ContactRequest.bob`, fades where the feet stand at different heights), COP lean, look-at, hand IK | done |
| `motion/compose.rs` | **Only bone writer**: layers → springs → body → contact → bone `Transform`s; also publishes the pose in model space as `RigPose` | done |
| `motion/tweak.rs` | F2 pose tweak tool | **stub** |
| `play/mod.rs` | Shared types: `Intent Mover Action Fighter Loadout Hit StaminaSpend Interactable Climbable Water Feel`, capsule consts, `spawn_character` | done |
| `play/world.rs` | Terrain, pond, stairs, mantle boxes, climb walls, props, interactables, characters | done |
| `play/input.rs` | Keyboard/mouse/gamepad → `Intent` | done |
| `play/camera.rs` | Orbit camera, lock-on framing, `CameraShake` | done |
| `play/controller.rs` | Kinematic capsule (MoveAndSlide), floating support probe, steps, jump, knockback | done |
| `play/select.rs` | **Only writer of `MotionLayers.layers`** + `ContactRequest` (incl. the `Bounce` character: step dip, its acceleration in `accel.y`, squash rhythm, idle breath) | done |
| `play/interact.rs` | Focus, `offer` (per held tool; `requires` → head-shake refusal), radial stands, walk-in, use clips, `beat` (effect / loop landing + hitstop freeze; tool loops land when the blade stops, `Swing`), `reach` IK windows, emits `UseEffect` / `UseBeat` | done |
| `play/tools.rs` | `Holding` + `ToolItem`: the axe on the table, left-hand pickup, hand-over to the right hand, carry/chop grips, settle on every re-parent | new |
| `play/voxel.rs` | `VoxelGrid` (carve, split, `mesh`, `smooth_mesh`) and the shared `Lattice` + `surface_nets` mesher for any density field (each quad wound as a whole, split along its flatter diagonal) | done |
| `play/armor.rs` | Plate armour (bascinet, one cuirass with arm holes, pauldrons of 3 concentric spherical lames, vambraces, greaves) built at startup on the async pool, one thread per plate, from the body mesh: ray-cast profiles → designed shells (gap and thickness along the normal) → Surface Nets + UVs (cylindrical with a seam split, band, or azimuthal about the pole) + tangents + Metal049C with mips generated at load; each plate rigid, fitted per frame to the skin under it (Kabsch / Müller, warm-started; pauldron lames rotate only about the shoulder joint, so they never cut into each other); `BodyPadding` derived from the built plates; armour stand + E, plates leave one by one | new |
| `play/grass.rs` + `assets/shaders/grass.wgsl` | Lush grass: chunked LOD blades, vertex-shader wind (gust fronts, flutter), trampling by `Mover`s, `GrassWind` (incl. a `gust` slot for wind magic) | new |
| `play/items.rs` | Item entities (log, stick, stone, apple): physics drops on `Phys::Debris`, magnet flight into the bag, hill scatter + respawn spots | new |
| `play/inventory.rs` | `Inventory` (required by `Player`, 20 stacks), I-key bag panel, pickup toasts | new |
| `play/trees.rs` | Voxel trees drawn smooth: notch carving on `UseBeat`, chips/shake/recoil/sound, felling into a `Phys::Debris` body, landing, breaking into logs/sticks/apples, regrow; `chop` / `fell` / `drop` log lines | new |
| `play/fx.rs` | `Burst` particles (chips, leaves) and `Sfx` one-shot sounds (first audio in the game) | new |
| `play/hud.rs` | Interaction prompt only | partial |
| `play/tuning.rs` | F1 live tuning panel (Exaggeration 0.9–1.4, Arm swing 0–2, Bounce 0–2), F5 saves `profiles/feel.json` | done |
| `play/monitor.rs` | F4 joint-relation monitor + `staging/relations.log` | new |
| `play/parkour.rs` | Graded landings: soft / momentum roll / hard land (`Owner::Move`); logs `land_*` lines | new |
| `play/moveset.rs` | Moveset JSON schema + loader (`assets/anim/movesets/*.json`) | done, **no JSON files yet** |
| `play/combat.rs` | Melee FSM + **only `Hit` resolver** | **stub** |
| `play/ai.rs` | NPC brain → `Intent` | **stub** |
| `play/ragdoll.rs` | Impact-only active ragdoll; `RagdollRequest` (the contract with `combat`) is defined | **stub** |
| `play/ranged.rs` | Bow/crossbow/throwing, projectiles | **stub** |
| `play/magic.rs` | Schools as movesets, cast/charge/channel | **stub** |
| `play/traverse.rs` | Swim, climb, ledge, mantle | **stub** |

## Frame order

1. `Update / PlaySet::Input`: `input` writes the player's `Intent`.
2. `Update / PlaySet::Think`: `ai` writes NPC `Intent`.
3. `Update / PlaySet::Act`: action owners (`combat ranged magic interact traverse`) run on `Action`, emit `Hit`, and request `Mover.root_motion`. `interact::animate_props` runs before `interact::drive`; `tools` (take/return, hand-over, grip, place) and `trees::chop` run after `drive` and react to its `UseEffect` / `UseBeat`.
4. `Update / PlaySet::Select`: `select` → `MotionLayers`.
5. `FixedUpdate / PlaySet::Move`: `controller` moves capsules.
6. `PostUpdate / MotionSet::Compose`: `compose` writes bones. After `TransformSystems::Propagate`, `monitor` measures.

## Ownership: one writer per quantity

| Quantity | Writer | Readers |
|---|---|---|
| `Intent` | `input` (player), `ai` (NPC) | everything in Act, `select`, `controller` |
| `Mover.velocity/grounded/mode(Ground,Air)/accel/landing/air_time/yaw` | `controller` | `select`, `interact`, monitor |
| `Mover.mode` Swim/Climb/Ledge | `traverse` | controller skips those modes |
| `Mover.mode` Ragdoll | `ragdoll` | controller, select skip |
| `Mover.phase` | `select` | `select` |
| `Mover.impact`, `Mover.takeoff_speed` | `controller` | `parkour` (grade), `select` (leap poses) |
| `Mover.step_ease` | `controller` (running total of drawn-root step easing) | `select` → `ContactRequest.root_ease` → `contact` |
| `Mover.root_motion / move_scale / face` | the current `Action` owner | `controller` (root_motion cleared after the frame's last tick) |
| `Mover.external` (knockback) | `combat` | `controller` decays it |
| `Fighter` (hp, poise, stamina, dead) | `combat` only; others spend stamina through the `StaminaSpend` message (`traverse`: climbing, fast swimming) | `traverse` (out of stamina), `ai`, `hud` |
| `RagdollRequest` | `combat` inserts it (knockback above `Feel.ragdoll_threshold`) | `ragdoll` consumes it, then owns the body, `Mover.mode` Ragdoll, `MotionLayers.raw` and the get-up `Action` (`Owner::Ragdoll`) |
| `Action` | its `owner`; starting one needs `is_free()`; `Owner::Hit` may interrupt anything | `select` (layers), owners |
| `MotionLayers.layers`, `.contact` | `select` (characters with `Mover`); `armor` for armour-stand mannequins | `compose` |
| `RigPose` | `compose` | `armor` (plate fits) |
| `BodyPadding` | `armor` (inserted on whoever wears the set) | `compose` (`body::resolve`), monitor |
| Armour plates, `ArmorSet`, stand `PropState.on` (worn) | `armor` | `interact` (`offer`) |
| Grass chunks, `GrassWind` (`gust` for wind magic) | `grass` | |
| `MotionLayers.impulses` | `combat` and other play systems (append) | `compose` drains |
| `MotionLayers.raw` | `ragdoll` | `compose` |
| `HandTargets` | the action owner (`interact` now; `ranged`, `traverse` later); `tools` adds the right hand during the pickup hand-over | `select` fades → `ContactRequest.hands` |
| `ContactRequest.bob`, `accel.y` | `select` (scaled by `MotionFeel.bounce`) | `contact` (pelvis dip), `secondary` (nod) |
| `Holding`, `ToolItem`, tool parent/transform, table `PropState.on` | `tools` | `interact` (`offer`), `trees` (blade point) |
| Tree voxels, `Tree`, felled bodies, standing-tree part transforms (trunk, collider, canopy incl. the chop shake), tree `PropState.regrow` / `.shake` (set) | `trees` | `interact` (availability, shake decay) |
| Cursor grab / visibility | `camera::capture_cursor` only; `ui_mode` is derived each frame from the open pages (`BagOpen`, `TuningPanel.open`, F2) after the `PageToggles` set | input, monitor |
| `UseEffect` / `UseBeat` messages | `interact` emits | `tools`, `trees` |
| `Particle`s, `Sfx` audio entities | `fx` (from `Burst` / `Sfx` messages) | |
| `Item` entities, flying stand-ins, hill `Spot`s | `items` (spawned by `items`, `trees` via `items::spawn_item`) | `inventory` (via messages) |
| `Inventory` contents | `inventory` only (applies `ItemCollected`, answers `ItemRejected`) | `items` (room check before the magnet) |
| Bone `Transform`s | `compose` only (and `tweak` while active) | rendering, monitor |
| Visible pelvis height | `contact` only (it cancels the controller's step easing) | |
| Limb–body separation | `body::resolve` only | monitor |
| `Hit` message | `combat`, `ranged`, `magic` emit | **`combat` alone resolves** |
| `InteractPrompt` | `interact` | `hud` |
| `Feel`, `MotionFeel` | `tuning` | controller, compose |

## Data files

- `assets/anim/{ual1,ual2,sekiro}.glb`: donor libraries. `ual2.glb` is the model and the canonical skeleton.
- `assets/anim/poses.json` (optional): style plus per-key overrides. Hot-reloaded.
- `assets/anim/movesets/*.json`: one `Moveset` per file (schema in `play/moveset.rs`: `name, kind(unarmed|melee|ranged|magic|tool), weapon{glb path, hand, offset}, offhand, stance, light[], heavy[], sprint_attack, air_attack, guard{clip, parry}, aim{clip, draw, release, reload}, projectile, cooldown`). Every `track` is a bank name `"{lib}/{clip}"`. **None exist yet.**
- `assets/packs/weapons/*.glb`: longsword, shortsword, greatsword, claymore, knight_sword, battleaxe, mace, warhammer, spear, glaive, battle_staff, elder_staff.
- `assets/audio/kenney/impactWood_*`, `impactPlank_*`: Kenney Impact Sounds (CC0), chop / fall / pick-up sounds.
- `profiles/feel.json`: written by F5.
- `staging/relations.log` (gitignored): monitor output, rewritten each run.

## Monitor invariants (`play/monitor.rs`)

| Relation | Broken when |
|---|---|
| `knee_backward` / `elbow_inward` | flex ≥ 10° and the bend side points against natural (body front / out-down-back) with cos < −0.3 |
| `knee_pop` / `elbow_pop` | the bend side turns by cos < −0.5 in one frame |
| `foot_sink` | heel or ball more than 3 cm below the ground |
| `foot_float` | grounded, body and foot still, lowest of heel/ball more than 5 cm above the ground |
| `foot_slide` | grounded, foot down (< 2 cm), foot moving > max(0.6, speed / 2) m/s |
| `toe_inside` / `knee_inside` / `hand_inside` | the joint lies inside `Phys::World` geometry |
| `hand_miss` | `HandTargets` weight ≥ 0.95 and hand more than 5 cm from the target |
| `arm_in_body` / `legs_crossed` | `motion::body` capsules overlap by more than 1 cm |

Log line: `t character relation side value mode action speed`.

## Fixes this session

- Stairs: contact cancels the controller's per-frame step easing (`root_ease`), so the pelvis has one smoother. The two smoothers in series had made the body dip and rebound.
- Knees and elbows: `bend_pole` drops any bend component against the natural side and adds a small bias, so near-straight limbs keep a stable bend side. Limbs never extend past 99.9 % of their length, so they no longer lock and snap.
- Interaction: hands ride the moving grip part (`Interactable.grip`: lever arm, chest lid). The walk-in pivots on the spot before stepping when the target is behind.

## Grass and armour (latest milestone, awaiting playtest)

- Armour design facts: the bind pose equals the skeleton rest (FK) to 1 µm and the mesh is in model space, so plates are authored directly on the T-pose body and placed by `rigid fit(rest samples → skinned samples)`. A review simulated the first design on real clips (Python replica against `ual2.glb`) and found plates fitted to different bones passing through each other, so the set was redesigned: no belly lames (one cuirass, flared rolled bottom, arm holes cut below the hanging arms' armpits, pelvis skin in its profile), pauldron lames are concentric spheres about the shoulder joint with strictly increasing radii (`nest_radii`) and `Fit::Pivot` (rotation only), gap and thickness are scaled by the profile slope so steep areas keep `THICK` (11 mm, over the 4.8 mm lattice diagonal), and `BodyPadding` is measured from the meshes (`derive_padding`) instead of constants. Clearances (gap): cuirass 2.6 cm, vambrace 1.2 cm, greave 1.3 cm, bascinet 1.6 cm, pauldron lames ≥ 2.05 cm apart radially. Rotation extraction runs up to 32 Müller iterations and stops once a step is under 1 µrad (a cold start needs ~30, a warm frame a few).
- Grass: WGSL compiles only at runtime; if grass is missing, check the log for a `shaders/grass.wgsl` error. Startup logs `[grass] N blades ...` and `[armor] built N plates (V vertices), padding ...`. The flutter clock wraps at `FLUTTER_WRAP` (3600 s) and the gust-noise scroll at `SCROLL_WRAP` (2000 m); the shader rounds flutter frequencies to whole cycles per wrap and repeats its noise lattices over the scroll period, so neither wrap shows (constants duplicated in `grass.rs` and `grass.wgsl`: keep them equal).

## Axe, trees, bounce (previous milestone)

- Clip facts behind the numbers: `TreeChopping_Loop` (0.967 s) is a one-handed right-hand chop that leads with the pinky edge; the axe lands at ≈0.17 s with the hand 0.54 m ahead. `PickUp_Table` (0.833 s) reaches with the LEFT hand (≈0.45 m ahead, 0.87 m high, 0.2–0.3 s) and brings it across to the right side of the chest by 0.5 s; the right shoulder pulls back out of reach.
- Calibrate from `staging/relations.log`: `chop` lines give the blade's distance to the bark (`value`, negative = inside) and its height; tune `trees::STAND`, `tools::{FIST, CHOP_TILT, AXE_GRIP}` from them.
- Playtest 1 (2026-10-16): legs folded too much on stairs. The pelvis dropped by the full step for the lower foot, even mid-swing; now it drops only as far as the leg cannot reach (`LEG_REACH` 0.97 of its length) and swing feet are ignored. The chop beat fired with the axe overhead (blade 1.3 m from the bark, 2.03 m up), so tool loops now land when the blade stops. The `chop` line also logs `hand=(x,y,z)` in the user frame (clip impact is (0, 1.06, 0.54)) and `clip_t`.
- Known rough edge: putting the axe down flies it from the right hand to the table over 0.22 s (the clip only reaches with the left hand).

## Next lanes

Each lane stays in its own stub files. The shared files (`play/mod.rs`, `select.rs`, `world.rs`) take small additive edits only: record them here.

- **A. Melee:** `combat.rs`, `ai.rs`, `ragdoll.rs`, plus `assets/anim/movesets/{unarmed,longsword}.json`. Hit tests sweep the weapon/limb between frames; damage scales with closing speed; hitstop through `Time<Virtual>`; knockback through `Mover.external`; impulses into `MotionLayers.impulses`.
- **B. Traversal, then ranged and magic:** `traverse.rs` (select already handles the Swim/Climb/Ledge layers), then `ranged.rs` and `magic.rs` (projectiles emit `Hit`).
  - **Tightrope / balance beam (user idea):** low exaggeration with high lean already plays like balancing. Add a beam to the world and a balance state: sideways lean drift that the player counters, arms spread for balance (arm swing / `HandTargets`), and too much tilt → wobble → fall. Build it on contact lean, `secondary` arm inertia, and `body` self-collision.
- **C. Character systems:** `docs/CHARACTER_SYSTEMS.md`. Its 6 open questions need the user's answers before any code.
- **D. Pose tweak tool:** `motion/tweak.rs` (F2).
- **Parkour reference:** `docs/reference/parkour_jump.md` (frames pending: Vimeo needs a login, so drop the mp4 at `staging/ref/parkour.mp4`).

Rules: keep the module header docs in step with the code. Use the monitor log to back motion claims. Stop for a playtest at each milestone.
