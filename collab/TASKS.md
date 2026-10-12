# Tasks (Bevy)

| # | Task | Owner | Status |
|---|------|-------|--------|
| 1 | Install the Rust toolchain (rustup, stable MSVC) | claude | done: rustc 1.99.0; builds capped at 6 jobs |
| 2 | Workspace: plain sim crate (no Bevy types) + Bevy game crate, Bevy version pinned | claude | done: Bevy 0.19.1 (Decision 22), sim 34/34 tests, dev panel + `gb.*` remote verified live |
| 3 | Port the deterministic combat core and its fixtures (`git show unity-final:Assets/Game/Combat/...`) to the sim crate, `StateHash()` replay test | claude / codex | current motor/combat/magic/mob slice implemented, 41 sim tests incl. replay and expanded rush/smash/dash; complete alpha fixture parity not audited |
| 4 | Licence check: may Asset Store pack meshes/clips be used outside Unity? | boss | done: boss holds the licences for all packs (Decision 21) |
| 5 | Pick one hero rig; offline retarget + FBX -> glTF pipeline (Blender) | codex | done: RPG Character hero; explicit bone/role map; 89 clips / 801 audited poses plus 2405 contact samples; scale-preserving isolated bakes and source-root unit/axis correction |
| 6 | Convert every pack: one GLB per rig (model + its own clips) with `Tools/blender/fbx_pack_to_glb.py`; packs in `guildbound-unity-archive/bevy-move-2026-10-11/Assets` (Explosive, Kevin Iglesias, MoCapCentral, Idle MoCap, Brawler; props: RPGPP, Tavern, Anime Trees) | claude | in progress: FightingMotions (42 clips), RPG Character unarmed (40 clips, baked from 3ds Max empties) |
| 7 | Panel: header `Animation [<library v>]` dropdown over every GLB in `assets/packs/*/`; picking one swaps model + clip list (model picker) | claude | done: roles + clusters resolve per pack, gb.library |
| 8 | Normalization in the Blender script: one height (1.80 m), feet at y=0, +Z forward, root motion stripped to in-place with travel saved in the report; then retarget clips across rigs onto the chosen hero so every model plays every clip | codex | RPG hero done: 40 native + 42 retargeted clips, in-place runtime root lock, measured stride sidecar; other target models not verified |
| 9 | Combat clusters: extend `anim::CLUSTER_SPECS` (type + limb, rotating variants) across all converted packs, then drive them from the sim move table (COMBAT_MOTION_PLAN phase 1) | claude / codex | 10 clusters, 32 RPG hero attacks with measured contact phase landmarks; other target rigs not audited |
| 10 | RPG hero controller, eight-direction walk/run, sprint, jump/fall, roll and combat transitions | codex | done: 20 live controller checks; sole sim motor, distance gait phase, rule-clock actions, pause/focus/reset fixes |
| 11 | Beta presentation cleanup: courtyard, outcome feedback profiles, HUD, implemented menu actions | codex | done: captures reviewed; overlays hidden in play; rematch action; adjustable camera shake |
| 12 | Expanded BT3-inspired controller and animation math | codex | implemented: four-hit rush, hit-confirm buffers, charged directional smash, counter blow, energy-cost dash rush; continuous phase matching and analytical aim/lean springs; Tools/check_combat.py verifies live |
| 13 | Overgrowth-inspired knockdown physics and procedural pipeline | codex | implemented: 11 dynamic RPG capsules, 10 limited joints, impact momentum, pause/step physics clock, eased get-up and cleanup; live stability probe gb.physics; anatomical hinges/self-collision/foot IK incomplete |
| 14 | Mage fireball/lightning on the RPG hero | codex | implemented: 7 retargeted mage clips, hand-mounted charge orbs, phase-aware driver, measured projectile launch point and readable fireball release; 3 live mage checks pass |
| 15 | Absorb pass: Sekiro prototype, UE5 melee, RPG template, souls-like template; `absorb` skill | claude | done: posture/dizzy/deathblow/backstab/notice/crit/move-cancel/timed drinks in sim (58 tests); study notes |
| 16 | RPG hero as nucleus: retarget non-idle Mixamo + UAL1/UAL2 clips, fps fix, audit | claude | done: 130 clips, audit pass; source packs moved to assets/packs/_sources |
| 17 | Smooth running: phase-synced 8-direction walk/run blend space with idle fade | claude | implemented, needs boss review in the viewer |
| 18 | Game feel: trauma shake, damage numbers, posture bars, notice markers, Kenney audio layer | claude | implemented, needs boss review |
| 19 | Courtyard dressing from Quaternius kits (props, nature, village fence), convex-hull physics | claude | implemented, needs boss review |
| 20 | Beta V2: Wenrexa UI rework (HUD, inventory, menus, icons) | claude | next |
| 21 | Beta V2: open world beyond the courtyard (streamed terrain, village, nature), Bestiary mobs | claude | planned |
| 22 | Beta V2: souls-like systems from Cat's template (quick slot, spawn sites, ladders/levers/gates, patrols, dodge-or-sprint key) | claude | planned |

Unity tasks 1-12 closed with the engine change on 2026-10-11; history: `git show unity-final:collab/TASKS.md`.
Aura was Unity-only, so its task/design/review authority ended with Unity. Review authority sits with the boss until reassigned.
