# Guildhouse art set — what was run, what each file is, and what to check

Every prompt from `guildhouse_higgsfield_prompts.md` was run as written, in run order, one per run.
Model: **`nano_banana_flash`** (the unlimited one), date **2026-10-18**. Nothing here was generated
on a paid model, and nothing is committed — this is a handover pack.

```
guildhouse_art/
  assets/textures/guildhouse/     T1–T11 albedo + height + normal, banner, rug, check output
  assets/art/guildhouse/          G1–G7 building views and I1–I10 interiors
  tools/model/guild_tex.py        the texture processor (tiling, alignment check, normals)
  README.md                       this file
```

## 1. Building views

The chain is a tree, not a list: each view attaches its parents, which is how the design stays one
building across sixteen images.

```
G1 concept (two takes)  ─┬─ G2 front elevation ─┬─ G3 side elevation
                         │                      ├─ G4 ground plan ─┬─ I1 upper plan ─┬─ I6 office
                         │                      │                  │                 ├─ I7 records
                         │                      │                  │                 ├─ I8 lounge
                         │                      │                  ├─ I2 front wall  └─ I9 stairs+tower
                         │                      │                  ├─ I3 back wall
                         │                      │                  ├─ I4 left wall
                         │                      │                  ├─ I5 right wall
                         │                      └─ G7 hall interior ── I10 furniture sheet
                         └─ (unused in this run)
```

| file | run | attaches | what it is |
| --- | --- | --- | --- |
| `G1_concept_A.png` | G1 | — | concept, 3/4 front. **The design pick.** |
| `G1_concept_B.png` | G1 | — | concept, 3/4 front, second take of your "rerun until you like it" |
| `G2_front_elevation.png` | G2 | G1 A | orthographic front, 1 m grid |
| `G3_side_elevation.png` | G3 | G1 A, G2 | orthographic right side, 1 m grid |
| `G4_ground_plan.png` | G4 | G1 A, G2 | top-down ground floor, 1 m grid |
| `G7_interior_hall.png` | G7 | G1 A, G4 | hall interior from the door |
| `I1_upper_plan.png` | I1 | G1 A, G2, G4 | top-down upper floor, matches the ground plan's walls |
| `I2_wall_front.png` | I2 | G1 A, G4 | hall, front wall elevation (door side) |
| `I3_wall_back.png` | I3 | G1 A, G4 | hall, back wall (front desk, ledger shelves) |
| `I4_wall_left.png` | I4 | G1 A, G4 | hall, left wall (notice board, stairs) |
| `I5_wall_right.png` | I5 | G1 A, G4 | hall, right wall (hearth, weapon rack) |
| `I6_office.png` | I6 | G1 A, I1 | guildmaster's office |
| `I7_records.png` | I7 | G1 A, I1 | records room |
| `I8_lounge.png` | I8 | G1 A, I1 | members' lounge |
| `I9_stairs_tower.png` | I9 | G1 A, G4, I1 | stairs + tower cutaway, 1 m grid |
| `I10_furniture.png` | I10 | G7 | furniture sheet, front + side per piece, 1 m grid |

**Two things about this run, stated plainly.**

- **The chain was driven from `G1_concept_A`**, because the plan makes G1's approval the gate and
  no pick came back. If you prefer B, the whole chain is a four-level re-run (G2 → G3/G4 → I1/I2–I5/G7
  → I6–I10), all free on this model.
- **`G5` (optional back elevation) was not run.** Say the word and it is one run.

## 2. Textures

Eleven tiled slots plus two single images, processed by `tools/model/guild_tex.py`.

| slot | albedo seam | slot | albedo seam |
| --- | --- | --- | --- |
| `podium_stone` | 0.12x | `iron_trim` | 0.01x |
| `wall_stone` | 0.10x | `interior_plaster` | 0.10x |
| `plaster` | 0.23x | `wainscot` | 0.20x |
| `timber` | 0.02x | `ceiling_boards` | 0.07x |
| `roof_slate` | 0.02x | `floor_planks` | 0.14x |

