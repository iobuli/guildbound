# Guildbound — larger map concept art

Concept art to guide the floor plan and greybox (branch grok/map-floorplan). It is AI-generated, using cropped in-game screenshots as style references (`ref/`). It is a mood and layout guide, not exact geometry. The PNGs and this note are committed in docs/map/art/; the ref/ crops are not.

## Images
| File | Area | Key visual ideas | Palette |
|---|---|---|---|
| map_overview.png | Whole world (isometric, labelled with PIL) | Guildhall hub dead centre. Dirt roads run out to the four edges in a cross with a gate on each road (4 gates). Ruins/dungeon N, Arena NE, Market E, Move Test Area SE, Training Grounds S, Cliff & River SW, Forest Path W, Hills NW | #C9A97E #7FA650 #4E7A3A #8A857A #5A4130 #B5392B #2E3F78 #4A90C2 |
| hub.png | Guildhall hub / courtyard | Two-storey half-timbered guildhall with navy crest banners, porch and steps, notice board, stone well, red-capped straw dummies, low stone walls, a timber gate out to the hills | #C9A97E #EADCB8 #5A4130 #2E3F78 #8A857A #7FA650 |
| market.png | Market street | Long dirt street, coral-red tiled roofs, red and navy awnings, glowing paper lanterns, bunting, fruit crates and barrels, villagers in straw hats; narrow lanes for alley fights | #B89A72 #E27D5F #C0392B #2E3F78 #F2A65A #6B4A2E |
| training_grounds.png | Training grounds | Grass field with rope-and-post sparring rings, wooden striking posts, straw dummies, archery targets, balance beam, wooden lookout tower with navy banner | #7FA650 #C9A97E #8B6A44 #C0392B #2E3F78 |
| arena.png | Arena / colosseum | Stone archway entrance, round sand floor, tiered grey stands, navy and red banners, fire braziers, judge's box, a lone opponent in the middle; open 1v1 duel floor | #D2B48C #8A857A #2E3F78 #B5392B #F28C28 |
| forest.png | Forest path | Winding dirt trail, faceted oaks and pines, mossy boulders, fallen logs for cover, ferns, stream with a wooden footbridge, stone lantern and a small red torii, sun shafts | #9C7E5A #4E7A3A #2F5A2E #7D8C6A #4A90C2 #B5392B |
| ruins.png | Ruins / dungeon entrance | Hilltop, worn stone steps, tall red torii, broken pillars, cracked flagstones with grass, red banners, square dungeon doorway with a purple glow; uneven ground and pillars as cover | #B5392B #8A857A #6E6A62 #7FA650 #5B2A86 #9FA8B0 |
| cliff_river.png | Cliff & river edge | Grassy cliff ledge, faceted grey rock walls, rope bridge across a gorge, waterfall, stepping stones, sandy bank, wooden lookout; drop points for dive attacks, narrow-bridge duels | #4A90C2 #E8F4FA #8A857A #7FA650 #D8C08A #5A4130 |
| test_area.png | Move test area | Flat pale sand floor with a crisp white 1 m grid, raised light-grey platform with a ledge and steps, tall plain grey wall, row of 5 red-capped dummies, wooden fence, even bright light | #E3D3B0 #FFFFFF #BFC2C4 #D9B45A #B8352E #7FA650 |
| all_art.png | Contact sheet | The overview at the top, then the eight areas in two rows | — |

## Shared style guide (for the level builder)
**Look:** stylised low-poly, flat-shaded faceted meshes, chunky proportions, no fine texture detail. It matches the existing courtyard and guildhouse.

**Core palette**
- Ground sand/dirt #C9A97E (sunlit) and #AC9473 (shade). Paths #B89A72.
- Grass #7FA650; foliage #4E7A3A, dark #2F5A2E.
- Stone #8A857A, dark #6E6A62. Wood #5A4130, light #8B6A44.
- Accent red (torii, banners) #B5392B. Guild navy #2E3F78. Roof coral #E27D5F.
- Water #4A90C2. Lantern/fire #F2A65A / #F28C28. Dungeon glow #5B2A86. Sky haze #9FB7CC.

**Materials:** matte, with a little roughness and no metallic sheen. Straw dummies are #D9B45A with a red cap #B8352E. Plaster walls are #EADCB8 with dark beams.

**Lighting:** one warm directional sun (afternoon, about 40° elevation) with soft shadows, a blue-grey sky ambient and light distance fog/haze. Exceptions:
- Forest: darker under the canopy, with sun shafts.
- Ruins: a lower sun and a cooler haze.
- Test area: flat, even noon light with short shadows, so poses read clearly.

**Scale:** character about 1.8 m. Doors 2.2 m. Gates 3 m wide. Stalls 3×3 m. Arena floor 30–40 m across. Test-area platform 1, 2 and 3 m tiers, wall 4 m tall. Grid lines exactly 1 m, white, 3–4 cm wide.

**Props per area**
- Hub: guildhall, navy banners, notice board, well, barrels, low stone walls, fence, straw dummies, timber gates.
- Market: stalls with coral roofs, cloth awnings, paper lanterns, bunting ropes, crates, barrels, pots, NPC villagers.
- Training grounds: rope-post rings, wing-chun posts, dummies, archery targets, weapon rack, balance beam, lookout tower.
- Arena: stone arch gate, tiered stands, banners on poles, braziers, judge's box, sand floor.
- Forest: oaks, pines, mossy boulders, fallen logs, ferns, stream, footbridge, stone lantern, small torii.
- Ruins: red torii, broken pillars, toppled blocks, cracked flagstones, tattered red banners, dungeon doorway and steps.
- Cliff & river: cliff rock kit, rope bridge, waterfall, stepping stones, sandy bank, lookout platform, clinging pines.
- Test area: 1 m grid floor, tiered platform and ledge, steps, plain wall, dummies (hittable, damage readout), fence. Keep it clutter-free.

**Fights per area**
- Hub: safe sparring.
- Market: tight lanes and crowds.
- Training grounds: group sparring.
- Arena: boss duels.
- Forest: ambush around cover.
- Ruins: pillars and uneven height.
- Cliff & river: vertical play, with ledge dives and knock-offs.
- Test area: move QA.
