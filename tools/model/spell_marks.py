#!/usr/bin/env python3
"""spell_marks.py - H3: the E13 spell world marks (decals), drawn as vectors, no generator involved.

Four tileable-by-nature alpha decals, white in alpha so the game tints them per element:

  crack.png   radial ground cracks, for Earthbreaker
  scorch.png  a burnt ring, for Cinder Bolt
  frost.png   a rime bloom, for Frost Bloom
  char.png    a lightning scorch fork, for Storm Lance
  crack_n.png a normal map for the crack, derived from its own alpha (the crack is a groove)

Format: 512x512, centred, alpha falling to zero well inside the edge (nothing in the outer 16 px),
RGB white wherever alpha is non-zero. Deterministic: one seed, no RNG state.

    python3 tools/model/spell_marks.py --out assets/textures/marks
"""
from __future__ import annotations

import argparse
import math
import os
import sys

import numpy as np
from PIL import Image, ImageDraw

SIZE = 512
C = (SIZE - 1) / 2.0
SS = 2                 # supersample for anti-aliasing
FALLOFF_R = 228.0      # alpha reaches zero here, well inside the 16 px border at 240
WHITE = (255, 255, 255, 255)


def intensity():
    return Image.new("L", (SIZE * SS, SIZE * SS), 0)


def down(im):
    return im.resize((SIZE, SIZE), Image.LANCZOS)


def fade(arr):
    """Radial falloff so every decal dies out before the canvas edge."""
    yy, xx = np.mgrid[0:SIZE, 0:SIZE].astype(np.float32)
    r = np.hypot(xx - C, yy - C)
    t = np.clip((FALLOFF_R - r) / 60.0, 0, 1)
    return arr * (t * t * (3 - 2 * t))


def mottle(seed, scale=9, amount=0.30):
    """Seeded low-frequency mottle, no dependency on the sim's noise (decals may tile, may not)."""
    rng = np.random.default_rng(seed)
    g = rng.random((scale, scale))
    im = Image.fromarray((g * 255).astype(np.uint8)).resize((SIZE, SIZE), Image.BICUBIC)
    n = np.asarray(im, np.float32) / 255.0
    return 1.0 - amount + amount * 2.0 * n


def draw_crack():
    im = intensity()
    d = ImageDraw.Draw(im)
    rng = np.random.default_rng(11)

    def branch(x, y, ang, length, width, depth):
        steps = max(3, int(length / 9))
        pts, a = [(x, y)], ang
        for i in range(steps):
            a += rng.uniform(-0.20, 0.20)
            x += math.cos(a) * length / steps
            y += math.sin(a) * length / steps
            pts.append((x, y))
        for i in range(len(pts) - 1):
            w = max(1.0, width * (1 - i / len(pts)) ** 0.7)
            d.line([pts[i][0] * SS, pts[i][1] * SS, pts[i + 1][0] * SS, pts[i + 1][1] * SS],
                   fill=int(min(255, 210 * (1 - i / len(pts)) + 45)), width=int(w * SS))
        if depth < 2:
            for k in (0.35, 0.62):
                i = int(len(pts) * k)
                branch(pts[i][0], pts[i][1], a + rng.choice([-1, 1]) * rng.uniform(0.5, 1.0),
                       length * rng.uniform(0.3, 0.5), width * 0.55, depth + 1)

    for i in range(7):                                    # seven fissures, unevenly spaced
        a = i * (2 * math.pi / 7) + rng.uniform(-0.18, 0.18)
        branch(C + math.cos(a) * 6, C + math.sin(a) * 6, a, rng.uniform(150, 205), 9.0, 0)
    a = np.asarray(down(im), np.float32) / 255.0
    return np.clip(fade(a) * mottle(3, 12, 0.35), 0, 1)


