#!/usr/bin/env python3
"""skins.py - build every HUD skin pack to the layers.rs brief.

    python3 tools/ui/skins.py --out assets/ui/skins

Rules kept for every PNG: RGBA, straight alpha, neutral white RGB with the shape in the alpha
channel (the JSON tints it), exact canvas size, nothing drawn outside it, one piece at 12 o'clock
because the code does the rotating. The only non-white layers are the `glass*` backdrops, which
carry their own dark colour.

Radii note: the ability pack is 128 px, so its half-size is 63.5 px. The brief's outer halo at
r = 64..66 cannot fit in that canvas - it clips at the four axis points. `over.png` therefore
ships as a halo inside the band (r 46..48), and `--wide` builds a 144 px variant of the whole
pack where a true r = 64..66 outer halo does fit, for whoever prefers that.
"""
from __future__ import annotations

import argparse
import json
import math
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

SS = 4
WHITE = (255, 255, 255, 255)
DARK = (14, 30, 28)
BAND_IN, BAND_OUT = 51.0, 57.0

# palette that belongs in the JSON tints, never baked into the white layers
ACCENT, ACCENT2 = "#2EE6C5", "#7CFFB2"
BW_GOLD, MP_VIOLET = "#FFC857", "#B79CFF"
DANGER, WARN, INFO = "#FF5C7A", "#FFC857", "#7FD8FF"


def force_white(img):
    """Keep only the alpha from a blurred white layer: the blur pulls RGB toward the transparent
    neighbours, which would leave a grey halo instead of a tintable white one."""
    a = np.asarray(img.convert("RGBA"), np.float32).copy()
    a[..., :3] = 255.0
    return Image.fromarray(a.astype(np.uint8), "RGBA")


def canvas(size):
    if isinstance(size, int):
        size = (size, size)
    return Image.new("RGBA", (size[0] * SS, size[1] * SS), (0, 0, 0, 0))


def down(im, size):
    if isinstance(size, int):
        size = (size, size)
    return im.resize(size, Image.LANCZOS)


def centre(size):
    if isinstance(size, int):
        size = (size, size)
    return (size[0] - 1) / 2.0, (size[1] - 1) / 2.0


def annulus(size, r_in, r_out, width_deg=None, centre_deg=-90.0, gap_deg=0.0):
    """Polygon for an annulus or one sector. Angles: 0 = +x, y down, so -90 = 12 o'clock."""
    cx, cy = centre(size)
    if width_deg is None:
        width_deg = 360.0
    half = (width_deg - gap_deg) / 2
    steps = max(8, int(abs(width_deg) * 2))
    th = np.linspace(math.radians(centre_deg - half), math.radians(centre_deg + half), steps)
    outer = [(cx + r_out * math.cos(t), cy + r_out * math.sin(t)) for t in th]
    inner = [(cx + r_in * math.cos(t), cy + r_in * math.sin(t)) for t in reversed(th)]
    return [(x * SS, y * SS) for x, y in outer + inner]


def poly(size, pts):
    im = canvas(size)
    ImageDraw.Draw(im).polygon(pts, fill=WHITE)
    return down(im, (size, size) if isinstance(size, int) else size)


