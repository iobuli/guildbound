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
- Engine: Bevy (Rust), code only, no editor. Branch `build/bevy`. Unity was dropped 2026-10-11 (tag `unity-final`, read with `git show unity-final:<path>`).
- Start from Documentation/HANDOFF.md. Read Documentation/ALPHA_LESSONS.md and MASTER.md (section 4 math) only when the task needs them. GDD of record is the last MASTER_GDD block in TANDEM_REASONING.log (large: grep it, never read it whole).
- Combat sim is a plain Rust crate (`crates/sim`, no Bevy types) so `cargo test` runs it headless; `crates/game` is Bevy presentation. Pin the Bevy version and check API names against its docs, not memory.
- Old engines are reference only: `_OldGodot/` zip (git-ignored) and the `unity-final` tag. Never copy GPL code (Lugaru, Wasted, Naikari, Illarion) and never use asset-pack demo scripts as our code. Asset Store packs: the boss holds the licences (Decision 21 in collab/DECISIONS.md); converted content stays git-ignored in assets/packs/.
- Team: the user is the boss (direction, scope, art). Claude does the dev work end to end. Codex is not in the loop; collab/ is history plus DECISIONS.md. No subagents or workflows unless the boss asks.
- Boss tests in the viewer: after a change, `cargo build -p guildbound`, relaunch, report what changed, read boss notes from collab/devfeed.log. No screenshot loops.
- Commit only when the boss says. Never commit asset packs, target/, or _OldGodot/. Ask before any push.
- If the user is Joe (or says so), read JOE.md first.
