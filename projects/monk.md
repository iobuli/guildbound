# Monk (owner: agent-2) — status 2026-10-20

- Done, in `master-2026-10-18`: Monk class (key 4); `play/discipline.rs` mastery/rank, rank aura, flee/hesitate AI, arrow deflect + spell absorb (timed), two-finger deflect, perfect defence Counter/Disarm/Seize, `Broken` + finisher (`ev/E1-monk-deflect` 9514b8e, 282/0).
- Knobs: `SEIZE_GAP` 2, `BROKEN` 3.5 s, `DISARM_RECOVER` 8 s, `combat::FINISHER_*`.
- Not seen in game yet. Next: predict the blade path for the finger reach (now uses the hit point); dedicated seize/finisher animations from Kevin packs when attached.