def dark_plate(size, radius=10.0, inset=4.0, alpha=184, scanlines=True, vignette=False):
    """The one non-white layer: a dark backdrop plate."""
    im = Image.new("RGBA", (size[0] * SS, size[1] * SS), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    if vignette:
        c = centre(size)
        yy, xx = np.mgrid[0:size[1] * SS, 0:size[0] * SS].astype(np.float32) / SS
        r = np.hypot(xx - c[0], yy - c[1]) / (size[0] / 2.0)
        a = np.clip(0.25 + 0.75 * r ** 2, 0, 1) * alpha
        layer = np.zeros((size[1] * SS, size[0] * SS, 4), np.uint8)
        layer[..., :3] = DARK
        layer[..., 3] = a.astype(np.uint8)
        layer = Image.fromarray(layer, "RGBA")
        mask = Image.new("L", (size[0] * SS, size[1] * SS), 0)
        ImageDraw.Draw(mask).ellipse([0, 0, size[0] * SS - 1, size[1] * SS - 1], fill=255)
        layer.putalpha(Image.fromarray((a * (np.asarray(mask, np.float32) / 255.0)).astype(np.uint8), "L"))
        return down(layer, size)
    d.rounded_rectangle([inset * SS, inset * SS, (size[0] - inset) * SS, (size[1] - inset) * SS],
                        radius=radius * SS, fill=DARK + (alpha,))
    if scanlines:
        px = im.load()
        for y in range(size[1] * SS):
            if (y // SS) % 3 == 0:
                for x in range(size[0] * SS):
                    r0, g0, b0, a0 = px[x, y]
                    if a0:
                        px[x, y] = (r0, g0, b0, int(min(255, a0 * 1.08)))
    return down(im, size)


def gradient_bar(size, top=210, mid=255, bot=196, radius=3.0):
    w, h = size
    yy = np.mgrid[0:h * SS, 0:w * SS][0].astype(np.float32) / (h * SS)
    a = np.where(yy < 0.5, top + (mid - top) * (yy / 0.5), mid + (bot - mid) * ((yy - 0.5) / 0.5))
    layer = np.zeros((h * SS, w * SS, 4), np.uint8)
    layer[..., :3] = 255
    layer[..., 3] = np.clip(a, 0, 255).astype(np.uint8)
    im = Image.fromarray(layer, "RGBA")
    mask = canvas(size)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, w * SS - 1, h * SS - 1], radius=radius * SS, fill=WHITE)
    im.putalpha(Image.fromarray((np.asarray(im)[..., 3].astype(np.float32) *
                                 (np.asarray(mask)[..., 3].astype(np.float32) / 255.0)).astype(np.uint8), "L"))
    return down(im, size)


def rounded(size, radius=3.0, width=1.0, fill=None, inset=0.0):
    im = canvas(size)
    d = ImageDraw.Draw(im)
    box = [inset * SS, inset * SS, (size[0] - 1 - inset) * SS, (size[1] - 1 - inset) * SS]
    if fill is not None:
        d.rounded_rectangle(box, radius=radius * SS, fill=fill)
    else:
        d.rounded_rectangle(box, radius=radius * SS, outline=WHITE, width=int(width * SS))
    return down(im, size)


def blurred_ring(size, r, width, blur=1.5):
    im = canvas(size)
    ImageDraw.Draw(im).ellipse(annulus_box(size, r - width / 2, r + width / 2), fill=WHITE)
    return down(im.filter(ImageFilter.GaussianBlur(blur * SS)), size)


def annulus_box(size, r_in, r_out):
    c = centre(size)
    b = [c[0] * SS]
    return [c[0] * SS - r_out * SS, c[1] * SS - r_out * SS, c[0] * SS + r_out * SS, c[1] * SS + r_out * SS]


# ---------------------------------------------------------------------------------------
# packs

