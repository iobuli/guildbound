# GUILDBOUND — Magic, Stats & Items (v0.1)

Power in this game is **magic**, and magic is mostly **buffs**: you make yourself, your allies, your
weapon and the ground around you better, then fight and traverse with those numbers. Projectile
spells exist but are the minor branch. Stats work like a **MOBA**: readable, stackable, capped, and
heavily shaped by items.

Companion to [GDD.md](GDD.md). Where this document conflicts with GDD v0.2 §3 (Paths), this one wins
(see §9 for the changes it forces).

---

## 1. Design goals

1. **Buffs are the main verb.** Pre-fight setup, mid-fight timing, and traversal shortcuts all come from buffs.
2. **Stats are legible.** The player can open a stat sheet and see exactly where each number comes from.
3. **Items matter as much as spells.** Same stat engine, same stacking rules. A non-caster can reach similar numbers with gear, food and potions.
4. **Everyone can do everything, but specialising pays.** Nothing is locked; breadth is expensive.
5. **Light is the floor, Dark is the ceiling.** Light is dependable and shareable. Dark is stronger and costs you. (Intentional contrast.)
6. **Magic is part of the world.** It leaves traces, NPCs react to it, it creates markets and crimes.

---

## 2. Stat model (MOBA-style)

### 2.1 Attributes
Three primary attributes (replace the earlier 5-stat list):

| Attribute | Per point (starting tuning) |
|---|---|
| **STR** | +22 Max HP · +0.10 HP regen · +2 kg carry · +1 Attack Damage if your weapon scales STR |
| **AGI** | +1% Attack Speed · +0.15 Armor · +1 Attack Damage if your weapon scales AGI |
| **INT** | +12 Max Mana · +0.05 Mana regen · +1 Spell Power |

Characters gain attributes per level (race and Calling tilt the growth). Items and buffs can add more.

### 2.2 Derived stats

| Group | Stats |
|---|---|
| **Offence** | Attack Damage · Attack Speed · Crit Chance · Crit Damage (base 175%) · Armor Penetration · Magic Penetration · Spell Power |
| **Defence** | Max HP · HP Regen · Armor · Magic Resist · Tenacity (reduces crowd control duration) · Shield |
| **Magic** | Max Mana · Mana Regen · Cooldown Reduction · Buff Potency · Buff Duration · Attunement Slots |
| **Movement / world** | Move Speed · Climb Speed · Jump Height · Stamina · Carry Capacity |
| **Social / stealth** | Stealth · Noise · Perception · Intimidation · Charm |

Derived stats are **recomputed from base + modifiers every time**. Never stored as truth.

### 2.3 Modifier stack (how a stat is computed)

Every effect (item, buff, race, food, debuff) is a **Modifier**:

```
Modifier { source_id, stat, op: flat | add_pct | more, value,
           duration|permanent, tags[], stacking_group, priority }
```

Order of calculation, always the same:

```
1. base                     (level + attributes + race growth)
2. + flat                   (items, buffs, food)
3. × (1 + Σ add_pct)        (all % bonuses add together)
4. × Π more                 (rare multipliers: unique item passives, racial surges)
5. soft cap → hard cap
6. debuffs: slows use the strongest only; reductions apply last
```

**Stacking rules:**
- Same `stacking_group` (e.g. `swiftness.move`): the **strongest wins**, a recast refreshes duration. No stacking a spell with itself.
- Different groups: add together in step 3.
- `more` multipliers are scarce and flagged. They are how we avoid runaway scaling.
- Tags allow **dispel/cleanse** (`curse`, `poison`, `slow`, `dark`, `light`, `magic`).

### 2.4 Caps and diminishing returns

| Stat | Rule |
|---|---|
| Armor / Magic Resist | Damage taken = 100 / (100 + value). Never reaches immunity. |
| Attack Speed | Soft cap 2.5 attacks/s, hard cap 3.5 |
| Move Speed | Soft cap +60% bonus (above counts half), hard cap +100%. Protects traversal and world streaming. |
| Cooldown Reduction | Hard cap 40% |
| Crit Chance | Cap 100% (overflow converts to Crit Damage at half rate) |
| Tenacity | Cap 75% |
| Attunement Slots | Start 3, hard cap 8 |

### 2.5 Worked example (so the engine is testable)

A level-10 Beastkin Hunter, running:

```
Attack Speed (weapon base 1.00 /s)
  AGI 25                    +25%
  Beastkin passive          +10%
  Gloves                    +15%
  Swiftness buff (Gale Step) +30%
  ---------------------------------
  additive pool             +80%   -> 1.00 x 1.80 = 1.80
  Ring unique (more x1.15)         -> 2.07 /s   (under the 2.5 soft cap)

Armor 60 -> takes 100/160 = 62.5%.  Ward buff +40 -> armor 100 -> takes 50%.
Move Speed (base 6 m/s): boots +20, Gale Step +30, Beastkin +10 = +60% -> 9.6 m/s (at the soft cap).
```

