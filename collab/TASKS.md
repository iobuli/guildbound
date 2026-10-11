# Tasks (Bevy)

| # | Task | Owner | Status |
|---|------|-------|--------|
| 1 | Install the Rust toolchain (rustup, stable MSVC) | claude | done: rustc 1.99.0; builds capped at 6 jobs |
| 2 | Workspace: plain sim crate (no Bevy types) + Bevy game crate, Bevy version pinned | claude | done: Bevy 0.20.0, sim 7/7 tests, dev panel + `gb.*` remote verified live |
| 3 | Port the deterministic combat core and its fixtures (`git show unity-final:Assets/Game/Combat/...`) to the sim crate, `StateHash()` replay test | claude | open |
| 4 | Licence check: may Asset Store pack meshes/clips be used outside Unity? | claude | open; until done, pack assets are reference only |
| 5 | Pick one hero rig; offline retarget + FBX -> glTF pipeline (Blender) | codex | open |

Unity tasks 1-12 closed with the engine change on 2026-10-11; history: `git show unity-final:collab/TASKS.md`.
Aura was Unity-only, so its task/design/review authority ended with Unity. Review authority sits with the boss until reassigned.
