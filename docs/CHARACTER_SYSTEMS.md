# Character systems: stats, skills, arts, jobs, traits

Status: **v1 implemented (2026-10-17)**: deeds, skills, discovery and discovered arts/spells
(`play/deeds.rs`, `play/skills.rs`, `assets/data/skills.json`, the SKILLS tab). The rest of this
document (titles, occupations, contracts, events, traits) is still the proposal. The user's
answers to the open questions are recorded at the end; where they differ from the proposal below,
the answers win.

## What v1 built

- **Hybrid progression.** The six stats of `play/stats.rs` (Vigor, Endurance, Strength, Dexterity,
  Intelligence, Faith) keep levels and points. Skills sit on top, rise only from deeds, and add a
  small per-level `bonus` to `Derived` (`stats::SkillBonus`). The 12-stat derived-from-skills model
  below is not built.
- **Deeds:** `hit{weapon}`, `block`, `parry`, `kill`, `cast{ability}`, `cast_hit{ability}`,
  `shoot{arrow|bolt|knife}`, `shoot_hit{...}`, `swim` and `climb` per metre, `mantle`,
  `land{Roll|Hard}`, `chop`, `fell`, `gather{item}`.
- **16 skills:** Swordsmanship, Unarmed, Axemanship, Defence, Archery, Throwing, Aura Arts, Fire /
  Frost / Lightning / Arcane Manipulation, Swimming, Climbing, Breakfall, Woodcutting, Gathering.
  RuneScape curve (99 = 13 034 431 XP); at most 600 XP per skill per 30 s.
- **Discovery:** `p = base · a / (a + 4)` per matching deed, certain at `pity`; xorshift RNG seeded
  per character. Arts and spells: once every skill that `unlocks` one has reached its level,
  deeds that train those skills build its affinity (`p = 0.15 · a / (a + 3)`, certain at 8).
  Start: Crescent Edge and Cinder Bolt.
- **Mastery** (per ability, from landed blows) is unchanged and now called **Mastered** so the
  word "skill" means only these skills.
- Player and NPCs carry the same `Sheet`; the Opponent starts with Swordsmanship 20, Defence 12.

## Words (use exactly these)

| Term | Meaning |
|---|---|
| **Stat** | Innate attribute (12 trainable + Fortune). Never trained directly; rises from skill XP. |
| **Derived stat** | Formula over stats, skills and traits (max HP, ki pool, carry). New ones are added as data. |
| **Skill** | Aptitude branching off weighted parent stats. Level 1–99. Discovered, never picked from a menu. |
| **Manipulation** | Skill family over a force or element (Flame, Gravity, Frost...). Gates arts. |
| **Art** | Applied magic: a concrete technique enabled by skills (Crushing Blow from Gravity Manipulation). Unlocked, then mastered. Never "spell". |
| **Deed** | Anything a character does or suffers, as a tagged record (`fell{oak}`, `hit{katana, landed}`, `fall{6 m, survived}`). The only input to progression. |
| **Affinity** | Hidden, per character, per undiscovered skill or art. Deeds raise it; it drives the discovery roll. |
| **Discovery** | The random moment a skill or art is gained. |
| **Event** | A world happening that offers unique skills to characters present. |
| **Occupation** | What a character does for a living. Runs NPC days; the player can hold one. |
| **Title** | Recognition earned from skills and deeds (Smith, Pyromancer). Grants perks and access; shows on the body. |
| **Contract** | Posted work: issuer, requirements, objectives (deed patterns), rewards. |
| **Standing** | Holdings (business, land) and honours (knighted, lord). Gates who may issue contracts. |
| **Trait** | Personality tag from the creator madlib. Drives motion style. |

## Principles

- **One system for everyone.** Player and NPCs carry the same `Sheet`. NPCs discover, level, work and earn titles the same way.
- **Progress only from deeds.** Nothing is bought, allocated or picked from a tree.
- **Data, not code.** Every stat, skill, art, title, occupation, contract template, event and trait is a row with a string id. Rows reference each other by id; references, cycles and curves are validated at load; files hot-reload.
- **Show, don't tell.** Numbers exist under the hood. What a character can perceive depends on its own skills (Aura Sight reads power, Appraisal reads gear). Power shows on the body: gear, clothing, title insignia, how they move.
- **Reproducible randomness.** Discovery rolls use a per-character seeded RNG; a pity floor guarantees discovery once affinity is high enough.

## Stats

| Group | Stat | Feeds (hidden) |
|---|---|---|
| Body | Strength | melee damage, knockback, hit weight, carry |
| | Dexterity | precision, craft quality, ranged accuracy |
| | Agility | move-speed bonus, dodge window, climbing |
| | Toughness | damage reduction, poise, stagger resistance |
| | Endurance | stamina pool and regen, sprint time |
| | Vitality | max HP, regen, poison and disease resistance |
| Mind | Intellect | learning rate of knowledge skills, art complexity |
| | Focus | charge and channel stability, crit precision |
| | Perception | detection range, finding hidden things, reveal skills |
| Spirit | Will | art power ceiling, ki pool, fear resistance |
| | Presence | prices, disposition, occupation and contract access |
| | Attunement | sensitivity to forces: manipulation discovery, art efficiency |
| — | Fortune | discovery and event odds. Rolled at birth, changed only by events, never trained. |

Growth is derived, not stored:

```
stat_xp[s] = Σ over skills  weight[skill][s] · xp[skill]
stat[s]    = birth[s] + curve(stat_xp[s])
```

Rebalancing a weight re-derives every character. Derived stats are rows too:

