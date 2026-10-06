# GUILDBOUND — Game Design Document (working title)

> **v0.3 note:** [MAGIC_AND_STATS.md](MAGIC_AND_STATS.md) now overrides parts of this file. "Paths" are
> **Jobs** you can all do at once (Calling / Sideline / Dabbling, with proficiency). Demonoid and Spirit
> are **magic Power Sources**, not jobs. Stats are MOBA-style STR/AGI/INT. Where this document says
> "choose one path", read the Calling model.

**v0.2** — revised: the game is built on **Paths + Level + Gold + Jobs**. Investigation and heists
are one aspect of the game (mostly the criminal side), not the whole game.

Open-world 3D anime fantasy sandbox. Single-player now, multiplayer-ready by architecture.

Marked **[P]** = in the first playable prototype. Everything else is designed now, built later.

---

## 1. Pillars

1. **Pick a path, live a different game.** Hunter, Knight, Rogue, Bandit, Merchant, Demonoid and
   Spirit each earn, fight, and get treated differently by the world.
2. **Level, gold and jobs drive everything.** You level by doing jobs, jobs pay gold, gold buys power,
   licences, gear and safety, and level unlocks better jobs.
3. **One shared living world.** A hunter's wolf cull moves hide prices; the merchant profits; the
   bandit raids the caravan; the knight hunts the bandits. Paths feed each other.
4. **Heavy resource and crafting systems** under every path.
5. **Discovery over hand-holding.** Some of the best content (heists, conspiracies, rare deals) is found
   by investigating, not by accepting a quest.
6. **Consequences that don't scale with level:** reputation, access and money bite at every tier.

---

## 2. The core triangle: Level · Gold · Jobs

```
        JOBS ──── pay ────▶ GOLD
         ▲  \                 │
         │   \ xp             │ buys gear, licences, bribes,
   unlocks    ▼               │ upkeep, crafting, safety
         │   LEVEL ◀──────────┘ (makes you able to do harder jobs)
```

### 2.1 Level (character power)
- Gained from XP from **any** job. Capped for the prototype at 15; design target 50.
- Level gates: **gear tier**, **region danger** (you *can* go anywhere, you just die), **job tier**.
- Stats are MOBA-style (STR, AGI, INT + derived stats; see MAGIC_AND_STATS.md). Derived stats are computed from base + modifiers; never store derived.
- XP per job = job tier × performance, with diminishing returns when overlevelled. Fast early, slow late.

### 2.2 Gold (the economy limiter)
Gold is both reward and constraint. **Sinks must always exist at every level**, otherwise gold stops mattering:
- Gear and repairs · crafting materials · licences and permits · guild/crown dues.
- Rent, stabling, storage · food, potions · bribes, fines, bail, ransoms.
- Upgrading your base, shop, hideout or camp · hiring NPC help.
Gold from jobs is paid **out of real treasuries** (guild branch, crown, merchant stock), so jobs can run
dry when the local economy is broken.

### 2.3 Jobs (the repeatable activity)
A **Job** is a unit of work with a tier, requirements, risk, pay and XP. Jobs come from three places:
1. **Boards and employers** (guild board, crown notices, merchant requests, underworld fixers).
2. **Local demand** (the economy generates jobs: wolves are a problem, ore is short, a caravan is overdue).
3. **Discovery** (you find or make your own: spot an arbitrage, find a heist, ambush a route).

Every path has its own **job family**, plus shared civilian jobs (see 6, Crafting) anyone can do.

---

## 3. Paths

A **Path** is your main way of life. It defines your job family, your employers, your signature
mechanic, your risks and your rank ladder. Choose one at the start; switching later costs gold and reputation.

### 3.1 The seven paths

