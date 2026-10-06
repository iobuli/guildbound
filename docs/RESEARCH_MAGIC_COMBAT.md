# Research → design: magic, isekai powers, abilities in Godot, PvP / PvE

Short research pass (Oct 2026). Each finding is followed by what it means for Guildbound.

## 1. Magic systems: hard rules, costs, limits

- **Sanderson's laws** (widely used for game magic):
  - (1) magic can only solve problems as far as the player *understands* it;
  - (2) **limitations are more interesting than powers**;
  - (3) deepen what exists before adding more.

  Sources: [Sanderson's Second Law](https://www.brandonsanderson.com/?p=11964), [LegendKeeper summary](https://legendkeeper.com/sandersons-three-laws-of-magic), [Doc McKee: magic in your world](https://docmckee.com/fun/alacrity-fantasy/magic-in-your-world/).
- **For us:** our magic is already "hard". Buffs are numbers in the stat sheet (MOBA modifier stack), with mana, upkeep, cooldowns, attunement slots and Dark backlash. To honour law 1, every number must be **visible**:
  - spell tooltips list each modifier;
  - the status window shows each stat's breakdown (base → +flat → ×% → caps);
  - casting gives a clear aura and flare so you can read who is buffed.

  Law 2 is our identity: Light is reliable but capped; Dark is a 1.4× peak with HP drain and a backlash. Law 3 means we grow by *combining* the four Boons with combat (Flurry, a perfect chain), not by adding forty spells.

## 2. Isekai powers: the "status window" fantasy

- **Common elements:**
  - a game-like world with levels, stats, skills and quests, read from a **status screen**;
  - **cheat / unique skills**;
  - **appraisal** (kantei) skills that reveal the stats of items, people and magic;
  - growth through skill ranks.

  Sources: [Kodansha: classic isekai tropes](https://kmanga.kodansha.com/blog/?p=1536), [MAL: anime where the MC has a status window](https://myanimelist.net/stacks/46804).
- **For us:**
  - **Menu = "Status Window":** level, race, job callings, attributes and derived stats with breakdowns, equipment, bag, skills.
  - **Appraisal as a mechanic:** hovering any item shows its stats and the exact change to *your* numbers (a +/- diff). Later, a skill could reveal enemy stats.
  - **Unique skills** fit the Demonoid / Spirit power sources and racial passives. Each race already has one passive; later, one "unique skill" per power source.
  - **Skill ranks:** combat moves can rank up with use (planned). Perfect chains count double.

## 3. Ability frameworks in Godot

- Existing options:
  - **GodotGAS:** Unreal-GAS-style, in GDScript. Effects are Resources with stacking, duration policies (instant / duration / infinite) and modifier maths.
  - **Godot Gameplay Abilities:** node-based abilities with cooldown hooks.
  - **Forge:** C# only.
  - A common Godot-native pattern: abilities as **Resources**, an **AbilityComponent** node that runs them, and signals like `ability_activated`, `ability_failed(reason)`, `cooldown_started` and `cooldown_finished`.

  Sources: [GodotGAS](https://store.godotengine.org/asset/indiegamedad/godotgas/), [Godot Gameplay Abilities](https://store.godotengine.org/asset/octod/godot-gameplay-abilities/), [Forge](https://godotengine.org/asset-library/asset/edit/20684), [ability-system pattern](https://www.skills.sh/jame581/godotprompter/ability-system).
- **For us:** we already have the GAS core in our own code: `Stats` / `CharModel` hold modifiers with groups, durations, caps and a stacking rule, shared with the Python sim. We keep it rather than adopt an addon, because the Python parity tests are our balance tool.
  - From GAS we take: **gameplay cues**, meaning visual and audio flare fired by effects (cast flash, aura, hit sparks); **tags** (stunned, staggered, guarding); and **the signal set above**.
  - Combat moves are data (`data/moves.json`) in the same spirit.

## 4. Melee AI: approach, circle, take turns

- **Battle circle:** enemies circle the player and attack one at a time, only when the player can react. This ranges from the "cinematic" style (Arkham, Assassin's Creed) to the "dangerous" one (Dark Souls).
- **God of War attack tokens:** a token pool, ranked by threat, decides who may attack. The size of the pool is the difficulty knob.

  Sources: [Tuts+: Battle Circle AI](https://code.tutsplus.com/battle-circle-ai-let-your-player-feel-like-theyre-fighting-lots-of-enemies--gamedev-13535t), [GamerBoom on GoW tokens](https://gamerboom.com/archives/82979).
- **For us:** fighters get a simple **perception → approach → circle → token → strike → back off** loop.
  - **Approach:** run, using **navigation**, around props and up ramps, until in range.
  - **Circle:** hold a ring distance when no attack token is free.
  - **Attack token:** each target hands out N tokens (1 for cinematic, more for hard).
  - **Telegraph:** wind-ups are readable, so parry and perfect dodge are fair.
  - **Back off** when low.

  The same brain fills an `Intent`, so a remote player in PvP drives the very same body.

## 5. PvP vs PvE

- PvE and PvP need **different tuning**. Hard crowd control is shorter in PvP; raid bosses shrug off light CC. PvP often normalises stats or scales damage.

  Sources: [TV Tropes: PvP-Balanced](https://tvtropes.org/pmwiki/pmwiki.php/Main/PVPBalanced), [Kitemetric: PvE vs PvP in MMORPGs](https://kitemetric.com/blogs/mastering-the-balance-pve-vs-pvp-in-mmorpg-development), [Path of Exile: PvP damage scaling](https://pathofexile.com/forum/view-thread/1119964).
- **For us:**
  - **Every hit knows the attacker and victim kind** (player or NPC), so rules can branch.
  - **PvP multipliers live in data:**
    - damage ×0.6;
    - stagger and knockdown durations ×0.6;
    - **diminishing returns** on repeated knockdowns (each within 15 s is half as long, the third is immune);
    - Flurry / perfect-dodge windows unchanged, since skill should decide fights.
  - **Tenacity** already reduces impact, which is the CC-resist stat.
  - **Factions / teams:** a fighter only attacks hostile factions, and the guild hub stays a safe zone.
