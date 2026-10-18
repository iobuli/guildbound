"""glyph_ink.py - lift just the glyph strokes out of a generated icon sheet.

    python3 tools/ui/glyph_ink.py <sheet.png> <cols> <rows> <names.txt> <out_dir> [--size 96]

The generator paints every cell with a frame, corner tags ("WEAPON 1", "Q BW"), a caption and,
often, a fake checkerboard instead of transparency. None of that belongs in a sprite: the HUD
draws frames, keys and states itself (gbui `widgets::slot`, `icons.rs`). So per cell this keeps
only the centre box (`BOX`, clear of tags and caption), finds the glowing strokes (bright and
saturated against the dark glass), and writes them as colour + alpha from their brightness.

`names.txt`: one name per cell in reading order; `-` skips a cell; `name@r,c` takes that sprite
from another cell (1-based), e.g. when the generator drew the wrong glyph in the idle row.
Writes <out_dir>/sheet.png (a packed atlas, `size` px cells) and manifest.json (gbui icons.rs).
"""
from __future__ import annotations

import json
import os
import sys

import numpy as np
from PIL import Image
from scipy import ndimage

BOX = (0.12, 0.18, 0.88, 0.84)  # x0, y0, x1, y1 as cell fractions: inside tags and caption


def ink(cell: np.ndarray) -> np.ndarray:
    """RGBA strokes: alpha from how far a pixel stands out from the cell's dark glass."""
    rgb = cell[..., :3].astype(np.float32) / 255.0
    v = rgb.max(-1)
    s = (v - rgb.min(-1)) / np.maximum(v, 1e-4)
    bg = np.median(v)
    a = np.clip((v - bg - 0.12) / 0.35, 0, 1) * np.clip(s / 0.35, 0, 1)
    out = np.zeros(cell.shape[:2] + (4,), np.float32)
    out[..., :3] = rgb / np.maximum(v, 1e-4)[..., None] * np.clip(v, 0.6, 1.0)[..., None]
    out[..., 3] = a * centred(a)
    return out


def centred(a: np.ndarray) -> np.ndarray:
    """Mask of the stroke clusters that reach the middle of the box (the glyph); corner tags,
    counts and caption fragments are separate clusters near the edges and drop out."""
    h, w = a.shape
    groups, n = ndimage.label(ndimage.binary_dilation(a > 0.08, iterations=3))
    yy, xx = np.mgrid[0:h, 0:w]
    core = np.hypot((yy - h / 2) / h, (xx - w / 2) / w) < 0.22
    keep = np.zeros(n + 1, bool)
    keep[np.unique(groups[core & (groups > 0)])] = True
    keep[0] = False
    return keep[groups].astype(np.float32)


def square(img: np.ndarray, size: int, pad: float = 0.08) -> Image.Image:
    ys, xs = np.nonzero(img[..., 3] > 0.05)
    if len(xs) == 0:
        return Image.new("RGBA", (size, size))
    crop = img[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
    im = Image.fromarray((crop * 255).astype(np.uint8), "RGBA")
    inner = int(size * (1 - 2 * pad))
    im.thumbnail((inner, inner), Image.LANCZOS)
    out = Image.new("RGBA", (size, size))
    out.paste(im, ((size - im.width) // 2, (size - im.height) // 2))
    return out


def main():
    args = sys.argv[1:]
    size = 96
    if "--size" in args:
        i = args.index("--size")
        size = int(args[i + 1])
        del args[i:i + 2]
    sheet, cols, rows, names, out = args[0], int(args[1]), int(args[2]), args[3], args[4]
    src = np.asarray(Image.open(sheet).convert("RGB"))
    h, w = src.shape[:2]
    cw, ch = w / cols, h / rows

    def cell(r, c):
        x0, y0, x1, y1 = BOX
        return src[int((r + y0) * ch):int((r + y1) * ch), int((c + x0) * cw):int((c + x1) * cw)]

    wanted = [l.strip() for l in open(names) if l.strip() and not l.startswith("#")]
    picks = []
    for k, name in enumerate(wanted):
        if name == "-":
            continue
        r, c = divmod(k, cols)
        if "@" in name:
            name, at = name.split("@")
            r, c = (int(x) - 1 for x in at.split(","))
        picks.append((name, square(ink(cell(r, c)), size)))
    n = len(picks)
    pc = min(n, 8)
    pr = (n + pc - 1) // pc
    atlas = Image.new("RGBA", (pc * size, pr * size))
    for i, (_, im) in enumerate(picks):
        atlas.paste(im, ((i % pc) * size, (i // pc) * size))
    os.makedirs(out, exist_ok=True)
    atlas.save(os.path.join(out, "sheet.png"), optimize=True)
    manifest = {"sheet": os.path.basename(out.rstrip("/")), "source": os.path.basename(sheet),
                "grid": {"cols": pc, "rows": pr, "cell_w": size, "cell_h": size},
                "icons": [{"name": nm} for nm, _ in picks]}
    json.dump(manifest, open(os.path.join(out, "manifest.json"), "w"), indent=1)
    print(f"{n} glyphs -> {out}/sheet.png ({pc}x{pr} at {size}px)")


if __name__ == "__main__":
    main()
