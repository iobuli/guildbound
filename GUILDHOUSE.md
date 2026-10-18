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
| `interior_plaster` | warm interior plaster | smoke-stained near the top | 2.0 m | inner walls |
| `wainscot` | dark oak panelling | framed panels | 1.2 m | lower inner walls |
| `ceiling_boards` | ceiling boards | tongue-and-groove, soot | 1.5 m | ceilings |
| `rug` | guild blue / madder wool rug | not tiled: one 2:3 image | whole rug | rugs |
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


## 6. Build parameters: what the builder reads (fill from the views)
The builder (`play/guildhouse.rs`) generates the whole building from **`assets/data/guildhouse.json`**. Every number below is one field: unit, allowed range, default (used until a view says otherwise), and the view to measure it from. Fill a field by reading it off G2/G3/G4/G6 against the 1 m grid. The builder clamps every value to its range, so a bad reading can't break the game.

### 6.1 Site and podium
| field | unit | range | default | from |
|---|---|---|---|---|
| `site.x`, `site.z` | m | inside ±45, off the courtyard, terrace, pond and ruins | chosen by agent-3 from ground heights | — |
| `site.face` | — | `courtyard` or a yaw in degrees | `courtyard` | — |
| `podium.margin` | m | 0.5–3 | 1.5 (podium edge past the walls, sides and back) | G2, G3 |
| `podium.front` | m | 1–4 | 2.5 (terrace in front of the door) | G3, G4 |
| `podium.min_height` | m | 0.3–1.5 | 0.5 (above the highest ground under it) | G2 |
| `podium.material` | slot | — | `podium_stone` | G1 |
| `steps.width` | m | 3–10 | 6 | G2 |
| `steps.rise` / `steps.tread` | m | 0.2–0.3 / 0.35–0.5 | 0.25 / 0.4 | G3 |
| `steps.side` | — | `front`, `front_and_sides` | `front` | G2, G4 |

### 6.2 Main block and storeys
| field | unit | range | default | from |
|---|---|---|---|---|
| `block.width` (along the front) | m | 12–18 | 16 | G2 |
| `block.depth` | m | 8–12 | 10 | G3, G4 |
| `block.wall_thickness` | m | 0.3–0.8 | 0.6 | G4 |
| `ground.height` | m | 3.8–4.8 | 4.5 | G2 |
| `ground.material` | slot | — | `wall_stone` | G1 |
| `upper.height` | m | 2.8–3.8 | 3.4 | G2 |
| `upper.jetty` (overhang past the ground walls) | m | 0–0.8 | 0.4 | G3 |
| `upper.style` | — | `timber_frame`, `stone` | `timber_frame` | G1 |
| `frame.post_spacing` | m | 1.2–3 | 2.0 | G2 |
| `frame.rails` (heights within the upper storey, 0–1) | list | 0–1 | [0, 0.45, 1] | G2 |
| `frame.braces` | — | `none`, `corners`, `every_bay` | `corners` | G2 |
| `frame.member` (beam section) | m | 0.15–0.35 | 0.25 | G6 |

### 6.3 Entrance
| field | unit | range | default | from |
|---|---|---|---|---|
| `door.width` / `door.height` | m | 2.6–4 / 3.2–4.2 | 3.0 / 3.6 | G2, G6 |
| `door.arch` | — | `flat`, `stepped`, `pointed` | `stepped` | G6 |
| `door.arch_rise` | m | 0–1.5 | 0.6 | G6 |
| `door.leaves` | count | 1–2 | 2 | G6 |
| `porch.depth` / `porch.width` | m | 0–3 / 3–7 | 1.8 / 4.2 | G3, G6 |
| `porch.roof_height` | m | 3.4–4.5 | 4.0 | G6 |
| `banners` | list of {`x`, `width`, `height`, `bottom`} on the front | width 0.6–1.5, height 1.5–4 | two at x = ±2.6, 0.9 × 3.0, bottom 1.0 | G2, G6 |
| `lanterns` | list of {`x`, `height`} | height 2–3.5 | two at x = ±1.9, 2.6 | G6 |

### 6.4 Windows (per facade: `front`, `back`, `left`, `right`)
| field | unit | range | default | from |
|---|---|---|---|---|
| `windows.<facade>.ground` | list of x centres | — | front: [−5.5, 5.5]; sides: [0] | G2, G3, G4 |
| `windows.<facade>.upper` | list of x centres | — | front: [−5.5, −2, 2, 5.5] | G2, G3 |
| `windows.size_ground` / `size_upper` | m (w × h) | 0.8–2 × 1–2.4 | 1.2 × 1.8 / 1.0 × 1.4 | G2 |
| `windows.sill_ground` / `sill_upper` | m above the storey floor | 0.6–1.4 | 1.1 / 0.9 | G2 |
| `windows.style` | — | `recessed_dark`, `shuttered`, `leaded` | `shuttered` | G6 |

