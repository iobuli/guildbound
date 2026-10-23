# Sparking combat build

The current default is the expanded [Brookmere village build](../../../../docs/BROOKMERE.md):
360m map, village NPCs, neutral gate watch, a swimmable lake, goblins and procedural animals.
The controls and animation library below remain available. The original single-opponent
setup is only spawned with `GB_VILLAGE=0`.

The default scene now starts the player and one opponent with the `sparking` moveset.
Rook, a friendly sword and ki instructor, stands just left of the player's spawn.
Other NPCs, guests, the boss and training dummies are disabled. `GB_WORLD_FULL=1`
restores the populated sandbox. The default duel does not load or overwrite the sandbox save.

## Controls

- WASD: move; Shift: sprint; Space: jump; Ctrl: dodge; Tab: lock on.
- Hold Space for a higher jump, release early to hop. Air steering preserves momentum when
  movement is released. Buffered takeoff, a brief apex float and stronger falling gravity
  improve jump control. Regular jumps use a soft landing rather than forcing a roll.
- Z: switch Sparking fists / katana. X: draw / sheathe. The katana chain includes extracted
  Trunks (Sword) and Tapion attacks alongside the UAL sword clips and authored `shonen_duel` cuts.
- F near Rook: talk, equip either style, restore health/stamina/energy or read controls.
  He stays friendly and looks toward the player; the opponent ignores him.
- Tap primary: right punch → left follow-up → right kick. Buffer taps to chain.
- Hold primary: charged kick; release early or let it reach full charge.
- Hold secondary: guard; timing the raise provides the existing parry window.
- Hold Q/E: gather ki for up to 1.5 seconds of power; release to cast the equipped art/spell.
  Dodge, secondary or jump cancels charging. More charge costs more energy; affordable charge
  is capped to the current pool so a long hold cannot make an affordable base cast fail.
  Separate Q/E recovery survives changing the equipped ability. The hotbar shows charge
  percentage and remaining recovery. Binding changes wait until charging/release is finished.
- T or mouse button 5: next spell. Shift + T / mouse button 5: next art.
- The full library is available: ten arts and ten spells. This does not make energy unlimited.
- Kev: **Preview combat library**, then choose one of the sixteen converted clips.

## Included

Three rush sections combined into complete attacks, ground stance, guard, ki gathering and releases,
body/head/side hit reactions, guard break, walk and three step clips. Step clips are available in Kev;
the existing controller still supplies its shared dodge/roll behavior. Attack hitboxes follow the measured
active hand/foot, with existing swept collision and per-limb hurtboxes. Small gaps close during windup,
with forward root travel limited near the target and capsule collision still authoritative.

Camera has wider duel framing, lower roll shake, a larger collision probe, collision-limited shake,
and Q/E over-shoulder aiming. The flipbook shader uses atlas-safe texel bounds. Charge and release
effects use small, element-tinted hand motes and flat ground sigils. Berserk uses six shallow
sigil layers instead of 36 full-body vortex layers, and wind/ward shells emit a thin broken rim.
Release flashes are smaller; charge slightly expands the released effect. Crescent Edge retains
its sword-swing clip instead of the generic ki-blast pose. Spell impacts and ground slams damage breakable scenery.

Two modular destructible buildings and surrounding props occupy the sides of the duel lane.
Three additional **actual game meshes** (rock, wall section, building section) are in
`assets/environment/sparking/`, with source paths, scale, and matching collision triangles in `manifest.json`.
They use neutral prototype materials; Unreal material graphs, textures and Chaos destruction graphs
are not converted. Destruction uses this project's rubble system, not the original game's simulation.

## Build and follow-up

`cargo build -p guildbound` produces `target/debug/guildbound.exe`.
The saved feel profile now makes sprint faster than run and tunes jump speed/air steering.
No test suite or gameplay validation was run for this pass, as requested. Collision handling is wired,
but absence of all pose/environment clipping is **not verified**. Hit timing, charge balance, camera feel,
imported mesh orientation and shader rendering need the deferred playtest.

`build_basics.py` creates `sparking_combat.glb` from the local archive reader. `build_environment.py`
uses the `ArchiveProbe --mesh` path to produce normalized GLBs and triangle collision data.
Generated game assets remain local; the environment directory is gitignored.
