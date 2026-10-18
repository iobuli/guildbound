#!/usr/bin/env python3
"""glyphs.py - the HUD glyph set (H1), drawn as vectors, no generator involved.

30 glyphs, each its own PNG at 96x96: neutral white RGB with the stroke carried in alpha, so the
HUD tints them (turquoise for items, gold for arts, violet for spells). Stroke 5.5 px at 96, the
glyph fills the 80 percent centre box, no frame, no tag, no caption.

Also writes, per folder, `sheet.png` (8 columns of 96 px cells, in name order) and a
`manifest.json` in the existing format (grid + icons[{name}]), which is what gbui::icons loads.

    python3 tools/ui/glyphs.py --out assets/ui/icons

Names come from tools/ui/names/bag.txt (20) and tools/ui/names/abilities_glyphs.txt (10).
"""
from __future__ import annotations

import argparse
import json
import math
import os

import numpy as np
from PIL import Image, ImageDraw

CELL = 96
BOX = 0.80            # the glyph fills the 80 percent centre box
STROKE = 5.5          # px at 96
SS = 4
WHITE = (255, 255, 255, 255)


def rbox(p):
    """Normalised glyph coords (0..1, y down) -> canvas coords, padded by half a stroke so the
    drawn ink lands exactly inside the 80 percent centre box."""
    pad = CELL * (1 - BOX) / 2 + STROKE / 2
    span = CELL - 2 * pad
    return (pad + p[0] * span, pad + p[1] * span)


def stroke(d, pts, w=STROKE):
    p = [rbox(q) for q in pts]
    p = [(x * SS, y * SS) for x, y in p]
    d.line(p, fill=WHITE, width=int(w * SS), joint="curve")
    for x, y in (p[0], p[-1]):                      # round caps: PIL has none
        r = w * SS / 2
        d.ellipse([x - r, y - r, x + r, y + r], fill=WHITE)


def poly(d, pts, w=STROKE, filled=False):
    p = [(x * SS, y * SS) for x, y in (rbox(q) for q in pts)]
    if filled:
        d.polygon(p, fill=WHITE)
    else:
        d.line(p + [p[0]], fill=WHITE, width=int(w * SS), joint="curve")


def circ(d, c, r, filled=False, w=STROKE):
    cx, cy = rbox((c[0], c[1]))
    rr = r * CELL * BOX * SS
    if filled:
        d.ellipse([cx * SS - rr, cy * SS - rr, cx * SS + rr, cy * SS + rr], fill=WHITE)
    else:
        d.ellipse([cx * SS - rr, cy * SS - rr, cx * SS + rr, cy * SS + rr], fill=None, outline=WHITE, width=int(w * SS))


def arc(d, c, r, a0, a1, w=STROKE):
    cx, cy = rbox((c[0], c[1]))
    rr = r * CELL * BOX * SS
    d.arc([cx * SS - rr, cy * SS - rr, cx * SS + rr, cy * SS + rr], a0, a1, fill=WHITE, width=int(w * SS))


def dot(d, c, r):
    circ(d, c, r, filled=True)


# ---------------------------------------------------------------------------------------
# the glyphs: strokes in a normalised box, y down

def g_log(d):
    stroke(d, [(0.14, 0.5), (0.86, 0.5)], 22)
    stroke(d, [(0.14, 0.34), (0.14, 0.66)], 4)
    stroke(d, [(0.86, 0.34), (0.86, 0.66)], 4)
    circ(d, (0.86, 0.5), 0.11)


def g_stick(d):
    stroke(d, [(0.16, 0.92), (0.52, 0.38), (0.78, 0.14)])
    stroke(d, [(0.46, 0.46), (0.76, 0.56)])
    stroke(d, [(0.4, 0.58), (0.6, 0.72)])


