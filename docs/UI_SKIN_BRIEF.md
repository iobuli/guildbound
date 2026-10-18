# UI skin brief for Higgsfield (the one contract for HUD art)

Code: `crates/gbui/src/layers.rs`. Placeholders that this art replaces: `assets/ui/skins/`.

**Universal rules (every PNG):**
- PNG RGBA with straight alpha, never JPEG.
- **Neutral white RGB with the shape in alpha only**, so the JSON tints it. Exception: the `glass*` backdrops, which carry their own dark colour.
- Exact canvas size as listed, nothing drawn outside it, centred.
- Rotations are made by the code, so draw one piece at 12 o'clock.
- Deliver as one zip that mirrors `assets/ui/skins/…`, plus one `skin.json` per skin with the opacity numbers.
- Draw procedurally (exact geometry); generators only for textured surfaces (glass grain, parchment).
- Palette: accent #2EE6C5, accent2 #7CFFB2, bg #03110F, BW gold #FFC857, MP violet #B79CFF, danger #FF5C7A, warn #FFC857, info #7FD8FF. These belong in the JSON tints, not baked into the white layers.

**Skin JSON schema (already implemented, do not change):**
```
{ "dir": "<image folder, optional>", "layers": [ { "file": "x.png", "slice": px, "segments": n, "drive": "progress|cover|fill|trail", "off": 0..1, "pulse": s, "top": bool, "tint": "#rrggbb", "alpha": { "idle": a, "<state>": a } } ] }
```
- Layers are drawn in list order. `top` draws above the widget's text and glyph.
- **States** come from two places:
  - automatic: hovered, pressed, checked, disabled;
  - set by the game: casting, dim, mastered, over (above max pool), low (below 25 %), open.
- When several states are active, the highest in this fixed order wins: idle < hovered < pressed < checked < open < mastered < casting < over < low < dim < disabled (JSON key order doesn't matter).
- **Segments:** segment k of n lights when drive ≥ (k+0.5)/n; unlit segments draw at `off` × alpha.

**Packs, in priority order:**

1. **`ability/`**: the Q/E ring. Each image is 128², and each skin JSON goes in its own folder, `ability_bw/` (gold) and `ability_mp/` (violet):

   | File | Contents | Notes |
   |---|---|---|
   | `glass.png` | dark rounded plate, r≈10 | 9-slice 16 |
   | `frame.png` | 1–2 px border + corner brackets | 9-slice 16 |
   | `track.png` | full ring, band r = 51..57 | |
   | `wedge.png` | one 30° wedge of that band at 12 o'clock, about 3° total gap | code repeats it ×12, clockwise |
   | `wedge_ccw.png` | mirror of `wedge.png` | for recovery draining |
   | `tick.png` | one tick, r = 59..63, about 8° wide, at 12 o'clock | code repeats it ×10 |
   | `glow.png` | soft light peaking at r≈54 | |
   | `star.png` | 4-point star at (104,22), r≈14 | |
   | `over.png` | thin outer halo ring, r = 64..66 | for over-mana |

2. **`slot/`**: the weapon slot and bag button, 128²:
   - `glass.png`, `frame.png`: same rules as the ability pack;
   - `check.png`: the selected-state inner glow edge;
   - `coin.png`: a small coin mark at the bottom-right for the gold count, about 24² placed at (100,104).
   - Skins `weapon/`, `bag/`.

3. **`vital/`**: the HP/ST/BW/MP bars, 256×24, all 9-slice 6:
   - `track.png`: bar trough;
   - `fill.png`: soft vertical shade, white; the JSON tints it per pool;
   - `trail.png`: the damage trail;
   - `ticks.png`: 10 notches, tiled;
   - `cap.png`: a bright end-cap, 24×24.

   Skins `vital_hp`, `vital_st`, `vital_bw`, `vital_mp`. Fill and trail widths are set by code (drives `fill`, `trail`); `low` state glow.

4. **`panel/`**: the log, the stats page, the bag and the tooltip, 256², with:
   - `glass.png`: 9-slice 24, with faint scanlines and grain baked in, in the dark colour;
   - `frame.png`: 9-slice 24;
   - `bracket.png`: 48², one corner, top-left; the code mirrors it to the other corners;
   - `title_rule.png`: 256×8, the header underline;
   - `tab.png`: 96×28, 9-slice 8, with states idle, hovered and checked.
5. **`map/`**: the minimap bezel, 256²:
   - `bezel.png`: ring band r = 112..124;
   - `ticks.png`: one tick at 12 o'clock, repeated ×36;
   - `north.png`: an N marker at 12 o'clock, outside the bezel;
   - `glass.png`: inner vignette disc, dark.
6. **`reticle/`**: the E13 aim reticle, 128²:
   - `ring.png`, `dot.png`;
   - `tick.png`: one tick at 12 o'clock, ×4;
   - `range.png`: a dashed ring, 256²;
   - `land.png`: a landing marker, 128², drawn top-down;
   - states idle, aiming, in-range, out-of-range.
7. **`prompt/`**: the "[E] Talk" chip:
   - `chip.png`: 128×32, 9-slice 10;
   - `key.png`: 32², the key-cap box.

**Also send:**
- one `preview.png` per pack, assembled on dark, showing every state;
- the generator script, so pieces can be regenerated at other sizes.

**Character art (separate track, unchanged):** ears, tails, horns, wings and halos as mount-point parts. Send PNG references front and side at model scale.
