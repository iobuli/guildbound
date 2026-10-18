# Session context (agent-2, combat / motion session) — as of 2026-10-18 15:30Z

Read this to pick up where agent-2's chat left off. `LOG.md` has the per-change entries.

## The user's standing rules
- Agents do all coding and building; run `cargo test` once at the end of a batch, not after every edit.
- Light CPU: `CARGO_INCREMENTAL=0 nice cargo build -p guildbound -j 4`. Disk is tight (~12 GB free): delete recording frames after encoding.
- The user playtests on their own PC (RTX 3060). Headless CPU screenshots/videos are fine (lavapipe + xvfb, `GB_RECORD`).
- No LLM in NPCs. Commits as `Claude <noreply@anthropic.com>` with the Co-Authored-By / Claude-Session trailers.
- After each push, post to this board (`post.sh`). Agent-1 can't push: it hands over bundles/log lines; agent-2 pushes and posts them.
- The user stops runs when usage is low: when told "no testing", stop building/testing.

## Branches (GitHub iobuli/guildbound)
| Branch | Head | What |
|---|---|---|
| `overhaul-2026-10-16` | `fc15f67` | Shared base: joined work + wardrobe (agent-1) + Monk/discipline (agent-2). Build OK, 276 tests pass. |
| `feat-monk` | `fc15f67` | Same as above (merged by fast-forward). |
| `feat-bag-craft` | not pushed | agent-2's helper: bag/satchel tabs, crafting (table, furnace, blueprints, skills that lift limits), debug stats panel. Lives in a local worktree; not merged yet. |
| `agent-log` | — | This board: README, post.sh, LOG.md, CONTEXT.md. |

## Who does what
- **agent-1** (wardrobe/cast): hero bodies, 13 garments, own textures, `appearance.json` (looks, classes, cast, areas), class outfits. Doing: close-up stills of the cast + cast showcase video on 26c5552 (no Monk/Oni in them).
- **agent-2** (this chat): combat, Monk/discipline, AI, motion (hands, climbing, mocap), merging and pushing.
- **helper** (agent-2's sub-agent): bag, crafting, debug stats (`feat-bag-craft`).

## What agent-2 added in this chat (all on overhaul-2026-10-16)
- `play/discipline.rs`: `Mastery { path, rank }` (player: class + guild rank F..S = 0..6; NPC: monks of the order by name, else weapon; rank from level/3). Monk: fists ×1.35+, guard absorbs +0.25 at 0.6 stamina, guard inside the blow = auto counter → attacker `Stunned` 1.5 s → ragdoll on the next hit that lands. Max rank (S): every path counters with its own flare (Blade iai, Arcanist frost, Hunter snare). Rank aura (wind shell) from rank D. Humans 2+ ranks below flee, outranked NPCs hesitate before attacking, demons (`Species::Demon`) never flee. Monk guard in the parry window deflects arrows and drinks spells (1.2× damage as mana, pool may reach 1.5× max, bleeds 4/s). Rank-based mocap variety (more library strikes at higher rank; monks most).
- Monk class (class panel key 4, `GB_CLASS=monk`); NPCs Brother Ishin, Sister Ren (hostile), Elder Tadao (peaceful, rank S), Oni (demon, west of the ruins). Monk dress from the `order` area in `appearance.json`.
- Hands: `motion/hand.rs` `Form` (fist, cupped, two-finger, open, claw) via `select::HandForm`; fit checker `HandRig::fit` (tests on the real UAL hands; `GB_CHECK=1` logs `[fit]`).
- Climbing: `motion/body.rs::body_out` pushes torso/head/knees off the wall before limb IK (`ContactRequest.wall_clear`); weapon sheathes at once when climbing.
- Armour no longer changes motion (`BodyPadding` off; `GB_ARMOR_PADDING=1` restores).
- Mocap: all CMU takes placed on the floor at build (`motion/bank.rs::place_on_floor`, target = UAL planted foot 0.015 m); `cargo test -p guildbound floor_report -- --ignored --nocapture`.
- Earlier in the session (before this chat's summary): beam/tightrope, climb hands, F10 peaceful, aiming, ragdoll fling + air recovery, air blink + superhero landing, wind aura, anti-clipping, senses, debug toggles, skin/looks, ground/ruins, Thoral boss on a horse, compass + shrines, class choice.

## Keys
WASD/Shift/Alt/Space move·sprint·walk·jump · Ctrl dodge · C crouch · LMB attack (hold = heavy) · RMB guard/aim · Tab lock-on · Q art (Flash Step blinks to lock-on) · E spell, MB5 cycles · F interact/rest at shrine · X draw/sheathe · G grab · R reset · I bag · K stats · 1–4 class choice · Esc close · F1/F5 tuning/save · F2 free cam · F3 perf · F4 monitor · F6 ragdoll · F7/F8 Rival/Hunter · F9 time · F10 peaceful · F12 debug list (1–0, Shift+1–3).

## Smoke / config vars added here
`GB_CLASS=monk`, `GB_RANK=0..6`, `GB_CHECK=1`, `GB_ARMOR_PADDING=1`. Recording: `GB_RECORD=<dir> GB_RECORD_FROM/TO/FPS`, `GB_SPAWN=x,z,yaw[,cam]`, `GB_PEACEFUL=1`, `GB_SCREENSHOT(_AT)`, `GB_AUTOQUIT`.

## Open items
1. Merge the helper's `feat-bag-craft` (bag tabs, crafting, debug stats) once it reports; build + test once.
2. See the new content up close: Monk player, rank aura, monks, Oni (one screenshot so far had the camera pressed into the character).
3. Record a video of Monk counters / catches / flee.
4. CMU "glue" moves per class (edit/trim takes, attack windows per strike).
5. agent-1: cast stills + showcase; Quaternius licence still unconfirmed (LICENSES.md).
6. Armour plates may now clip the arms slightly (padding off); check in game.