---

## 3. The magic system

### 3.1 Structure at a glance

```
POWER SOURCE      how you get and pay for magic        Arcane · Demonoid · Spirit
   │
SCHOOL            what the spell does                  Boon · Bane · Bolt
   │
FAMILY            which stats it touches               Swiftness · Might · Ward · Vigor · Insight · Veil
   │
ASPECT (branch)   how it behaves                       Neutral (base) → Light or Dark at rank 3
```

### 3.2 Schools

| School | Purpose | Weight in the game |
|---|---|---|
| **Boon** | Buff yourself, allies, weapon or an area | **Primary.** Most of the system |
| **Bane** | Debuff enemies: slow, weaken, expose armor, silence | Medium. Mirror of Boon on the same stat engine |
| **Bolt** | Projectile and world-interaction magic | Minor. Basic elemental + Light/Dark bolts |

### 3.3 Boon families (the six buff lines)

Each family has a **base spell** (Neutral) learnt first. At rank 3 you pick a **Light** or **Dark** branch
(respec costs gold and a tutor visit).

| Family | Stats it moves | Light branch (floor) | Dark branch (ceiling) | Open-world use |
|---|---|---|---|---|
| **Swiftness** | Move Speed · Attack Speed · Jump · Climb | *Gale Step*: clean speed, low upkeep, can share with allies | *Shadowrush*: huge speed and attack rate, drains HP/s, Exhaustion after | Cross gaps, outrun threats, climb faster |
| **Might** | Attack Damage · Carry · Break | *Bright Valor*: AD aura that also buffs allies at 50% | *Bloodrage*: big AD and lifesteal, but -Armor | Shift boulders, smash weak walls, haul loot |
| **Ward** | Armor · Magic Resist · Shield | *Aegis*: absorb shield that cleanses on break | *Bone Carapace*: massive armor, -Move Speed, paid in HP | Hazard zones (cold, heat, poison) |
| **Vigor** | HP Regen · Stamina · Tenacity | *Renewal*: regen + cleanse | *Leech Pact*: lifesteal, adds Corruption | Long treks, endurance, surviving attrition |
| **Insight** | Crit · Penetration · CDR · Perception | *Clear Eye*: reveal traces, magic residue, hidden doors | *Predator's Sight*: crit and pen, see targets through walls briefly, -Armor | Investigation, stakeouts, detecting lies |
| **Veil** | Stealth · Noise · Disguise | *Mirage*: illusion and disguise (for undercover) | *Shroud*: near-invisible in dark, but strong residue | Heists, infiltration |

**Aspect design rules (so one is never strictly better):**
- **Light = floor.** Predictable, lower peak, mana-only cost, shareable with allies, cleanses. Little residue.
- **Dark = ceiling.** About +40% peak, self-only, costs HP/Corruption/another stat, leaves a **Backlash** debuff, high residue, visible aura.
- **Neutral** is fully viable and cheapest to learn. Aspects are *specialisation*, not mandatory power.

### 3.4 Casting model (third-person, BotW/GTA-feel)

- **Hotbar of 4** + a **radial wheel** for the rest. Hold to channel, tap to cast.
- Targets: **Self**, **Ally** (aim), **Weapon** (imbue), **Ground zone** (e.g. campfire ward), **Object** (empower a lever, chest, door).
- **Attunement slots** limit sustained buffs. A normal buff uses 1 slot, a greater buff uses 2.
- **Sustained** buffs drain mana per second. **Timed** buffs cost once and expire.
- **Consumables** (food, potions) give the same effects without slots but with a **stomach limit** of 3 active.
- Buffs show as a **MOBA-style bar** with timers. Aura VFX uses colour (gold = Light, violet = Dark).
- **Weapon imbues** attach a buff to attacks (on-hit slow, flame, drain) with an internal cooldown. This is the link between spells and items.

### 3.5 Bolts (the minor branch)

| Bolt | Combat | Sandbox interaction |
|---|---|---|
| **Fire** | Damage-over-time | Lights torches, burns grass and ropes |
| **Frost** | Slow | Freezes water into a bridge, brittle ice |
| **Storm** | Chain and stun | Conducts through metal and water, powers devices |
| **Stone** | Knockback | Raises a platform or cover |
| **Radiant** (Light) | Bonus vs undead/demons | Reveals hidden marks |
| **Umbral** (Dark) | Drain life | Snuffs lights, dims an area for stealth |

Bolts scale with Spell Power, use mana, have short cooldowns, and are deliberately *not* the main damage source
for most builds.