def g_stone(d):
    poly(d, [(0.2, 0.74), (0.3, 0.34), (0.64, 0.24), (0.86, 0.54), (0.7, 0.84), (0.34, 0.86)])
    stroke(d, [(0.34, 0.46), (0.6, 0.4)], 4)


def g_apple(d):
    circ(d, (0.5, 0.62), 0.3)
    stroke(d, [(0.5, 0.32), (0.54, 0.12)], 4)
    stroke(d, [(0.54, 0.18), (0.76, 0.1)], 4)


def g_axe(d):
    stroke(d, [(0.36, 0.95), (0.6, 0.26)])
    poly(d, [(0.58, 0.26), (0.88, 0.2), (0.92, 0.46), (0.62, 0.5)], filled=True)
    stroke(d, [(0.6, 0.5), (0.5, 0.62)], 4)


def _fist(d, c, r):
    circ(d, c, r, w=6)
    for k in (-0.4, 0.0, 0.4):
        stroke(d, [(c[0] - r * 0.7, c[1] + k * r), (c[0] + r * 0.7, c[1] + k * r)], 3)


def g_unarmed(d):
    _fist(d, (0.32, 0.66), 0.2)
    _fist(d, (0.68, 0.4), 0.2)


def g_katana(d):
    stroke(d, [(0.12, 0.2), (0.26, 0.32), (0.4, 0.48), (0.5, 0.62)], 7)
    stroke(d, [(0.42, 0.56), (0.6, 0.72)], 5)
    stroke(d, [(0.55, 0.68), (0.8, 0.9)], 5)


def g_greatsword(d):
    poly(d, [(0.5, 0.06), (0.4, 0.2), (0.6, 0.2)], filled=True)
    stroke(d, [(0.5, 0.16), (0.5, 0.66)], 9)
    stroke(d, [(0.26, 0.68), (0.74, 0.68)], 6)
    stroke(d, [(0.5, 0.68), (0.5, 0.86)], 5)
    dot(d, (0.5, 0.9), 0.05)


def g_dagger(d):
    poly(d, [(0.5, 0.14), (0.4, 0.3), (0.6, 0.3)], filled=True)
    stroke(d, [(0.5, 0.26), (0.5, 0.54)], 8)
    stroke(d, [(0.34, 0.56), (0.66, 0.56)], 5)
    stroke(d, [(0.5, 0.56), (0.5, 0.74)], 6)
    dot(d, (0.5, 0.78), 0.045)


def _knife(d, o):
    stroke(d, [(o[0], o[1]), (o[0] + 0.16, o[1] + 0.24)], 6)
    stroke(d, [(o[0] - 0.09, o[1] + 0.3), (o[0] + 0.12, o[1] + 0.24)], 4)
    stroke(d, [(o[0] + 0.06, o[1] + 0.2), (o[0] + 0.22, o[1] + 0.36)], 4)


def g_knives(d):
    _knife(d, (0.16, 0.16))
    _knife(d, (0.44, 0.4))


def g_crossbow(d):
    stroke(d, [(0.12, 0.72), (0.88, 0.44)], 7)
    arc(d, (0.5, 0.62), 0.36, 200, 340)
    stroke(d, [(0.2, 0.66), (0.8, 0.4)], 3)


def g_bow(d):
    arc(d, (0.36, 0.5), 0.44, 300, 60)
    stroke(d, [(0.72, 0.16), (0.72, 0.84)], 3)
    stroke(d, [(0.36, 0.32), (0.5, 0.5), (0.36, 0.68)], 4)


def g_axe_battle(d):
    stroke(d, [(0.5, 0.96), (0.5, 0.2)])
    poly(d, [(0.5, 0.24), (0.16, 0.34), (0.2, 0.56), (0.5, 0.52)], filled=True)
    poly(d, [(0.5, 0.24), (0.84, 0.34), (0.8, 0.56), (0.5, 0.52)], filled=True)
    poly(d, [(0.5, 0.06), (0.4, 0.24), (0.6, 0.24)], filled=True)


