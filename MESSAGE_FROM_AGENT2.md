# To agent-1, from agent-2 (combat / motion) — 2026-10-20

Pushed into your active branch `ev/kev-categoriser` as a note (this file only; delete it when read).
**Pull before your next push**: this commit sits on top of `0f06fe1`.

## Where I am
- Nothing uncommitted. My last code is `ev/E1-monk-deflect` (`9514b8e`), already inside
  `master-2026-10-18`: two-finger deflect, perfect defence → Counter / Disarm / Seize, `Broken`
  foes, monk finisher (`play/discipline.rs`, `play/combat.rs` resolve). 282/0 when pushed.
- I'm stopping here unless the user gives me more; the Monk is the only thing I was asked to fix.

## Things your branch should know
1. **User's call: no CMU/Rokoko strikes** (agent-3's `068a97c`, in your branch). That overrides my
   earlier Monk mocap plan. Consequences in my code:
   - `discipline::variety` (rank → chance of a library strike) now draws from the UAL/Kevin library
     only — fine, no change needed, but monks will look samey until Kevin packs exist.
   - `motion/bank.rs::place_on_floor` still places the 67 CMU takes on the floor at build; harmless,
     but they're now dead weight in the bank (~1 s build, memory). Your call: drop `CMU_DIR` loading
     once nothing uses it.
2. **Monk perfect-defence tiers live in one place:** `discipline::perfect_outcome(defender, attacker,
   attacker_unarmed)` and the `Riposte::Perfect` arm of `combat::resolve`. If Kev h2h adds per-weapon
   heavies/guards, keep blows that are `HitKind::Melee` from a weapon going through it so disarm /
   seize still trigger. `SEIZE_GAP` (2 ranks), `BROKEN` (3.5 s), `DISARM_RECOVER` (8 s) are the knobs.
3. **Per-limb hurtboxes (`60cf925`) vs the monk catch:** the deflect hand reach uses `hit.point`
   (`DeflectReach`). With limb hurtboxes the point moves to the limb — should look better; check the
   fingers still land on the blade side, not inside the forearm.
4. **Charge clash (`8c38431`) vs Broken:** a `Broken` fighter is in a long react; make sure a clash
   can't start on it (it would cancel the finisher window).
5. **Drift:** your branch is 48 commits over master. Land it (or a slice) soon; I can review it
   read-only if the user wants.

## Open items nobody owns (from the board)
- E11 bag / crafting / debug stats: my helper produced nothing (worktree empty). Unclaimed.
- `GB_RIVAL=1` panic (insert on a despawned entity after dressing / insignia) — was yours; I don't
  see a fix on any branch.
- Stale branches to tidy when convenient: `ev/ultra-posture-aim`, `ev/E20-contract-board`,
  `ev/E21-rumours`, `ev/E22-weather(-v2)`, `ev/E7b-limp-loop` (all behind master, nothing new),
  `NEW_ASSETS_TesterUAT]` (odd name with a `]`, a revert on top of the user's asset commit).

User ideas arrive on `agent-log/IDEAS.md` (`ps <text>` from their PC) — read it each turn.
