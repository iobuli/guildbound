# Sheet icons: generated art -> GUI sprites

The AI makes **reference art and glyph sheets**. The HUD draws the states. This is the path from
one to the other, so any variant of a sheet can be used, and so the parts that move stay movable.

```
generate (ART_PROMPTS section 4)      tools/ui/sheet_slice.py            crates/gbui
  sheet.png  ---------------------->  sprites/*.png + manifest.json  -->  IconSheets + Icon
  5x4, 6x8, any grid                  alpha kept, cells trimmed          atlas cell per glyph
```

## 1. Slice a sheet

```bash
# a bag grid: 5 columns x 4 rows, named from a list, caption dropped
python3 tools/ui/sheet_slice.py --sheet art/bag.png --cols 5 --rows 4 --name bag \
    --names tools/ui/names/bag.txt --out assets/ui/icons --mode glyph --size 128 --shell

# an ability sheet: 6 columns x 8 rows (four states per ability, Q block over E block)
python3 tools/ui/sheet_slice.py --sheet art/abilities.png --cols 6 --rows 8 --name abilities \
    --names tools/ui/names/abilities.txt --out assets/ui/icons --mode glyph --size 128
```

`--mode glyph` keeps the icon and drops the caption line; `--mode cell` keeps the whole cell as
drawn. Names come in reading order; `-` skips a cell. Cells whose interior is empty (a bag slot
frame with nothing in it) are detected and skipped, so an incomplete sheet still slices clean.

If a sheet comes back on a solid colour instead of transparency, `--bg key` cuts the background
out (`--bg-colour`, `--bg-tol`). It warns when a sheet has no alpha at all.

Output per sheet: `sheet.png` (the atlas texture), `sprites/<name>.png` (standalone sprites),
`manifest.json` (grid, per-icon atlas rect and uv, which cells are empty), `preview.png` (a
contact sheet, so a cut can be checked at a glance), `strip.png` with `--shell`.

## 2. Show it

`crates/gbui/src/icons.rs` loads a sheet folder as one `TextureAtlasLayout` over `sheet.png`
and gives a slot an icon:

```rust
p.spawn(icons::icon("abilities", "cinder_bolt_idle", 30.0).tinted(icons::IconTint::Dim));
```

`IconTint` covers the three states the ability table needs: `Full`, `Dim` (a pool cannot pay the
cost), `Mastered`. Everything else about how a slot looks stays with the theme classes.

## 3. Keep the moving parts moving

A sprite is **one glyph**. These are sibling nodes above it, not baked into the art:

| part | drawn as | driven by |
| --- | --- | --- |
| casting progress arc | an arc node over the icon (`ring`-style material) | `SpellDef`/`Ability` prepare + recovery, or a `Tween` on fill |
| pool gauge under the slot | a thin `widgets::bar` child | pool / cost |
| cooldown or recovery sweep | the same arc material, reversed | recovery time |
| mastered | accent border + a small star sprite at low alpha | the ability's mastered flag |
| can't afford | `IconTint::Dim` + a dark gauge track | pool < cost |

That is why the state rows in a generated sheet (idle / casting / dim / mastered) are reference
art only: shipping them as sprites would freeze each ability into four pictures and stop the HUD
animating one.

## 4. Alpha is the overlay control

An icon draws at `IconTint::alpha() * Icon::alpha * inherited Opacity`, so the same sprite can be
used again at any lower alpha: a ghost of the previous weapon, a pool-fill hint behind the glyph,
a watermark on a locked slot.

```rust
p.spawn(icons::Icon::overlay("abilities", "veil_idle", 0.22)); // same sprite, translucent
```

Two consequences worth keeping:

- **Sliced sprites must stay PNG with alpha.** Converting a sheet or a sprite set to JPEG throws
  transparency away, and a JPEG background cannot be faded - it fades as a grey box. Keep the
  reference art as JPEG if space matters, but ship the sprites as PNG.
- Fading is a multiplied product up the tree, so putting an `Opacity` on a slot fades its icon and
  its labels together, which is what a disabled slot wants.

## 5. Attribution and the data the sheets must match

- Prompts live in `docs/ART_PROMPTS.md` (section 4 for UI), and every used asset gets a line in
  `LICENSES.md` with model, date and prompt id.
- Sheet contents must come from the **game's** tables, not `sim`: items and the axe tool from
  `play/items.rs` + `play/tools.rs`, weapons from `assets/anim/movesets/*.json`, armour from
  `play/armor.rs`, and abilities from `play/magic.rs` (`ABILITIES`: Q arts on Battlewill, E spells
  on Magicules, costs as listed, mastered at `MASTERED_COST`). `tools/ui/names/` holds the
  in-reading-order name lists for those two sheets.
- Digits 1-0 belong to the weapon slots; the two ability slots are Q and E.

## What ships (agent-1, 2026-10-18)
The generated sheets came back RGB with a painted checkerboard and with tags, captions and frames in
every cell, so `sheet_slice.py`'s sprites kept all of that. What ships is cut by
`tools/ui/glyph_ink.py` instead: per cell, only the stroke clusters that reach the centre, alpha
from their brightness, packed into a small atlas (`assets/ui/icons/{bag,abilities}/sheet.png` +
`manifest.json`, 96 px cells, ~250 KB together). Ability glyphs: idle row only, `orbit_motes` from
its mastered cell (the idle one repeated Storm Lance's arrow); names in `tools/ui/names/`.
In the HUD (`play/hud/hotbar.rs`): Q / E show the bound ability's glyph over a cast fill and above
a pool gauge (theme classes `gauge.bw`, `gauge.mp`, `cast.fill`); the weapon slot shows its weapon.
Known flaws: a stray digit beside the greatsword; Orbit Motes' ring is thin.
