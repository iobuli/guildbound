# Guildhouse: design matrix for Higgsfield (designs it) → agent-3 (builds it) → Higgsfield (textures it)

A **new** building, separate from the existing Guild Hall (`play/hall.rs`). Higgsfield makes the design. agent-3 builds it in Bevy from the images. Higgsfield then makes its textures through agent-1's pipeline. Follow `docs/ART_PROMPTS.md` rules: free generations only, one asset per run, and prepend the **master style block** (section 0 there) to every prompt.

## 1. What the building is (the brief Higgsfield designs to)
- **Role:** the guild's headquarters, where adventurers register, take contracts, rank up, and meet the order (the monks). It should read as more important than the village houses and the old Guild Hall, but still be a guild house, not a castle.
- **Must have:**
  - a grand front entrance with double doors
  - a hall on the ground floor (front desk, contract board, hearth)
  - an upper floor
  - one tower or belfry as a landmark visible from the courtyard
  - guild blue #334D8C and gold #C79E42 banners, a chimney, and lanterns by the door
- **Site:** on hillside, standing on a **stone podium**. The podium levels the floor and has steps up from the front, which faces the courtyard. The podium is part of the design.
- **Style:** the master block (hand-painted toon, 4 light bands, ink outline, earthy palette). Low-poly-friendly: big clear shapes, no fine filigree. Every surface is one of the materials in section 4.

## 2. Hard limits (so it can be built)
| item | limit |
|---|---|
| total footprint incl. podium and steps | **≤ 24 m × 18 m** |
| main block (the walls) | 12–18 m wide × 8–12 m deep |
| storeys | 2 (ground 4–4.5 m, upper 3–3.5 m), plus roof |
| tower | ≤ 6 m × 6 m, ≤ 18 m to the roof tip |
| ridge height | ≤ 13 m above the podium top |
| podium height above ground | 0.5–2.5 m (varies with the hill) |
| steps | 0.25 m rise, ≥ 0.35 m tread, ≥ 3 m wide |
| door | ≥ 2.6 m wide × 3.2 m high (characters are 1.75 m) |
| shapes | boxes, wedges, prisms, cylinders. Arches may be stepped or faceted. No organic sculpting. |

## 3. Views to generate (one run each, in this order)
Every view needs **a metre grid or a scale figure (a 1.75 m person) in shot**, a flat neutral background, and the same building each time. Upload the previous approved view with each later run.

| # | view | prompt (after the master block) | what agent-3 takes from it |
|---|---|---|---|
| G1 | concept, 3/4 front, daylight | "Concept art of a fantasy adventurers' guild headquarters on a low stone podium with wide front steps, two storeys: stone ground floor, timber-framed plastered upper floor, steep slate roof, one square bell tower, a chimney, guild blue and gold banners at the entrance, lanterns. Three-quarter front view, a 1.75 m figure at the steps for scale, plain sky." | the design. Pick one before going on. |
| G2 | front elevation, orthographic | "Orthographic front elevation of the attached building, no perspective, flat even light, a 1 m grid behind it, every part visible, materials clearly separated." | widths and heights of everything on the front |
| G3 | side elevation, orthographic | the same, "side elevation (from the right)" | depths, roof pitch, tower height |
| G4 | ground floor plan | "Top-down floor plan of the attached building's ground floor on a 1 m grid: walls, door, windows, front desk, contract board, hearth, stairs up; labels in plain text." | wall layout and interior placement |
| G5 | back elevation (optional) | the same as G2, "back elevation" | the back, if it differs |
| G6 | entrance detail | "Close-up of the attached building's entrance: door, arch, banners, lanterns, steps; front view, 1 m grid." | door and arch shape, banner size |
| G7 | interior concept | "Interior of the attached building's ground-floor hall: front desk, contract board, hearth, timber beams, warm lantern light, a 1.75 m figure." | interior mood and props |

## 4. Material matrix (each becomes one texture slot; Higgsfield makes one tileable albedo + height per run)
Use the ART_PROMPTS section 1 template (seamless, orthographic top view, even light, plus a height map). The files go through agent-1's `tools/model/ai_texture.py` / `textures.py` and land in **`assets/textures/guildhouse/<slot>_albedo.png`** and **`<slot>_normal.png`**. The game loads them if present and otherwise falls back to its own procedural maps.

| slot | material (for the prompt) | detail | one tile covers | used on |
|---|---|---|---|---|
| `podium_stone` | large worn ashlar blocks | weathered edges, moss in joints | 2.0 m | podium faces, steps |
| `wall_stone` | coursed ashlar wall | regular courses, thin mortar | 2.0 m | ground-floor walls, tower |
| `plaster` | lime plaster, linen tint #DBCCA8 | soft trowel marks, faint cracks | 2.0 m | upper-floor infill panels |
| `timber` | dark oak beams | long grain, adze marks | 1.0 m | frame posts, braces, doors |
| `roof_slate` | slate tiles, charcoal-blue | overlapping rows, chipped edges | 1.5 m | roofs, tower cap |
| `floor_planks` | weathered wood planks (T10) | grain, nail holes | 1.5 m | floors |
| `iron_trim` | forged iron straps and studs | hammer dents | 0.5 m | door straps, lantern frames |
| `banner_cloth` | guild blue wool, gold border | not tiled: one banner image 1:3, guild emblem | whole banner | banners |

## 5. Build plan (agent-3) once G1–G4 are approved
- **Module.** A new `play/guildhouse.rs`, self-contained, with its own material slots and placement, and one plugin line in `mod.rs`. `hall.rs` and `world.rs` stay untouched.
- **Site.** Chosen from measured ground heights (inside the rim, off the courtyard, terrace, pond and ruins).
  - The podium top is levelled at the highest ground under the footprint.
  - The podium walls run down into the hill.
  - Steps run from the front down to the ground.
  - The front faces the courtyard.
- **Geometry.** Built from the elevations as boxes and wedges at the measured sizes. Colliders on the podium, steps, walls and tower. The ground floor is walkable. The upper floor is exterior-only in v1.
- **Materials.** Each surface tagged with its slot, and the texture loader swaps them in when the files exist. Tiling follows "one tile covers".
- **Checks.**
  - A screenshot set (front, side, 3/4) compared against G1–G3.
  - A walk test: up the steps, through the door, into the hall.
  - The full test suite.

## Status
- [ ] G1 concept (agent-1 / Higgsfield) · [ ] G2 · [ ] G3 · [ ] G4 · [ ] G6 · [ ] G7
- [ ] textures: podium_stone · wall_stone · plaster · timber · roof_slate · floor_planks · iron_trim · banner_cloth
- [ ] build (agent-3) · [ ] texture loader (agent-3) · [ ] in-game comparison