def pack_ability(out, wide=False):
    """Q/E ring at 128 px, or at 144 px where the brief's true outer halo (r 64..66) fits."""
    B = 0.0
    s = 144 if wide else 128
    made = {}
    made["glass.png"] = dark_plate((s, s), radius=10, inset=4, alpha=184)
    im = canvas((s, s))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([4 * SS, 4 * SS, (s - 4) * SS, (s - 4) * SS], radius=10 * SS, outline=WHITE, width=1 * SS)
    arm, inset = 9, 16
    for x0, y0, sx, sy in ((4, 4, 1, 1), (s - 4 - inset, 4, 1, 1), (4, s - 4 - inset, 1, 1),
                           (s - 4 - inset, s - 4 - inset, 1, 1)):
        d.line([x0 * SS, y0 * SS, (x0 + sx * arm) * SS, y0 * SS], fill=WHITE, width=2 * SS)
        d.line([x0 * SS, y0 * SS, x0 * SS, (y0 + sy * arm) * SS], fill=WHITE, width=2 * SS)
    made["frame.png"] = down(im, (s, s))
    made["track.png"] = poly(s, annulus(s, BAND_IN + B, BAND_OUT + B))
    made["wedge.png"] = poly(s, annulus(s, BAND_IN + B, BAND_OUT + B, width_deg=30.0, gap_deg=3.0))
    made["wedge_ccw.png"] = made["wedge.png"].transpose(Image.FLIP_LEFT_RIGHT)
    made["tick.png"] = poly(s, annulus(s, 59 + B, 63 + B, width_deg=8.0))
    c = centre(s)
    yy, xx = np.mgrid[0:s, 0:s].astype(np.float32)
    r = np.hypot(xx - c[0], yy - c[1])
    g = np.zeros((s, s, 4), np.uint8)
    g[..., :3] = 255
    edge = np.clip((c[0] - 0.5 - r) / 7.5, 0, 1)   # fade out fully before the canvas edge
    g[..., 3] = (np.clip(np.exp(-((r - (54 + B)) / 13.0) ** 2) * 0.9 * edge, 0, 1) * 255).astype(np.uint8)
    made["glow.png"] = force_white(Image.fromarray(g, "RGBA").filter(ImageFilter.GaussianBlur(1.2)))
    im = canvas((s, s))
    d = ImageDraw.Draw(im)
    cx, cy, R, rin = 104 + B, 22 + B, 14.0, 4.6
    pts = []
    for i in range(8):
        ang = math.radians(i * 45 - 90)
        rad = R if i % 2 == 0 else rin
        pts.append(((cx + rad * math.cos(ang)) * SS, (cy + rad * math.sin(ang)) * SS))
    d.polygon(pts, fill=WHITE)
    made["star.png"] = down(im, (s, s))
    # over-mana halo: r 64..66 only fits a bigger canvas, so at 128 px it sits inside the band
    made["over.png"] = poly(s, annulus(s, *( (64, 66) if wide else (46, 48) )))
    for n, img in made.items():
        img.save(os.path.join(out, n))
    return made, (s, s)


def pack_slot(out):
    s = 128
    made = {}
    made["glass.png"] = dark_plate((s, s), radius=10, inset=4, alpha=184)
    im = canvas((s, s))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([4 * SS, 4 * SS, (s - 4) * SS, (s - 4) * SS], radius=10 * SS, outline=WHITE, width=1 * SS)
    for x0, y0, sx, sy in ((4, 4, 1, 1), (s - 4 - 16, 4, 1, 1), (4, s - 4 - 16, 1, 1), (s - 4 - 16, s - 4 - 16, 1, 1)):
        d.line([x0 * SS, y0 * SS, (x0 + sx * 9) * SS, y0 * SS], fill=WHITE, width=2 * SS)
        d.line([x0 * SS, y0 * SS, x0 * SS, (y0 + sy * 9) * SS], fill=WHITE, width=2 * SS)
    made["frame.png"] = down(im, (s, s))
    # check: the selected-state inner edge glow
    im = canvas((s, s))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([9 * SS, 9 * SS, (s - 9) * SS, (s - 9) * SS], radius=7 * SS, outline=WHITE, width=2 * SS)
    made["check.png"] = force_white(down(im.filter(ImageFilter.GaussianBlur(1.4 * SS)), (s, s)))
    # coin: the gold count mark, 24 px at (100, 104)
    im = canvas((s, s))
    d = ImageDraw.Draw(im)
    cx, cy, r = 110.0, 114.0, 10.0
    d.ellipse([(cx - r) * SS, (cy - r) * SS, (cx + r) * SS, (cy + r) * SS], outline=WHITE, width=2 * SS)
    d.ellipse([(cx - 4.5) * SS, (cy - 4.5) * SS, (cx + 4.5) * SS, (cy + 4.5) * SS], fill=WHITE)
    made["coin.png"] = down(im, (s, s))
    for n, img in made.items():
        img.save(os.path.join(out, n))
    return made, (s, s)