### 3.6 Bane (debuffs)
Same engine in reverse: slow (strongest only), weaken (-AD), expose (-Armor), silence, curse. Bane resistance is
Tenacity and Magic Resist. Monsters have tags and resistances so buff-stacking alone doesn't trivialise a fight.

---

## 4. Power Sources (where the "paths" Demonoid and Spirit went)

How you *obtain and pay* for magic. You start Arcane, others are unlocked in the world.

| Source | Payment | Character | Unlock |
|---|---|---|---|
| **Arcane** (default) | Mana | Learned from tutors and tomes; steady and neutral | Start |
| **Demonoid** | Mana + **Corruption** | Pact power. Dark branches cheaper and stronger. Light branches penalised. Visible demonic features at high Corruption | Pact event (~Lv 8-10) |
| **Spirit** | **Favor** + conditions | Bind a spirit; each grants a fixed Boon family. Little or no upkeep, but terrain/time/taboo conditions | Bonding quest (~Lv 8-10) |

**Demonoid (fast, variable):**
- Corruption 0-100. Tiers give stat bonuses and unlock pact powers.
- Tolerance and rebound: each use is slightly less efficient; when Corruption drops it can swing your baseline lower.
- Social cost: guards, Sylvara and Light shrines react. Route out: penance rites, Light shrines, time.

**Spirit (slow, dependable):**
- Bond with 1-3 spirits. Bond rank grows by *relationship*, not luck.
- Favor is earned by offerings and kept by obeying taboos. Break one and the spirit withdraws until you atone.
- Condition examples: a Stag Spirit's Swiftness is stronger in forests and at dawn.

**Sources are not jobs.** A Demonoid can be a merchant. A Spirit bonded Knight can serve the crown.

---

## 5. Races: passives and tilt

Each race has: a **stat tilt**, a **magic affinity** (cheaper to learn, small bonus), and one **named passive**.

| Race | Stat tilt | Affinity | Passive |
|---|---|---|---|
| **Human** | Balanced | None (can learn anything cheaper) | **Adaptable:** +10% XP in all jobs; cuts the untrained-job penalty by 25% |
| **Ironkin** | +STR, +Armor growth | Ward · Might | **Stoneblood:** +Tenacity, knockback resistance; forging bonus |
| **Sylph** | +INT, +Mana regen | Insight · Light | **Wellspring:** regen mana faster; read magic residue at scenes |
| **Beastkin** | +AGI, +Move Speed | Swiftness | **Wild Instinct:** +base climb and scent tracking |
| **Drakari** | +STR, heat/poison resist | Might · Fire bolt · Dark | **Dragonblood:** once per rest, *Awaken* for 10s of large AD and Armor via a `more` multiplier |
| **Wisp** | +INT, low HP, high Magic Resist | Veil · Spirit bonds | **Spirit-touched:** slip through narrow gaps, hear echoes, cheaper spirit bonds |

Rule: racial passives are **bonuses, not locks.** Any race can learn any family.

---

## 6. Jobs: the "do everything, but it's harder" model

This revises the Paths in GDD §3. Jobs are **what you do for gold and XP**; magic **power source** is separate.

### 6.1 Proficiency
- Every job has **Proficiency 0-100**: Novice · Apprentice · Journeyman · Expert · Master.
- Doing a job raises its proficiency (not only your character level).
- Proficiency unlocks: better pay multiplier, higher-tier contracts, job perks, better success rates.

### 6.2 Calling and sidelines
- **Calling** (1): your primary job. +25% pay, full perks, employers trust you.
- **Sidelines** (2): reduced perks, normal pay.
- **Dabbling** (any other): allowed always at the lowest tier, with an untrained penalty:
  slower progress (XP ×0.5), pay ×0.6, higher failure, no perks, tools cost more.
- You can swap Calling for gold + a short cooldown. Proficiency is kept, not lost.

### 6.3 Costs that make breadth hard (not walls)
- **Time:** the day is finite; every job takes time.
- **Licences, tools, bonds:** gold sinks per job.
- **Reputation conflicts:** a crown Knight who is also a Bandit sideline works, until someone finds out.
- **Experience split:** proficiency XP is shared by what you actually do.

### 6.4 Jobs list (replaces the path list)
Hunter · Knight · Rogue · Bandit · Merchant, plus civilian crafting jobs (Smith, Alchemist, Cook, Enchanter, Tailor, Gatherer).
Each job leans toward certain families: Hunter (Might, Vigor, Insight) · Knight (Ward, Might, Light) ·
Rogue (Swiftness, Veil, Dark) · Bandit (Might, Swiftness, Dark) · Merchant (Insight, Vigor, enchanting trade). These are *leans*, not rules.

---

## 7. Items

### 7.1 Item anatomy

```
Item { base_type, tier 1-7, rarity, slot, base stats, affixes (from materials),
       sockets, unique passive/active (optional), set tags, level req, weight, durability }
```