def g_bascinet(d):
    arc(d, (0.5, 0.56), 0.36, 180, 360)
    stroke(d, [(0.14, 0.56), (0.86, 0.56)])
    poly(d, [(0.34, 0.58), (0.66, 0.58), (0.62, 0.78), (0.38, 0.78)])
    stroke(d, [(0.5, 0.78), (0.5, 0.94)], 4)


def g_cuirass(d):
    poly(d, [(0.3, 0.14), (0.7, 0.14), (0.84, 0.44), (0.66, 0.92), (0.34, 0.92), (0.16, 0.44)])
    stroke(d, [(0.5, 0.18), (0.5, 0.88)], 4)
    arc(d, (0.22, 0.42), 0.16, 90, 270, 4)
    arc(d, (0.78, 0.42), 0.16, 270, 90, 4)


def g_pauldrons(d):
    arc(d, (0.5, 0.58), 0.3, 200, 340)
    arc(d, (0.5, 0.58), 0.38, 200, 340)
    arc(d, (0.5, 0.58), 0.46, 200, 340)


def g_vambraces(d):
    poly(d, [(0.32, 0.12), (0.64, 0.12), (0.74, 0.88), (0.4, 0.88)])
    stroke(d, [(0.34, 0.24), (0.66, 0.24)], 4)
    stroke(d, [(0.38, 0.74), (0.72, 0.74)], 4)


def g_rerebraces(d):
    poly(d, [(0.3, 0.2), (0.68, 0.14), (0.76, 0.7), (0.34, 0.8)])
    stroke(d, [(0.32, 0.36), (0.7, 0.3)], 4)
    stroke(d, [(0.36, 0.6), (0.74, 0.54)], 4)


def g_cuisses(d):
    poly(d, [(0.28, 0.12), (0.68, 0.14), (0.78, 0.9), (0.3, 0.88)])
    stroke(d, [(0.28, 0.28), (0.68, 0.3)], 4)
    stroke(d, [(0.5, 0.3), (0.54, 0.88)], 4)


def g_greaves(d):
    poly(d, [(0.36, 0.14), (0.64, 0.18), (0.62, 0.9), (0.38, 0.88)])
    arc(d, (0.5, 0.28), 0.2, 195, 345, 5)
    stroke(d, [(0.38, 0.66), (0.62, 0.68)], 4)


def g_crescent_edge(d):
    arc(d, (0.44, 0.5), 0.4, 300, 240, 7)
    arc(d, (0.6, 0.5), 0.3, 300, 240, 5)
    stroke(d, [(0.66, 0.16), (0.9, 0.66)], 4)


def g_flash_step(d):
    dot(d, (0.36, 0.2), 0.1)
    stroke(d, [(0.36, 0.3), (0.44, 0.56)])
    stroke(d, [(0.42, 0.4), (0.2, 0.5)])
    stroke(d, [(0.44, 0.56), (0.24, 0.9)])
    stroke(d, [(0.44, 0.56), (0.66, 0.88)])
    stroke(d, [(0.62, 0.42), (0.88, 0.42)], 4)
    poly(d, [(0.88, 0.42), (0.74, 0.34), (0.74, 0.5)], filled=True)


def g_aura_bullet(d):
    circ(d, (0.5, 0.5), 0.18)
    stroke(d, [(0.28, 0.42), (0.06, 0.36)], 4)
    stroke(d, [(0.28, 0.6), (0.06, 0.66)], 4)


def g_earthbreaker(d):
    _fist(d, (0.5, 0.3), 0.18)
    stroke(d, [(0.5, 0.48), (0.5, 0.68)], 5)
    for a in (200, 250, 300, 340):
        t = math.radians(a)
        stroke(d, [(0.5 + 0.2 * math.cos(t), 0.72 + 0.1 * math.sin(t)), (0.5 + 0.36 * math.cos(t), 0.92)], 4)


