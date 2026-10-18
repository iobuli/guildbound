"""skin_placeholders.py - draw placeholder layer PNGs for a gbui skin (see gbui layers.rs).

    python3 tools/ui/skin_placeholders.py            # writes assets/ui/skins/ability/*.png

Every layer is NEUTRAL WHITE on transparent (straight alpha); the skin's JSON tints it and sets
its opacity per state. These are stand-ins with the right sizes, orientation and alpha, made to be
replaced one for one by painted art (same file names, same canvas):

  glass.png  128²  rounded dark glass plate (the one layer not white: it is the backdrop), 9-slice 16
  frame.png  128²  1-2 px border + corner brackets, 9-slice 16
  track.png  128²  full ring, radius 52-58 px: the unlit cast/pool track
  wedge.png  128²  ONE 30° ring wedge at 12 o'clock (repeated ×12, rotated by the code)
  tick.png   128²  ONE short tick at 12 o'clock just outside the ring (repeated ×10)
  glow.png   128²  soft radial glow, brightest at the ring
  star.png   128²  small 4-point star in the top-right corner
"""
from __future__ import annotations

import os

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "assets", "ui", "skins", "ability")
N = 128
C = (N - 1) / 2


def save(name, img):
    os.makedirs(OUT, exist_ok=True)
    img.save(os.path.join(OUT, name), optimize=True)


def white(alpha):
    a = np.clip(alpha, 0, 1)
    rgba = np.zeros((N, N, 4), np.uint8)
    rgba[..., :3] = 255
    rgba[..., 3] = (a * 255).astype(np.uint8)
    return Image.fromarray(rgba, "RGBA")


def polar():
    y, x = np.mgrid[0:N, 0:N].astype(float)
    r = np.hypot(x - C, y - C)
    ang = (np.degrees(np.arctan2(x - C, -(y - C))) + 360) % 360  # 0 at 12 o'clock, clockwise
    return r, ang


def band(r, lo, hi, soft=1.0):
    return np.clip((r - lo) / soft + 1, 0, 1) * np.clip((hi - r) / soft + 1, 0, 1)


def main():
    r, ang = polar()
    # glass: dark teal plate with faint scanlines, rounded corners
    g = Image.new("RGBA", (N, N))
    d = ImageDraw.Draw(g)
    d.rounded_rectangle([0, 0, N - 1, N - 1], radius=10, fill=(3, 17, 15, 235))
    a = np.asarray(g).copy()
    a[::3, :, :3] = np.minimum(a[::3, :, :3].astype(int) + 6, 255)
    save("glass.png", Image.fromarray(a, "RGBA"))
    # frame: thin border + corner brackets
    f = Image.new("RGBA", (N, N))
    d = ImageDraw.Draw(f)
    d.rounded_rectangle([1, 1, N - 2, N - 2], radius=8, outline=(255, 255, 255, 120), width=2)
    L = 18
    for (x0, y0, sx, sy) in ((3, 3, 1, 1), (N - 4, 3, -1, 1), (3, N - 4, 1, -1), (N - 4, N - 4, -1, -1)):
        d.line([(x0, y0), (x0 + sx * L, y0)], fill=(255, 255, 255, 255), width=3)
        d.line([(x0, y0), (x0, y0 + sy * L)], fill=(255, 255, 255, 255), width=3)
    save("frame.png", f)
    # ring track, one wedge, one tick
    save("track.png", white(band(r, 51, 57) * 0.9))
    gap = 3.0  # degrees left dark between wedges
    wedge = band(r, 51, 57) * np.clip((15 - gap / 2 - np.minimum(ang, 360 - ang)) / 1.5, 0, 1)
    save("wedge.png", white(wedge))
    tick = band(r, 59, 63) * np.clip((4 - np.minimum(ang, 360 - ang)) / 1.0, 0, 1)
    save("tick.png", white(tick))
    # glow: soft ring of light
    glow = np.exp(-((r - 54) / 9.0) ** 2) * 0.9
    save("glow.png", white(glow))
    # star: 4-point star top-right
    s = Image.new("L", (N, N))
    d = ImageDraw.Draw(s)
    cx, cy, R, q = 104, 22, 14, 4
    d.polygon([(cx, cy - R), (cx + q, cy - q), (cx + R, cy), (cx + q, cy + q), (cx, cy + R), (cx - q, cy + q), (cx - R, cy), (cx - q, cy - q)], fill=255)
    s = s.filter(ImageFilter.GaussianBlur(0.6))
    save("star.png", white(np.asarray(s, float) / 255))
    print("wrote", sorted(os.listdir(OUT)))


if __name__ == "__main__":
    main()