def pack_vital(out):
    size = (256, 24)
    made = {}
    made["track.png"] = rounded(size, radius=3.0, width=1.5)
    made["fill.png"] = gradient_bar(size, top=206, mid=255, bot=192)
    made["trail.png"] = rounded(size, radius=3.0, fill=(255, 255, 255, 170))
    im = canvas(size)
    d = ImageDraw.Draw(im)
    for i in range(10):
        x = (i + 1) * (256 / 11.0)
        d.line([x * SS, 3 * SS, x * SS, 21 * SS], fill=WHITE, width=1 * SS)
    made["ticks.png"] = down(im, size)
    cap = (24, 24)
    im = canvas(cap)
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([9 * SS, 2 * SS, 15 * SS, 22 * SS], radius=2 * SS, fill=WHITE)
    made["cap.png"] = force_white(down(im.filter(ImageFilter.GaussianBlur(0.8 * SS)), cap))
    for n, img in made.items():
        img.save(os.path.join(out, n))
    return made, size


def pack_panel(out):
    s = 256
    made = {}
    made["glass.png"] = dark_plate((s, s), radius=6, inset=3, alpha=150)
    im = canvas((s, s))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([3 * SS, 3 * SS, (s - 3) * SS, (s - 3) * SS], radius=6 * SS, outline=WHITE, width=1 * SS)
    made["frame.png"] = down(im, (s, s))
    b = 48
    im = canvas((b, b))
    d = ImageDraw.Draw(im)
    d.line([0, 2 * SS, 14 * SS, 2 * SS], fill=WHITE, width=2 * SS)
    d.line([2 * SS, 0, 2 * SS, 14 * SS], fill=WHITE, width=2 * SS)
    made["bracket.png"] = down(im, (b, b))
    rule = (256, 8)
    im = canvas(rule)
    d = ImageDraw.Draw(im)
    d.line([0, 3.5 * SS, 256 * SS, 3.5 * SS], fill=WHITE, width=1 * SS)
    arr = np.asarray(down(im, rule), np.float32)
    arr[..., 3] *= np.linspace(1.0, 0.35, rule[0])[None, :]
    made["title_rule.png"] = Image.fromarray(arr.astype(np.uint8), "RGBA")
    ts = (96, 28)
    made["tab.png"] = rounded(ts, radius=3.0, width=1.5, fill=None)
    for n, img in made.items():
        img.save(os.path.join(out, n))
    return made, (s, s)


def pack_map(out):
    s = 256
    made = {}
    made["glass.png"] = dark_plate((s, s), vignette=True, alpha=190)
    made["bezel.png"] = poly(s, annulus(s, 112, 124))
    made["ticks.png"] = poly(s, annulus(s, 104, 112, width_deg=3.0))
    im = canvas((s, s))
    d = ImageDraw.Draw(im)
    c = centre(s)
    top = c[1] - 124.5     # on the bezel band: outside it there is no room in a 256 px canvas
    w, h = 6.0, 13.0
    d.line([c[0] * SS - w * SS, top * SS, c[0] * SS - w * SS, (top + h) * SS], fill=WHITE, width=2 * SS)
    d.line([c[0] * SS - w * SS, top * SS, c[0] * SS + w * SS, (top + h) * SS], fill=WHITE, width=2 * SS)
    d.line([c[0] * SS + w * SS, top * SS, c[0] * SS + w * SS, (top + h) * SS], fill=WHITE, width=2 * SS)
    made["north.png"] = down(im, (s, s))
    for n, img in made.items():
        img.save(os.path.join(out, n))
    return made, (s, s)


