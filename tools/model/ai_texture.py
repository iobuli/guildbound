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

STRENGTH = 2.5  # cloth strength, as in textures.cloth()


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
    ap.add_argument("--size", type=int, default=1024)
    ap.add_argument("--colour", default="DBCCA8", help="palette colour for the tinted variant")
    ap.add_argument("--band", type=int, default=32, help="seam cross-fade width in px")
    ap.add_argument("--out", default=".")
    args = ap.parse_args()

    src = np.asarray(Image.open(args.swatch).convert("RGB"), np.float32) / 255.0
    print(f"{os.path.basename(args.swatch)}: {src.shape[1]}x{src.shape[0]}, sharpness {sharpness(src):.4f}, "
          f"seam {seam_ratio(src):.2f}x")

    lit = tx.even_lighting(src, radius=128)
    lat = tx.lattice(lit)
    n_hint = args.n or int(round(args.size / lat["pitch"]))
    print(f"lattice: pitch {lat['pitch']:.3f} px ({lat['nx']:.1f} x {lat['ny']:.1f} threads), "
          f"skew {np.degrees(lat['rotation']):+.2f} deg, peak {lat['peak']:.0f}x; affine scale for n={n_hint}")

    # n=None lets phase_lock use the cycle count it measures on each axis (least stretch)
    tile, info = tx.straighten(lit, n=args.n, size=args.size)
    print(f"phase-locked: {{axis: (threads, measured cycles)}} = {info['locked']}")
    print(f"straighten: scale {info['scale']:.4f} -> pitch {info['pitch_after']:.3f} px, "
          f"sharpness {sharpness(tile):.4f} ({100 * sharpness(tile) / max(sharpness(lit), 1e-9):.0f}% of the lit swatch), "
          f"seam {seam_ratio(tile):.2f}x")
    faded = tx.seam_fade(tile, band=args.band)
    print(f"seam_fade({args.band}px): seam {seam_ratio(faded):.2f}x  -> "
          f"{'SEAMLESS' if seam_ratio(faded) <= 1.2 else 'STILL SEAMED'}")

    height = tx.height_from_albedo(faded, small=1.0, large=24.0)
    normal = tx.normal_from_height(height, STRENGTH)
    alb = neutral(faded)
    tinted = tx.tint(alb, tx._rgb(args.colour))
    print(f"albedo mean {alb.mean():.3f} (cloth(\"linen\") is 0.909), max {alb.max():.3f}, "
          f"rgb {alb.mean((0, 1)).round(3)} | tinted "
          f"{tinted.mean((0, 1)).round(3)} vs palette {tx._rgb(args.colour).round(3)}")
    print(f"height range {height.min():.3f}-{height.max():.3f} | normal mean rgb {normal.mean((0, 1)).round(3)}")

    os.makedirs(args.out, exist_ok=True)
    k = args.kind
    to_image(alb).save(os.path.join(args.out, f"{k}_albedo.png"))
    to_image(tinted).save(os.path.join(args.out, f"{k}_albedo_tinted.png"))
    to_image(height).convert("L").save(os.path.join(args.out, f"{k}_height.png"))
    to_image(normal).save(os.path.join(args.out, f"{k}_normal.png"))
    preview(alb, os.path.join(args.out, f"preview_{k}_albedo_2x2.png"))
    preview(normal, os.path.join(args.out, f"preview_{k}_normal_2x2.png"))
    print("wrote", ", ".join(sorted(f for f in os.listdir(args.out) if f.startswith(("linen", "preview")))))


if __name__ == "__main__":
    main()
