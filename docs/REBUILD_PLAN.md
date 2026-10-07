# Rebuild plan (living document)

Edited as the work moves: finished items get ticked, dropped ideas get deleted, new ones get added.
Last updated: 2026-10-07.

## What the user asked for
1. Delete every animation in use and rebuild the animation system properly, from a large set of
   downloaded animations (git / sites that allow it). Keep the logic that works.
2. A better base character.
3. Hand-to-hand that is fun: a skill tree, combos, grabs / throws, counters.
4. Sword use.
5. Buffs become **Arts**: many status-effect Arts, each with levels 1-5.
6. Magic = projectiles, negative-effect casting (debuffs), healing.
7. Combo QTE: during a combo a random 4-5 key string appears; typing it in time launches an
   unblockable attack sequence (fists, sword or magic), where the best animations get used.
8. Better sound effects, NPCs, real items (3D models, not boxes).
9. Godot AI (MCP into the live editor) and the Godot skill installed.
10. Design docs (this one, MAGIC_AND_STATS.md, RESEARCH_MAGIC_COMBAT.md, COMBAT_SPEC.md) are
    living: rewrite them as systems change.

## Decisions
| Topic | Choice | Why |
|---|---|---|
| Base character | VRoid Studio sample avatars, **CC0** (OpenGameArt "VRoid Studio CC0 models": base male / female, AvatarSample D, D Darkness, E, F, G, Sakurada Fumiriya, Sendagaya Shino) | Real anime look, faces with expressions, spring-bone hair / cloth, standard humanoid rig. The user can make more characters in the free VRoid Studio and drop the .vrm in. |
| VRM import | V-Sekai **godot-vrm** v2.1.0 (MIT) | Imports VRM 0.x / 1.0, MToon toon shader, spring bones, maps the rig to Godot's humanoid profile. |
| Retargeting | Godot's **SkeletonProfileHumanoid** + BoneMap for every model and every clip | The engine's own retargeting: any humanoid source works on any humanoid character. Replaces the one-rig-only pipeline. |
| Bone names in code | One logical-name table (pelvis, spine, chest, hand_l...) mapped to the profile's names | BodyRig, MotionIK, GetUpPose, RagdollPose keep working on any character. |
| Animation data | `data/anims.json`: role -> source clip, slice, speed, loop, tags | The tree is built from data; swapping or adding a clip is a data edit. |

## Animation sources (all free, downloadable without an account)
| Source | Licence | What it gives |
|---|---|---|
| Quaternius Universal Animation Library 1 + 2 (Standard) | CC0 | Locomotion basics, swim, climb-up, sword combos / block / dash, spell casts, interactions |
| 100STYLE (Mason et al., Zenodo) | CC BY 4.0 (credit) | 100 locomotion styles: forward / backward / sidestep walk and run, idles, transitions. Fight footwork. |
| ACCAD Motion Lab (Ohio State) | CC BY 3.0 (credit) | Male 2 martial arts: kicks, punches, stances, walks / turns; general and locomotion takes |
| CMU Graphics Lab | free for any use | Boxing, guard, kicks, falls, get-ups, ducks (2500+ takes on demand) |
| Motifect Combat Complete | free incl. commercial, no redistribution of the raw files | Strikes, blocks, parries, hit reactions, grabs / throws, knockdowns |

Raw downloads live in `D:\Godot\assets\...`, outside the repo. Imported, retargeted clips are in the repo.

## What is kept (logic)
BodyRig (physical body), RagdollPose, GetUpPose, MotionIK maths (foot IK, strike / kick IK, reach
assist, stance), Combat (timeline, hit sweep, guard / parry, hit-stop, PvP rules), Intent, LockOn,
FighterAI, CharModel stats engine, side panel / HUD / status window, Sfx / Vfx, self-tests.

## What is deleted
The Quaternius UBC character, every animation resource in `assets/animations/`, the hand-built
AnimationTree, the old bone-name assumptions. Rebuilt on the new base.

## Phases
- [ ] **0. Tools**: Godot AI plugin (in `addons/godot_ai`, MCP registered in `.mcp.json`, telemetry
      off), Godot skill (`.claude/skills/godot`). Done when the editor is open and the tools respond.
- [ ] **1. New base + retarget pipeline**: godot-vrm in, VRoid avatars imported, logical bone table,
      BodyRig / MotionIK / GetUp on the humanoid profile, importer for GLB / FBX / BVH sources onto
      the profile, `data/anims.json`, tree built from data. Old animations deleted.
- [ ] **2. Animation library**: locomotion (100STYLE + UAL), fight footwork, strikes, kicks, blocks,
      reactions, grabs / throws, sword set, casts. A contact sheet for every role.
- [ ] **3. Hand-to-hand skill tree**: branches below; moves unlock and upgrade; combos, grabs,
      throws, counters.