| Path | Fantasy | Core jobs | Gold comes from | Signature mechanic | Main risk |
|---|---|---|---|---|---|
| **Hunter** **[P]** | Monster slayer, guild member | Culls, bounties, gathering, escorts, dungeon runs | Guild contracts + selling monster parts | Tracking and harvesting quality | Injury, repair costs, rank trials |
| **Knight** | Crown/order soldier | Patrols, investigations, escorts, sieges, arrests | Salary + stipend + seizure rewards | Authority: warrants, arrest, jurisdiction | Politics, orders you disagree with |
| **Rogue** **[P]** | City thief, spy, fixer | Heists, burglary, pickpocketing, smuggling, info brokering, assassination contracts | Loot, fences, fixers | Stealth, lockpicking, **investigation and planning** | Heat, informants, knights |
| **Bandit** | Wilderness raider | Ambush, caravan raids, extortion, camp control | Tribute, loot, ransoms | Camp/gang management, ambush setup | Bounties, hunter and knight reprisals |
| **Merchant** **[P]** | Trader, shopkeeper, caravan owner | Trade runs, shops, contracts, brokerage, workshops | Margins, ownership, services | Market reading, logistics, hiring | Robbery, market crashes, debt |
| **Demonoid** | Pact-bearer with demonic power | Pact missions, corrupted-site work, hunting rivals, black-market power | Dark patrons, forbidden trade | **Corruption** meter: power now, costs later | Being exposed and hunted; losing yourself |
| **Spirit** | Shaman, medium, spirit-walker | Rites, spirit contracts, guiding, cleansing, healing | Offerings, rites, patronage | **Bonds** with spirits: dependable and slow | Taboos, obligations, spirit debts |

### 3.2 Why these seven don't overlap
- **Rogue vs Bandit:** Rogue = *urban*, stealth, planning, solo or small crew. Bandit = *wilderness*, force, camps, large gang.
- **Hunter vs Knight:** Hunter works for contracts and the guild. Knight works for authority and enforces law.
- **Demonoid vs Spirit** are the intentional contrast pair:
  - Demonoid is **high-variance and fast**: big power now, rebound and exposure later.
  - Spirit is **dependable and slow**: power scales by relationship and refinement, with obligations.
  - (This follows the rule that a variable system should be paired with a dependable one.)

### 3.3 Path rank ladders (separate from character level)
Level = raw power. **Path rank = standing in your path.** It unlocks job tiers, employers and perks.

| Path | Ranks |
|---|---|
| Hunter | F · E · D · C · B · A · S |
| Knight | Squire · Guard · Knight · Captain · Commander · Paladin |
| Rogue | Cutpurse · Thief · Shadow · Phantom · Mastermind |
| Bandit | Outlaw · Raider · Captain · Chief · Warlord |
| Merchant | Peddler · Trader · Merchant · Master · Magnate |
| Demonoid | Tainted · Marked · Pactbearer · Fiend · Sovereign |
| Spirit | Initiate · Medium · Shaman · Spirit-Walker · Sage |

Rank gates are **mixed**: some need level, some need gold (a fee or bond), some need reputation, some need a trial.
That keeps ranking from being "just grind".

### 3.4 Unlocking and switching
- Hunter, Knight, Rogue, Bandit, Merchant are available at character creation.
- **Demonoid and Spirit are *awakened* paths** — unlocked by a pact/ritual (around level 8–10), not picked at start.
  They **replace your primary path's signature mechanic** but you keep the old path's jobs at lower priority
  (a Demonoid Rogue still burgles; a Spirit Hunter still hunts, differently). *(Open question 13.1)*
- Switching a mortal path: pay a fee, lose some reputation in the old path, restart that path's rank at a
  fraction of your old standing.

### 3.5 Law and conflict between paths (the shared-world loop)
- Rogue/Bandit actions generate **Heat** and **Bounties**.
- Knights and Hunters respond to those (the Knight path *is* the investigation counterpart of the Rogue path).
- Merchants are the targets and the funders of everyone.
- Demonoid/Spirit are seen differently per nation (feared, honoured, or hunted).

---

## 4. Races and Nations

### 4.1 Races (6) — innate flavour, not a path

| Race | Combat trait | Utility trait |
|---|---|---|
| **Human** | Faster skill XP | Best with officials, law and trade |
| **Ironkin** | Stagger resistance, forge affinity | Appraise metal and stone origin |
| **Sylph** | Mana efficiency | Sense residual magic |
| **Beastkin** | Mobility, claws | Scent tracking |
| **Drakari** | Heat/poison resistance, burst | Intimidation, hard to lie to |
| **Wisp** | Fragile, fits tight spaces | Hear echoes of recent events (limited) |

Racial prejudice is a **reputation modifier per nation**, not a stat penalty.

### 4.2 Nations (5) **[P: 1 town of 1 nation]**