```
max_hp = 60 + 4·Vitality + 1·Toughness
ki     = 20 + 3·Will + 2·Attunement
carry  = 15 + 2.5·Strength + 1·Endurance
```

## Skills

- Levels 1–99 on the RuneScape XP curve (99 ≈ 13.0 M XP; 92 is half of that).
- Row: `id, name, family, parents {stat: weight} (sum 1), sources [deed pattern → xp, affinity], discovery {base, pity, requires}, unlocks [arts, titles, recipes], world_unique`.
- Families: combat, gathering, craft, movement, social, perception, manipulation, unique.
- XP only from resolved, useful deeds (landed hit, felled tree, finished craft). No XP for whiffs or practice, and gains are capped per encounter so fights can't be farmed.

### Discovery

On each deed, for every undiscovered skill whose sources match:

```
affinity += a · (1 + parent-stat bonus) · trait bias
p         = base · affinity / (affinity + k) · fortune
discover if roll < p, or affinity ≥ pity
```

Feedback is diegetic: a short line and a flourish, not a number. Examples:
`swim{s}` → Swimming · `fall{≥4 m, survived}` → Breakfall · `fell{tree}` → Woodcutting ·
standing in fire or near flame arts → Flame Manipulation affinity.

### Hybrids

Hybrids are authored rows whose parents are other skills; only their discovery is emergent:

```
frostforging:  requires [smithing ≥ 40, frost_manipulation ≥ 30]
               sources  [smith{} while frost aura active]
```

Hybrids can parent further hybrids (a graph). The loader rejects cycles.

### Events

Row: `id, trigger (world condition | schedule | place), radius, offers [(skill, chance, requires)], world_unique`.
Examples: struck by lightning and survived → Storm Vessel (unique); slept at an old shrine → chance of Aura Sight.

## Arts

- Applied magic. An art requires skills at levels:
  Crushing Blow ← Gravity Manipulation 20 · Flame Lance ← Flame Manipulation 35 ·
  Molten Fist ← Flame Manipulation 30 + Unarmed 30. A character's arts are limited to the skills it holds.
- Gained by discovery: using the parent skills builds affinity once the requirements are met.
- Mastery 0–100 % from resolved, useful uses: lowers cost and cast time, then unlocks authored variants (Crushing Blow → Crater at 60 %).
- Power = base × parent-skill curve × Will/Attunement × mastery bonus.
- Row: `id, requires, cost, prepare/charge/recovery, effect (strike modifier | projectile | area | self | field), motion (bank track, mask, timing), mastery curve, variants`.

## Jobs

**Occupations.** Row: `id, workplaces (interactable kinds), schedule (work/rest/eat/sleep), produces, requires, employers`.
NPC AI runs the schedule through the same interact actions the player uses. The player can take an occupation at an employer for wages and daily tasks.

**Titles.** Row: `id, requires (skills, deed counts, standing), perks, insignia`. Granted automatically when the requirements are met. A character can hold many and displays one; the insignia is a visible item on the model.

**Contracts.** Template: `issuer_requires, taker_requires, objectives [deed pattern × count], rewards (gold, items, standing), expiry`.
XP comes from the deeds themselves, not the reward. Issuers are the guild board or any NPC with enough standing (business owner, lord).
Multiplayer later: any character, players included, whose standing meets `issuer_requires` (owns a business or land, lord, knighted) can post contracts through the same path.

## Traits: creator madlib → motion

> I'm a **[stoic]** **[hunter]** who walks **[heavily]** and fights **[recklessly]**, **[slow]** to anger.

Each slot option maps to a per-character `MotionStyle` (exaggeration, hold, stepped fps, spring hz and damping, lean, gait clip variants, idle fidgets, turn rate), plus small affinity biases (a curious character finds perception skills a little more often).
NPCs roll traits, so every character moves differently. This needs a per-character style override in `motion::compose` (today `MotionFeel` is global).

## Runtime shape

- `assets/data/{stats,derived,skills,arts,titles,occupations,contracts,events,traits}.ron`: hot-reloaded and validated at load.
- `Sheet` component on every character: birth stats, skill XP, affinities, arts and mastery, titles, occupation, standing, traits, RNG seed.
- `Deed` message emitted by interact, combat, traverse and magic. One `progress` system turns deeds into XP, affinity, discovery, mastery, titles and contract progress.
- `perceive(observer, target)` returns what the observer is able to know; the HUD only shows that.

## Answers (2026-10-16)

1. Self-knowledge: **exact numbers** (levels, XP, affinity % toward discovery on the SKILLS tab).
2. Art resource: **Battlewill** for arts, **magicules** for spells (Tensura-inspired; no predation).
3. Stats: the six of `stats.rs` stay, with points per level (**hybrid**).
4. Art unlock: **discovered** after the requirements are met, with a pity floor.
5. Guild standing: not decided (not in v1).
6. Permanence: **permanent**, like RuneScape.

## Open questions (as first asked)

1. **Self-knowledge.** What does the player see about themselves? Proposal: skill names with a band (Novice … Grandmaster); exact numbers only with an Introspection-type skill.
2. **Art resource name.** Ki, Essence, Aether, other?
3. **Stat list.** Keep, add, remove or rename (Presence vs Charisma, Fortune vs Luck)?
4. **Art unlock.** Discovered after requirements are met (proposed), or granted automatically at the level?
5. **Guild standing.** Keep letter ranks F–S?
6. **Permanence.** Do skills ever decay or get lost (curses, events), or are they permanent like RuneScape?
