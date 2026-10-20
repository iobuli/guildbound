"""Slice a sprite sheet into frames and write their UV rectangles.

    python3 tools/kev/sheet.py <sheet.png> <frames.json> [--cols N] [--rows N] [--fps F] [--loop A B]

Frames are found from the sheet itself: columns (and rows) that are fully transparent split it into cells; a
sheet whose cells touch needs `--cols` / `--rows` (equal cells). Each frame is written as a UV rectangle
[u0, v0, u1, v1] of the whole sheet, plus its pixel rectangle, the frame size, `fps`, and the loop range
(`--loop A B`, inclusive frame indices; default all). Pixel art is meant to be sampled nearest.
"""
import argparse
import json

import numpy as np
from PIL import Image


def runs(filled):
    """[start, end) spans of consecutive True values."""
    out, start = [], None
    for i, f in enumerate(list(filled) + [False]):
        if f and start is None:
            start = i
        elif not f and start is not None:
            out.append((start, i))
            start = None
    return out


def cells(alpha, cols=None, rows=None):
    """Pixel rects (x0, y0, x1, y1) of the frames, left to right, top to bottom."""
    h, w = alpha.shape
    if cols or rows:
        cols, rows = cols or 1, rows or 1
        cw, ch = w // cols, h // rows
        return [(c * cw, r * ch, (c + 1) * cw, (r + 1) * ch) for r in range(rows) for c in range(cols)]
    xs = runs(alpha.any(axis=0))
    ys = runs(alpha.any(axis=1))
    return [(x0, y0, x1, y1) for (y0, y1) in ys for (x0, x1) in xs]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("sheet")
    ap.add_argument("out")
    ap.add_argument("--cols", type=int)
    ap.add_argument("--rows", type=int)
    ap.add_argument("--fps", type=float, default=12.0)
    ap.add_argument("--loop", type=int, nargs=2)
    args = ap.parse_args()
    im = Image.open(args.sheet).convert("RGBA")
    a = np.asarray(im)[:, :, 3] > 0
    rects = cells(a, args.cols, args.rows)
    w, h = im.size
    frames = [{"px": list(r), "uv": [r[0] / w, r[1] / h, r[2] / w, r[3] / h]} for r in rects]
    loop = args.loop or [0, len(frames) - 1]
    with open(args.out, "w") as f:
        json.dump({"sheet": args.sheet.split("/")[-1], "size": [w, h], "fps": args.fps, "loop": loop, "frames": frames}, f, indent=1)
    print(f"{len(frames)} frames, loop {loop}")


if __name__ == "__main__":
    main()
