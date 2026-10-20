# Kev + hand-to-hand combat (owner: agent-1) — status 2026-10-20

Branch `ev/kev-categoriser` (latest b48fea5), not yet in `master-2026-10-18`. Game 332 tests, Kev 44 tests pass.

- Kev: `tools/kev/nodes.py` (joint table, body algebra, `--llm`), `roles.json` rules for UE / Mixamo / Kevin /
  Rigify / Auto-Rig Pro, `graph.py` + `pipeline.json` stages, `motion.py` per-clip labels, `h2h.py` per-weapon
  planner, `targets.py` body landing, `layout.py` region placement, `statue.py`, `fix_textures.py`.
  Read `tools/kev/ARCHITECTURE.md` first.
- Game: `play/clash.rs` (charge clash: mash MB1, streaks, impact frames, clash_fx), `play/hurtbox.rs` (11
  capsules per fighter; melee, spells, arrows refine against them; head 1.5x), `play/goop.rs` (rubber-band
  fists), `play/regions.rs` (packs on the doubled map from `assets/data/regions.json`), skill-shot spells in
  `play/magic.rs`, `repertoire::segment_explained`, `Movesets::versus`, `motion::SAMPLE_HZ` 60.
- Recordings: `tools/showcase/gif.sh`, `tools/showcase/turntable.sh` (viewer `GB_VIEWER_TURNTABLE`); `GB_RES=WxH`.
- Next: the user wants more videos (models tour, clash effects, regions); land a slice of the branch on master.
- Known: statues are baked bind poses (T-pose); the mech is not animated; packs are local-only (not in git).
