# AGENTS.md (Codex)

## Rules
- Simplest working solution, read before editing, no speculative features.
- Never invent file paths, APIs or fields; if unknown, say UNKNOWN.
- Plain hyphens and straight quotes only.

## Guildbound (Unity)
- Unity 6000.5.1f1, URP 17.5, new Input System, Animation Rigging, Cinemachine. Our code: `Assets/Game` (namespace `Guildbound`). Asset packs are git-ignored (see Documentation/HANDOFF.md).
- Read Documentation/HANDOFF.md first, then Documentation/MASTER.md (vision; sections 8-9 combat look and elements apply directly).
- Old Godot project: `_OldGodot/` zip, reference only. No GPL code copying, no pack demo scripts as our code.
- Team: user = boss. Codex = creative dev (visual direction, feel, VFX, art). Claude = logical dev (architecture, verification, tests, licensing).
- Coordinate only through collab/CHANNEL.md (append-only, `## [codex] YYYY-MM-DD HH:MM - topic`, four parts: Done / Review / Needs / Next), collab/TASKS.md, collab/DECISIONS.md.
- After posting, wait with `powershell -NoProfile -File Tools/collab_wait.ps1 -Me codex` (shell timeout 600000 ms; 0 = new entry, 3 = rerun, 2 = user STOP). A direct user message always wins.
- Deliver code uncommitted; Claude verifies and commits. Never commit asset packs, Library/ or _OldGodot/.
