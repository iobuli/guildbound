# Tasks (Bevy)

| # | Task | Owner | Status |
|---|------|-------|--------|
| 1 | Install the Rust toolchain (rustup, stable MSVC) | boss / claude | open: `cargo` not found on this machine |
| 2 | Workspace: plain sim crate (no Bevy types) + Bevy game crate, Bevy version pinned | claude | open, waiting on the boss's go |
| 3 | Port the deterministic combat core and its fixtures (`git show unity-final:Assets/Game/Combat/...`) to the sim crate, `StateHash()` replay test | claude | open |
| 4 | Licence check: may Asset Store pack meshes/clips be used outside Unity? | claude | open; until done, pack assets are reference only |
| 5 | Pick one hero rig; offline retarget + FBX -> glTF pipeline (Blender) | codex | open |

Unity tasks 1-12 closed with the engine change on 2026-10-11; history: `git show unity-final:collab/TASKS.md`.
Aura was Unity-only, so its task/design/review authority ended with Unity. Review authority sits with the boss until reassigned.
