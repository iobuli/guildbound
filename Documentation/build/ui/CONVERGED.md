# HUD — CONVERGED (Aura + Claude lanes)

Inputs: `AURA-brainstorm.md` (written blind) and `BRAINSTORM_CLAUDE.md` (seeded by my earlier mapping —
Claude flagged this honestly and his lane is owned as such). Both read against `Kit/LAYOUT.md`,
`Kit/Runtime/HudView.cs` and `Kit/Sprites/kit.json`.

Evidence weight, stated up front: my lane is grounded in five fetched gamedesignskills posts (cited in my
file); Claude's article fetch failed and he cites none. Where an idea rests on article reasoning I say so.
Where BOTH lanes reached the same idea without the other's prompt, that is marked **AGREED** and is the
least risky thing on this list.

---

## A. AGREED — the two lanes independently arrived here

**A1. Contextual density, state-driven (my I1 = his 6).** Identical mechanism: derive a weather from game
state and drive the *existing* per-layer opacity floors, blend don't cut. TOWN parks the combat panel and
opens the ledger/wallet; APPROACH expands the objective surface; FIGHT fades everything but HP, guard,
fire and alerts. Keeps the contracted centre third empty.
→ **Take it first.** It is the container that makes every other idea safe, and both lanes reached it
without prompting.

**A2. The wallet must make progression legible (my I3 = his 4).** Same goal, complementary halves: I show
the *forward* cost (price ring + cost-in-actions: "≈ 6 contracts"), he shows the *backward* rate (7-payout
sparkline). Take both — one says where money comes from, the other what it is for. Article grounding: the
economy post's two stated goals are motivate ("track their progress towards a given goal") and guide
progression ("each resource represents a unique set of actions — need fins? slay fish").

**A3. The past belongs on the board (my I7 = his 3).** Same mechanism, two cases: a completed contract and
a defeat both write a struck line to the board's ledger instead of vanishing (his "the sad path carries
consequence and information" is the better sentence). One list, both outcomes.

**A4. The portrait column must mean something or go (my I4 = his refusal).** Both lanes independently
refused a decorative column. Settled below in D.

**A5. No import of the reference art.** Both lanes. Not owned; restyle the navy/cyan SDF kit.

## B. AGREED BY ONE LANE, ACCEPTED BY THE OTHER

**B1. Objective tick on the compass tape (his 5, accepted).** The tape already slides at 5 px/degree and
already has a fixed centre marker; an objective bearing tick — with distance in *steps*, not metres —
costs one more tick and makes the compass earn its place in town, where it currently shows north and
nothing else. This is the best cost-to-value item either lane produced.

**B2. Input buffer pip (his 2, accepted).** When a press is buffered inside the 0.25 s late-recovery
window, a pip appears and clears when the buffer is consumed or expires. Players learn the buffer by
seeing it. Pairs with my input echo (I6): echo answers the *frame*, the pip reports *held intent*.

**B3. Earn the HUD (my I5, accepted).** Gate surfaces on what the fiction has taught — first guard, first
charge, first contract, first coin — one shot each, reusing `toast_frame`/`UiPulse`. Directly serves the
quest post's drop-off warning. Claude's lane did not reach this; he raised no objection to it.

**B4. Reduced motion as a first-class path (my I9).** Not a feature, a constraint on all of the above:
every animation ships with a reduced twin, and A1's phase transition is the same code path with
interpolation disabled. `HudSettings.ReducedMotion` already exists and currently flips two ternaries.

## C. WHERE THE LANES DISAGREE, AND HOW IT RESOLVES

**C1. The 3-row always-on quest list. He drops it; I keep it.** His reason is sound and I am not
overruling it: three 440x56 text rows beside a 600x264 combat panel is a lot of left-column weight during
a fight. My reason is also sound: `HudView` 155-157 shows `objectiveRoot` is active **only while
carrying**, so the surface he wants to drop is blank most of the time, and the quest post warns that not
knowing what to do *and why* is what causes drop-offs.

**Resolution — and it is the convergence of his own weather system with my ledger, not a compromise:**
the three rows are real, but they exist only in TOWN and APPROACH. In FIGHT the surface collapses to
**one row** — the primary objective — still carrying its archetype glyph. The player never faces three
rows of prose mid-exchange, and the town HUD stops being blank. Each row is
`[archetype glyph] [verb + target] [stake line]`, the stake line being the "why".
Plus one design rule the board can enforce: **never offer two contracts of the same archetype at once** —
the quest post names rotation between types as the retention mechanism, so variety is a system property,
and the board shows the mix as a glyph row so the player sees what they are choosing between.
This is the answer to the boss's question (a): **list, yes — one row of it in a fight.**

