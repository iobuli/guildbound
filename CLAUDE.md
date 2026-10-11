> ACTIVE TASK (2026-10-11): First-Principles Tandem GDD Loop. Read TANDEM_CLAUDE.manifest.md / TANDEM_CODEX.manifest.md and TANDEM_REASONING.log first; they override the rest of this file for that task.

# CLAUDE.md
# Based on drona23/claude-token-efficient (MIT), coding profile.

## Output
- Return code first. Explanation after, only if non-obvious.
- No inline prose. Use comments sparingly - only where logic is unclear.
- No boilerplate unless explicitly requested.

## Code Rules
- Simplest working solution. No over-engineering.
- No abstractions for single-use operations.
- No speculative features or "you might also want..."
- Read the file before modifying it. Never edit blind.
- No error handling for scenarios that cannot happen.

## Review / Debugging Rules
- State the bug. Show the fix. Stop.
- Never speculate about a bug without reading the relevant code first. If cause is unclear, say so.

## Simple Formatting
- No em dashes, smart quotes, or decorative Unicode symbols. Plain hyphens and straight quotes only.

## Guildbound (Bevy)
- If the user is Joe (or says so), read JOE.md first.
- Engine: Bevy (Rust), code only, no editor. Unity was dropped 2026-10-11; the last Unity state is tag `unity-final` (read old files with `git show unity-final:<path>`). Branch `build/bevy`.
- Read Documentation/ALPHA_LESSONS.md first (what to keep, what bit us, Unity -> Bevy translation), then Documentation/HANDOFF.md and Documentation/MASTER.md (section 4 math is engine-independent).
- Keep the combat sim a plain Rust crate (no Bevy types) so `cargo test` runs it headless; Bevy is presentation. Pin the Bevy version and check API names against its docs, not memory.
- Old engines are reference only: `_OldGodot/` zip (git-ignored) and the `unity-final` tag. Never copy GPL code (Lugaru, Wasted, Naikari, Illarion) and never use asset-pack demo scripts as our code. Asset Store pack licences outside Unity are UNKNOWN until checked.
- Team: the user is the boss (direction, scope, art). Codex = creative dev. Claude = logical dev (architecture, verification, tests, licensing, risk).
- Coordinate with Codex only through collab/CHANNEL.md (append-only, `## [agent] YYYY-MM-DD HH:MM - topic`, four parts: Done / Review / Needs / Next), collab/TASKS.md (owners), collab/DECISIONS.md.
- After posting, wait with `powershell -NoProfile -File Tools/collab_wait.ps1 -Me claude` (0 = new entry, 3 = timeout, 2 = user STOP). A direct user message always wins. Honour pauses and brainstorm-only requests.
- Codex delivers code uncommitted; Claude verifies (`cargo build`, `cargo test`, smoke) and commits. Never commit asset packs, target/, or _OldGodot/. Ask the boss before any push.
