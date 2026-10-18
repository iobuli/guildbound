#!/usr/bin/env python3
"""check_ai_tex.py - the H2 checker: seam, exposure against the procedural weave, normal sanity.

    ../venv/bin/python tools/model/check_ai_tex.py tools/model/ai_tex twill leather

Per kind: the three files exist, are 512 px RGBA with straight alpha; the albedo's per-axis seam
ratio is <= 1.2; the albedo's mean brightness is within 0.03 of the procedural cloth(kind) mean;
and the normal map's z mean is >= 0.95 (a sane tangent-space normal, not a flat or reversed one).
"""
from __future__ import annotations

import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import textures as tx  # noqa: E402

SEAM_MAX = 1.2
MEAN_TOL = 0.03
Z_MIN = 0.95
SIZE = 512


def seam_ratio(g):
    """Per-axis seam against the internal step on that axis: a sum-vs-mean comparison is wrong."""
    rx = float(np.abs(g[:, 0] - g[:, -1]).mean()) / max(float(np.abs(np.diff(g, axis=1)).mean()), 1e-9)
    ry = float(np.abs(g[0] - g[-1]).mean()) / max(float(np.abs(np.diff(g, axis=0)).mean()), 1e-9)
    return max(rx, ry)


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "tools/model/ai_tex"
    kinds = sys.argv[2:] or ["twill", "leather"]
    worst = []
    for kind in kinds:
        problems = []
        paths = {s: os.path.join(root, f"{kind}_{s}.png") for s in ("albedo", "height", "normal")}
        for s, p in paths.items():
            if not os.path.isfile(p):
                problems.append(f"{s}: missing")
        if problems:
            print(f"  {kind}: " + "; ".join(problems))
            worst += problems
            continue
        ims = {s: Image.open(p) for s, p in paths.items()}
        for s, im in ims.items():
            if im.mode != "RGBA":
                problems.append(f"{s}: mode {im.mode}, expected RGBA (straight alpha)")
            if im.size != (SIZE, SIZE):
                problems.append(f"{s}: {im.size}, expected {SIZE}x{SIZE}")
        alb = np.asarray(ims["albedo"].convert("RGB"), np.float32) / 255.0
        nrm = np.asarray(ims["normal"].convert("RGB"), np.float32) / 255.0
        ref = np.asarray(tx.cloth(kind, size=256)[0], np.float32).mean() / 255.0
        seam = seam_ratio(alb.mean(2))
        mean = float(alb.mean())
        z = float((nrm[..., 2] * 2 - 1).mean())
        if seam > SEAM_MAX:
            problems.append(f"seam {seam:.2f}x > {SEAM_MAX}")
        if abs(mean - ref) > MEAN_TOL:
            problems.append(f"albedo mean {mean:.3f} vs procedural {ref:.3f} (tolerance {MEAN_TOL})")
        if z < Z_MIN:
            problems.append(f"normal z mean {z:.3f} < {Z_MIN}")
        status = "PASS" if not problems else "FAIL"
        print(f"  {kind:8s} {status} | seam {seam:.2f}x | albedo mean {mean:.3f} (procedural {ref:.3f}) "
              f"| normal z mean {z:.4f}")
        for p in problems:
            print(f"           {p}")
        worst += problems
    print(f"{'all kinds pass' if not worst else str(len(worst)) + ' problem(s)'}")
    return 1 if worst else 0


if __name__ == "__main__":
    raise SystemExit(main())