def draw_scorch():
    im = intensity()
    d = ImageDraw.Draw(im)
    rng = np.random.default_rng(23)
    for i in range(220):                                  # an irregular soot ring, not a clean circle
        a = i / 220 * 2 * math.pi
        r_in = 58 + 22 * math.sin(a * 3 + 1.2) + rng.uniform(-6, 6)
        r_out = 150 + 26 * math.sin(a * 2 - 0.4) + rng.uniform(-10, 10)
        d.line([ (C + math.cos(a) * r_in) * SS, (C + math.sin(a) * r_in) * SS,
                 (C + math.cos(a) * r_out) * SS, (C + math.sin(a) * r_out) * SS ],
               fill=int(rng.uniform(170, 245)), width=int(rng.uniform(10, 26) * SS))
    for i in range(14):                                   # streaks and sparks thrown outward
        a = rng.uniform(0, 2 * math.pi)
        r0 = rng.uniform(120, 150)
        d.line([(C + math.cos(a) * r0) * SS, (C + math.sin(a) * r0) * SS,
                (C + math.cos(a) * (r0 + rng.uniform(25, 70))) * SS,
                (C + math.sin(a) * (r0 + rng.uniform(25, 70))) * SS],
               fill=int(rng.uniform(90, 160)), width=int(rng.uniform(2, 6) * SS))
    a = np.asarray(down(im), np.float32) / 255.0
    return np.clip(fade(a * mottle(7, 8, 0.4)), 0, 1)


def draw_frost():
    im = intensity()
    d = ImageDraw.Draw(im)
    rng = np.random.default_rng(31)

    def spike(ang, length, width, depth):
        x, y = C, C
        steps = int(length / 6)
        pts, a = [(x, y)], ang
        for i in range(steps):
            a += rng.uniform(-0.05, 0.05)
            x += math.cos(a) * length / steps
            y += math.sin(a) * length / steps
            pts.append((x, y))
        for i in range(len(pts) - 1):
            f = 1 - i / len(pts)
            d.line([pts[i][0] * SS, pts[i][1] * SS, pts[i + 1][0] * SS, pts[i + 1][1] * SS],
                   fill=int(200 * f + 40), width=int(max(1.0, width * f) * SS))
        if depth < 2:
            for k, off in ((0.35, 0.7), (0.62, 0.9)):
                i = int(len(pts) * k)
                for s in (-1, 1):
                    spike2 = (pts[i][0], pts[i][1], a + s * off, length * rng.uniform(0.22, 0.38),
                              width * 0.5, depth + 1)
                    spike_from(spike2, d, rng)

    def spike_from(args, d, rng):
        x, y, ang, length, width, depth = args
        steps = max(3, int(length / 6))
        pts, a = [(x, y)], ang
        for i in range(steps):
            a += rng.uniform(-0.08, 0.08)
            x += math.cos(a) * length / steps
            y += math.sin(a) * length / steps
            pts.append((x, y))
        for i in range(len(pts) - 1):
            f = 1 - i / len(pts)
            d.line([pts[i][0] * SS, pts[i][1] * SS, pts[i + 1][0] * SS, pts[i + 1][1] * SS],
                   fill=int(190 * f + 30), width=int(max(1.0, width * f) * SS))

    for i in range(6):                                    # six-fold rime bloom
        spike(i * math.pi / 3 + rng.uniform(-0.1, 0.1), rng.uniform(170, 205), 7.0, 0)
    for i in range(24):                                   # fine needles between the arms
        a = rng.uniform(0, 2 * math.pi)
        r0 = rng.uniform(20, 90)
        spike_from((C + math.cos(a) * r0, C + math.sin(a) * r0, a,
                    rng.uniform(20, 55), 2.0, 2), d, rng)
    d.ellipse([(C - 16) * SS, (C - 16) * SS, (C + 16) * SS, (C + 16) * SS], fill=225)
    a = np.asarray(down(im), np.float32) / 255.0
    return np.clip(fade(a * mottle(13, 10, 0.3)), 0, 1)


