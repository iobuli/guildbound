# Tasks (Bevy)

| # | Task | Owner | Status |
|---|------|-------|--------|
| 1 | Install the Rust toolchain (rustup, stable MSVC) | claude | done: rustc 1.99.0; builds capped at 6 jobs |
| 2 | Workspace: plain sim crate (no Bevy types) + Bevy game crate, Bevy version pinned | claude | done: Bevy 0.20.0, sim 7/7 tests, dev panel + `gb.*` remote verified live |
| 3 | Port the deterministic combat core and its fixtures (`git show unity-final:Assets/Game/Combat/...`) to the sim crate, `StateHash()` replay test | claude | open |
| 4 | Licence check: may Asset Store pack meshes/clips be used outside Unity? | boss | done: boss holds the licences for all packs (Decision 21) |
| 5 | Pick one hero rig; offline retarget + FBX -> glTF pipeline (Blender) | codex | open |
| 6 | Convert every pack: one GLB per rig (model + its own clips) with `Tools/blender/fbx_pack_to_glb.py`; packs in `guildbound-unity-archive/bevy-move-2026-10-11/Assets` (Explosive, Kevin Iglesias, MoCapCentral, Idle MoCap, Brawler; props: RPGPP, Tavern, Anime Trees) | claude | in progress: FightingMotions (42 clips), RPG Character unarmed (40 clips, baked from 3ds Max empties) |
| 7 | Panel: header `Animation [<library v>]` dropdown over every GLB in `assets/packs/*/`; picking one swaps model + clip list (model picker) | claude | done: roles + clusters resolve per pack, gb.library |
| 8 | Normalization in the Blender script: one height (1.80 m), feet at y=0, +Z forward, root motion stripped to in-place with travel saved in the report; then retarget clips across rigs onto the chosen hero so every model plays every clip | claude | partly done: converter normalizes height (1.80 m) and feet; runtime root-motion lock keeps travel clips in place; cross-rig retarget still open |
| 9 | Combat clusters: extend `anim::CLUSTERS` (type + limb, rotating variants) across all converted packs, then drive them from the sim move table (COMBAT_MOTION_PLAN phase 1) | claude | in progress: 7 clusters (J K U I L O Y incl. run punch) resolve per pack; sim move table next |

Unity tasks 1-12 closed with the engine change on 2026-10-11; history: `git show unity-final:collab/TASKS.md`.
Aura was Unity-only, so its task/design/review authority ended with Unity. Review authority sits with the boss until reassigned.
