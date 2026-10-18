#!/usr/bin/env python3
"""check_skins.py - validate a skin tree against the layers.rs brief.

    python3 tools/ui/check_skins.py assets/ui/skins

Checks, per pack: every expected file present with the exact canvas size, RGBA with straight
alpha, neutral white RGB where the shape is (except glass*, which must be dark), nothing drawn
outside the canvas. Then every skin.json: parses, uses only schema keys, names states the code
knows, alpha in 0..1, and every file it references exists in its `dir`.

Exits non-zero on the first pack that fails, listing each problem, so it can be a test.
"""
from __future__ import annotations

import json
import os
import sys

import numpy as np
from PIL import Image

EXPECT = {
    "ability": {"glass.png": (128, 128), "frame.png": (128, 128), "track.png": (128, 128),
                "wedge.png": (128, 128), "wedge_ccw.png": (128, 128), "tick.png": (128, 128),
                "glow.png": (128, 128), "star.png": (128, 128), "over.png": (128, 128)},
    "slot": {"glass.png": (128, 128), "frame.png": (128, 128), "check.png": (128, 128), "coin.png": (128, 128)},
    "vital": {"track.png": (256, 24), "fill.png": (256, 24), "trail.png": (256, 24),
              "ticks.png": (256, 24), "cap.png": (24, 24)},
    "panel": {"glass.png": (256, 256), "frame.png": (256, 256), "bracket.png": (48, 48),
              "title_rule.png": (256, 8), "tab.png": (96, 28)},
    "map": {"glass.png": (288, 288), "bezel.png": (288, 288), "ticks.png": (288, 288), "north.png": (288, 288)},
    "reticle": {"ring.png": (128, 128), "dot.png": (128, 128), "tick.png": (128, 128),
                "range.png": (256, 256), "land.png": (128, 128)},
    "prompt": {"chip.png": (128, 32), "key.png": (32, 32)},
}
# pieces that legitimately touch the canvas edge: 9-sliced frames, bar fills that stretch, a
# bracket the code mirrors into a corner, a full-width rule. Everything else must sit inside.
BLEED = {"track.png", "wedge.png", "wedge_ccw.png", "tick.png", "over.png", "bezel.png", "ticks.png",
         "range.png", "north.png", "glass.png", "fill.png", "trail.png", "cap.png", "tab.png",
         "chip.png", "title_rule.png", "bracket.png"}

LAYER_KEYS = {"file", "slice", "segments", "drive", "off", "pulse", "top", "tint", "alpha"}
STATES = {"idle", "hovered", "pressed", "checked", "disabled", "casting", "dim", "mastered",
          "over", "low", "open", "aiming", "in-range", "out-of-range"}
DRIVES = {"progress", "cover", "fill", "trail"}


def check_image(path, size, problems):
    if not path.endswith(".png"):
        problems.append(f"{path}: not a PNG")
        return
    im = Image.open(path)
    if im.mode != "RGBA":
        problems.append(f"{path}: mode {im.mode}, expected RGBA (straight alpha)")
    if im.size != size:
        problems.append(f"{path}: {im.size[0]}x{im.size[1]}, expected {size[0]}x{size[1]}")
    a = np.asarray(im.convert("RGBA"), np.float32)
    al = a[..., 3]
    on = al > 8
    if not on.any():
        problems.append(f"{path}: nothing drawn")
        return
    rgb = a[..., :3][on]
    name = os.path.basename(path)
    if name.startswith("glass"):
        if rgb.mean() > 90:
            problems.append(f"{path}: glass backdrop should carry its own dark colour, mean RGB {rgb.mean():.0f}")
    else:
        if rgb.min() < 250:
            problems.append(f"{path}: should be neutral white RGB with the shape in alpha (min RGB {rgb.min():.0f})")
    if al.max() > 255 or al.min() < 0:
        problems.append(f"{path}: alpha out of range")
    # nothing drawn outside the canvas is impossible by construction, but check the border is quiet
    edge = np.concatenate([al[0], al[-1], al[:, 0], al[:, -1]])
    if (edge > 8).any() and name not in BLEED:
        problems.append(f"{path}: pixels on the canvas edge - the piece should sit inside it")


