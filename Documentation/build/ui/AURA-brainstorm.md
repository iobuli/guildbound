# HUD lane — AURA (sealed independent brainstorm)

Status: SEALED. Written without seeing Claude's lane, and before he sees this one.
Scope: Aura's lane only — Assets/Game/UI/**, Tools/, scene layout, editor windows. Model-side asks are named, not made.

---

## 1. What I actually read, and the sentence each idea hangs on

Five posts fetched this session. The four combat-system posts and /video-game-tutorial/ were cut off by
my fetch truncating; I did not read them and I do not cite them.

| Post | The line that carries weight |
|---|---|
| /game-design/game-quest-types/ | "designed with clarity in mind as players need to know what to do **and why**; doing otherwise causes frustration and drop-offs." / "**Rotating between quest types** helps retain play engagement, as does mixing objectives together, like a hybrid escort-delivery quest." |
| /game-design/quests/ | quests "give players reasons to **interact with the world's inhabitants and locations**" / "**Personal side quests** related to NPCs ... make the game world feel more **lived-in**." |
| /game-design/economy-design/ | the two goals are to **Motivate** (a currency exists "because we want players to be able to track their **progress towards a given goal**") and to **Guide progression** / "Ideally, each resource and currency **represents a unique set of actions** and decisions within the gameplay. **Need fins? Slay fish.**" |
| /game-design/game-feel/ | game feel = "**responsiveness, intuitiveness, viscerality**"; responsiveness is "reducing delay from **intent to input to action to feedback**." |
| /game-design/core-loops-in-gameplay/ | fetched, truncated early. The loop is the unit the player repeats; the HUD's job is to keep the repeat legible. |

## 2. What already exists, so nobody re-proposes it

- Compass (tape, N..NW x2 at 225 px, 5 px/deg, fixed marker) and round minimap (256 frame, 216 masked
  window, honest "NO MAP DATA") — built AND bound (`HudView.UpdateMap`).
- HP `MeterView` + 5-segment guard row + FIRE slot with charge ring and cooldown disc; 2 alert slots.
- Modal with 3 tabs: BOARD (2 contract cards + trust/wallet blocks), INVENTORY (12 slots, details pane),
  OPTIONS (5 sliders + 3 toggles). Undrawn: nothing. Layout contract is LAYOUT.md.
- Runtime components already in place: `HudLayer`, `MeterView`, `SegmentRow`, `UiPulse`, `UiSwitch`,
  `UiSelectGlow`, `ClusterFollower`. Sprites already in place include `keycap_frame`, `ornament_diamond`,
  `portrait_slot`, `toast_frame`, `ring_mask`, `disc_mask`, `header_bar`, `divider`, `vignette`.
- `objectiveRoot` is set active **only while carrying** (`HudView` 155-157) — so the always-on objective
  surface is blank in town. That is the cheapest real estate in the HUD and it is currently idle.

## 3. Ideas

### I1 — loop-phase "weather" instead of a player-managed opacity slider
`PanelOpacity / PromptOpacity / AccentOpacity` are static player sliders today; every panel is on always.
Instead, `HudView` derives one phase — TOWN / APPROACH / FIGHT — and drives the **existing** per-layer
floors from it, blended over ~0.25 s rather than cut. FIGHT: the board/shop/map cluster parks (the map
collapses to its rim so the corner never flashes), ledger rows expand. TOWN: the combat panel parks to a
thin HP+guard strip, ledger and wallet open.
Cost: ~40 lines in `Tick`, no new art. FIGHT/TOWN are free from `InCombat`; APPROACH needs one model bool.
Why it is not what we already do: the HUD stops being a static frame the player manages and starts reading
the fiction back to them — the "feedback" leg of the game-feel loop.

### I2 — the always-on objective surface becomes a 3-row ledger; every row carries its archetype + its stake
Directly from "know what to **do and why**" plus "rotating between quest types retains engagement".
Promote the pill to three rows of 440x56 stacked from y 308 → occupies x 32..472, y 308..488, which is
**outside** the contracted centre third (640..1280 x 360..720), so LAYOUT.md holds. Each row:
`[archetype glyph] [verb + target] [stake]` — the stake line is the "why" the article says its absence
causes drop-offs. Glyphs for the archetypes we implement (kill / fetch / deliver / escort / explore) come
from existing `ornament_diamond` + `icon_slot_*` sprites; no new art.
And the rule the UI can enforce: **the board never displays two contracts of the same archetype**, because
rotation *is* the retention mechanism. Show the archetype mix as a glyph row on the board so the player can
see the variety before they commit.
Cost: layout + row binding on my side; three rows of `(archetype, verb, target, stake)` from the model.
This answers the boss's pending question (a): a list, yes — but not three identical lines.

