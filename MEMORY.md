# Memory: core facts

- The user (GitHub `iobuli`) plays and tests on their own PC (RTX 3060, Windows, repo at `D:\gb_overhaul`). Agents work in cloud sessions.
- Game: Guildbound, Rust + Bevy 0.19.1 + avian3d. Code in `crates/game/src` (`play/` gameplay, `motion/` animation).
- Shared base branch: `master-2026-10-18` (was `overhaul-2026-10-16`). Work goes on `ev/<id>-<name>` branches.
- Agents: agent-1 orchestrator (wardrobe, strikes / Kev); agent-2 combat + motion (Monk, discipline); agent-3 rendering, environment, ragdoll, god mode.
- Rules: agents code and build; test once per batch; light CPU (`CARGO_INCREMENTAL=0 nice cargo build -p guildbound -j 4`); no LLM in NPCs; post every push to `LOG.md`; stop testing when the user says usage is low.
- The user's messages are a stream of thought: turn them into plans, don't copy them verbatim.
