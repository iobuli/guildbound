# To agent-3, from agent-2 (combat / motion) — 2026-10-20

Pushed into your active branch `ev/E24-fixes` as a note (this file only; delete it when read).
**Pull before your next push**: this commit sits on top of `068a97c`.

## Where I am
- Nothing uncommitted. My last code is `ev/E1-monk-deflect` (`9514b8e`), inside `master-2026-10-18`:
  monk two-finger deflect, perfect defence → Counter / Disarm / Seize, `Broken` foes, finisher.
- I'm stopping unless the user gives me more.

## Thanks / notes on your fixes
1. **Black capsule on monks** (`d938dbf`): that was my rank aura (`discipline::rank_aura`, a
   `WindAura` capsule child). Your alpha-0 fix in `wind_aura.wgsl` is right. Two follow-ups if you
   touch it again: the aura is a plain `Capsule3d` sized for the mannequin — on Thoral (×1.2) and the
   hero bodies it may sit inside the shoulders; and the counter flare (`discipline::flare`,
   expanding `WindAura` sphere) uses the same shader, so check it still shows after the alpha change.
2. **Eyes** (`e356de2`): good catch. The hand fit checker (`HandRig::fit`, `GB_CHECK=1`) is the same
   idea for fingers if you ever want a measured check instead of a screenshot.
3. **Ragdoll (E7 / E7b limp loop):** the monk finisher throws the body with a `RagdollRequest`
   (`combat::FINISHER_THROW`, 6 m/s + lift). If your limp/down logic counts deaths per body, a
   finisher on a `Broken` foe is one more path into it — worth one run with `GB_GOD_KILL` vs a
   finisher to see both settle with `[ragdoll] down` ≈ 0.
4. **Disarm drops a blade prop** (`discipline::disarm`): a `Debris`-layer dynamic cuboid that
   despawns after 8 s. If your environment pass changes `Phys::Debris` collisions or ground
   colliders, it should still land on the terrain.

## Open items nobody owns (from the board)
- E11 bag / crafting / debug stats — unclaimed (my helper produced nothing). Your E8 panel could
  absorb the debug-stats part.
- Kevin Iglesias packs: you asked the user to attach them; until then the monk's library strikes
  are UAL-only.

User ideas arrive on `agent-log/IDEAS.md` (`ps <text>`) — read it each turn.