- [ ] **4. Combo QTE (Art Sequence)**: random 4-5 key prompt mid-combo, unblockable cinematic chain.
- [ ] **5. Sword**: weapon equip changes the move set; blade hit detection; sword guard / parry.
- [ ] **6. Arts (levels 1-5) and Magic** (projectiles, debuffs, heals) on one status-effect engine.
- [ ] **7. NPCs and real items**: VRoid NPCs (trainer, merchant, villagers), 3D item models in the
      world and in the hands, shop.
- [ ] **8. Sound pass**: CC0 / royalty-free packs for hits, swishes, clashes, spells, footsteps by
      surface, UI.
- [ ] **9. Slimes, jobs, Hunter's Card, Magic Cores** (below).
- [ ] **10. UI pass**: one logical layout (see UI rules below), timers on everything that runs out.

## Monsters: slimes (first PvE enemy)
- Bouncy blob with a squash-and-stretch hop, jump attack, splits when big ones die (later).
- Sizes / colours = level and element; physical (they can be grabbed, thrown, knocked away).
- Drop table: Magic Cores (always a roll), slime gel (crafting), coins.

## Jobs and the Hunter's Card
- **Hunter's Card**: an item every player carries (inventory slot that can't be dropped). It shows
  rank, completed jobs, active jobs with progress, and earned rewards. Opens from the side panel.
- **Jobs** come from the guild board / NPCs: kill N of a monster, gather, deliver, escort, bounty.
  Each has: id, giver, objective(s) with counters, rank requirement, rewards (coins, rank points,
  cores, items), repeatable or one-off.
- First job: **"Slime Cull"** - kill 5 slimes; reward coins + rank points + a core.
- Rank: F -> E -> D -> C -> B -> A -> S from rank points; rank gates harder jobs.

## Magic Cores
- Dropped by monsters. Rarity: Common, Uncommon, Rare, Epic, plus **special named cores** from
  bosses / rare monsters (e.g. **"Death Dragon Core"**) with unique uses.
- Uses (one item, many sinks): sell to the guild; fuel and level up Arts (L1 -> L5 needs cores of
  rising rarity); craft / enchant gear (add a stat line, element); power magic items; job turn-ins.
- Rarity colours shared with items: common grey, uncommon green, rare blue, epic purple,
  special orange/gold with its own name and lore.

## Arts feedback (self buffs)
- Cast: a short cast animation (upper body), a burst of particles in the Art's colour, a sound.
- While up: an aura (particles / outline) and an icon with a radial timer and seconds in the buff
  bar; it blinks in its last 5 s; a sound when it ends.

## UI rules
- One place for each thing: vitals top-left, target top-centre, buffs under vitals, hotbar
  bottom-centre, side panel bottom-right (Tab), messages bottom-centre above the hotbar.
- Everything that runs out shows a timer. Everything clickable shows what it does on hover.
- Rarity colours everywhere the same; keyboard shortcuts shown next to every action.

## Hand-to-hand skill tree (draft)
Skill points from levels and training. Each node unlocks a move or upgrades one.
- **Striker**: longer strings (4 -> 6 hits), new string enders, faster recovery, counter-hit damage.
- **Grappler**: grab (LMB + RMB up close), throws (direction + grab), clinch knees, ground pin,
  breaking a turtle guard (grabs beat blocks).
- **Counter**: parry window +, parry -> counter strike, slip -> counter, guard-break resistance.
- **Rhythm**: PERFECT window +, finisher after fewer PERFECTs, Art Sequence charge faster.
Rock-paper-scissors core: strikes beat grabs (interrupt), grabs beat blocks, blocks beat strikes.

## Art Sequence (combo QTE, draft)
- A combo of N landed hits (or a full Focus meter) opens a 1.2-2 s window: a random string of 4-5
  keys (from a readable pool, not the movement keys) appears over the target.
- Typed in order and in time: an unblockable, uninterruptible chain plays (camera framing, slow-mo
  on the last hit), drawn from the best clips of the current style (fists, sword, magic).
- A wrong key or timeout: the combo just continues; the meter is kept at half.
- Difficulty scales the string length / time; PvP: the defender gets a mirrored string to break it.

## Arts and Magic (draft, to be written into MAGIC_AND_STATS.md when built)
- **Arts** (were "buffs"): self / ally status effects, each with levels 1-5 (magnitude, duration,
  cost, side effects scale). Many effects: Haste, Might, Stoneskin, Regeneration, Focus, Iron Will,
  Battle Trance, Thorns, Vampiric Edge, Clarity, Featherfall, Night Eye, Second Wind...
- **Magic**: projectiles (fire bolt, ice lance, chain lightning, stone shot), negative-effect casts
  (slow, burn, poison, weaken, silence, root, blind, fear), healing (heal, regeneration, cleanse).
- One status-effect engine for Arts, Magic and items: stacks, durations, ticks, immunity, cleanse.

## Credits to show in game
100STYLE (CC BY 4.0), ACCAD (CC BY 3.0), Quaternius / Kenney / VRoid samples (CC0), CMU mocap,
Motifect, 496 RPG icons by Henrique Lazarini (CC0).