Slots: Weapon · Off-hand · Head · Body · Hands · Feet · 2 Accessories.

### 7.2 Materials drive affixes

| Material property | Item effect |
|---|---|
| Hardness | Armor, weapon damage |
| Flexibility | Attack Speed, Move Speed, Stamina |
| Conductivity | Mana regen, Storm bonuses |
| Magic affinity | Spell Power, Buff Potency |
| Toxicity | On-hit DoT, poison resist |
| Weight | Carry load, Move Speed penalty |
| Rarity | Raises item tier cap and affix count |

### 7.3 Gold efficiency (MOBA-style pricing)
Each stat has a **gold value**. An item's price comes from its stats + materials + passives. This gives the
economy a built-in sense of what items are worth and lets players judge deals.

| Stat | Starting gold value |
|---|---|
| +1 Attack Damage | 35 |
| +1% Attack Speed | 25 |
| +1 Armor / +1 Magic Resist | 20 / 18 |
| +1 Max HP | 2.7 |
| +1 Max Mana | 1.4 |
| +1% Move Speed | 40 |
| +1% Crit | 40 |
| +1% Cooldown Reduction | 30 |
| +1 Spell Power | 30 |

Unique passives are priced at a premium. Tune after playtests.

### 7.4 Actives, passives, sockets
- **Unique passive:** e.g. *Frostbrand*: attacks slow for 1s (internal cooldown 3s). Only one copy counts.
- **Active:** e.g. *Boots of the Gale*: +30% Move Speed 3s, 20s cooldown.
- **Sockets:** runes/gems that are mini-buffs (+4% Move Speed). Enchanters craft them.
- **Imbue** (spell on weapon) and **sockets** are different layers so enchanting has real depth.

### 7.5 Items vs spells
- Spells: flexible, drain mana, limited by Attunement slots, build around cooldowns.
- Items: always on, no upkeep, require gold/materials, limited by slots and weight.
- Consumables: temporary, no slots, limited by stomach, produced by Cook/Alchemist (this is the non-caster's route).

---

## 8. How magic touches the rest of the game

- **Traversal puzzles with multiple solutions.** A cliff can be reached with Swiftness (Light or Dark), a potion, boots with a climb affix, or Wisp passive. No one route is mandatory.
- **Investigation integration.** Spells leave **residue**: Light small, Dark large, Veil reduces it. Insight (Light) and Sylphs read it. Dark auras are visible to guards and raise Heat. Mages are detectable, not just powerful.
- **Economy.** Mana crystals, rune sockets, imbued weapons and enchanting services become goods. Merchants can sell buffs (potions) and enchanters can sell services.
- **Law & factions.** Dark use triggers different reactions per nation (Sylvara hunts Demonoids, Merrow doesn't care, Aurelis licences mages).
- **Combat balance.** Monsters have Tenacity, resist tags, and enrage windows so pre-buffing isn't a free win; dispel and Bane matter.

---

## 9. Changes this forces to GDD v0.2

1. **Paths → Jobs.** Replaced by the Calling / Sideline / Dabbling model (§6). Rank ladders stay, attached to jobs.
2. **Demonoid and Spirit → Power Sources**, not jobs (§4). They are the "awakened" options.
3. **Stats:** STR/AGI/INT replace the 5 old attributes (§2.1).
4. **Open questions 1-2 in GDD §13.1 are resolved:** no exclusive path; Demonoid/Spirit are unlocked sources that do not replace jobs.
5. **Crafting:** material properties now map explicitly to stats (§7.2).

## 10. Prototype scope (magic)

- **Arcane only.** Demonoid and Spirit are data stubs.
- **4 Boon families** (Swiftness, Might, Ward, Vigor) × (base + Light + Dark) = 12 spells.
- **6 bolts** (Fire, Frost, Storm, Stone, Radiant, Umbral).
- **Stat engine** with modifier stack, caps, and a debug panel showing each stat's sources.
- **~30 items** and **6 consumables** using the same engine.
- All 6 races as data (passives live in the engine), 2 fully tested.
- 3 jobs with Proficiency + Calling/Sideline/Dabbling rules.

**Question the prototype answers:** *does stacking buffs and swapping gear feel powerful, readable, and different between Light and Dark, in third-person combat and traversal?*

Kill criteria: if players pick the same family/aspect every time, or buffs become "press all at start, forget", then upkeep, slots or monster counters are too weak.

## 11. Open questions

1. One shared mana pool, or a second pool per source (Corruption/Favor)? (Proposal: one pool + one meter per source.)
2. Aspect respec cost and frequency.
3. Do Light/Dark get day-night modifiers (+10% in matching light)?
4. Co-op: do buffs on allies scale with the caster's stats? (Proposal: yes, at 50%.)
5. Can Light/Dark be mixed across families? (Proposal: yes; Demonoid pays extra to use Light.)
