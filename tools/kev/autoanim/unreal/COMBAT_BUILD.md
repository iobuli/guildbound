# Sparking combat build

The default scene now starts the player and one opponent with the `sparking` moveset.
Other NPCs, guests, the boss and training dummies are disabled. `GB_WORLD_FULL=1`
restores the populated sandbox. The default duel does not load or overwrite the sandbox save.

## Controls

- WASD: move; Shift: sprint; Space: jump; Ctrl: dodge; Tab: lock on.
- Tap primary: right punch → left follow-up → right kick. Buffer taps to chain.
- Hold primary: charged kick; release early or let it reach full charge.
- Hold secondary: guard; timing the raise provides the existing parry window.
- Hold Q/E: gather ki for up to 1.5 seconds of power; release to cast the equipped art/spell.
  Dodge or secondary cancels charging. More charge costs more energy.
- T or mouse button 5: next spell. Shift + T / mouse button 5: next art.
- All ten existing arts/spells are unlocked. This does not make energy unlimited.
- Kev: **Preview combat library**, then choose one of the sixteen converted clips.

## Included

Three rush sections combined into complete attacks, ground stance, guard, ki gathering and releases,
body/head/side hit reactions, guard break, walk and three step clips. Step clips are available in Kev;
the existing controller still supplies its shared dodge/roll behavior. Attack hitboxes follow the measured
active hand/foot, with existing swept collision and per-limb hurtboxes. Small gaps close during windup,
with forward root travel limited near the target and capsule collision still authoritative.

Camera has wider duel framing, lower roll shake, a larger collision probe, collision-limited shake,
and Q/E over-shoulder aiming. The flipbook shader uses atlas-safe texel bounds. Charge and release
effects use short-lived, tinted effects. Spell impacts and ground slams damage breakable scenery.

Two modular destructible buildings and surrounding props occupy the sides of the duel lane.
Three additional **actual game meshes** (rock, wall section, building section) are in
`assets/environment/sparking/`, with source paths, scale, and matching collision triangles in `manifest.json`.
They use neutral prototype materials; Unreal material graphs, textures and Chaos destruction graphs
are not converted. Destruction uses this project's rubble system, not the original game's simulation.

## Build and follow-up

`cargo build -p guildbound` produces `target/debug/guildbound.exe`.
No test suite or gameplay validation was run for this pass, as requested. Collision handling is wired,
but absence of all pose/environment clipping is **not verified**. Hit timing, charge balance, camera feel,
imported mesh orientation and shader rendering need the deferred playtest.

`build_basics.py` creates `sparking_combat.glb` from the local archive reader. `build_environment.py`
uses the `ArchiveProbe --mesh` path to produce normalized GLBs and triangle collision data.
Generated game assets remain local; the environment directory is gitignored.