def pack_reticle(out):
    s = 128
    made = {}
    made["ring.png"] = poly(s, annulus(s, 39, 41))
    im = canvas((s, s))
    d = ImageDraw.Draw(im)
    c = centre(s)
    d.ellipse([(c[0] - 2.5) * SS, (c[1] - 2.5) * SS, (c[0] + 2.5) * SS, (c[1] + 2.5) * SS], fill=WHITE)
    made["dot.png"] = down(im, (s, s))
    made["tick.png"] = poly(s, annulus(s, 42, 50, width_deg=6.0))
    rs = 256
    im = canvas((rs, rs))
    d = ImageDraw.Draw(im)
    c2 = centre(rs)
    for i in range(12):
        a0 = math.radians(i * 30 - 90)
        a1 = math.radians(i * 30 - 90 + 15)
        pts = [(c2[0] + rr * math.cos(t), c2[1] + rr * math.sin(t)) for rr in (110, 116) for t in ((a0, a1) if rr == 110 else (a1, a0))]
        d.polygon([(x * SS, y * SS) for x, y in pts], fill=WHITE)
    made["range.png"] = down(im, (rs, rs))
    im = canvas((s, s))
    d = ImageDraw.Draw(im)
    c3 = centre(s)
    d.ellipse([(c3[0] - 28) * SS, (c3[1] - 28) * SS, (c3[0] + 28) * SS, (c3[1] + 28) * SS], outline=WHITE, width=1 * SS)
    d.polygon([((c3[0] - 7) * SS, (c3[1] + 6) * SS), ((c3[0] + 7) * SS, (c3[1] + 6) * SS), (c3[0] * SS, (c3[1] + 16) * SS)], fill=WHITE)
    arr = np.asarray(down(im, (s, s)), np.float32)
    made["land.png"] = Image.fromarray(arr.astype(np.uint8), "RGBA")
    for n, img in made.items():
        img.save(os.path.join(out, n))
    return made, (s, s)


def pack_prompt(out):
    chip = (128, 32)
    made = {"chip.png": rounded(chip, radius=5.0, width=1.5)}
    key = (32, 32)
    im = canvas(key)
    d = ImageDraw.Draw(im)
    d.rounded_rectangle([3 * SS, 3 * SS, 28 * SS, 28 * SS], radius=3 * SS, outline=WHITE, width=int(1.5 * SS))
    d.rounded_rectangle([8 * SS, 8 * SS, 23 * SS, 23 * SS], radius=2 * SS, fill=(255, 255, 255, 60))
    made["key.png"] = down(im, key)
    for n, img in made.items():
        img.save(os.path.join(out, n))
    return made, chip


# ---------------------------------------------------------------------------------------
# skins (the opacity numbers: the look)

def A(**kw):
    """Alpha table in the order that makes later states win."""
    order = ["idle", "hovered", "pressed", "checked", "disabled", "casting", "dim", "mastered",
             "over", "low", "open", "aiming", "in-range", "out-of-range"]
    return {k: kw[k] for k in order if k in kw}


