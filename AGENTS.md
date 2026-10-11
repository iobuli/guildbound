> ACTIVE TASK (2026-10-11): First-Principles Tandem GDD Loop. Read TANDEM_CLAUDE.manifest.md / TANDEM_CODEX.manifest.md and TANDEM_REASONING.log first; they override the rest of this file for that task.

# AGENTS.md (Codex)

## Rules
- Simplest working solution, read before editing, no speculative features.
- Never invent file paths, APIs or fields; if unknown, say UNKNOWN.
- Plain hyphens and straight quotes only.

## Guildbound (Bevy)
- Engine: Bevy (Rust), code only, no editor. Unity was dropped 2026-10-11; last Unity state is tag `unity-final` (`git show unity-final:<path>`). Branch `build/bevy`.
- Read Documentation/ALPHA_LESSONS.md first, then Documentation/HANDOFF.md and Documentation/MASTER.md (vision; sections 8-9 combat look and elements apply directly).
- Old engines are reference only: `_OldGodot/` zip and the `unity-final` tag. No GPL code copying, no pack demo scripts as our code. Asset Store pack licences outside Unity are UNKNOWN until Claude checks them.
- Team: user = boss. Codex = creative dev (visual direction, feel, VFX, art). Claude = logical dev (architecture, verification, tests, licensing).
- Coordinate only through collab/CHANNEL.md (append-only, `## [codex] YYYY-MM-DD HH:MM - topic`, four parts: Done / Review / Needs / Next), collab/TASKS.md, collab/DECISIONS.md.
- After posting, wait with `powershell -NoProfile -File Tools/collab_wait.ps1 -Me codex` (shell timeout 600000 ms; 0 = new entry, 3 = rerun, 2 = user STOP). A direct user message always wins.
- Deliver code uncommitted; Claude verifies (`cargo build`, `cargo test`) and commits. Never commit asset packs, target/ or _OldGodot/.
