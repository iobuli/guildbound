#!/usr/bin/env python3
"""sheet_slice.py - cut a generated icon or state sheet into GUI-ready sprites.

Any variation of a sheet works with the same call: a 5x4 bag grid, a 6x8 ability grid, a 5x3
state sheet. It splits the sheet into equal cells, trims each icon to its own alpha bounds,
normalises every sprite onto one square canvas, keeps the alpha (so the GUI can fade or overlay
it), and writes a manifest the HUD can load.

  python3 tools/ui/sheet_slice.py --sheet art/sheet_items.png --cols 5 --rows 4 --name bag \
      --names tools/ui/names/bag.txt --out assets/ui/icons

Why it exists: the HUD draws its own states, so an icon is a glyph, not a picture of a phone
screen. Slicing the sheet into per-state sprites would bake the state into the art and stop the
GUI animating it. The sheet is reference art; the sprite set plus the manifest is what ships.

Options
  --mode cell    keep the whole cell: frame, glyph and caption, as the sheet draws it
  --mode glyph   keep only the upper part of the cell, dropping the caption line
  --bg key       the sheet came back on a solid colour instead of transparency: key it out
  --bg-colour    the colour to key (default: the corner pixel)
  --size N       square canvas per sprite, px (default 128)
  --pad N        transparent margin kept around the trimmed art (default 2)
  --shell        also write one horizontal strip of all sprites (a lazy single-texture atlas)

Writes <out>/<name>/<sprite>.png, <out>/<name>/manifest.json, <out>/<name>/preview.png
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time

import numpy as np
from PIL import Image, ImageDraw

BG_DARK = (3, 17, 15)
ACCENT = (46, 230, 197)


def load_rgba(path, key: bool, colour, tol: float, soft: float):
    """Sheet -> float RGBA 0..1, with a note about where the alpha came from."""
    im = Image.open(path)
    mode = im.mode
    a = np.asarray(im.convert("RGBA"), np.float32) / 255.0
    note = f"{os.path.basename(path)}: {im.size[0]}x{im.size[1]}, mode {mode}"
    rgb, alpha = a[..., :3], a[..., 3]
    if alpha.min() < 0.99:
        return a, note + " | alpha channel present"
    if not key:
        return a, note + " | WARNING: sheet is fully opaque - pass --bg key to cut the background out"
    c = np.asarray(colour if colour is not None else np.asarray(im.convert("RGB"))[0, 0], np.float32) / 255.0
    d = np.sqrt(((rgb - c) ** 2).sum(-1))
    a[..., 3] = np.clip((d - tol) / max(soft, 1e-6), 0.0, 1.0)
    return a, note + f" | keyed background {tuple(int(v * 255) for v in c)} (tolerance {tol})"


def content_cover(cell, ring, luma_floor=0.22):
    """Fraction of the cell's interior that is bright enough to be art, not frame or empty glass."""
    c = cell[ring:cell.shape[0] - ring or None, ring:cell.shape[1] - ring or None]
    if c.size == 0:
        return 0.0
    luma = c[..., :3].max(-1)
    return float(((c[..., 3] > 0.5) & (luma > luma_floor)).mean())


def trim(cell, pad, min_alpha):
    """Alpha bounding box of a cell, padded and clamped."""
    m = cell[..., 3] > min_alpha
    if not m.any():
        return None
    ys, xs = np.where(m)
    y0, y1 = max(0, ys.min() - pad), min(cell.shape[0], ys.max() + 1 + pad)
    x0, x1 = max(0, xs.min() - pad), min(cell.shape[1], xs.max() + 1 + pad)
    return cell[y0:y1, x0:x1]