def check_icons(root, problems):
    """Icons mode for H1: sheet + manifest, sizes, alpha present, neutral white RGB, cell count
    equal to the manifest, ink inside the glyph box, and empty cells genuinely empty."""
    checked = packs = 0
    for pack in sorted(os.listdir(root)):
        d = os.path.join(root, pack)
        man = os.path.join(d, "manifest.json")
        sh = os.path.join(d, "sheet.png")
        if not os.path.isfile(man):
            continue
        packs += 1
        m = json.load(open(man))
        g = m["grid"]
        cols, rows, cw, ch = g["cols"], g["rows"], g["cell_w"], g["cell_h"]
        if not os.path.isfile(sh):
            problems.append(f"{pack}: no sheet.png")
            continue
        im = Image.open(sh)
        if im.mode != "RGBA":
            problems.append(f"{pack}/sheet.png: mode {im.mode}, expected RGBA")
        if im.size != (cols * cw, rows * ch):
            problems.append(f"{pack}/sheet.png: {im.size[0]}x{im.size[1]}, expected {cols * cw}x{rows * ch}")
        icons = m.get("icons", [])
        if len(icons) != cols * rows:
            problems.append(f"{pack}/manifest.json: {len(icons)} icons for {cols * rows} cells")
        a = np.asarray(im.convert("RGBA"), np.float32)
        box = float(m.get("glyph_box", 0.8))
        inset = int(cw * (1 - box) / 2) - 1
        named = 0
        for ic in icons:
            r, c = ic["row"] - 1, ic["col"] - 1
            cell = a[r * ch:(r + 1) * ch, c * cw:(c + 1) * cw]
            if cell.size == 0:
                problems.append(f"{pack}/{ic.get('name')}: cell outside the sheet")
                continue
            al = cell[..., 3]
            if ic.get("empty"):
                if (al > 8).any():
                    problems.append(f"{pack}/{ic.get('name')}: marked empty but has ink")
                continue
            named += 1
            if al.max() <= 8:
                problems.append(f"{pack}/{ic.get('name')}: cell is blank")
                continue
            rgb = cell[..., :3][al > 8]
            if rgb.min() < 250:
                problems.append(f"{pack}/{ic.get('name')}: not neutral white RGB (min {rgb.min():.0f})")
            outer = al.copy()
            outer[inset:ch - inset, inset:cw - inset] = 0
            if (outer > 8).any():
                problems.append(f"{pack}/{ic.get('name')}: ink outside the {int(box * 100)}% glyph box")
            checked += 1
    print(f"checked {checked} glyphs in {packs} packs in {root}")
    return checked, packs


def main():
    if "--icons" in sys.argv:
        root = sys.argv[sys.argv.index("--icons") + 1]
        problems = []
        check_icons(root, problems)
        if problems:
            print(f"{len(problems)} problem(s):")
            for p in problems:
                print("  " + p)
            return 1
        print("all glyph packs match the brief")
        return 0
    root = sys.argv[1] if len(sys.argv) > 1 else "assets/ui/skins"
    problems, checked = [], 0
    for pack, files in EXPECT.items():
        d = os.path.join(root, pack)
        if not os.path.isdir(d):
            problems.append(f"{pack}/: missing pack folder")
            continue
        for name, size in files.items():
            p = os.path.join(d, name)
            if not os.path.isfile(p):
                problems.append(f"{pack}/{name}: missing")
                continue
            check_image(p, size, problems)
            checked += 1
    skins = 0
    for entry in sorted(os.listdir(root)):
        sj = os.path.join(root, entry, "skin.json")
        if not os.path.isfile(sj):
            continue
        skins += 1
        try:
            sk = json.load(open(sj))
        except Exception as e:                      # noqa: BLE001
            problems.append(f"{entry}/skin.json: does not parse ({e})")
            continue
        extra = set(sk) - {"dir", "layers"}
        if extra:
            problems.append(f"{entry}/skin.json: unexpected keys {sorted(extra)}")
        image_dir = os.path.join(root, sk.get("dir", entry))
        for i, layer in enumerate(sk.get("layers", [])):
            bad = set(layer) - LAYER_KEYS
            if bad:
                problems.append(f"{entry}/skin.json layer {i}: unexpected keys {sorted(bad)}")
            f = layer.get("file")
            if not f:
                problems.append(f"{entry}/skin.json layer {i}: no file")
                continue
            if not os.path.isfile(os.path.join(image_dir, f)):
                problems.append(f"{entry}/skin.json layer {i}: {f} not found in {os.path.relpath(image_dir, root)}/")
            if layer.get("drive") and layer["drive"] not in DRIVES:
                problems.append(f"{entry}/skin.json layer {i}: unknown drive {layer['drive']}")
            for st, a in (layer.get("alpha") or {}).items():
                if st not in STATES:
                    problems.append(f"{entry}/skin.json layer {i}: unknown state {st}")
                if not (0.0 <= float(a) <= 1.0):
                    problems.append(f"{entry}/skin.json layer {i}: alpha {a} out of 0..1 for {st}")
    print(f"checked {checked} images and {skins} skins in {root}")
    if problems:
        print(f"{len(problems)} problem(s):")
        for p in problems:
            print("  " + p)
        return 1
    print("all packs match the brief")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
