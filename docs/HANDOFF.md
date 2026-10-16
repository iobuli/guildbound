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
| `motion/secondary.rs` | Per-bone angular springs, impulses, squash | done |
| `motion/contact.rs` | Foot IK (heel + ball), pelvis drop, COP lean, look-at, hand IK | done, fixed this session |
| `motion/compose.rs` | **Only bone writer**: layers → springs → contact → bone `Transform`s | done |
| `motion/tweak.rs` | F2 pose tweak tool | **stub** |
| `play/mod.rs` | Shared types: `Intent Mover Action Fighter Loadout Hit Interactable Climbable Water Feel`, capsule consts, `spawn_character` | done |
| `play/world.rs` | Terrain, pond, stairs, mantle boxes, climb walls, props, interactables, characters | done |
| `play/input.rs` | Keyboard/mouse/gamepad → `Intent` | done |
| `play/camera.rs` | Orbit camera, lock-on framing, `CameraShake` | done |
| `play/controller.rs` | Kinematic capsule (MoveAndSlide), floating support probe, steps, jump, knockback | done |
| `play/select.rs` | **Only writer of `MotionLayers.layers`** + `ContactRequest` | done |
| `play/interact.rs` | Focus, walk-in (pivot then step), use clips, grip-following hand IK, prop state | done, fixed this session |
| `play/hud.rs` | Interaction prompt only | partial |
| `play/tuning.rs` | F1 live tuning panel, F5 saves `profiles/feel.json` | done |
| `play/monitor.rs` | F4 joint-relation monitor + `staging/relations.log` | new |
| `play/moveset.rs` | Moveset JSON schema + loader (`assets/anim/movesets/*.json`) | done, **no JSON files yet** |
| `play/combat.rs` | Melee FSM + **only `Hit` resolver** | **stub** |
| `play/ai.rs` | NPC brain → `Intent` | **stub** |
| `play/ragdoll.rs` | Impact-only active ragdoll | **stub** |
| `play/ranged.rs` | Bow/crossbow/throwing, projectiles | **stub** |
| `play/magic.rs` | Schools as movesets, cast/charge/channel | **stub** |
| `play/traverse.rs` | Swim, climb, ledge, mantle | **stub** |

## Frame order

1. `Update / PlaySet::Input`: `input` writes the player's `Intent`.
2. `Update / PlaySet::Think`: `ai` writes NPC `Intent`.
3. `Update / PlaySet::Act`: action owners (`combat ranged magic interact traverse`) run on `Action`, emit `Hit`, and request `Mover.root_motion`. `interact::animate_props` runs before `interact::drive`.
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
| `Mover.step_ease` | `controller` (running total of drawn-root step easing) | `select` → `ContactRequest.root_ease` → `contact` |
| `Mover.root_motion / move_scale / face` | the current `Action` owner | `controller` (root_motion cleared after the frame's last tick) |
| `Mover.external` (knockback) | `combat` | `controller` decays it |
| `Action` | its `owner`; starting one needs `is_free()`; `Owner::Hit` may interrupt anything | `select` (layers), owners |
| `MotionLayers.layers`, `.contact` | `select` | `compose` |
| `MotionLayers.impulses` | `combat` and other play systems (append) | `compose` drains |
| `MotionLayers.raw` | `ragdoll` | `compose` |
| `HandTargets` | the action owner (`interact` now; `ranged`, `traverse` later) | `select` fades → `ContactRequest.hands` |
| Bone `Transform`s | `compose` only (and `tweak` while active) | rendering, monitor |
| Visible pelvis height | `contact` only (it cancels the controller's step easing) | |
| `Hit` message | `combat`, `ranged`, `magic` emit | **`combat` alone resolves** |
| `InteractPrompt` | `interact` | `hud` |
| `Feel`, `MotionFeel` | `tuning` | controller, compose |

## Data files

- `assets/anim/{ual1,ual2,sekiro}.glb`: donor libraries. `ual2.glb` is the model and the canonical skeleton.
- `assets/anim/poses.json` (optional): style plus per-key overrides. Hot-reloaded.
- `assets/anim/movesets/*.json`: one `Moveset` per file (schema in `play/moveset.rs`: `name, kind(unarmed|melee|ranged|magic|tool), weapon{glb path, hand, offset}, offhand, stance, light[], heavy[], sprint_attack, air_attack, guard{clip, parry}, aim{clip, draw, release, reload}, projectile, cooldown`). Every `track` is a bank name `"{lib}/{clip}"`. **None exist yet.**
- `assets/packs/weapons/*.glb`: longsword, shortsword, greatsword, claymore, knight_sword, battleaxe, mace, warhammer, spear, glaive, battle_staff, elder_staff.
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

Log line: `t character relation side value mode action speed`.

## Fixes this session

- Stairs: contact cancels the controller's per-frame step easing (`root_ease`), so the pelvis has one smoother. The two smoothers in series had made the body dip and rebound.
- Knees and elbows: `bend_pole` drops any bend component against the natural side and adds a small bias, so near-straight limbs keep a stable bend side. Limbs never extend past 99.9 % of their length, so they no longer lock and snap.
- Interaction: hands ride the moving grip part (`Interactable.grip`: lever arm, chest lid). The walk-in pivots on the spot before stepping when the target is behind.

## Next lanes

Each lane stays in its own stub files. The shared files (`play/mod.rs`, `select.rs`, `world.rs`) take small additive edits only: record them here.

- **A. Melee:** `combat.rs`, `ai.rs`, `ragdoll.rs`, plus `assets/anim/movesets/{unarmed,longsword}.json`. Hit tests sweep the weapon/limb between frames; damage scales with closing speed; hitstop through `Time<Virtual>`; knockback through `Mover.external`; impulses into `MotionLayers.impulses`.
- **B. Traversal, then ranged and magic:** `traverse.rs` (select already handles the Swim/Climb/Ledge layers), then `ranged.rs` and `magic.rs` (projectiles emit `Hit`).
- **C. Character systems:** `docs/CHARACTER_SYSTEMS.md`. Its 6 open questions need the user's answers before any code.
- **D. Pose tweak tool:** `motion/tweak.rs` (F2).

Rules: keep the module header docs in step with the code. Use the monitor log to back motion claims. Stop for a playtest at each milestone.