| Nation | Flavour | Economic role | Path culture |
|---|---|---|---|
| **Aurelis Kingdom** | Human castle towns, bureaucratic | Grain, banking, general goods | Strong Knights, strict licences, rich merchants |
| **Kaelthorn Highlands** | Ironkin mountain holds | Ore, smithing | Hunter guild is a craft-and-war union |
| **Sylvara Canopy** | Sylph forest cities | Herbs, alchemy, enchanting | Spirit path honoured; Demonoid hunted |
| **Vaskar Steppes** | Beastkin clans | Hides, livestock, raw monster hunting | Bandit clans are semi-legitimate; clan law |
| **Port Merrow Free League** | Merchant city-states | Trade routes, smuggling, information | Rogue and Merchant heartland; weak law |

---

## 5. The Hunter Guild (local economy hub) **[P]**

Not only for Hunters: **every path interacts with it**.
- **Contract board:** requests from townsfolk, merchants, the crown. Hunters take them. Others may post them or exploit them.
- **Buyer/seller of materials:** the main legal market for monster parts and herbs.
- **Bank and registry:** holds funds, ledgers, bounty lists (a Bandit or Rogue shows up here as a target).

Each branch has a **treasury, inventory and demand tags**. If monsters are ignored, flocks die, prices rise
and contract pay climbs. If a branch is robbed, it posts fewer, worse contracts. The guild is the visible
"pulse" of a town's health and how much the player's path moved it.

---

## 6. Resources and Crafting (heavy)

### 6.1 Pipeline
`Gather / Hunt / Buy → Refine → Craft → Enhance → Use / Sell / Fence`

### 6.2 Material properties, not fixed recipes
Materials have numeric properties (*hardness, flexibility, conductivity, magic affinity, toxicity, weight, rarity*)
and a quality tier. Recipes define **slots and requirements**; different materials fill slots with different
results. Variety without thousands of hand-written recipes.

### 6.3 Who uses it how
| Path | Crafting angle |
|---|---|
| Hunter | Own gear from parts; harvest quality matters |
| Knight | Issued gear, upgrades via smith; seizures supply materials |
| Rogue | Tools (picks, poisons, disguises); forged papers |
| Bandit | Camp builds, traps, stolen-goods processing |
| Merchant | Owns workshops; buys labour, sells goods |
| Demonoid | Corrupted materials, profane crafts |
| Spirit | Charms, offerings, spirit vessels |

### 6.4 Crafting professions **[P: smith, alchemist]**
Smith · Alchemist · Leather/Tailor · Cook · Enchanter. Mastery by repetition and experimentation; discoveries
go into a recipe book. These are **civilian jobs anyone can take** for steady gold.

### 6.5 Economy sim
Per town: supply, demand, price, NPC workforce. Price = base × demand/supply, modified by tariffs, caravans, rumours.
Markets saturate and recover over days, so nothing can be endlessly farmed. The same data feeds Merchant play,
guild contract pricing, and economic clues for investigators.

---

## 7. Discovery and Investigation (one aspect, not the spine)

Investigation is how players **find** certain jobs. It is not required for every path, but each path uses
a flavour of it:

| Path | How they "discover" jobs |
|---|---|
| **Rogue** | Case a building, tail a mark, bribe a clerk, read a schedule, plan a **heist** |
| **Knight** | Investigate crimes: examine scenes, interview witnesses, trace stolen goods, serve warrants |
| **Hunter** | Track monsters, read signs, find nests, find *why* the monsters moved |
| **Merchant** | Spot price gaps, hear about shortages, vet partners |
| **Bandit** | Scout routes, learn caravan schedules |
| **Demonoid/Spirit** | Read omens, sense corrupted/sacred sites |

### 7.1 Heists (localised, criminal play)
A **Heist** is a specific type of content, scoped to one location (a vault, a mansion, a caravan stop, a guild
branch) and run from the Rogue (and sometimes Bandit) path. It is:
- **Planned:** casing → crew → tools → timing → entry → escape → fence.
- **Staffed:** crew roles (lockpick, muscle, driver/rider, inside-man, lookout); NPC crews or player co-op later.
- **Investigable:** every step leaves **traces** (witness, physical, document, economic, behavioural).
- **Opposed:** Knights investigate the traces afterwards, use the same clue data and can catch you.
- **Fail-forward:** a mess is a story: someone is wounded, loot is marked, a fence is flipped.