def g_aura_ward(d):
    poly(d, [(0.5, 0.08), (0.86, 0.26), (0.8, 0.68), (0.5, 0.94), (0.2, 0.68), (0.14, 0.26)])
    circ(d, (0.5, 0.44), 0.14)


def g_veil(d):
    poly(d, [(0.5, 0.06), (0.78, 0.3), (0.82, 0.68), (0.72, 0.94), (0.28, 0.94), (0.18, 0.68), (0.22, 0.3)])
    arc(d, (0.5, 0.46), 0.2, 180, 360)
    stroke(d, [(0.3, 0.5), (0.3, 0.66)])
    stroke(d, [(0.7, 0.5), (0.7, 0.66)])
    stroke(d, [(0.34, 0.8), (0.66, 0.8)], 4)


def g_cinder_bolt(d):
    poly(d, [(0.5, 0.06), (0.7, 0.42), (0.62, 0.74), (0.38, 0.74), (0.3, 0.42)])
    stroke(d, [(0.5, 0.36), (0.42, 0.66)], 4)
    stroke(d, [(0.5, 0.74), (0.5, 0.94)], 4)


def g_frost_bloom(d):
    for i in range(6):
        a = math.radians(i * 60 - 90)
        tip = (0.5 + 0.44 * math.cos(a), 0.5 + 0.44 * math.sin(a))
        stroke(d, [(0.5, 0.5), tip], 4)
        for s in (-1, 1):
            b = a + s * math.radians(35)
            mid = (0.5 + 0.28 * math.cos(a), 0.5 + 0.28 * math.sin(a))
            stroke(d, [mid, (mid[0] + 0.14 * math.cos(b), mid[1] + 0.14 * math.sin(b))], 3)
    dot(d, (0.5, 0.5), 0.06)


def g_storm_lance(d):
    stroke(d, [(0.5, 0.18), (0.5, 0.78)], 6)
    poly(d, [(0.5, 0.04), (0.32, 0.28), (0.44, 0.28), (0.3, 0.5), (0.56, 0.24), (0.42, 0.24), (0.6, 0.06)])
    stroke(d, [(0.34, 0.56), (0.66, 0.56)], 4)
    dot(d, (0.5, 0.84), 0.05)


def g_orbit_motes(d):
    circ(d, (0.5, 0.5), 0.38)
    for i in range(3):
        a = math.radians(i * 120 - 90)
        dot(d, (0.5 + 0.38 * math.cos(a), 0.5 + 0.38 * math.sin(a)), 0.085)


GLYPHS = {
    # bag and weapons, matching tools/ui/names/bag.txt
    "log": g_log, "stick": g_stick, "stone": g_stone, "apple": g_apple, "axe": g_axe,
    "unarmed": g_unarmed, "katana": g_katana, "greatsword": g_greatsword, "dagger": g_dagger,
    "knives": g_knives, "crossbow": g_crossbow, "bow": g_bow, "axe_battle": g_axe_battle,
    "bascinet": g_bascinet, "cuirass": g_cuirass, "pauldrons": g_pauldrons, "vambraces": g_vambraces,
    "rerebraces": g_rerebraces, "cuisses": g_cuisses, "greaves": g_greaves,
    # abilities, matching tools/ui/names/abilities_glyphs.txt
    "crescent_edge": g_crescent_edge, "flash_step": g_flash_step, "aura_bullet": g_aura_bullet,
    "earthbreaker": g_earthbreaker, "aura_ward": g_aura_ward, "veil": g_veil,
    "cinder_bolt": g_cinder_bolt, "frost_bloom": g_frost_bloom, "storm_lance": g_storm_lance,
    "orbit_motes": g_orbit_motes,
}
BAG = ["log", "stick", "stone", "apple", "axe", "unarmed", "katana", "greatsword", "dagger", "knives",
       "crossbow", "bow", "axe_battle", "bascinet", "cuirass", "pauldrons", "vambraces", "rerebraces",
       "cuisses", "greaves"]