def skin_ability(tint):
    return {"dir": "ability", "layers": [
        {"file": "glass.png", "slice": 16, "alpha": A(idle=0.55, hovered=0.65, pressed=0.70, checked=0.62,
                                                      disabled=0.32, casting=0.60, dim=0.45, mastered=0.60, open=0.6)},
        {"file": "glow.png", "tint": ACCENT, "pulse": 1.6,
         "alpha": A(idle=0.0, hovered=0.22, pressed=0.34, checked=0.26, disabled=0.0, casting=0.50,
                    dim=0.0, mastered=0.42)},
        {"file": "frame.png", "slice": 16, "tint": ACCENT,
         "alpha": A(idle=0.55, hovered=0.80, pressed=0.86, checked=0.90, disabled=0.20, casting=1.0,
                    dim=0.30, mastered=0.90)},
        {"file": "track.png", "tint": ACCENT,
         "alpha": A(idle=0.34, hovered=0.38, pressed=0.40, checked=0.38, disabled=0.14, casting=0.40,
                    dim=0.20, mastered=0.38)},
        {"file": "wedge.png", "segments": 12, "drive": "progress", "off": 0.14, "tint": tint,
         "alpha": A(idle=0.60, hovered=0.72, pressed=0.80, checked=0.74, disabled=0.25, casting=1.0,
                    dim=0.50, mastered=0.80)},
        {"file": "tick.png", "segments": 10, "drive": "cover", "off": 0.12, "tint": tint,
         "alpha": A(idle=0.0, hovered=0.0, pressed=0.0, checked=0.0, disabled=0.0, casting=0.0,
                    dim=0.90, mastered=0.90)},
        {"file": "over.png", "tint": MP_VIOLET,
         "alpha": A(idle=0.0, hovered=0.0, pressed=0.0, checked=0.0, disabled=0.0, casting=0.0,
                    dim=0.0, mastered=0.0, over=0.85)},
        {"file": "star.png", "top": True, "tint": tint,
         "alpha": A(idle=0.0, hovered=0.0, pressed=0.0, checked=0.0, disabled=0.0, casting=0.0,
                    dim=0.0, mastered=1.0)},
    ]}


def skin_slot(with_coin=False):
    layers = [
        {"file": "glass.png", "slice": 16, "alpha": A(idle=0.55, hovered=0.65, pressed=0.70, checked=0.62,
                                                      disabled=0.32, open=0.6)},
        {"file": "frame.png", "slice": 16, "tint": ACCENT,
         "alpha": A(idle=0.55, hovered=0.80, pressed=0.86, checked=0.90, disabled=0.20)},
        {"file": "check.png", "tint": ACCENT2, "pulse": 2.4,
         "alpha": A(idle=0.0, hovered=0.0, pressed=0.0, checked=0.85, disabled=0.0)},
    ]
    if with_coin:
        layers.append({"file": "coin.png", "top": True, "tint": WARN,
                       "alpha": A(idle=0.9, hovered=0.9, pressed=0.9, checked=0.9, disabled=0.4)})
    return {"dir": "slot", "layers": layers}


def skin_vital(tint, low_tint=DANGER):
    return {"dir": "vital", "layers": [
        {"file": "track.png", "slice": 6, "tint": ACCENT,
         "alpha": A(idle=0.40, hovered=0.40, disabled=0.2, low=0.55)},
        {"file": "fill.png", "slice": 6, "drive": "fill", "tint": tint,
         "alpha": A(idle=1.0, hovered=1.0, disabled=0.35, low=1.0)},
        {"file": "fill.png", "slice": 6, "drive": "fill", "tint": low_tint,
         "alpha": A(idle=0.0, hovered=0.0, disabled=0.0, low=0.75)},
        {"file": "trail.png", "slice": 6, "drive": "trail", "tint": WARN,
         "alpha": A(idle=0.85, hovered=0.85, disabled=0.3, low=0.0)},
        {"file": "ticks.png", "segments": 10, "off": 0.18, "tint": "#03110F",
         "alpha": A(idle=0.55, hovered=0.55, disabled=0.25, low=0.55)},
        {"file": "cap.png", "tint": ACCENT2, "pulse": 2.0,
         "alpha": A(idle=0.55, hovered=0.75, disabled=0.2, low=1.0)},
    ]}


