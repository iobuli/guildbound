# E18 plan: baked NPC dialogue, lore and madlib events (Tavern's logic, our own files)

## Context
The user wants rich NPC dialogue, lore and choice-driven madlib events. The decision is to **bake** them: models write data offline, and the game ships JSON and picks lines deterministically. "No LLM in NPCs" at runtime stays true.

There is no need for TavernAI itself. We take its two useful ideas into our own simpler JSON:
- **character cards**: a persona plus voice samples;
- **keyword-triggered lore entries** ("World Info"): only the lore relevant to a situation goes into a prompt.

**Lore must come from the user.** The plan is built around a short intake loop where agent-1 drafts and the user corrects, so the user's effort stays small.

What exists to build on:
| Piece | Where |
|---|---|
| talk panel (line + numbered options, `Chose` → `guild` rewrites `Dialogue`) | `crates/game/src/play/talk.rs`, `guild.rs` |
| hand-written lines | `assets/data/guild.json` `lines` |
| deeds | `play/deeds.rs` `Deed{kind, tag, amount}` |
| traits madlib | `assets/data/traits.json`, `play/traits.rs` |
| contracts, titles, honours, occupations | `assets/data/*.json` |
| gibberish speech plan | `docs/GUI_PLAN.md` step 3 |

`docs/NPC_LORE_DESIGN.md` is written but uncommitted. It gets rewritten to this plan, with the Tavern dependency dropped.

## Phase 0: lore intake (user input, agent-1 drafts first)
1. agent-1 drafts a **starter lore kit** in `docs/lore/` from what the game already has:
   - names, areas (guild, village, ruins, pond), roles, `guild.json` lines;
   - contracts, honours, the order's monks, the oni folk, Thoral.

   Every guess is marked `[GUESS]` so the user only corrects.
   - `WORLD.md`: setting, era, tone, magic (Battlewill / Magicules), taboos (words and themes never to use).
   - `PLACES.md`: one block per area: look, history, who lives there, rumours.
   - `FACTIONS.md`: the guild, the order, the oni, bandits, Thoral's riders: aims, relations, ranks.
   - `CAST.md`: one block per named NPC (Mira, Varro, Thoral, Brother Ishin, Sister Ren, Elder Tadao, the Smith, the Farmer): look, wants, fears, voice, secrets, how they speak to each rank.
   - `HISTORY.md`: the events that shaped the region, and what NPCs believe versus what is true.
   - `EVENTS.md`: 10–20 one-line story-beat ideas, which the user can add to.
2. **The user corrects**, either by editing the files or by answering a short **interview in chat**. agent-1 asks about 10 questions per round and writes the answers in.
3. A compiler turns `docs/lore/*.md` into the data files (Phase 1). The markdown stays the human source of truth.

## Phase 1: formats, loader, checks (agent-1)
- **`assets/lore/cast/<id>.json`:** `name, role, look, personality, wants, voice, samples[], knows[]` (lore ids). Generic NPCs are built at bake time from `traits.rs` words plus their role.
- **`assets/lore/entries/<book>.json`:** `{keys[], text, order, always}`, Tavern's World Info logic in our own format.
- **`assets/lore/events/<id>.json`:** madlib beats:
  - `when` (situation tags, deed counts, rank, flags);
  - `cast` (roles bound to real NPCs and places);
  - `slots` (madlib words);
  - `text` templates;
  - `choices` → outcomes the game already has: contract patterns, standing, flags, follow-up event.
- **Situation tags (fixed list):**
  - `time`, `place`, `rank`, `standing`;
  - `last_deed` (a `deeds.rs` kind);
  - `wounded`, `armed`, `contract`, `met`, `weather`, plus event flags.
- **Intents (closed list):**
  - `greet`, `farewell`, `gossip`, `warn`, `offer_contract`, `turn_in`, `refuse`;
  - `react_deed`, `teach`, `trade`, `lore`, `taunt`, `plead`.
- **New `play/lore.rs`:** loads and validates all of it at start-up, as a resource `Lore`.
  - **Tests:** every file parses; every `knows` / `cast` / contract pattern resolves against the real data; `when` uses only known tags and deeds.
- **Compiler:** `tools/lore/compile.py`, `docs/lore/*.md` → `assets/lore/*`.

## Phase 2: the bake (tools/lore, dev machine with a model)
- **`plan.py`:** bake targets. Each target is a cast member × an intent × a pruned tag combination, plus each event × each slot combination. A budget report, so the user approves the size before any generation.
- **`bake.py`:** builds one prompt per target:
  - the card;
  - the lore entries whose keys appear in the situation (the World Info logic);
  - the intent and the situation in words;
  - the style rules from `WORLD.md`.

  It sends the prompt to any model server that speaks the OpenAI-style API (Ollama, KoboldCpp, llama.cpp, or a hosted model) and asks for N candidates. The run happens on a GPU box: Higgsfield's, or the user's PC with Ollama.
- **`filter.py`:**
  - length;
  - taboo words;
  - only known names and places;
  - duplicate removal;
  - optional kev-4b fit scoring: keep the best K.
- **kev-4b decision tables:** for each role × tag combination, the probability of each intent, written to `assets/lore/decide/<role>.json`. This needs about a 16 GB GPU, so it's optional; without it, hand rules decide.
- **Review loop (user input):**
  1. `review.csv` lists every kept line.
  2. The user strikes lines.
  3. `struck.json` keeps the strikes across re-bakes.
- **Output:** `assets/dialogue/<id>.json` (lines by intent and tags), with the events' text filled in. All committed JSON.

## Phase 3: runtime (deterministic; wiring with agent-2)
- **`lore.rs` lookup:**
  1. intent, from the rules plus the baked decide table;
  2. line, by (cast, intent, tags), with tag fallback, then `guild.json` lines;
  3. the pick is seeded by (npc, day, player), so it replays the same.
- **Events:** a slow check (every in-game hour, or on a deed or arrival):
  - binds `cast`, marks the giver, shows the beat on talk;
  - the choices write outcomes (a contract via `contracts`, standing, flags).
- **`guild.rs` / `talk.rs`:** call the lookup instead of the fixed `lines`. That's agent-2's area: either it does the wiring, or agent-1 does it with a board notice.
- **Speech:** gibberish → decode, per `GUI_PLAN` step 3, if it isn't built yet. The lines show as plain text until then.

## First slice (to prove it end to end)
- **Lore:** Mira and Varro, plus the guild and village entries.
- **Intents:** `greet`, `gossip`, `react_deed`, `offer_contract`.
- **Events:** two, for example "missing shipment" and "Varro's challenge".
- **Bake size:** about 300 lines.

Then the user plays it, reviews it, and we widen to the rest of the cast and places.

## Verification
- `cargo test -p guildbound` covers the `lore.rs` loader and validation tests and deterministic picks (same seed, same line; the lookup always finds a line).
- `tools/lore/check.py`: schema and cross-reference check of `assets/lore/*` in Python, run before every commit.
- In-game: talk to Mira after a kill, a contract, and at night. One screenshot each shows the right intent and situation line. Trigger one event by its deed and take its choices; the contract appears on the board.

## Order and who
| Step | Owner | Needs from the user |
|---|---|---|
| 0 starter lore kit + interview | agent-1 | corrections / answers |
| 1 formats, `lore.rs`, compiler, checks | agent-1 | — |
| 2 bake tools | agent-1; the run on a GPU box (Higgsfield or the user's PC) | approve the bake size; strike lines in `review.csv` |
| 3 runtime wiring | agent-2, or agent-1 with notice | play-test the first slice |
