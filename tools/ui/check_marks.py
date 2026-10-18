#!/usr/bin/env python3
"""check_marks.py - the H3 checker: E13 spell world-mark decals.

    python3 tools/ui/check_marks.py assets/textures/marks

Per decal: RGBA at 512x512; alpha is exactly 0 across a 16 px border, and 0 beyond radius 232 so the
falloff is well inside the edge; RGB is white wherever alpha is non-zero; the mark is non-empty and
centred. crack_n.png is checked as a normal map instead: RGB present, z mean >= 0.95.
Exits non-zero on any failure, so it can be a test.
"""
from __future__ import annotations

import os
import sys

import numpy as np
from PIL import Image

SIZE = 512
BORDER = 16
R_MAX = 232
DECALS = ["scorch.png", "frost.png", "crack.png", "char.png"]


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "assets/textures/marks"
    problems, checked = [], 0
    yy, xx = np.mgrid[0:SIZE, 0:SIZE].astype(np.float32)
    r = np.hypot(xx - (SIZE - 1) / 2, yy - (SIZE - 1) / 2)
    for name in DECALS:
        p = os.path.join(root, name)
        if not os.path.isfile(p):
            problems.append(f"{name}: missing")
            continue
        im = Image.open(p)
        if im.mode != "RGBA":
            problems.append(f"{name}: mode {im.mode}, expected RGBA")
        if im.size != (SIZE, SIZE):
            problems.append(f"{name}: {im.size}, expected {SIZE}x{SIZE}")
        a = np.asarray(im.convert("RGBA"), np.float32)
        al, rgb = a[..., 3], a[..., :3]
        if al.max() < 25:
            problems.append(f"{name}: nothing drawn")
            continue
        border = max(al[:BORDER].max(), al[-BORDER:].max(), al[:, :BORDER].max(), al[:, -BORDER:].max())
        if border > 0:
            problems.append(f"{name}: alpha {border:.0f} on the {BORDER} px border (must be 0)")
        outside = al[r > R_MAX].max() if (r > R_MAX).any() else 0
        if outside > 0:
            problems.append(f"{name}: alpha {outside:.0f} beyond radius {R_MAX} (falloff must be inside the edge)")
        on = al > 0
        if rgb[on].min() < 250:
            problems.append(f"{name}: not neutral white RGB where the shape is (min {rgb[on].min():.0f})")
        ys, xs = np.where(al > 25)
        c = (SIZE - 1) / 2
        off = float(np.hypot(xs.mean() - c, ys.mean() - c))
        if off > SIZE * 0.15:
            problems.append(f"{name}: centroid {off:.0f} px off centre")
        checked += 1
        print(f"  {name:11s} OK  coverage {100 * on.mean():5.1f}%  centroid offset {off:4.1f} px  "
              f"border {border:.0f}  beyond r{R_MAX} {outside:.0f}")
    n = os.path.join(root, "crack_n.png")
    if not os.path.isfile(n):
        problems.append("crack_n.png: missing")
    else:
        im = Image.open(n)
        arr = np.asarray(im.convert("RGB"), np.float32) / 255.0
        z = float((arr[..., 2] * 2 - 1).mean())
        if im.size != (SIZE, SIZE):
            problems.append(f"crack_n.png: {im.size}, expected {SIZE}x{SIZE}")
        if z < 0.95:
            problems.append(f"crack_n.png: normal z mean {z:.3f} < 0.95")
        checked += 1
        print(f"  crack_n.png  OK  normal z mean {z:.4f}")
    print(f"checked {checked} files in {root}")
    if problems:
        print(f"{len(problems)} problem(s):")
        for p in problems:
            print("  " + p)
        return 1
    print("all spell marks pass")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
