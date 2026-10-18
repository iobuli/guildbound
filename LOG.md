# Agent log (newest first)

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