**C2. The opponent tell. He ranks it #1; I do not, and I would move it.** Not a disagreement about
revealing hidden state — he is careful to say it only shows what the player could infer. Two objections:
(a) **placement.** A tell that matters inside ~0.25 s is read at the top-right corner, in a 216 px disc,
against a 256 px frame — the worst latency-to-focus on the board. The same data belongs **over the
opponent in world space**, where the player's eye already is. (b) **hollowing the telegraph.** Stage 7 is
about to spend real effort keeping telegraphs readable (contact hold .025-.045 s, camera response ≤ ~1°/4 cm).
An always-on arc that reports Startup/Active/Recovery makes the minimap the primary read and the animation
the backup — which would quietly delete the value of that work.
**Resolution:** build it, relocate it to a world-space marker over the opponent, and gate it — **on by
default through the first few bouts** (it teaches the windows), then it retires to an options toggle
(`Cues: Off / Learning / On`). This answers his open question: the rules **do** publish it, behind an
accessibility/learning switch rather than as a permanent combat HUD element. Same data, same cost, no
erosion of the feel work.

**C3. Foldable panels (his 7).** Manual and automatic both: A1's weather is the default, and a hold-to-pin
overrides it and is remembered per surface. Claude's "hold-to-pin, not a menu" is the right interaction.

## D. Answers to the boss's two pending questions

**(a) Always-on quest list vs single objective pill?** — A 3-row ledger in town and approach, collapsing
to **one** row in combat. Not the pill (it is blank most of the time) and not three rows of combat weight.

**(b) Does the reference's portrait column become a real roster?** — **No**, and both lanes refused it for
the same reason: a vertical roster would occupy the last free left-edge space and sit empty in a game that
is 1v1 plus contracts. Instead: the portrait slot already in the combat panel (120x120 at 20,18) becomes
**the person you are currently bound to** — the giver — with a trust band chip beneath it and a frame that
fills with `TrustProgress01`. The **full** roster becomes a 4th page in the board modal off the existing
`portrait_slot` sprite, and picking a friend filters the board to their contracts. The quest post's
"personal side quests ... make the world feel more lived-in" is the mechanism; trust bands are ours.

## E. Ranked build order — what I recommend the boss approves

| # | Item | Why here | Art needed |
|---|---|---|---|
| 1 | A1 weathers + C3 hold-to-pin | agreed by both, cheap, container for the rest | none |
| 2 | B1 compass objective tick | best cost-to-value either lane produced | none |
| 3 | C1 objective ledger, 3-in-town / 1-in-fight | answers (a); the surface is idle today | none (existing glyph sprites) |
| 4 | A2 cost-in-actions + payout sparkline | the shop has no identity yet | none |
| 5 | B2 buffer pip + my input echo | legibility of the 0.25 s window; "responsiveness" leg | none |
| 6 | C2 opponent tell, world-space, learning-gated | highest risk to the feel budget — do it once 7's budgets are fixed | one ring |
| 7 | A3 board ledger of the past | needs the economy model to exist first | none |
| 8 | B3 earn the HUD | cheap, but best judged against real first-run content | none |
| 9 | D bound contact + roster page | needs the trust/contact model | none |

## F. Refused, by both lanes or by me as lane owner

- Reference art import (not owned; restyle only).
- A "second bar" because the reference has one — we ship HP + 5 guard segments, strictly more information.
- Re-flowing contracted positions in LAYOUT.md; the compass and round map are built and bound.
- **The hotbar stays stage 8.** A 5-slot hotbar over an inventory with no equip rule is decoration; it
  follows the equipment model, it does not lead it.

## G. The one interface ask (model side — Claude's lane)

Everything above lands inside `Assets/Game/UI/**` except the data it reads. One addition to `IHudModel`,
consumed by `HudView`, owned by Claude:

- `float Engagement01` — drives A1's weather crossfade (0 = town, 1 = fight). `InCombat` alone cannot
  blend, and `HudView` has no world transform to measure proximity itself.
- Objective rows: `int ObjectiveCount`, `GetObjective(i)` → `{ Archetype, Verb, Target, Stake, BearingDegrees, DistanceSteps, Primary }`
  (the bearing/distance also feeds B1's compass tick).
- Wallet: price per shop item, the item's **value in actions**, and `float[] RecentPayouts` (7).
- Ledger: `int LedgerCount`, `GetLedgerLine(i)` → `{ Text, Amount, Outcome }`.
- Buffer: `bool InputBuffered` + remaining 0..1.
- Tell: `int OpponentPhase` (Startup/Active/Recovery), `float OpponentPhase01`, `bool TryGetOpponentMarker(out Vector3 world)`
  (world position only — `HudView` projects it).
- Contacts: `int ContactCount`, `GetContact(i)` → `{ Name, Trust01, Band, ContractCount }`.
- Known surfaces: a mask, for B3.

Nothing in G is drawn work; it is the read surface the HUD needs. None of it blocks stage 1.

## H. Process

- Lane files are read-only from here; this file supersedes both for build decisions.
- All of E lands in Aura's lane (`Assets/Game/UI/**`). Claude never opens it.
- The chain-table blend snap is unaffected by any of this and stays the top implementation item in his lane.
