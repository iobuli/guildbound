"""ai_texture.py - turn one generated swatch into game-ready tiles (ART_PROMPTS section 1).

    python3 tools/model/ai_texture.py <swatch.png> <kind> [--n 61] [--colour DBCCA8] [--size 1024]

Pipeline, all of it from `textures.py` so every T-run shares one normal style:

    even_lighting      remove the generator's baked light gradient
    lattice            measure the thread vectors (pitch + skew) from the 2-D FFT
    straighten         warp so a whole number of threads fills the tile, skew removed
    seam_fade          cosine cross-fade over the wrap seams
    height_from_albedo the height map comes from the albedo, so ridges sit under threads
    normal_from_height the tangent-space normal map, cloth strength

Writes next to --out:
    <kind>_albedo.png         neutral near-white, the drop-in for build.py's cloth_<kind>.png
    <kind>_albedo_tinted.png  mean = the palette colour, for a white baseColorFactor
    <kind>_height.png         greyscale height, white raised
    <kind>_normal.png         drop-in for cloth_<kind>_normal.png
    preview_<kind>_albedo_2x2.png, preview_<kind>_normal_2x2.png
"""
from __future__ import annotations

import argparse
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import textures as tx  # noqa: E402

STRENGTH = 2.5  # cloth strength, as in textures.cloth(); glossy cloth wants 1.2-1.5


def to_image(a):
    a = np.asarray(a, np.float32)
    a = a if a.max() > 1.0 else a * 255.0
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8))


def seam_ratio(img):
    g = tx._grey(img)
    wrap = float(np.abs(g[:, 0] - g[:, -1]).mean() + np.abs(g[0] - g[-1]).mean())
    step = float((np.abs(np.diff(g, axis=1)).mean() + np.abs(np.diff(g, axis=0)).mean()) / 2)
    return wrap / max(step, 1e-9)


def sharpness(img):
    """Thread crispness: local contrast of the luminance, in 0..1 units per px."""
    return float(np.abs(np.diff(tx._grey(img), axis=1)).mean())


def neutral(albedo, mean=0.909, knee=0.97):
    """Near-white and colour-neutral, like cloth()/skin(), so the material factor does the tinting.

    The exposure matches cloth("linen")'s own mean (0.909) so swapping the painted weave in for
    the procedural one does not shift the garment's brightness, and highlights above `knee`
    compress instead of clipping, which keeps the thread crowns round.
    """
    a = np.asarray(albedo, np.float32)
    a = a / 255.0 if a.max() > 1.0 else a
    if a.ndim == 2:
        a = a[..., None]
    n = a / np.maximum(a.mean((0, 1)), 1e-6) * mean
    hi = n > knee
    if hi.any():
        n[hi] = knee + (1.0 - knee) * np.tanh((n[hi] - knee) / (1.0 - knee))
    return np.clip(n, 0, 1)


def preview(tile, path, size=512):
    a = to_image(np.asarray(tile, np.float32))
    a = a.resize((size, size), Image.LANCZOS) if a.size != (size, size) else a
    t = np.asarray(a)
    grid = np.concatenate([np.concatenate([t, t], 1), np.concatenate([t, t], 1)], 0)
    Image.fromarray(grid).save(path)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("swatch")
    ap.add_argument("kind")
    ap.add_argument("--n", type=int, default=None, help="threads per tile (default: from the lattice)")
    ap.add_argument("--size", type=int, default=1024, help="output size (cloth ai_tex uses 512)")
    ap.add_argument("--colour", default="DBCCA8", help="palette colour for the tinted variant")
    ap.add_argument("--band", type=int, default=32, help="seam cross-fade width in px")
    ap.add_argument("--no-lattice", action="store_true",
                    help="skip lattice/straighten: for materials with no weave lattice, e.g. leather")
    ap.add_argument("--mean", type=float, default=0.909,
                    help="target albedo mean, so the tile matches the procedural cloth(kind) mean")
    ap.add_argument("--strength", type=float, default=STRENGTH,
                    help="normal_from_height strength: 2-3 woven cloth, 1.2-1.5 glossy knit, 4 leather")
    ap.add_argument("--out", default=".")
    args = ap.parse_args()

    src = np.asarray(Image.open(args.swatch).convert("RGB"), np.float32) / 255.0
    print(f"{os.path.basename(args.swatch)}: {src.shape[1]}x{src.shape[0]}, "
          f"seam {seam_ratio(src):.2f}x")
    lit = tx.even_lighting(src, radius=128)

    if args.no_lattice:
        # No weave lattice to solve: even lighting and a seam cross-fade is the whole tiling step.
        tile = tx.seam_fade(lit, band=args.band)
        print(f"no lattice: even_lighting + seam_fade({args.band}px) -> seam {seam_ratio(tile):.2f}x")
    else:
        lat = tx.lattice(lit)
        n_hint = args.n or int(round(args.size / lat["pitch"]))
        print(f"lattice: pitch {lat['pitch']:.3f} px, skew {np.degrees(lat['rotation']):+.2f} deg, "
              f"peak {lat['peak']:.0f}x; affine scale for n={n_hint}")
        # n=None lets phase_lock use the cycle count it measures on each axis (least stretch)
        tile, info = tx.straighten(lit, n=args.n, size=args.size)
        print(f"phase-locked: {info['locked']} | straighten scale {info['scale']:.4f}, "
              f"sharpness {100 * sharpness(tile) / max(sharpness(lit), 1e-9):.0f}% of the lit swatch, "
              f"seam {seam_ratio(tile):.2f}x")
        tile = tx.seam_fade(tile, band=args.band)

    if tile.shape[0] != args.size or tile.shape[1] != args.size:
        tile = np.asarray(Image.fromarray((np.clip(tile, 0, 1) * 255 + 0.5).astype(np.uint8), "RGB")
                          .resize((args.size, args.size), Image.LANCZOS), np.float32) / 255.0
    print(f"final tile {tile.shape[1]}x{tile.shape[0]} | seam {seam_ratio(tile):.2f}x")

    height = tx.height_from_albedo(tile, small=1.0, large=24.0)
    normal = tx.normal_from_height(height, args.strength)
    alb = neutral(tile, mean=args.mean)
    tinted = tx.tint(alb, tx._rgb(args.colour))
    print(f"albedo mean {alb.mean():.3f} (target {args.mean:.3f}), max {alb.max():.3f} | tinted "
          f"{tinted.mean((0, 1)).round(3)} vs palette {tx._rgb(args.colour).round(3)}")
    z = (np.asarray(to_image(normal).convert("RGB"), np.float32)[..., 2] / 127.5 - 1.0).mean()
    print(f"height range {height.min():.3f}-{height.max():.3f} | normal strength {args.strength} "
          f"| normal z mean {z:.4f}")

    os.makedirs(args.out, exist_ok=True)
    k = args.kind
    to_image(alb).convert("RGBA").save(os.path.join(args.out, f"{k}_albedo.png"))
    to_image(tinted).convert("RGBA").save(os.path.join(args.out, f"{k}_albedo_tinted.png"))
    to_image(height).convert("RGB").convert("RGBA").save(os.path.join(args.out, f"{k}_height.png"))
    to_image(normal).convert("RGBA").save(os.path.join(args.out, f"{k}_normal.png"))
    preview(alb, os.path.join(args.out, f"preview_{k}_albedo_2x2.png"))
    preview(normal, os.path.join(args.out, f"preview_{k}_normal_2x2.png"))
    print(f"wrote {k}_albedo/height/normal at {args.size}px plus 2x2 previews")


if __name__ == "__main__":
    main()