def skin_panel():
    return {"dir": "panel", "layers": [
        {"file": "glass.png", "slice": 24, "alpha": A(idle=0.92, hovered=0.92, open=0.94, disabled=0.5)},
        {"file": "frame.png", "slice": 24, "tint": ACCENT,
         "alpha": A(idle=0.55, hovered=0.75, open=0.7, disabled=0.2)},
        {"file": "bracket.png", "tint": ACCENT,
         "alpha": A(idle=0.9, hovered=0.95, open=0.95, disabled=0.3)},
        {"file": "title_rule.png", "slice": 4, "tint": ACCENT2,
         "alpha": A(idle=0.5, hovered=0.5, open=0.6, disabled=0.2)},
    ]}


def skin_tab():
    return {"dir": "panel", "layers": [
        {"file": "tab.png", "slice": 8, "tint": ACCENT,
         "alpha": A(idle=0.45, hovered=0.7, checked=0.95, disabled=0.2)},
    ]}


def skin_map():
    return {"dir": "map", "layers": [
        {"file": "glass.png", "alpha": A(idle=0.85, hovered=0.85, open=0.9, disabled=0.4)},
        {"file": "bezel.png", "tint": ACCENT,
         "alpha": A(idle=0.85, hovered=0.85, open=0.9, disabled=0.3)},
        {"file": "ticks.png", "segments": 36, "off": 0.35, "tint": ACCENT2,
         "alpha": A(idle=0.55, hovered=0.55, open=0.6, disabled=0.2)},
        {"file": "north.png", "tint": WARN,
         "alpha": A(idle=1.0, hovered=1.0, open=1.0, disabled=0.4)},
    ]}


def skin_reticle():
    return {"dir": "reticle", "layers": [
        {"file": "range.png", "tint": ACCENT,
         "alpha": A(idle=0.0, aiming=0.35, **{"in-range": 0.5, "out-of-range": 0.5})},
        {"file": "ring.png", "tint": ACCENT,
         "alpha": A(idle=0.0, aiming=0.9, **{"in-range": 0.95, "out-of-range": 0.95})},
        {"file": "tick.png", "segments": 4, "off": 0.2, "tint": ACCENT2,
         "alpha": A(idle=0.0, aiming=0.8, **{"in-range": 0.9, "out-of-range": 0.9})},
        {"file": "dot.png", "tint": ACCENT2,
         "alpha": A(idle=0.0, aiming=1.0, **{"in-range": 1.0, "out-of-range": 1.0})},
        {"file": "land.png", "tint": INFO,
         "alpha": A(idle=0.0, aiming=0.0, **{"in-range": 0.75, "out-of-range": 0.45})},
    ]}


def skin_prompt():
    return {"dir": "prompt", "layers": [
        {"file": "chip.png", "slice": 10, "alpha": A(idle=0.85, hovered=0.95, disabled=0.4)},
    ]}


# ---------------------------------------------------------------------------------------
# per-pack previews: assemble the way layers.rs draws, on dark

def tint(img, hexc):
    if not hexc:
        return img
    a = np.asarray(img, np.float32) / 255.0
    c = np.array([int(hexc[i:i + 2], 16) for i in (1, 3, 5)], np.float32) / 255.0
    a[..., :3] = c[None, None, :]
    return Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8), "RGBA")