### 6.5 Roof, tower, chimney
| field | unit | range | default | from |
|---|---|---|---|---|
| `roof.pitch` | deg | 35–60 | 48 | G3 |
| `roof.ridge_axis` | — | `width`, `depth` | `width` | G2, G3 |
| `roof.overhang` | m | 0.3–1.2 | 0.6 | G2, G3 |
| `roof.material` | slot | — | `roof_slate` | G1 |
| `roof.dormers` | list of {`x`, `width`} on the front | width 1.2–2.5 | [] | G2 |
| `tower.corner` | — | `front_left`, `front_right`, `back_left`, `back_right`, `none` | `front_left` | G1, G4 |
| `tower.size` | m | 4–6 | 5 | G2, G4 |
| `tower.height` (to the eaves) | m | 9–14 | 12 | G2 |
| `tower.roof` | — | `pyramid`, `spire`, `crenellated` | `pyramid` | G1 |
| `tower.roof_height` | m | 2–6 | 4 | G2 |
| `tower.belfry` | — | `true` / `false` (open arches at the top) | true | G1, G2 |
| `chimney.side` | — | `left`, `right`, `none` | `right` | G2 |
| `chimney.size` / `chimney.above_ridge` | m | 0.8–1.6 / 0.5–2 | 1.2 / 1.0 | G2 |

### 6.6 Interior (ground floor, from G4 and G7)
| field | unit | range | default | from |
|---|---|---|---|---|
| `interior.floor` | slot | — | `floor_planks` | G7 |
| `interior.desk` | {`x`, `z`, `length`, `facing`} | inside the walls | centre-back, 4 m, facing the door | G4 |
| `interior.board` | {`wall`, `offset`, `width`} | — | left wall, 0, 2.4 m | G4 |
| `interior.hearth` | {`wall`, `offset`, `width`} | — | right wall, 0, 2.2 m | G4 |
| `interior.stairs` | {`corner`, `width`} | — | back_left, 1.2 m (visual in v1) | G4 |
| `interior.beams` | spacing m | 1.5–3.5 | 2.5 | G7 |


