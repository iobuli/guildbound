# GUILDBOUND — Technical Architecture (v0.1)

Goal: a data-driven, simulation-first open-world game that can later gain co-op without a rewrite.

## 1. Engine recommendation

**Godot 4.x (GDScript for gameplay, C# or GDExtension only if profiling demands it).**
- Free, light, good 3D for stylised/cel-shaded art, built-in high-level multiplayer, hot reload.
- Runs comfortably on RTX 3060 / 16 GB with stylised assets.
- Alternative: Unity (bigger asset store, heavier). Choose Godot unless you already know Unity well.

Open-world note: use **chunked world streaming** (e.g., 128 m cells) and keep the simulation
**decoupled from rendering** so far-away NPCs simulate cheaply.

## 2. The rule that makes multiplayer possible later

> **The simulation owns the state. The client only sends intents and renders results.**

Even in single-player, the architecture is client ⇄ simulation:

```
Input / UI  ──(Command)──▶  Simulation  ──(Event)──▶  Presentation (3D, UI, audio)
                               │
                               └── World State (pure data, serialisable)
```

- **Commands:** `BuyItem{actor, item, shop}`, `Interact{actor, target}`, `AskNpc{actor, npc, topic}`.
- **Events:** `ItemSold`, `FactObserved`, `SchemeStepCompleted`, `PriceChanged`.
- **No UI or scene node mutates game state directly.** Everything goes through a command validator.
- All entities have stable **UUIDs**; no node-path references in save data.
- A fixed **simulation tick** (e.g., 10 Hz for world sim, slower for far-away systems) with a seeded RNG, so runs are reproducible and testable.
- Later co-op: the host runs the Simulation, clients send commands. The code path already exists.

## 3. Module layout

```
guildbound/
  docs/                 GDD, this file
  data/                 ALL content as data (json/tres): races, items, materials,
                        recipes, monsters, npc archetypes, scheme archetypes, contracts
  sim/                  Pure logic, NO engine nodes (testable headless)
    core/               tick, clock, rng, ids, command bus, event bus
    world/              regions, locations, chunks
    actors/             npcs, schedules, needs, relationships, factions
    progression/        level, xp, attributes, job proficiency (recomputed, never stored)
    stats/              modifier stack (flat / add_pct / more), caps, stacking groups, tags, breakdown for UI
    magic/              spell data, families, aspects, attunement slots, sustain/upkeep, residue emission
    items/              item anatomy, affixes from materials, sockets, imbues, gold-value pricing
    paths/              path definitions, ranks, perks, signature mechanics (data-driven)
    jobs/               job templates, generators from local demand, payout from treasuries
    law/                heat, bounties, jurisdiction, arrests
    knowledge/          facts, traces, claims, leads, cases (discovery layer, used by all paths)
    schemes/            heist/ambush/smuggling runner + archetypes (criminal content, not the spine)
    economy/            markets, prices, caravans, workforce
    guild/              branches, contracts, ranks, reputation
    crafting/           materials, properties, recipes, quality
    combat/             damage, status, monster behaviour data
    save/               versioned serialisation + migrations
  game/                 Godot scenes, rendering, UI, input, audio (thin layer on top of sim)
  tests/                headless sim tests
```

Rule of thumb: if it decides *what happens*, it lives in `sim/`. If it decides *how it looks*, `game/`.

## 4. Core data models (sketch)

```text
Fact     { id, subject, predicate, object, place, time, truth: true }
Trace    { id, fact_id, channel, location|npc_id, discoverable_by: [skill_reqs], decay_time }
Claim    { id, fact_text, source: {type, id}, confidence, learned_at }      # player/NPC side
NpcKnow  { npc_id, fact_id, confidence, will_share: {trust, price, fear}, lies: bool }
Scheme   { id, archetype, owner, roles{}, steps[{when, who, where, action, emits_traces[]}],
           objective, state, timeline }
Lead     { id, requires_claims[], reveals {place, time}, expires }
Case     { id, linked_claims[], hypothesis{who,what,where,when}, status }
Material { id, properties{hardness, flexibility, conductivity, magic, toxicity, weight}, rarity, tier }
Recipe   { id, slots[{requires:{prop: range}}], output_rules }
Market   { town, goods{ supply, demand, price }, tariff_by_nation }
```

Scheme example (data, not code):

```yaml
id: vault_heist_01
archetype: heist
objective: steal(item: "Stormsteel Ingots", from: guild_vault_oakridge)
roles: { mastermind: npc:varo, inside_man: npc:clerk_henna, muscle: npc:two_bridges }
steps:
  - { day: 1, who: inside_man, action: copy_key, emits: [doc:ledger_gap, behaviour:henna_nervous] }
  - { day: 2, who: mastermind, action: buy_tools, emits: [econ:lockpick_spike, witness:smith_saw_varo] }
  - { day: 4, who: mastermind, action: scout_vault, emits: [witness:night_guard, phys:chalk_mark] }
  - { day: 6, who: all, action: execute, at: night, emits: [phys:broken_lock, echo:guard_hit] }
leads:
  - { requires: [ledger_gap, lockpick_spike], reveals: {place: vault, window: day_5_to_6} }
```

## 5. Key systems and how they connect

- **Knowledge pipeline:** `Scheme step runs → emits Fact + Traces → Traces sit in the world / in NPC memory →
  player perception (skill gated) converts to Claims → Case Board → Lead opens`.
- **Shared economy data:** market anomalies are produced by the same economy sim as normal prices; schemes
  *perturb* the economy (buying tools, dumping loot), which creates organic traces.
- **NPC simulation LOD:** near the player = full behaviour; far = abstract schedule tick (location by time of day, no pathfinding).
- **Contracts:** generated from demand tags and monster pressure in each region, not a static list.
- **Reputation:** a table `(actor, faction) → value` with event-driven modifiers; read by dialogue, shops, contracts, informants.

## 6. Save/load

- Save = serialised `WorldState` (JSON or binary) + `version` + `rng_state`.
- Migration functions per version bump. No node references. Autosave on sleep/sector change.
- Because state is pure data, this also gives free replay/debug tooling.

## 7. Dialogue

Use a data-driven node/choice graph with conditions on facts, reputation, items, and skill checks
(see the `dialogue-systems` approach: Ink or Yarn Spinner, or a small custom runner).
Information dialogue is generated from `NpcKnow`, not hand-written trees, with templates per topic and personality.

## 8. Tooling for a solo dev

- **Headless sim runner:** `godot --headless` or pure script to run 100 in-game days and print events.
  Verify schemes complete, economy stays stable, and no NPC starves.
- **Debug overlay:** show all facts/traces/NPC knowledge (cheat view) to author and test schemes.
- **Scheme validator:** checks the redundancy rule (≥3 traces, ≥2 channels per critical fact) and that
  every lead is reachable with each starting build.
- **Data hot-reload** for balancing.

## 9. Performance budget (RTX 3060 / 16 GB)

- Cel-shaded materials, baked lighting where possible, aggressive LOD, impostors for distant trees.
- Max full-AI NPCs near player: ~40. Everything else on schedule-LOD.
- Chunk streaming on a background thread; no per-NPC nodes beyond the active radius.

## 10. Build order

1. `sim/core` + tests (tick, ids, command/event bus, save).
2. `progression` + `paths` + `jobs` + `guild` + `economy` with a text-mode runner:
   *prove Level → Jobs → Gold feels good per path in a console*.
3. Greybox town in Godot, NPC schedules, Hunter loop (combat, harvest, sell).
4. Merchant and Rogue paths; `law` (heat/bounty).
5. `knowledge` + `schemes`: the one heist and its investigation, text-mode first.
6. Art pass.

Steps 2 and 5 are deliberately text-first: if the loop isn't compelling as text, no amount of 3D will fix it.

## 11. Data additions for Paths / Jobs

```text
Path    { id, ranks[{name, gates{level, gold, rep, trial}, perks[]}], job_families[], signature }
Job     { id, path|civilian, tier, requires{level, rank, items}, risk, pay{gold, from_treasury}, xp, generator }
Character { level, xp, base_stats, path_id, path_rank, gold, rep{faction: n}, heat{nation: n}, states[] }
```
