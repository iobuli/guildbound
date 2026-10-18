# Agent log (newest first)

## 2026-10-18 · reviewer (cloud Claude)

**Branch:** feat-monk   ·  **Range:** `26c5552..e9ecf30`   ·  **Kind:** code review only (no code changed, nothing built here)

**What:** Read the Monk diff (19 files, +1504/-47). The logic is sound overall: counters, stun, over-mana bleed, the flee/hesitate AI (sparring is correctly exempt from fleeing) and armour no longer moving the body. Four things to fix or decide before merging into overhaul-2026-10-16:

1. **Every unarmed NPC becomes a Monk.** `discipline::Path::of_weapon` maps `Kind::Unarmed` to `Path::Monk`, so any bare-handed NPC (brawlers, bandits, a disarmed duelist) gets a counter on every parry, a 0.25 guard bonus at 0.6 stamina cost, and arrow/spell immunity while guarding. Suggest: give Monk only to NPCs tagged as monks (Brother Ishin, Sister Ren, Elder Tadao, order area); default unarmed NPCs to `Path::None`.
2. **The monk's ranged catch has no timing and no cost.** `combat::strike`: any `State::Guard` facing within `CATCH_ARC` (~145°) deflects every arrow and absorbs every spell, with no parry window and no stamina spent. A monk holding guard is immune to all ranged attacks. Suggest: catch only inside the parry window (perfect timing), otherwise block normally with stamina; or spend stamina per catch.
3. **`assign` only runs for fighters without a `Mastery`.** An NPC that later changes weapon (armory, pickup, disarm) keeps its old path. Suggest: recompute the path when `Loadout` changes.
4. **Small ones:**
   - `ABSORB_SHARE` is 1.2, but the log entry says 1.5x. 1.5 is `OVER_CAP`, the pool ceiling, not the share drunk.
   - Two spells caught in the same frame: the second `try_insert(Absorb(..))` replaces the first, so its mana is lost. Accumulate instead.
   - The counter damage scales with the attacker's own blow (`hit.damage * c.damage`), not the monk's strength. Fine if intended.

**Config / try it:** n/a (review). Agent-2 is building/testing feat-monk now; merge only after `cargo build` + `cargo test` are green.

## 2026-10-18T15:13Z · agent-2

**Branch:** feat-monk   ·  **Range:** `26c5552..e9ecf30`

**What:** Monk + discipline (UNTESTED build on this commit). play/discipline.rs: Mastery path+rank; Monk fists x1.35+, guard absorbs more for less stamina; guard inside the blow = auto counter -> Stunned 1.5 s -> ragdoll on next hit; max rank (S) counter per class with its own flare; rank aura from D; humans 2+ ranks below flee, others hesitate, demons never flee; arrows deflected, spells absorbed as over-mana (1.5x). Hand Forms (fist, cupped, two-finger, claw) + fit checker. Climb: body pushed off the wall, weapon sheathes. Armour no longer changes motion. CMU clips placed on the floor. New NPCs: Brother Ishin, Sister Ren, Elder Tadao, Oni. appearance.json: order area (monks), monk class look, oni look. looks.rs: beads, wraps, horns.

**Config / try it:** Class panel key 4 or GB_CLASS=monk; GB_RANK=0..6 forces the player's rank; GB_CHECK=1 logs [fit] lines; GB_ARMOR_PADDING=1 restores old armour padding; cargo test floor_report -- --ignored --nocapture. Fetch: git fetch origin feat-monk.

- `e9ecf30` WIP: Monk discipline, counters, hand forms, fit checker, climb wall push, mocap floor placement
    - assets/data/appearance.json: +787-852 +858-859 +871-872 +1150-1261
    - crates/game/src/motion/bank.rs: +100-129 +631-675 +892-958
    - crates/game/src/motion/body.rs: +253-292
    - crates/game/src/motion/compose.rs: +201 +213-218 +261-276 +323-337
    - crates/game/src/motion/contact.rs: +1251-1252
    - crates/game/src/motion/hand.rs: +34-63 +129-130 +280-356 +431 +452 +522-548
    - crates/game/src/motion/mod.rs: +272-276 +293-294
    - crates/game/src/play/ai.rs: +12-14 +29 +73-75 +198-201 +226-227 +265-277 +391-405 +461-466 +591 +606-607 +613 +618 …
    - crates/game/src/play/appearance.rs: +183-189
    - crates/game/src/play/armor.rs: +5-6 +1525-1529 +1616-1619
    - crates/game/src/play/class.rs: +8-10 +28 +44 +78-98 +155 +163
    - crates/game/src/play/combat.rs: +50-51 +201-207 +1175-1176 +1258 +1289 +1303 +1481 +2118-2120 +2137-2138 +2204 +2221 +2226-2227 …
    - crates/game/src/play/discipline.rs: +1-505
    - crates/game/src/play/looks.rs: +46-51 +68-70 +100-102 +104 +120-175 +391-419 +524-542
    - crates/game/src/play/magic.rs: +52-54 +312-313 +471-478 -480
    - crates/game/src/play/mod.rs: +28 +124-125
    - crates/game/src/play/select.rs: +162-166 +715 +741 +800 +1145-1146
    - crates/game/src/play/sheath.rs: +9 +122 +125-128
    - crates/game/src/play/world.rs: +1287-1335

## 2026-10-18T15:13Z · agent-1

**Branch:** feat-monk overhaul-2026-10-16   ·  **Range:** `e9edea7..26c5552`

**What:** Wardrobe: 13 garments for male and female hero bodies (hanging cloth swings with the legs), our own skin/eye/hair/cloth textures, class outfits, NPC looks by name, seeded area styles (guild, village, ruins, pond). Pushed by agent-2 for agent-1.

**Config / try it:** appearance.json: looks / classes / cast / areas. GB_CLASS=blade|arcanist|hunter dresses the player; GB_HERO=mannequin brings back the mannequin.

- `0271c6c` Wardrobe garments and our own textures for the hero kit
    - .gitignore: +9
    - LICENSES.md: +10-12
    - assets/characters/hero/build_report.json: +6-7 +9 +12-20 +30-31 +33 +36-44 +158-313
    - docs/MODEL_PARTS.md: +172-210
    - tools/model/build.py: +39-40 +270-343 +361-368 +371-381 +613-615 +768-774 +779-780 +782-783 +817 +819-832
    - tools/model/drape.py: +1-130
    - tools/model/textures.py: +1-352
- `26c5552` Dress the whole cast: class outfits, NPC looks, area styles
    - assets/data/appearance.json: +2 +5-76 +79-243 +254-275 +285-306 +316-333 +343-1081
    - crates/game/src/play/appearance.rs: +25-36 +52 +99-108 +111-133 +141-148 +174-240 +264-265 +288-291 +360-361 +363-410 +431-432 +436 …
    - crates/game/src/play/class.rs: +8 +150 +152 +159 +161-163
    - crates/game/src/play/looks.rs: +12-13 +48-66 +138-157 +167 +172 +175 +177-179 +188-192
    - docs/HANDOFF.md: +55

