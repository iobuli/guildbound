"""Side-view frame strips of a clip before and after the pass. Numpy and zlib only."""
import struct
import zlib

import numpy as np

from . import body


def write_png(path, rgb):
    h, w, _ = rgb.shape
    raw = b"".join(b"\x00" + rgb[y].astype(np.uint8).tobytes() for y in range(h))

    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


def _enrich(frames, rest):
    return [body.enrich(fr, rest)[0] for fr in frames]


def save_strip(path, before, after, rest, floor, n=8):
    """Two rows (authored, corrected), n frames, +Z to the right and +Y up. Each panel is centred on its pelvis."""
    bseq, aseq = _enrich(before, rest), _enrich(after, rest)
    if len(bseq) < 2:
        return
    idx = [int(round(i * (len(bseq) - 1) / (n - 1))) for i in range(n)]
    pw, ph, margin = 160, 200, 8
    img = np.zeros((ph * 2 + margin, pw * n, 3), np.uint8)
    img[:] = (18, 20, 26)
    for col, i in enumerate(idx):
        for row, (seq, colour) in enumerate(((bseq, (150, 164, 184)), (aseq, (120, 198, 255)))):
            _panel(img, seq[i], floor, col * pw, row * ph + (margin if row else 0), pw, ph, colour)
    write_png(path, img)


def _panel(img, lm, floor, x0, y0, pw, ph, colour):
    if "pelvis" not in lm:
        return
    pel = lm["pelvis"]
    span = 1.15
    def px(z):
        return int(x0 + pw / 2 + (z - float(pel[2])) / span * (pw * 0.86))

    def py(y):
        return int(y0 + ph * 0.88 - (y - floor) / span * (ph * 0.86))

    fy = py(floor)
    img[max(y0, min(y0 + ph - 1, fy)), x0 + 4:x0 + pw - 4] = (70, 120, 90)
    links = (("pelvis", "neck"), ("neck", "head_joint"), ("pelvis", "hip_l"), ("pelvis", "hip_r"),
             ("hip_l", "ankle_l"), ("hip_r", "ankle_r"), ("ankle_l", "ball_l"), ("ankle_r", "ball_r"),
             ("shoulder_l", "elbow_l"), ("elbow_l", "wrist_l"), ("shoulder_r", "elbow_r"), ("elbow_r", "wrist_r"))
    for a, b in links:
        if a in lm and b in lm:
            _line(img, px(lm[a][2]), py(lm[a][1]), px(lm[b][2]), py(lm[b][1]), colour)
    for name, tint in (("ankle_l", (240, 200, 120)), ("ankle_r", (240, 200, 120)),
                       ("ball_l", (240, 160, 90)), ("ball_r", (240, 160, 90)),
                       ("wrist_l", (190, 160, 255)), ("wrist_r", (190, 160, 255))):
        if name in lm:
            _dot(img, px(lm[name][2]), py(lm[name][1]), tint)
    com = body.centre_of_mass(lm)
    _dot(img, px(com[2]), py(com[1]), (255, 220, 80))


def _dot(img, x, y, colour):
    h, w, _ = img.shape
    for dy in range(-2, 3):
        for dx in range(-2, 3):
            if dx * dx + dy * dy > 6:
                continue
            yy, xx = y + dy, x + dx
            if 0 <= yy < h and 0 <= xx < w:
                img[yy, xx] = colour


def _line(img, x0, y0, x1, y1, colour):
    steps = max(abs(x1 - x0), abs(y1 - y0), 1)
    for i in range(steps + 1):
        t = i / steps
        _dot(img, int(x0 + (x1 - x0) * t), int(y0 + (y1 - y0) * t), colour)