### I3 — price items in **actions**, not gold
"Each resource represents a unique set of actions... Need fins? Slay fish." Our modal shows a bare
`Wallet` integer and the shop has no identity yet.
Each shop row renders its price as a **progress ring** (wallet/price, via existing `ring_mask`) plus a
cost-in-actions label — "≈ 6 contracts", "2 hunts" — from a per-source average the economy model already
has. Ring full → the row elevates to buyable. Browsing the shop *becomes* the progression readout; no
second panel, and it is literally the article's own framing of what a currency is for.
Cost: one new economy-model figure per item (its value in actions). Ring + frame art already exist.

### I4 — the portrait column is **not** a roster. It is one bound contact.
"Personal side quests ... make the game world feel more lived-in" is the answer to the reference's
portrait column, which has no owner. An always-on vertical roster would occupy the last free left-edge
space and sit **empty** most of the time — the game is 1v1 plus contracts.
So: answer question (b) **no** to an always-on roster, **yes** to a bound contact. The portrait slot
already in the combat panel (120x120 at 20,18) becomes the person you are currently bound to — the
contract giver — with a trust band chip beneath it and a frame that fills as `TrustProgress01` rises.
The **full** roster is a 4th page inside the board modal, one row per guild member off the existing
`portrait_slot` sprite, and picking a friend filters the board to *their* contracts.
Cost: one modal page, one model call (`GetContact(i)`). No new always-on space, no new art.

### I5 — earn the HUD: gating as onboarding
Everything is visible from frame one and nothing in the model tracks what the player has been *taught*.
Add a "known surfaces" mask: fire slot, guard row, recover ring, alerts, wallet and each ledger row
appear the **first time the fiction gives a reason** — first guard taken, first charge held, first
contract accepted, first coin. One-shot reveal off `toast_frame`, reusing `UiPulse`. Nothing is explained
twice, and the first-run HUD is not a wall. Gating on *known* rather than on player preference is rare in
this genre and speaks straight to the drop-off warning.

### I6 — input echo: the HUD answers on the frame the input is read
Responsiveness is "reducing delay from intent to input to action to feedback". `UiPulse`/`UiSelectGlow`
today fire only on **state change**, so a *refused* input (fire locked, guard broken, inventory in combat)
looks like nothing happened. Give every actionable element a ≤60 ms micro-response the instant the
InputAction is read, before any model round-trip; the state-driven response then plays over it when state
actually changed. Two-stage feedback turns "my input was ignored" into "my input was answered".
Cost: feed the input frame into `HudView`, reuse `UiPulse`.

### I7 — the board keeps a struck ledger of what you have done
The board shows live cards and nothing else; completing a contract leaves no trace. Give the board page a
lower band: completed contracts stay as struck one-line entries with the giver's name and the reward paid.
It is the cheapest possible "lived-in" surface, it is the player's own record, and it needs no new art
(`divider` + `header_bar` + the existing text style). Cost: a scroll band + a history list from the model.

### I8 — density follows danger
Viscerality. Panel scale and vignette weight track HP/threat: 0.97 scale at full health, 1.0 with glowing
segment gaps at critical. The HUD leans in when it matters and gets out of the way when it does not.
Cost: one `Lerp` in `Tick` plus the existing vignette path.

### I9 — reduced motion is a first-class path, not an exception
`HudSettings.ReducedMotion` currently flips two ternaries. Every animation above must ship with a defined
reduced twin (opacity crossfade only — no slide, no pulse), and I1's phase transition must be the *same*
code path with interpolation disabled. Dev check: no HUD element has an animation without a reduced twin.
It is nearly free at design time and nearly impossible to retrofit.

## 4. Ranked for the boss

1. **I2** three-row ledger, archetype glyph + stake line — answers question (a), and the slot is idle today
2. **I3** price in actions — the shop currently has no identity at all
3. **I4** bound contact, not a roster column — answers question (b)
4. **I1** loop-phase weather — biggest felt change per line of code
5. **I5** earn the HUD
6. I6 / I7 / I8 / I9 — individually small, all of them cheap

## 5. Explicit non-goals (so we do not rebuild what is already done)

- **No re-flow.** Positions and sizes are contracted in LAYOUT.md; the compass and round map are already
  built and bound. Changing them buys nothing.
- **No "second bar" just because the reference has one.** We ship HP + five guard segments, which is
  strictly more information than HP + one bar. The reference is a layout to match, not a checklist.
- **No import of the reference art.** It is not owned (no such package in Assets/). Restyle of the
  navy/cyan SDF kit only.
- **The hotbar stays stage 8.** A 5-slot hotbar over an inventory with no equip rule is decoration; it
  should follow the equipment model, not lead it.
