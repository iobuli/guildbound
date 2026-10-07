# Sound upgrades: free packs to replace the generated set

The game's combat, magic, monster and ambience cues (`assets/audio/synth/*.wav`) are built by
`tools/make_sfx.py` from the handful of Kenney CC0 recordings in `assets/audio` (impacts, metal,
plate, cloth, leather, paper, coins, footsteps). That is as far as a small library stretches:
there is no recorded blade, fire, ice, thunder, creature voice, bird or wind in it, so those are
made by processing look-alike foley (paper becomes fire crackle, coins become ice shards) with a
little synthesis on top. Real recordings of the real thing will always sound better. The packs
below are free; you download them on your PC (the build machine has no web access).

**Check the licence on the download page before you import anything.** The licences below are the
ones the packs carried when this was written. CC0 means no attribution is required; still add a
line to `assets/audio/CREDITS.txt` so you know where each file came from.

## CC0 (public domain): use freely

| Pack | Author / where | What it covers here | Licence |
|---|---|---|---|
| Impact Sounds | Kenney (kenney.nl/assets) | already used: punches, soft thuds, light metal, plate | CC0 |
| RPG Audio | Kenney (kenney.nl/assets) | already used: cloth, leather, book, coins, belt, metal latch | CC0 |
| Interface Sounds, UI Audio | Kenney (kenney.nl/assets) | menu open/close, item equip, coins, quest board, rank-up sting | CC0 |
| Voiceover Pack: Fighter | Kenney (kenney.nl/assets) | announcer lines for the duel referee ("Fight!", "Round over") | CC0 |
| RPG Sound Pack | artisticdude (opengameart.org) | swings, battle hits, spell casts, inventory and NPC sounds | CC0 |
| 80 CC0 creature SFX | rubberduck (opengameart.org) | goblin grunts, snarls, death cries; slime gurgles | CC0 |
| 75 CC0 breaking/falling/hit SFX | rubberduck (opengameart.org) | body falls, wood and stone impacts, ice/glass breaking | CC0 |
| 40 CC0 water/splash/slime SFX | rubberduck (opengameart.org) | slime hops, squelches and deaths; the pond | CC0 |
| Freesound.org, filtered to "Creative Commons 0" | many authors | anything missing: search the terms in the table below | CC0 per sound |

rubberduck has several more numbered "CC0 SFX" packs on OpenGameArt (metal, wood, footsteps, UI);
browse the author page. On Freesound, tick the **Creative Commons 0** licence filter before
searching, and prefer 48 kHz / 24-bit WAV uploads with a clean noise floor.

## Royalty-free but NOT CC0 (read the licence)

| Pack | Where | What it covers | Licence |
|---|---|---|---|
| GDC Game Audio Bundle (one every year since 2015) | Sonniss (sonniss.com/gameaudiogdc) | professional whooshes, sword and armour foley, magic, explosions, thunder, wind and forest ambiences, creatures | Sonniss royalty-free licence: commercial use, no attribution, no redistribution of the raw files |

The Sonniss bundles are by far the biggest upgrade (tens of gigabytes of studio recordings), but the
raw files may not be redistributed, so keep them out of a public repository: commit only the edited
game-ready cuts if the licence allows it for your use, or keep the audio folder private.

## What to search for, cue by cue

| Cue (game/sfx.gd) | Now built from | Replace with (search terms) |
|---|---|---|
| `swish_blade`, `swish_light`, `swish_heavy`, `swing_kick`, `swing` | cloth + swept air | "sword swing", "sword whoosh", "staff whoosh", "punch whoosh", "heavy weapon swing" |
| `hit_blade` | punch + metal + leather | "sword hit flesh", "blade stab", "sword slash impact" |
| `clang`, `parry`, `deflect` | plate + light metal + ring | "sword clash", "sword parry", "metal clang", "sword ring" |
| `block` | soft thud + book slap + plate | "shield block", "wood shield hit", "metal shield impact" |
| `fire_cast` / `fire_hit` | slowed cloth + paper crackle | "fire whoosh", "fireball", "fire burst", "fire crackle" |
| `ice_cast` / `ice_hit` | coins + metal pings | "ice crack", "ice shatter", "frost spell", "glass shatter" |
| `lightning_cast` / `lightning_hit` | metal/paper grains + buzz | "electric zap", "electricity crackle", "thunder clap", "lightning strike" |
| `wind_cast` / `wind_hit` | cloth flap + swept air | "wind gust", "air whoosh", "air blast" |
| `arcane_cast` / `arcane_hit` | plate swell + coin shimmer | "magic cast", "magic chime", "spell impact", "magic sparkle" |
| `shadow_cast` / `shadow_hit` | reversed thuds + drone | "dark magic", "reverse whoosh", "deep impact", "demonic" |
| `awaken` | reversed plate swell + boom | "power up", "magic charge", "epic impact", "boom" |
| `rank_up` | retuned metal chime | "level up", "achievement chime" |
| `slime_hop` / `slime_hit` / `slime_die` | leather + soft thuds | "slime", "squelch", "goo splat", "wet slap" |
| `step` / `step_stone` | grass steps + book slap | "footsteps stone", "footsteps gravel", "footsteps grass" |
| `bird`, the wind bed | synthesized | "forest birds ambience", "birdsong", "wind ambience loop", "meadow ambience" |
| goblins (no cue yet) | (none) | "goblin", "creature grunt", "monster laugh", "small creature attack" |

## Dropping new recordings in

1. Convert to mono `.ogg` (or `.wav`), 44.1 or 48 kHz, trimmed so the sound starts within a few
   milliseconds of the file start (late starts make hits feel laggy). Ambience loops stay stereo.
2. Put them in `assets/audio/` and open the project once so Godot imports them.
3. Point the cue at them in `game/sfx.gd` (`CUES`: the list of file names without extension, then the
   cue's volume in dB). Use 3 to 5 variants per cue so repeats never sound identical.
4. Match levels: the generated set sits at about -16 LUFS with peaks under -1 dBFS, and the dB in
   `CUES` is the mix on top of that (blows about -16, swishes about -23, footsteps about -37).
   Normalise new files to the same loudness (any loudness-normalise tool, or reuse
   `master()` from `tools/make_sfx.py`) and keep the cue's dB, or set the dB by ear.
5. Or feed them to the generator: add the file names to its source tuples (`CLOTH`, `METAL`, `PLATE`,
   `PUNCH_M`, ...) or straight into a recipe in `tools/make_sfx.py` (for example a real sword swing
   into `swish`) and run
   `python tools/make_sfx.py`; the layering, loudness matching, `levels.json` and `CREDITS.txt`
   follow automatically. Run `-- --selftest --only=sound,mix` afterwards.