ABILITIES = ["crescent_edge", "flash_step", "aura_bullet", "earthbreaker", "aura_ward", "veil",
             "cinder_bolt", "frost_bloom", "storm_lance", "orbit_motes"]


def render(name):
    im = Image.new("RGBA", (CELL * SS, CELL * SS), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    GLYPHS[name](d)
    return im.resize((CELL, CELL), Image.LANCZOS)


def sheet(names, folder, pack, cols=8):
    rows = int(math.ceil(len(names) / cols))
    sh = Image.new("RGBA", (cols * CELL, rows * CELL), (0, 0, 0, 0))
    icons = []
    for i, n in enumerate(names):
        r, c = divmod(i, cols)
        glyph = render(n)
        sh.alpha_composite(glyph, (c * CELL, r * CELL))
        glyph.save(os.path.join(folder, "glyphs", f"{n}.png"))
        icons.append({"name": n, "row": r + 1, "col": c + 1, "empty": False,
                      "uv": [round(c * CELL / (cols * CELL), 6), round(r * CELL / (rows * CELL), 6),
                             round(CELL / (cols * CELL), 6), round(CELL / (rows * CELL), 6)]})
    for i in range(len(names), cols * rows):                      # the intended empty cells
        r, c = divmod(i, cols)
        icons.append({"name": f"-r{r + 1}c{c + 1}", "row": r + 1, "col": c + 1, "empty": True, "uv": None})
    sh.save(os.path.join(folder, "sheet.png"))
    manifest = {
        "sheet": pack,
        "grid": {"cols": cols, "rows": rows, "cell_w": CELL, "cell_h": CELL,
                 "sheet_w": cols * CELL, "sheet_h": rows * CELL},
        "glyph_size": CELL,
        "stroke_px_at_96": STROKE,
        "glyph_box": BOX,
        "uv_origin": "top-left (glTF/Bevy TextureAtlas convention)",
        "generated_by": "tools/ui/glyphs.py",
        "source": "procedural vectors, drawn here; no generator, no licence to record",
        "icons": icons,
    }
    with open(os.path.join(folder, "manifest.json"), "w") as fh:
        json.dump(manifest, fh, indent=2)
    # preview on dark, one row per sheet
    prev = Image.new("RGBA", (cols * CELL, rows * CELL), (11, 24, 22, 255))
    for i, n in enumerate(names):
        r, c = divmod(i, cols)
        tinted = np.asarray(render(n), np.float32)
        tinted[..., :3] = np.array([46, 230, 197], np.float32)
        prev.alpha_composite(Image.fromarray(tinted.astype(np.uint8), "RGBA"), (c * CELL, r * CELL))
    prev.save(os.path.join(folder, "preview.png"))
    return rows, icons


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="assets/ui/icons")
    args = ap.parse_args()
    for pack, names in (("bag", BAG), ("abilities", ABILITIES)):
        folder = os.path.join(args.out, pack)
        os.makedirs(os.path.join(folder, "glyphs"), exist_ok=True)
        rows, icons = sheet(names, folder, pack)
        empties = sum(1 for i in icons if i["empty"])
        print(f"  {pack}: {len(names)} glyphs, sheet {8 * CELL}x{rows * CELL}, {empties} intended empty cells")
    # geometry spot check: nothing may leave the 80 percent box
    bad = []
    for n in GLYPHS:
        a = np.asarray(render(n))[..., 3]
        inset = int(CELL * (1 - BOX) / 2) - 1
        outer = a.copy()
        outer[inset:CELL - inset, inset:CELL - inset] = 0
        if (outer > 8).any():
            bad.append(n)
    print(f"  box check: {len(GLYPHS) - len(bad)}/{len(GLYPHS)} glyphs inside the 80 percent box"
          + (f"; outside: {', '.join(bad)}" if bad else ""))


if __name__ == "__main__":
    main()