Each slot ships `<slot>_albedo.png`, `<slot>_height.png`, `<slot>_normal.png` at 1024² and a
`preview_<slot>_2x2.png` on dark. `banner_cloth.png` (1536×2752) and `rug.png` (1696×2528) are kept
exactly as drawn: not tiled, no height map. The seam figure is the per-axis wrap step over the
internal step; below 1.2 is the shipping bar and every slot is well inside it.

### The height-map finding (read this before wiring normals)

Your prompts ask for the height map as a second image, and the model obliged: five of seven came
back as genuine two-panel sheets, colour left and greyscale right, which the processor splits. The
panels do **not** describe their own colour patterns. Correlation against the height derived from
each albedo:

| slot | correlation | slot | correlation |
| --- | --- | --- | --- |
| `podium_stone` | +0.50 | `interior_plaster` | +0.38 |
| `wall_stone` | **−0.29** | `wainscot` | +0.48 |
| `plaster` | +0.21 | `ceiling_boards` | +0.18 |
| `timber` | +0.49 | `floor_planks` | +0.27 |
| `roof_slate` | +0.52 | | |

So every shipped `_height.png` is **derived from its albedo** — the route already used for T1 linen
— which guarantees the ridges sit under the pattern. `wall_stone` is the clearest case: its panel is
*anti*-correlated, so a normal map from it would have lit every mortar joint as a ridge and every
stone face as a groove. The model's panels are not discarded: each is beside it as
`<slot>_height_model.png`, with its correlation in `check_guild_tex.txt`, in case you want to judge
them yourself.

### Prompt adaptations

- **T12 rug.** Its row says "NOT tiled: a single 2:3 image" while pointing at the tileable template.
  Written as one flat 2:3 rug, no repetition, no frame. Flag it if you wanted the tile.
- **T8 banner.** Your prompt asks for 1:3; the model's aspect list has no 1:3, so it ran at 9:16 and
  came back 1536×2752. That is 1:1.79 rather than 1:3 — taller than a banner needs, and you may want
  it re-run or simply cropped.
- **Texture prompts gained one line**: "A flat material sample only: no building, no figure, no
  scene." Flat material swatches without it kept getting read as scenes. Every one of the twelve came
  back clean.

## 3. What to check first

I have no vision tool in this session, so the measurements are yours to take. In order of risk:

1. **Are the 1 m grids actually present and evenly spaced** in G2, G3, G4, I1, I2–I5, I9 and I10?
   Everything downstream of that is your `guildhouse.json`.
2. **Do the plans agree with the elevations?** G4's outer walls and tower footprint against G2/G3,
   and I1's outer walls against G4. That is the most likely place for the chain to drift.
3. **Is the tower on the same corner** in G1, G2, G4 and I1, and does its footprint match?
4. **G7 vs I2–I5**: the hall view and the four wall elevations should describe one room.
5. **Texture scale**: the counts in your prompts (4 blocks, 6 courses, 5 planks, 8 slate rows, 2
   panels) are the thing to verify, since that sets the in-game texel density.

## 4. For `LICENSES.md`

```
| `assets/textures/guildhouse/*`, `assets/art/guildhouse/*` | Higgsfield, model `nano_banana_flash`,
2026-10-18, prompts G1–G7, I1–I10 and T1–T12 in `guildhouse_higgsfield_prompts.md` | ours
(AI-generated source, processed here) | Textures tiled, exposure-levelled and normal-mapped by
`tools/model/guild_tex.py`; heights derived from the albedos (model height panels kept as
`*_height_model.png`, see `check_guild_tex.txt`). No third-party pixels. |
```

## 5. Reproducing

```bash
# textures: raw swatches in one folder named T1.png … T12_rug.png
../venv/bin/python tools/model/guild_tex.py --raw <raw_dir> --out assets/textures/guildhouse

# views: one run per prompt, each attaching its parents by job id, in the order shown above
```

The processor is deterministic given the same swatches, so a re-run reproduces the same tiling,
exposure and normals.