def preview(pack_dir, sk, size, states, path):
    """Composite the pack for each state, side by side, honouring segments + rotation."""
    W = size[0] if not isinstance(size, int) else size
    H = size[1] if not isinstance(size, int) else size
    shots = []
    for st in states:
        out = Image.new("RGBA", (W, H), (11, 24, 22, 255))
        for layer in sk["layers"]:
            img = Image.open(os.path.join(pack_dir, layer["file"])).convert("RGBA")
            al = (layer.get("alpha") or {}).get(st, 0.0)
            n = layer.get("segments")
            if n:
                lit = int(round(n * (0.42 if st == "casting" else (0.0 if st == "dim" else n))))
                for i in range(n):
                    a = al if i < lit else layer.get("off", 0.2) * al
                    if a <= 0:
                        continue
                    t = tint(img, layer.get("tint"))
                    arr = np.asarray(t, np.float32)
                    arr[..., 3] *= a
                    t = Image.fromarray(arr.astype(np.uint8), "RGBA")
                    t = t.resize((W, H), Image.LANCZOS).rotate(-i * (360.0 / n), resample=Image.BICUBIC,
                                                                center=((W - 1) / 2, (H - 1) / 2))
                    out.alpha_composite(t)
                continue
            if al <= 0:
                continue
            t = tint(img, layer.get("tint"))
            if t.size != (W, H):
                t = t.resize((W, H), Image.LANCZOS)
            if al < 1.0:
                arr = np.asarray(t, np.float32)
                arr[..., 3] *= al
                t = Image.fromarray(arr.astype(np.uint8), "RGBA")
            out.alpha_composite(t)
        shots.append(out)
    strip = Image.new("RGBA", (W * len(shots), H), (11, 24, 22, 255))
    for i, s in enumerate(shots):
        strip.alpha_composite(s, (i * W, 0))
    strip.save(path)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="assets/ui/skins")
    ap.add_argument("--wide", action="store_true", help="also build a 144 px ability pack where the outer halo fits")
    args = ap.parse_args()
    out = args.out
    built = {}

    def run(pack, fn, *a):
        d = os.path.join(out, pack)
        os.makedirs(d, exist_ok=True)
        made, size = fn(d, *a)
        built[pack] = (d, size, made)
        return d, size, made

    run("ability", pack_ability)
    run("slot", pack_slot)
    run("vital", pack_vital)
    run("panel", pack_panel)
    run("map", pack_map)
    run("reticle", pack_reticle)
    run("prompt", pack_prompt)

    skins = {"ability_bw": skin_ability(BW_GOLD), "ability_mp": skin_ability(MP_VIOLET),
             "weapon": skin_slot(False), "bag": skin_slot(True),
             "vital_hp": skin_vital(ACCENT2), "vital_st": skin_vital(ACCENT),
             "vital_bw": skin_vital(BW_GOLD), "vital_mp": skin_vital(MP_VIOLET),
             "panel": skin_panel(), "tab": skin_tab(), "map": skin_map(),
             "reticle": skin_reticle(), "prompt": skin_prompt()}
    for name, sk in skins.items():
        d = os.path.join(out, name)
        os.makedirs(d, exist_ok=True)
        with open(os.path.join(d, "skin.json"), "w") as fh:
            json.dump(sk, fh, indent=2)

    preview(built["ability"][0], skins["ability_bw"], 128, ["idle", "casting", "dim", "mastered"], os.path.join(out, "ability", "preview.png"))
    preview(built["slot"][0], skins["weapon"], 128, ["idle", "hovered", "checked"], os.path.join(out, "slot", "preview.png"))
    preview(built["vital"][0], skins["vital_hp"], (256, 24), ["idle", "low", "disabled"], os.path.join(out, "vital", "preview.png"))
    preview(built["panel"][0], skins["panel"], (256, 256), ["idle", "open"], os.path.join(out, "panel", "preview.png"))
    preview(built["map"][0], skins["map"], 256, ["idle", "open"], os.path.join(out, "map", "preview.png"))
    preview(built["reticle"][0], skins["reticle"], 128, ["aiming", "in-range"], os.path.join(out, "reticle", "preview.png"))
    preview(built["prompt"][0], skins["prompt"], (128, 32), ["idle", "hovered"], os.path.join(out, "prompt", "preview.png"))

    if args.wide:
        run("ability_wide", pack_ability, True)

    for pack, (d, size, made) in sorted(built.items()):
        print(f"  {pack + '/':10s} {len(made)} images, canvas "
              f"{size[0] if isinstance(size, int) else size[0]}x{size[1] if isinstance(size, int) else size[1]}")
    print(f"{len(skins)} skins written: " + ", ".join(sorted(skins)))


if __name__ == "__main__":
    main()
