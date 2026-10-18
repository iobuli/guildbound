#!/usr/bin/env python3
"""guild_tex.py - turn the guildhouse texture runs into tileable albedo, height and normal maps.

The prompts asked for two images: the tileable colour and a greyscale height map. Some runs
answered with both panels on one sheet (colour left, greyscale right), which this splits; the rest
came back with colour only, so the height is derived from the albedo through the same pipeline
that T1 linen uses (height_from_albedo), which also guarantees the height matches the pattern.

Every model-supplied height panel is checked against the derived one before it is trusted: if the
two do not correlate, the height does not describe the albedo's own pattern and the derived one is
used instead. That check exists because a height map that is pretty but unaligned produces a
normal map that fights the colour map at every seam.

    ../venv/bin/python tools/model/guild_tex.py --raw ../../guild_raw --out ../../guild_textures
"""
from __future__ import annotations

import argparse
import os
import sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import textures as tx  # noqa: E402

SLOTS = [("T1", "podium_stone", 3.5), ("T2", "wall_stone", 3.5), ("T3", "plaster", 1.5),
         ("T4", "timber", 3.0), ("T5", "roof_slate", 2.5), ("T6", "floor_planks", 2.5),
         ("T7", "iron_trim", 3.5),
         # interior set
         ("T9", "interior_plaster", 1.5), ("T10", "wainscot", 3.0), ("T11", "ceiling_boards", 2.0)]
# T8 banner and T12 rug are single images, not tiled: kept exactly as drawn.
SINGLES = [("T8_banner.png", "banner_cloth.png"), ("T12_rug.png", "rug.png")]
SIZE = 1024
SEAM_MAX = 1.2


def seam_ratio(g):
    rx = float(np.abs(g[:, 0] - g[:, -1]).mean()) / max(float(np.abs(np.diff(g, axis=1)).mean()), 1e-9)
    ry = float(np.abs(g[0] - g[-1]).mean()) / max(float(np.abs(np.diff(g, axis=0)).mean()), 1e-9)
    return max(rx, ry)


def split_panels(a):
    """Return (colour, height|None). A greyscale right half means two panels were drawn."""
    h, w, _ = a.shape
    sat = ((a.max(2) - a.min(2)) / np.maximum(a.max(2), 1e-6)).mean(0)
    right = sat[int(w * 0.55):]
    if right.mean() > 0.06:
        return a, None
    # leftmost column of the greyscale region near the middle is the gutter
    band = np.arange(int(w * 0.40), int(w * 0.60))
    grey = band[sat[band] < 0.06]
    if len(grey) == 0:
        return a, None
    cut = int(grey.min())
    return a[:, :cut], a[:, cut:]


def trim_border(a, tol=0.004, max_frac=0.12):
    """Drop fully uniform margins the generator likes to add around a swatch."""
    g = a.mean(2) if a.ndim == 3 else a
    h, w = g.shape
    lim_y, lim_x = int(h * max_frac), int(w * max_frac)
    top = 0
    while top < lim_y and g[top].std() < tol:
        top += 1
    bot = h
    while bot > h - lim_y and g[bot - 1].std() < tol:
        bot -= 1
    left = 0
    while left < lim_x and g[:, left].std() < tol:
        left += 1
    right = w
    while right > w - lim_x and g[:, right - 1].std() < tol:
        right -= 1
    return a[top:bot, left:right]


