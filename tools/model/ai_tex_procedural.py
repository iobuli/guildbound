#!/usr/bin/env python3
"""ai_tex_procedural.py - write the ai_tex triple for a kind from the procedural weave.

Used when a kind has no painted swatch to process (or when a generator refuses a flat material
swatch): `cloth(kind)` already builds a tileable weave with its own normal, so the shipped triple
is albedo from cloth(), with height and normal through the same pipeline the painted kinds use.

    ../venv/bin/python tools/model/ai_tex_procedural.py --kind twill --size 512 --out tools/model/ai_tex
"""
from __future__ import annotations

import argparse
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import textures as tx  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--kind", required=True)
    ap.add_argument("--size", type=int, default=512)
    ap.add_argument("--strength", type=float, default=2.5)
    ap.add_argument("--out", default="tools/model/ai_tex")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    alb_img, _ = tx.cloth(args.kind, size=args.size)
    alb = np.asarray(alb_img, np.float32) / 255.0
    height = tx.height_from_albedo(alb, small=1.0, large=24.0)
    normal = tx.normal_from_height(height, args.strength)
    for suffix, img, mode in (("albedo", alb, "RGB"), ("height", height, "L"), ("normal", normal, "RGB")):
        a = (np.clip(img, 0, 1) * 255 + 0.5).astype(np.uint8)
        if mode == "L":
            a = np.repeat(a[..., None], 3, -1)
        Image.fromarray(a, "RGB").convert("RGBA").save(os.path.join(args.out, f"{args.kind}_{suffix}.png"))
    z = (np.asarray(Image.fromarray((np.clip(normal, 0, 1) * 255 + 0.5).astype(np.uint8), "RGB"),
                    np.float32)[..., 2] / 127.5 - 1.0).mean()
    print(f"{args.kind}: procedural {args.size}px triple written | albedo mean {alb.mean():.3f} "
          f"| normal z mean {z:.4f}")


if __name__ == "__main__":
    main()