### 7.2 Clue vocabulary (shared engine)
Fact → Trace (via a channel) → Claim (what you recorded; source + confidence; may be false) → Lead (place + time) → Case.
Each critical fact has ≥3 traces across ≥2 channels so builds are never walled off. No quest markers; the player
uses a notebook and self-placed map pins.

### 7.3 Where heists sit in the game
They are a **high-risk, high-pay Rogue job tier** (and a source of drama for everyone else), not the main loop.
A typical Rogue still spends most of their time on smaller jobs: fencing, burglary, pickpocketing, smuggling, info runs.

---

## 8. Combat (simple first)

Third-person action combat; stamina, dodge, hotbar. Monsters have weak points, behaviour states and ecosystem
roles. Path changes the kit: Hunter (weapons + traps), Knight (shield/armour/commands), Rogue (stealth, dagger,
tools), Bandit (ambush, gang tactics), Merchant (hired guards, gadgets), Demonoid (pact powers), Spirit (summons and wards).
Variance (crits, drops) lives here. **[P: 3 monsters, 3 paths]**

---

## 9. States: Corruption, Bonds, Heat, Injury

All temporary/persistent states obey two rules: a **traceable cause** and a **route out the player controls**.

| State | Cause | Route out | Applies to |
|---|---|---|---|
| **Heat** | Crimes seen / evidence | Lie low, bribe, pay fines, frame someone | Rogue, Bandit |
| **Bounty** | Heat crossing a threshold | Pay, serve, flee the nation | Rogue, Bandit |
| **Corruption** | Using demonic power | Rites, penance, or accept it | Demonoid |
| **Spirit debt / taboo** | Neglecting offerings, breaking a rite | Offerings, atonement | Spirit |
| **Injury / fatigue** | Combat, travel | Rest, treatment, potions | All |

Demonoid's fast power has **tolerance and rebound**; Spirit's slow bonds don't. That is the intended contrast.

---

## 10. Consequences that don't scale with level

At every level the world can still: **lock you out** of towns/shops/guilds, **raise your prices**, **cut your
information**, **stop offering jobs**, **hire people against you**. A S-rank Hunter with 20 bounties in Merrow still
can't buy a loaf there.

---

## 11. Art and tone

Stylised anime-look 3D: cel-shaded, strong silhouettes, expressive faces, bright palette per nation. Targets
RTX 3060 / 16 GB. All cast are adults.

---

## 12. Prototype scope **[P]**

**One question:** *Is the Level → Jobs → Gold loop satisfying, and does picking a different path feel like a different game?*

Build:
- 1 town (guild hall, market, tavern, warehouse district, outskirts) + 1 hunting zone.
- **3 paths: Hunter, Rogue, Merchant.** Knights exist as NPC enforcers (Heat, arrests) but aren't playable yet.
- Level 1–15, path ranks 1–3 each.
- ~8 jobs per path, generated from local demand, plus smith/alchemist civilian jobs.
- Guild board, treasury, 10-good market sim.
- **One heist** for the Rogue path (full trace set) as the "investigation" proof.
- Heat/bounty system, basic combat with 3 monster types.
- Save/load.

Kill criteria: if all three paths play as "do task, collect gold", the signature mechanics are too thin; fix before adding the other four.

## 13. Roadmap

| Milestone | Content |
|---|---|
| M0 | Sim core: tick, ids, command/event bus, save, headless tests |
| M1 | Level, gold, jobs engine + guild board + market sim (text-mode runnable) |
| M2 | Greybox town, NPC schedules, Hunter loop (combat + harvest + sell) |
| M3 | Merchant path (trade, shop) and Rogue path (stealth, jobs) |
| M4 | Heist + investigation + Heat/bounty + Knight NPC enforcers |
| M5 | Playtest, tune, decide: add Knight / Bandit playable |
| Later | Demonoid and Spirit, more nations, procedural heists, player-run crews, co-op |

## 13.1 Open questions

1. ~~Awakened paths~~ **Resolved (v0.3):** Demonoid/Spirit are magic Power Sources that sit alongside any job.
2. ~~Two paths at once~~ **Resolved (v0.3):** yes, everything is allowed; Calling/Sideline/Dabbling makes breadth costly, not forbidden.
3. Day length (proposal: 1 real hour = 1 day) and death penalty (proposal: money + reputation loss; NPCs die permanently).
4. Race restrictions per path (proposal: none; only reputation differences).