def fit(a, size=SIZE):
    a = np.squeeze(np.asarray(a, np.float32))
    im = Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8), "RGB" if a.ndim == 3 else "L")
    return np.asarray(im.resize((size, size), Image.LANCZOS), np.float32) / 255.0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--raw", default="../../guild_raw")
    ap.add_argument("--out", default="../../guild_textures")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    report = []
    for tag, slot, strength in SLOTS:
        p = os.path.join(args.raw, f"{tag}.png")
        if not os.path.isfile(p):
            report.append(f"{tag} {slot}: missing raw image")
            continue
        src = np.asarray(Image.open(p).convert("RGB"), np.float32) / 255.0
        colour, height_panel = split_panels(src)
        source = "model height panel" if height_panel is not None else "derived from albedo"
        colour = fit(trim_border(colour))
        alb = tx.seam_fade(tx.even_lighting(colour, radius=128), band=32)
        seam_alb = seam_ratio(alb.mean(2))
        if seam_alb > SEAM_MAX:                        # try the lattice path before giving up
            lit = tx.even_lighting(colour, radius=128)
            lat = tx.lattice(lit)
            if lat["peak"] > 300:
                st, _ = tx.straighten(lit, size=SIZE)
                alb = tx.seam_fade(st, band=32)
                seam_alb = seam_ratio(alb.mean(2))
                source += " + lattice straighten"
        derived = tx.height_from_albedo(alb, small=1.0, large=24.0)
        if height_panel is not None:
            hp = fit(trim_border(height_panel if height_panel.ndim == 2 else height_panel.mean(2)))
            hp = np.squeeze(hp)
            hp = tx.seam_fade(hp, band=32)
            hp = (hp - hp.min()) / max(float(hp.max() - hp.min()), 1e-6)
            # Does the model's height actually describe the albedo's own pattern? A strongly
            # negative correlation means the same pattern inverted, so it is still usable once
            # flipped; a weak one either way means the panel is a different pattern and would
            # produce a normal map that fights the colour map.
            corr = float(np.corrcoef(hp.ravel(), derived.ravel())[0, 1])
            keep_model = abs(corr) >= 0.6
            if keep_model:
                height = 1.0 - hp if corr < 0 else hp
                source = f"model height panel (corr {corr:+.2f}{', inverted' if corr < 0 else ''})"
            else:
                height = derived
                source = f"derived from albedo (model panel corr only {corr:+.2f})"
            keep_panel = hp
        else:
            corr = 1.0
            keep_panel = None
            height = derived
        normal = tx.normal_from_height(height, strength)
        if keep_panel is not None:            # the panel is kept either way, as the model drew it
            a = (np.clip(keep_panel, 0, 1) * 255 + 0.5).astype(np.uint8)
            Image.fromarray(np.repeat(a[..., None], 3, -1), "RGB").save(
                os.path.join(args.out, f"{slot}_height_model.png"))
        for suffix, img, mode in (("albedo", alb, "RGB"), ("height", height, "L"), ("normal", normal, "RGB")):
            a = (np.clip(img, 0, 1) * 255 + 0.5).astype(np.uint8)
            if mode == "L":
                a = np.repeat(a[..., None], 3, -1)
            Image.fromarray(a, "RGB").save(os.path.join(args.out, f"{slot}_{suffix}.png"))
        prev = np.asarray(Image.fromarray((np.clip(alb, 0, 1) * 255).astype(np.uint8)).resize((512, 512)))
        grid = np.concatenate([np.concatenate([prev, prev], 1), np.concatenate([prev, prev], 1)], 0)
        Image.fromarray(grid).save(os.path.join(args.out, f"preview_{slot}_2x2.png"))
        report.append(f"{tag} {slot:13s} albedo seam {seam_alb:4.2f}x | height {source:52s} "
                      f"| corr {corr:5.2f} | normal strength {strength}")
    for raw_name, out_name in SINGLES:
        path = os.path.join(args.raw, raw_name)
        if os.path.isfile(path):
            im = Image.open(path).convert("RGBA")
            im.save(os.path.join(args.out, out_name))
            report.append(f"{raw_name[:-4]:16s} -> {out_name:18s} {im.size[0]}x{im.size[1]} "
                          f"kept as drawn (not tiled, no height map)")
    out = os.path.join(args.out, "check_guild_tex.txt")
    with open(out, "w") as fh:
        fh.write("guildhouse textures: model nano_banana_flash, 2026-10-18\n\n")
        fh.write("\n".join(report) + "\n")
    print("\n".join(report))
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