### 6.6b Interior, full (from I1–I10)
The interior is walkable on both floors from v1 on: the stairs and the upper floor get colliders.
| field | unit | range | default | from |
|---|---|---|---|---|
| `interior.ceiling` (ground storey, beams' underside) | m | 3.2–4.2 | 3.8 | I2–I5 |
| `interior.wainscot_height` | m | 0–1.4 | 1.0 | I2–I5 |
| `interior.walls` / `wainscot` / `ceiling_material` | slot | — | `interior_plaster` / `wainscot` / `ceiling_boards` | I2–I5 |
| `interior.stairs` | {`corner`, `width`, `rise`, `tread`, `turn`} | width 1–1.6, rise 0.2–0.25, turn `straight`/`l_turn` | back_left, 1.2, 0.22, 0.3, `l_turn` | I4, I9 |
| `interior.upper_rooms` | list of {`name`, `x0`, `z0`, `x1`, `z1`, `door`: {`wall`, `offset`}} | inside the walls | office back-right, records back-left, lounge front | I1 |
| `interior.partition_thickness` | m | 0.1–0.3 | 0.15 | I1 |
| `interior.props` | list of {`kind`, `x`, `z`, `floor`, `yaw`} with kinds from I10: `desk`, `board`, `long_table`, `bench`, `stool`, `ledger_shelf`, `chest`, `map_table`, `weapon_rack`, `hearth`, `lantern`, `chandelier` | inside rooms | desk, board, hearth, 2 benches (ground); office desk, map table, 2 shelves; records 4 shelves, 2 chests; lounge long table, 2 benches, small hearth | I1, I6–I8, I10 |
| `interior.prop_sizes` | per kind {`w`, `d`, `h`} m | — | from I10 (desk 4×0.8×1.05, board 2.4×0.1×1.6, long table 3×1×0.8, bench 2.4×0.4×0.45, shelf 1.6×0.45×2.4, chest 1×0.6×0.6, map table 2×1.2×0.9, rack 1.6×0.4×1.8) | I10 |
| `interior.rugs` | list of {`x`, `z`, `floor`, `w`, `d`} | — | one 3×4.5 in the hall | G7 |
| `interior.lights` | list of {`kind`: `lantern`/`chandelier`/`hearth`, `x`, `z`, `floor`, `height`} | ≤ 6 lights (budget) | hall chandelier + hearth, one lantern per upper room | G7, I6–I8 |
| `tower.inside` | {`stair`: `spiral`, `bell`: true} | — | spiral to the belfry, bell at the top | I9 |

### 6.7 Template: `assets/data/guildhouse.json`
```json
{
  "site": { "x": null, "z": null, "face": "courtyard" },
  "podium": { "margin": 1.5, "front": 2.5, "min_height": 0.5, "material": "podium_stone" },
  "steps": { "width": 6, "rise": 0.25, "tread": 0.4, "side": "front" },
  "block": { "width": 16, "depth": 10, "wall_thickness": 0.6 },
  "ground": { "height": 4.5, "material": "wall_stone" },
  "upper": { "height": 3.4, "jetty": 0.4, "style": "timber_frame", "material": "plaster" },
  "frame": { "post_spacing": 2.0, "rails": [0, 0.45, 1], "braces": "corners", "member": 0.25, "material": "timber" },
  "door": { "width": 3.0, "height": 3.6, "arch": "stepped", "arch_rise": 0.6, "leaves": 2 },
  "porch": { "depth": 1.8, "width": 4.2, "roof_height": 4.0 },
  "banners": [ { "x": -2.6, "width": 0.9, "height": 3.0, "bottom": 1.0 }, { "x": 2.6, "width": 0.9, "height": 3.0, "bottom": 1.0 } ],
  "lanterns": [ { "x": -1.9, "height": 2.6 }, { "x": 1.9, "height": 2.6 } ],
  "windows": {
    "front": { "ground": [-5.5, 5.5], "upper": [-5.5, -2, 2, 5.5] },
    "back":  { "ground": [-4, 4], "upper": [-4, 0, 4] },
    "left":  { "ground": [0], "upper": [-2, 2] },
    "right": { "ground": [], "upper": [-2, 2] },
    "size_ground": [1.2, 1.8], "size_upper": [1.0, 1.4],
    "sill_ground": 1.1, "sill_upper": 0.9, "style": "shuttered"
  },
  "roof": { "pitch": 48, "ridge_axis": "width", "overhang": 0.6, "material": "roof_slate", "dormers": [] },
  "tower": { "corner": "front_left", "size": 5, "height": 12, "roof": "pyramid", "roof_height": 4, "belfry": true },
  "chimney": { "side": "right", "size": 1.2, "above_ridge": 1.0 },
  "interior": {
    "floor": "floor_planks",
    "desk": { "x": 0, "z": -2.5, "length": 4, "facing": "door" },
    "board": { "wall": "left", "offset": 0, "width": 2.4 },
    "hearth": { "wall": "right", "offset": 0, "width": 2.2 },
    "stairs": { "corner": "back_left", "width": 1.2 },
    "beams": 2.5,
    "ceiling": 3.8, "wainscot_height": 1.0,
    "walls": "interior_plaster", "wainscot": "wainscot", "ceiling_material": "ceiling_boards",
    "partition_thickness": 0.15,
    "upper_rooms": [
      { "name": "office",  "x0": 2,  "z0": -5, "x1": 8,  "z1": 0, "door": { "wall": "front", "offset": -1.5 } },
      { "name": "records", "x0": -8, "z0": -5, "x1": -2, "z1": 0, "door": { "wall": "front", "offset": 1.5 } },
      { "name": "lounge",  "x0": -8, "z0": 0,  "x1": 8,  "z1": 5, "door": null }
    ],
    "props": [],
    "rugs": [ { "x": 0, "z": 1, "floor": 0, "w": 3, "d": 4.5 } ],
    "lights": [ { "kind": "chandelier", "x": 0, "z": 0, "floor": 0, "height": 3.2 }, { "kind": "hearth", "x": 7.4, "z": 0, "floor": 0, "height": 0.6 } ]
  }
}
```
Coordinates: building-local metres, origin at the centre of the main block's floor (on the podium top), +z toward the front (the door), x across the front, left as seen from outside.

### 6.8 How the views get read into numbers (agent-3)
1. G2 front elevation: block width, storey heights, door and windows (x, size, sill), banners, lanterns, tower width and height, chimney.
2. G3 side elevation: depth, jetty, roof pitch and overhang, porch depth, steps.
3. G4 plan: wall thickness, interior placements, tower corner, stairs.
4. G6 entrance: door and arch, porch, banner and lantern detail.

Each reading is written into `guildhouse.json` with the view it came from (`"_source": {"block.width": "G2"}`). The builder logs every value it clamped.

## Status
- [ ] G1 concept (user / Higgsfield) · [ ] G2 · [ ] G3 · [ ] G4 · [ ] G6 · [ ] G7
- [ ] interior: I1 upper plan · I2–I5 hall walls · I6 office · I7 records · I8 lounge · I9 stairs + tower · I10 furniture
- [ ] textures: podium_stone · wall_stone · plaster · timber · roof_slate · floor_planks · iron_trim · banner_cloth · interior_plaster · wainscot · ceiling_boards · rug
- [ ] parameters read from G2/G3/G4/G6 into guildhouse.json (agent-3) · [ ] builder from the parameters (agent-3; can start now on the defaults)
- [ ] build (agent-3) · [ ] texture loader (agent-3) · [ ] in-game comparison