def draw_char():
    im = intensity()
    d = ImageDraw.Draw(im)
    rng = np.random.default_rng(47)

    def fork(x, y, ang, length, width, depth):
        steps = max(4, int(length / 8))
        pts, a = [(x, y)], ang
        for i in range(steps):
            a += rng.uniform(-0.42, 0.42)
            x += math.cos(a) * length / steps
            y += math.sin(a) * length / steps
            pts.append((x, y))
        for i in range(len(pts) - 1):
            f = 1 - i / len(pts)
            d.line([pts[i][0] * SS, pts[i][1] * SS, pts[i + 1][0] * SS, pts[i + 1][1] * SS],
                   fill=int(215 * f + 40), width=int(max(1.0, width * f) * SS))
        if depth < 2:
            for k in (0.45, 0.78):
                i = int(len(pts) * k)
                fork(pts[i][0], pts[i][1], a + rng.choice([-1, 1]) * rng.uniform(0.55, 1.1),
                     length * rng.uniform(0.35, 0.55), width * 0.6, depth + 1)

    fork(C, C, rng.uniform(0, 2 * math.pi), 195, 11.0, 0)
    for i in range(18):                                   # scorch spots around the impact
        a = rng.uniform(0, 2 * math.pi)
        r = rng.uniform(30, 150)
        rr = rng.uniform(4, 14)
        d.ellipse([(C + math.cos(a) * r - rr) * SS, (C + math.sin(a) * r - rr) * SS,
                   (C + math.cos(a) * r + rr) * SS, (C + math.sin(a) * r + rr) * SS],
                  fill=int(rng.uniform(60, 130)))
    a = np.asarray(down(im), np.float32) / 255.0
    return np.clip(fade(a * mottle(19, 11, 0.35)), 0, 1)


def to_rgba(alpha):
    out = np.zeros((SIZE, SIZE, 4), np.uint8)
    out[..., :3] = 255
    out[..., 3] = (np.clip(alpha, 0, 1) * 255).astype(np.uint8)
    return Image.fromarray(out, "RGBA")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="assets/textures/marks")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    marks = {"scorch": draw_scorch(), "frost": draw_frost(), "crack": draw_crack(), "char": draw_char()}
    for name, alpha in marks.items():
        to_rgba(alpha).save(os.path.join(args.out, f"{name}.png"))
    # the crack is a groove, so its normal comes from the inverted alpha
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import textures as tx  # noqa: E402
    n = tx.normal_from_height(1.0 - marks["crack"], 6.0)
    Image.fromarray((np.clip(n, 0, 1) * 255 + 0.5).astype(np.uint8), "RGB").convert("RGBA").save(
        os.path.join(args.out, "crack_n.png"))
    # preview on dark, all four side by side, tinted as the game will tint them
    tints = {"scorch": (255, 200, 87), "frost": (127, 216, 255), "crack": (184, 133, 56), "char": (199, 158, 66)}
    tile = 256
    prev = Image.new("RGBA", (tile * 4, tile), (11, 24, 22, 255))
    for i, (name, alpha) in enumerate(marks.items()):
        a = np.asarray(to_rgba(alpha).resize((tile, tile), Image.LANCZOS), np.float32)
        a[..., :3] = np.array(tints[name], np.float32)[None, None, :]
        prev.alpha_composite(Image.fromarray(a.astype(np.uint8), "RGBA"), (i * tile, 0))
    prev.save(os.path.join(args.out, "preview.png"))
    for name, alpha in marks.items():
        inner = alpha[:16].max(), alpha[-16:].max(), alpha[:, :16].max(), alpha[:, -16:].max()
        print(f"  {name:7s} 512x512 | alpha max {alpha.max() * 255:.0f} | coverage {100 * (alpha > 0.02).mean():5.1f}% "
              f"| border max {max(inner) * 255:.0f}")
    print("  crack_n.png derived from the inverted crack alpha (the crack is a groove)")
    print(f"wrote 5 PNGs + preview.png to {args.out}")


if __name__ == "__main__":
    main()
