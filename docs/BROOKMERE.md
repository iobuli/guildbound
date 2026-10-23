# Brookmere village build

The default map is 360 × 360 metres. The player starts in Brookmere's square at
`(-58, -48)`. Six houses have open doorways and basic furniture; market stalls, a
well, lamps, villagers and two watchtowers surround the main streets. Rook remains
beside the starting point for equipment, control advice and recovery.

The north gate is physically open. The two guards are neutral to the player and
defend against hostile factions. Four goblin raiders occupy the camp north of the
gate; two procedural wolves and two boars roam west of it. Creature bites have a
windup, range check and wall-occlusion check, and feed the normal damage resolver.
Wilderness enemies return to their home area and respawn after a delay.
Villagers and guards select their village outfits before loading a model; the
watch wears recognizable helmets and shields. Legacy duel breakables no longer
spawn around the player in the village. F10 peaceful mode also applies to animals.

The lake is west of the village, reached by walking around the outside of the north
wall. Its dock extends from the north shore. Terrain, circular swimming volume and
rendered water share the same shoreline. Deep water uses the existing swimming and
breath system. `village_water.wgsl` adds ripple normals, a restrained reflection,
shore foam and small highlights without requiring a refraction or scene-depth pass.

Controls remain WASD/Shift/Space/Ctrl for movement, Z for sword/fists and F to talk.
B smoothly switches the aiming shoulder; aimed mouse and stick sensitivity are
reduced. Tab acquires living enemies in sight, excludes friendly NPCs and avoids
locking through walls. A muzzle obstruction ray prevents aiming through a nearby
wall that the shoulder camera can see around.
The reticle turns amber when that obstruction offsets the shot, and the spell
preview follows the actual aim point rather than snapping to a melee lock target.

The collision controller discards stored velocity into walls and honours movement
locks during airborne braking. Terrain uses the previous 480 × 480 cells over a
larger area; grass density is reduced to keep the expanded map's blade budget near
the previous size. Village buildings share meshes and materials, decorative pieces
omit collision, and lantern lights do not cast extra shadow maps.

Goblins use the existing humanoid animation rig with green skin and procedural
equipment. Wolves and boars have primitive-based bodies and a procedural gait;
they are prototype creatures, not imported animal rigs. `GB_VILLAGE=0` disables the
new district and encounters. No broad gameplay suite was run for this pass.

Validation: `cargo build -p guildbound` passed. Short rendered runs captured the
lake and village; the final village run exited with code 0. No shader validation
error or panic was observed. Bevy still logged transient slab-allocator
`Use-after-free: attempted to copy element data for an unallocated key` messages
during asset loading; their cause is unresolved. The machine's Vulkan loader also
reports missing Epic overlay JSON files. Screenshots and logs stay in the ignored
`staging/kev/brookmere/` directory. Collision, combat balance and sustained
performance still need the deferred gameplay pass.
