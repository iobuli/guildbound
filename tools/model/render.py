"""Tiny CPU rasteriser for inspecting the models (NOT the game renderer).

Draws textured, vertex-coloured triangles with a z-buffer and a banded toon ramp similar to
`assets/shaders/toon.wgsl` (half-Lambert lift, 4 bands, rim). Used by `build.py --preview` and
`verify.py` to look at rest and posed meshes. Pure numpy + Pillow.
"""
from __future__ import annotations

import io

import numpy as np
from PIL import Image


class Surface:
    def __init__(self, pos, nrm, idx, color=(1, 1, 1, 1), uv=None, tex=None, vcol=None):
        self.pos = np.asarray(pos, float)
        self.nrm = np.asarray(nrm, float)
        self.idx = np.asarray(idx, np.int64).reshape(-1, 3)
        self.color = np.asarray(color[:3], float)
        self.uv = None if uv is None else np.asarray(uv, float)
        self.tex = tex  # float array HxWx3 linear-ish (we treat as sRGB-ish display values)
        self.vcol = None if vcol is None else np.asarray(vcol, float)[:, :3]


def load_tex(data: bytes, size=512):
    im = Image.open(io.BytesIO(data)).convert("RGB")
    if im.size[0] > size:
        im = im.resize((size, size), Image.BILINEAR)
    return np.asarray(im, float) / 255.0


def look_at(eye, target, up=(0, 1, 0)):
    eye = np.asarray(eye, float); target = np.asarray(target, float)
    f = target - eye; f /= np.linalg.norm(f)
    r = np.cross(f, up); r /= np.linalg.norm(r)
    u = np.cross(r, f)
    return eye, r, u, f


def render(surfaces, eye, target, size=(512, 768), fov_deg=30.0, light=(0.4, 0.8, 0.5), bg=(0.80, 0.84, 0.86)):
    W, H = size
    eye, r, u, f = look_at(eye, target)
    focal = 0.5 * H / np.tan(np.radians(fov_deg) / 2)
    img = np.ones((H, W, 3)) * np.asarray(bg)
    zbuf = np.full((H, W), np.inf)
    L = np.asarray(light, float); L /= np.linalg.norm(L)
    for s in surfaces:
        rel = s.pos - eye
        cx, cy, cz = rel @ r, rel @ u, rel @ f
        sx = W / 2 + focal * cx / np.maximum(cz, 1e-4)
        sy = H / 2 - focal * cy / np.maximum(cz, 1e-4)
        tri = s.idx
        for t in tri:
            if (cz[t] <= 0.01).any():
                continue
            x0, x1, x2 = sx[t]; y0, y1, y2 = sy[t]
            area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0)
            if abs(area) < 1e-9:
                continue
            xmin = max(int(np.floor(min(x0, x1, x2))), 0); xmax = min(int(np.ceil(max(x0, x1, x2))), W - 1)
            ymin = max(int(np.floor(min(y0, y1, y2))), 0); ymax = min(int(np.ceil(max(y0, y1, y2))), H - 1)
            if xmin > xmax or ymin > ymax:
                continue
            px, py = np.meshgrid(np.arange(xmin, xmax + 1) + 0.5, np.arange(ymin, ymax + 1) + 0.5)
            w0 = ((x1 - px) * (y2 - py) - (x2 - px) * (y1 - py)) / area
            w1 = ((x2 - px) * (y0 - py) - (x0 - px) * (y2 - py)) / area
            w2 = 1 - w0 - w1
            inside = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
            if not inside.any():
                continue
            iz = w0 / cz[t[0]] + w1 / cz[t[1]] + w2 / cz[t[2]]
            z = 1 / iz
            yy = py[inside].astype(int); xx = px[inside].astype(int)
            zz = z[inside]
            closer = zz < zbuf[yy, xx]
            if not closer.any():
                continue
            yy, xx, zz = yy[closer], xx[closer], zz[closer]
            b = np.stack([w0[inside][closer] / cz[t[0]], w1[inside][closer] / cz[t[1]], w2[inside][closer] / cz[t[2]]], 1) * zz[:, None]
            n = b @ s.nrm[t]
            n /= np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-9)
            facing = (n @ -f)
            n[facing < 0] *= -1
            col = np.broadcast_to(s.color, (len(zz), 3)).copy()
            if s.tex is not None and s.uv is not None:
                uv = b @ s.uv[t]
                th, tw = s.tex.shape[:2]
                ui = np.clip((uv[:, 0] % 1.0) * tw, 0, tw - 1).astype(int)
                vi = np.clip((uv[:, 1] % 1.0) * th, 0, th - 1).astype(int)
                col *= s.tex[vi, ui]
            if s.vcol is not None:
                col *= b @ s.vcol[t]
            lam = np.clip(n @ L, -1, 1) * 0.5 + 0.5
            band = np.floor(lam * 4.0) / 4.0
            shade = 0.38 + 0.72 * np.clip(band, 0, 1)
            rim = np.clip(1 - np.clip(n @ -f, 0, 1), 0, 1) ** 3 * 0.25
            img[yy, xx] = np.clip(col * shade[:, None] + rim[:, None], 0, 1)
            zbuf[yy, xx] = zz
    return Image.fromarray((img * 255).astype(np.uint8))


def sheet(images, cols=None):
    cols = cols or len(images)
    rows = (len(images) + cols - 1) // cols
    w, h = images[0].size
    out = Image.new("RGB", (w * cols, h * rows), (255, 255, 255))
    for i, im in enumerate(images):
        out.paste(im, ((i % cols) * w, (i // cols) * h))
    return out