def to_canvas(a, size, margin=0.06):
    """Centre the art on a square transparent canvas, scaled to fit, alpha kept."""
    im = Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8), "RGBA")
    inner = int(size * (1 - 2 * margin))
    scale = min(inner / im.width, inner / im.height, 4.0)
    im = im.resize((max(1, int(im.width * scale)), max(1, int(im.height * scale))), Image.LANCZOS)
    out = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out.paste(im, ((size - im.width) // 2, (size - im.height) // 2), im)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sheet", required=True)
    ap.add_argument("--cols", type=int, required=True)
    ap.add_argument("--rows", type=int, required=True)
    ap.add_argument("--name", required=True, help="sheet name, also the output folder")
    ap.add_argument("--names", default=None, help="text file, one name per line, in reading order ('-' skips)")
    ap.add_argument("--out", default="assets/ui/icons")
    ap.add_argument("--mode", choices=("cell", "glyph"), default="glyph")
    ap.add_argument("--bg", choices=("keep", "key"), default="key")
    ap.add_argument("--bg-colour", default=None, help="r,g,b to key out (default: the sheet's corner pixel)")
    ap.add_argument("--bg-tol", type=float, default=0.10)
    ap.add_argument("--bg-soft", type=float, default=0.10)
    ap.add_argument("--glyph-top", type=float, default=0.72, help="in glyph mode, the fraction of the cell kept")
    ap.add_argument("--size", type=int, default=128)
    ap.add_argument("--pad", type=int, default=2)
    ap.add_argument("--min-alpha", type=float, default=0.03)
    ap.add_argument("--empty-cover", type=float, default=0.008,
                    help="a cell whose interior is this empty is a slot frame, not an icon")
    ap.add_argument("--shell", action="store_true")
    args = ap.parse_args()

    colour = None
    if args.bg_colour:
        colour = tuple(float(v) / 255.0 for v in args.bg_colour.split(","))
    a, note = load_rgba(args.sheet, args.bg == "key", colour, args.bg_tol, args.bg_soft)
    print(note)

    h, w = a.shape[:2]
    cw, ch = w / args.cols, h / args.rows
    names = None
    if args.names:
        if os.path.exists(args.names):
            names = [ln.strip() for ln in open(args.names).read().splitlines() if ln.strip() and not ln.startswith("#")]
        else:
            names = [s.strip() for s in args.names.split(",") if s.strip()]
        if len(names) != args.cols * args.rows:
            print(f"WARNING: {len(names)} names for {args.cols * args.rows} cells; falling back to grid names")

    stem = os.path.join(args.out, args.name)
    os.makedirs(os.path.join(stem, "sprites"), exist_ok=True)
    icons, empty, sprites = [], [], []
    covers = {}
    for r in range(args.rows):
        for c in range(args.cols):
            y0, x0 = int(round(r * ch)), int(round(c * cw))
            cell = a[y0:int(round((r + 1) * ch)), x0:int(round((c + 1) * cw))]
            ring = max(2, int(min(cell.shape[0], cell.shape[1]) * 0.09))
            keep = max(1, int(cell.shape[0] * args.glyph_top)) if args.mode == "glyph" else cell.shape[0]
            covers[(r, c)] = content_cover(cell[:keep], ring)
    med = float(np.median(list(covers.values()))) if covers else 0.0
    empty_cover = max(args.empty_cover, 0.35 * med)
    if med and empty_cover > args.empty_cover:
        print(f"empty-cell threshold {empty_cover:.4f} (median cell content {med:.4f})")
    for r in range(args.rows):
        for c in range(args.cols):
            y0, x0 = int(round(r * ch)), int(round(c * cw))
            cell = a[y0:int(round((r + 1) * ch)), x0:int(round((c + 1) * cw))]
            box = [x0, y0, cell.shape[1], cell.shape[0]]
            if args.mode == "glyph":
                keep = max(1, int(cell.shape[0] * args.glyph_top))
                region = cell[:keep]
            else:
                region = cell
            cover = covers[(r, c)]
            cut = None if cover <= empty_cover else trim(region, args.pad, args.min_alpha)
            label = names[r * args.cols + c] if names else f"r{r + 1}c{c + 1}"
            entry = {"name": label, "row": r + 1, "col": c + 1, "cell": box,
                     "uv": [round(box[0] / w, 6), round(box[1] / h, 6),
                            round(box[2] / w, 6), round(box[3] / h, 6)],
                     "sprite": None, "empty": cut is None, "content": round(cover, 4)}
            if cut is None:
                empty.append(f"{label} (r{r + 1}c{c + 1})")
            else:
                canvas = to_canvas(cut, args.size)
                rel = f"sprites/{label}.png"
                canvas.save(os.path.join(stem, rel))
                entry["sprite"] = rel
                entry["pixel_size"] = [canvas.width, canvas.height]
                sprites.append((label, canvas))
            icons.append(entry)

    manifest = {
        "sheet": args.name,
        "source_sheet": os.path.abspath(args.sheet),
        "grid": {"cols": args.cols, "rows": args.rows, "cell_w": int(round(cw)), "cell_h": int(round(ch)),
                 "sheet_w": w, "sheet_h": h},
        "mode": args.mode,
        "sprite_size": args.size,
        "uv_origin": "top-left (glTF/Bevy TextureAtlas convention)",
        "generated": time.strftime("%Y-%m-%d"),
        "generated_by": "tools/ui/sheet_slice.py",
        "attribution": "AI-generated source (see LICENSES.md); reference art, not shipped UI.",
        "icons": icons,
    }
    with open(os.path.join(stem, "manifest.json"), "w") as fh:
        json.dump(manifest, fh, indent=2)

    # contact sheet, so a human can check the cut without opening twenty files
    cols = args.cols
    rows = int(np.ceil(len(sprites) / cols)) or 1
    tile = args.size + 12
    prev = Image.new("RGBA", (cols * tile, rows * tile), BG_DARK + (255,))
    d = ImageDraw.Draw(prev)
    for i, (label, im) in enumerate(sprites):
        x, y = (i % cols) * tile + 6, (i // cols) * tile + 6
        prev.alpha_composite(im, (x, y))
        d.rectangle([x - 1, y - 1, x + args.size, y + args.size], outline=ACCENT + (90,))
        d.text((x, y + args.size + 1), label[:18], fill=ACCENT + (255,))
    # ship the sheet itself: the GUI reads it as one texture and takes cells from the manifest
    Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8), "RGBA").save(os.path.join(stem, "sheet.png"))
    prev.save(os.path.join(stem, "preview.png"))
    if args.shell:
        strip = Image.new("RGBA", (len(sprites) * args.size, args.size), (0, 0, 0, 0))
        for i, (_, im) in enumerate(sprites):
            strip.alpha_composite(im, (i * args.size, 0))
        strip.save(os.path.join(stem, "strip.png"))

    print(f"cells {args.cols}x{args.rows} -> {len(sprites)} sprites at {args.size}px, {len(empty)} empty")
    if empty:
        print("  empty: " + ", ".join(empty))
    print(f"wrote {stem}/manifest.json, preview.png" + (", strip.png" if args.shell else ""))


if __name__ == "__main__":
    main()
