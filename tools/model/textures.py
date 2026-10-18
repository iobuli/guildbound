"""Our own textures for the hero kit, generated from the meshes (numpy + Pillow, seeded, so every
run gives the same images). Nothing here copies pixels from the Quaternius textures; only the
meshes' UV layouts and geometry are used.

- `skin(body)`: base colour and normal map in the body's UV layout, baked from the mesh:
  * cavity / occlusion (how much nearby surface stands above a point's tangent plane) darkens and
    cools creases; convex points (curvature) catch light;
  * flushed zones near the cheeks, nose, ears, lips, knuckles, elbows and knees;
  * low-frequency mottling and fine pores;
  * eye sockets and lids around the eye meshes; lips below the nose tip;
  * the normal map comes from a height field of the same cavity / curvature / pores.
  The result is near-white and neutral: the game multiplies it by the look's skin tone.
- `iris(colour)`: an eye texture (sclera with veins, a fibrous iris, limbal ring, pupil, catchlight)
  laid out where the eye mesh's UVs put the iris.
- `hair()`: strand base colour and normal map (anisotropic streaks), neutral grey for tinting.
- `cloth(kind)`: tileable weave / leather / felt textures with normal maps, and `cloth_uv` gives
  any garment shell UVs (wrapped around the body, or along the arm for sleeves) to tile them.
"""
from __future__ import annotations

import numpy as np
from PIL import Image
from scipy import ndimage, signal
from scipy.spatial import cKDTree


# ------------------------------------------------------------------------------------------
# Noise

def value_noise(size, cells, seed, octaves=4, tile=True):
    """Tileable fractal value noise in 0..1, `size` px square, `cells` cells at the coarsest level."""
    rng = np.random.default_rng(seed)
    out = np.zeros((size, size))
    amp, total = 1.0, 0.0
    for o in range(octaves):
        n = cells * 2 ** o
        grid = rng.random((n, n))
        yy, xx = np.mgrid[0:size, 0:size] * (n / size)
        x0 = np.floor(xx).astype(int); y0 = np.floor(yy).astype(int)
        fx = xx - x0; fy = yy - y0
        fx = fx * fx * (3 - 2 * fx); fy = fy * fy * (3 - 2 * fy)
        x1 = (x0 + 1) % n if tile else np.minimum(x0 + 1, n - 1)
        y1 = (y0 + 1) % n if tile else np.minimum(y0 + 1, n - 1)
        x0 %= n; y0 %= n
        v = (grid[y0, x0] * (1 - fx) + grid[y0, x1] * fx) * (1 - fy) + (grid[y1, x0] * (1 - fx) + grid[y1, x1] * fx) * fy
        out += v * amp
        total += amp
        amp *= 0.5
    return out / total


def normal_from_height(h, strength):
    """Tangent-space normal map (RGB 0..1) from a height field (wraps at the edges)."""
    dx = (np.roll(h, -1, 1) - np.roll(h, 1, 1)) * 0.5 * strength
    dy = (np.roll(h, -1, 0) - np.roll(h, 1, 0)) * 0.5 * strength
    n = np.stack([-dx, dy, np.ones_like(h)], -1)
    n /= np.linalg.norm(n, axis=-1, keepdims=True)
    return n * 0.5 + 0.5


def to_image(a):
    return Image.fromarray((np.clip(a, 0, 1) * 255 + 0.5).astype(np.uint8))


# ------------------------------------------------------------------------------------------
# Baking attributes into UV space

def rasterize(uv, idx, attrs, size):
    """Interpolate per-vertex `attrs` (n, k) over each triangle in UV space. Returns (size, size, k)
    and a coverage mask. V is flipped (glTF UV origin is top-left, like the image)."""
    k = attrs.shape[1]
    out = np.zeros((size, size, k))
    cov = np.zeros((size, size), bool)
    P = uv * size - 0.5
    for t in idx:
        a, b, c = P[t]
        lo = np.floor(np.minimum(np.minimum(a, b), c)).astype(int)
        hi = np.ceil(np.maximum(np.maximum(a, b), c)).astype(int)
        lo = np.clip(lo, 0, size - 1); hi = np.clip(hi, 0, size - 1)
        if hi[0] < lo[0] or hi[1] < lo[1]:
            continue
        xs, ys = np.meshgrid(np.arange(lo[0], hi[0] + 1), np.arange(lo[1], hi[1] + 1))
        p = np.stack([xs.ravel(), ys.ravel()], 1).astype(float)
        v0, v1 = b - a, c - a
        den = v0[0] * v1[1] - v1[0] * v0[1]
        if abs(den) < 1e-12:
            continue
        d = p - a
        w1 = (d[:, 0] * v1[1] - v1[0] * d[:, 1]) / den
        w2 = (v0[0] * d[:, 1] - d[:, 0] * v0[1]) / den
        w0 = 1 - w1 - w2
        inside = (w0 >= -0.02) & (w1 >= -0.02) & (w2 >= -0.02)
        if not inside.any():
            continue
        px = p[inside].astype(int)
        val = w0[inside, None] * attrs[t[0]] + w1[inside, None] * attrs[t[1]] + w2[inside, None] * attrs[t[2]]
        out[px[:, 1], px[:, 0]] = val
        cov[px[:, 1], px[:, 0]] = True
    return out, cov


def dilate(img, cov, steps=8):
    """Grow covered pixels into the gutter so mip-mapping and filtering don't pull in black."""
    img = img.copy(); cov = cov.copy()
    for _ in range(steps):
        grown = ndimage.binary_dilation(cov)
        new = grown & ~cov
        if not new.any():
            break
        acc = np.zeros_like(img); cnt = np.zeros(cov.shape)
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                m = np.roll(np.roll(cov, dy, 0), dx, 1)
                acc += np.roll(np.roll(img, dy, 0), dx, 1) * m[..., None]
                cnt += m
        img[new] = acc[new] / np.maximum(cnt[new], 1)[:, None]
        cov = grown
    return img


# ------------------------------------------------------------------------------------------
# Skin

def _cavity(pos, nrm, radius=0.035):
    """How far nearby surface stands above each point's tangent plane (m, >= 0): creases."""
    tree = cKDTree(pos)
    out = np.zeros(len(pos))
    for i, nb in enumerate(tree.query_ball_point(pos, radius)):
        d = pos[nb] - pos[i]
        h = d @ nrm[i]
        r = np.linalg.norm(d, axis=1)
        out[i] = max(0.0, float(np.max(h * (1 - r / radius))))
    return out


def _curvature(pos, nrm, nb):
    """Signed convexity: + where the point stands out from its neighbours' average."""
    out = np.zeros(len(pos))
    for i, ns in enumerate(nb):
        ns = np.asarray(list(ns), int)
        if len(ns):
            out[i] = (pos[i] - pos[ns].mean(0)) @ nrm[i]
    return out


def skin(body, joints, size=1024, seed=7):
    """Base colour and normal map (PIL images) for `body` (a `build.Body`)."""
    surf = body.surf
    cav = _cavity(surf.pos, surf.nrm)
    curv = _curvature(surf.pos, surf.nrm, body.nb)
    b = body.body
    ids = body.ids  # unwelded -> welded
    pos = b["pos"]
    w = body.wd
    # Flushed zones: distance to a few anatomical points (falloff radius, strength).
    head = joints["Head"]
    face_fwd = np.array([0.0, 0.0, 1.0])
    spots = [
        (head + np.array([0.045, 0.06, 0.085]), 0.035, 0.9),   # cheeks
        (head + np.array([-0.045, 0.06, 0.085]), 0.035, 0.9),
        (head + np.array([0.0, 0.075, 0.12]), 0.025, 1.0),     # nose tip
        (head + np.array([0.085, 0.08, -0.005]), 0.03, 0.8),   # ears
        (head + np.array([-0.085, 0.08, -0.005]), 0.03, 0.8),
    ]
    for s in "lr":
        spots += [(joints[f"lowerarm_{s}"], 0.05, 0.5), (joints[f"calf_{s}"] + face_fwd * 0.05, 0.06, 0.45),
                  (joints[f"hand_{s}"] + np.array([0.08 if s == "l" else -0.08, 0, 0]), 0.06, 0.6)]
    flush = np.zeros(len(pos))
    for c, r, k in spots:
        d = np.linalg.norm(pos - c, axis=1)
        flush = np.maximum(flush, k * np.clip(1 - d / r, 0, 1) ** 1.5)
    # Eye sockets: around the eye meshes' centres.
    eyes = body.eyes["pos"]
    sock = np.zeros(len(pos))
    for side in (eyes[:, 0] > 0, eyes[:, 0] <= 0):
        c = eyes[side].mean(0)
        d = np.linalg.norm(pos - c, axis=1)
        sock = np.maximum(sock, np.clip(1 - d / 0.035, 0, 1))
    # Lips: an ellipsoid just below the nose tip (the head's frontmost point near the midline).
    head_pts = pos[(np.abs(pos[:, 0]) < 0.012) & (pos[:, 1] > head[1]) & (pos[:, 1] < head[1] + 0.12)]
    nose = head_pts[np.argmax(head_pts[:, 2])]
    mouth = nose + np.array([0.0, -0.042, -0.012])
    q = (pos - mouth) / np.array([0.026, 0.011, 0.03])
    lips = np.clip(1.4 - np.linalg.norm(q, axis=1), 0, 1) * (pos[:, 2] > mouth[2] - 0.02)
    attrs = np.stack([cav[ids], curv[ids], flush, sock, lips], 1)
    A, cov = rasterize(b["uv"], b["idx"], attrs, size)
    cav_t, curv_t, flush_t, sock_t, lips_t = (A[..., i] for i in range(5))
    cav_t = cav_t / max(np.percentile(cav_t[cov], 98), 1e-6)
    curv_t = curv_t / max(np.percentile(np.abs(curv_t[cov]), 98), 1e-6)
    mottle = value_noise(size, 6, seed, octaves=4) - 0.5
    pores = value_noise(size, 96, seed + 1, octaves=2) - 0.5
    # Colour: neutral, warm where flushed, cooler and darker in creases, a touch of light on ridges.
    base = np.full((size, size, 3), 0.93)
    warm = np.array([1.0, 0.80, 0.78])
    cool = np.array([0.80, 0.80, 0.90])
    f = np.clip(flush_t, 0, 1)[..., None]
    base = base * (1 - 0.35 * f) + 0.93 * warm * 0.35 * f
    c = np.clip(cav_t, 0, 1)[..., None]
    base = base * (1 - 0.3 * c) + base * cool * 0.3 * c * 0.8
    base *= (1 + 0.05 * np.clip(curv_t, -1, 1))[..., None]
    base *= (1 + 0.06 * mottle)[..., None]
    base *= (1 + 0.025 * pores)[..., None]
    s = np.clip(sock_t, 0, 1)[..., None]
    base = base * (1 - 0.18 * s) + base * np.array([0.92, 0.82, 0.86]) * 0.18 * s
    lp = np.clip(lips_t, 0, 1)[..., None] ** 0.8
    base = base * (1 - 0.55 * lp) + np.array([0.84, 0.50, 0.50]) * 0.55 * lp
    base = dilate(base, cov)
    # Normal map from a height field of the same features.
    h = -0.6 * np.clip(cav_t, 0, 1) + 0.25 * np.clip(curv_t, -1, 1) + 0.08 * pores + 0.15 * mottle
    h = ndimage.gaussian_filter(dilate(h[..., None], cov)[..., 0], 1.0)
    nmap = normal_from_height(h, 6.0)
    return to_image(base), to_image(nmap)


# ------------------------------------------------------------------------------------------
# Eyes

def iris(colour, centre, radius, size=256, seed=3):
    """An eye texture with the iris at `centre` (pixels), `radius` px. `colour` is sRGB 0..1."""
    rng = np.random.default_rng(seed)
    yy, xx = np.mgrid[0:size, 0:size].astype(float)
    dx, dy = xx - centre[0], yy - centre[1]
    r = np.hypot(dx, dy) / radius
    ang = np.arctan2(dy, dx)
    # Sclera: off-white with faint red veins toward the edge.
    img = np.ones((size, size, 3)) * np.array([0.93, 0.91, 0.88])
    veins = np.zeros((size, size))
    for _ in range(14):
        a0 = rng.uniform(-np.pi, np.pi)
        wig = rng.uniform(0.04, 0.12)
        line = np.abs(np.angle(np.exp(1j * (ang - a0 - wig * np.sin(r * 6 + rng.uniform(0, 6))))))
        veins = np.maximum(veins, np.clip(1 - line / 0.012, 0, 1) * np.clip((r - 1.3) / 1.2, 0, 1))
    img = img * (1 - 0.35 * veins[..., None]) + np.array([0.75, 0.25, 0.25]) * 0.35 * veins[..., None]
    img *= (1 - 0.12 * np.clip((r - 1.2) / 2.5, 0, 1))[..., None]
    # Iris: radial fibres, darker collarette and limbal ring.
    c = np.asarray(colour, float)
    fib = value_noise(size, 4, seed, octaves=2)
    spokes = 0.5 + 0.5 * np.sin(ang * 48 + 3 * fib * 6)
    spokes2 = 0.5 + 0.5 * np.sin(ang * 23 + 1.7)
    tone = 0.75 + 0.2 * spokes * (0.4 + 0.6 * r) + 0.1 * spokes2
    tone *= 1 - 0.35 * np.clip(1 - np.abs(r - 0.5) / 0.08, 0, 1)  # collarette
    tone *= 1 - 0.55 * np.clip((r - 0.86) / 0.14, 0, 1)            # limbal ring
    light = np.clip(1.25 - r, 0.6, 1.0)
    iris_col = c[None, None, :] * tone[..., None] * light[..., None]
    inside = (r < 1.0)[..., None]
    img = np.where(inside, iris_col, img)
    # Pupil and catchlight.
    pupil = np.clip((0.36 - r) / 0.04, 0, 1)[..., None]
    img = img * (1 - pupil) + 0.03 * pupil
    hl = np.hypot(dx + radius * 0.32, dy + radius * 0.32) / (radius * 0.16)
    img = img + np.clip(1 - hl, 0, 1)[..., None] * 0.9
    # Soft anti-aliased edge of the iris.
    edge = np.clip((r - 0.97) / 0.06, 0, 1)[..., None]
    img = np.where(inside, img * (1 - edge) + np.array([0.93, 0.91, 0.88]) * edge, img)
    return to_image(img)


def find_iris(texture: Image.Image):
    """Centre (px) and radius (px) of the iris in an eye texture's layout (a saturated dark disc)."""
    a = np.asarray(texture.convert("RGB"), float) / 255
    mx, mn = a.max(2), a.min(2)
    sat = (mx - mn) / np.maximum(mx, 1e-6)
    m = (sat > 0.45) & (mx < 0.7)
    cy, cx = np.argwhere(m).mean(0)
    rad = np.sqrt(m.sum() / np.pi)
    return (cx, cy), rad * 1.15


# ------------------------------------------------------------------------------------------
# Hair

def hair(size=512, seed=11):
    """Neutral strand texture: long streaks along V with clumping, plus its normal map."""
    rng = np.random.default_rng(seed)
    base = np.zeros((size, size))
    yy, xx = np.mgrid[0:size, 0:size].astype(float)
    for k, (count, width, amp) in enumerate([(40, 6.0, 0.5), (140, 2.0, 0.35), (420, 0.8, 0.25)]):
        line = np.zeros(size)
        cols = rng.uniform(0, size, count)
        for c in cols:
            d = np.abs(((np.arange(size) - c + size / 2) % size) - size / 2)
            line += np.exp(-(d / width) ** 2) * rng.uniform(0.4, 1.0)
        wave = value_noise(size, 3, seed + k, octaves=2)
        shift = (wave - 0.5) * 18
        idx = (xx + shift).astype(int) % size
        base += amp * line[idx]
    base = base / base.max()
    v = 0.55 + 0.4 * base
    v *= 1 - 0.18 * (value_noise(size, 5, seed + 9, octaves=3) - 0.5)
    img = np.stack([v * 0.98, v * 0.97, v * 0.95], -1)
    return to_image(img), to_image(normal_from_height(base, 3.0))


# ------------------------------------------------------------------------------------------
# Cloth

CLOTH_KINDS = ("linen", "twill", "wool", "leather", "silk")


def cloth(kind, size=256, seed=21):
    """Tileable base colour (near white, for tinting) and normal map for a cloth kind."""
    yy, xx = np.mgrid[0:size, 0:size].astype(float)
    t = 2 * np.pi / size
    if kind == "linen":
        # Plain weave: alternating warp / weft bumps, slubs in the yarn.
        n = 64
        warp = np.sin(xx * t * n) ** 2
        weft = np.sin(yy * t * n) ** 2
        over = (np.sin(xx * t * n / 2) * np.sin(yy * t * n / 2) > 0)
        h = np.where(over, warp, weft)
        h += 0.35 * (value_noise(size, 8, seed) - 0.5)
        tone = 0.86 + 0.1 * h
    elif kind == "twill":
        n = 48
        diag = np.sin((xx + yy) * t * n) ** 2
        h = diag + 0.25 * np.sin(xx * t * n * 2) ** 2
        h += 0.2 * (value_noise(size, 8, seed) - 0.5)
        tone = 0.84 + 0.12 * h / h.max()
    elif kind == "wool":
        h = value_noise(size, 24, seed, octaves=4)
        h += 0.5 * value_noise(size, 64, seed + 1, octaves=2)
        h /= h.max()
        tone = 0.8 + 0.18 * h
    elif kind == "leather":
        cells = value_noise(size, 16, seed, octaves=3)
        grain = np.abs(value_noise(size, 40, seed + 2, octaves=2) - 0.5) * 2
        h = 0.6 * cells + 0.4 * (1 - grain)
        tone = 0.78 + 0.2 * cells - 0.08 * grain
    elif kind == "silk":
        h = 0.3 * np.sin(xx * t * 96) ** 2 + 0.2 * value_noise(size, 6, seed)
        tone = 0.9 + 0.08 * value_noise(size, 4, seed + 3)
    else:
        raise ValueError(kind)
    tone = tone * (1 + 0.05 * (value_noise(size, 4, seed + 5) - 0.5))
    img = np.stack([tone, tone, tone], -1)
    return to_image(img), to_image(normal_from_height(h, 2.5 if kind != "leather" else 4.0))


def cloth_uv(pos, armness, tile=0.12):
    """UVs that tile a cloth texture every `tile` metres: around the body (u = arc length about
    the vertical axis, v = height), or around and along the arm for sleeves (blended by
    `armness`)."""
    ang = np.arctan2(pos[:, 0], pos[:, 2] + 0.02)
    body_u = ang * 0.17 / tile
    body_v = -pos[:, 1] / tile
    arm_ang = np.arctan2(pos[:, 1] - 1.44, pos[:, 2] + 0.065)
    arm_u = arm_ang * 0.05 / tile
    arm_v = -np.abs(pos[:, 0]) / tile
    a = np.clip((armness - 0.3) / 0.4, 0, 1)
    u = np.where(a > 0.5, arm_u, body_u)
    v = np.where(a > 0.5, arm_v, body_v)
    return np.stack([u, v], 1).astype(np.float32)


# ------------------------------------------------------------------------------------------
# AI source textures (ART_PROMPTS section 1, the T-runs)
#
# A painted swatch off a generator is never tileable as it arrives: it carries a slow lighting
# gradient, and its weave lattice is slightly rotated and drifts in phase, so no crop of whole
# repeats can ever line up. Measure the lattice once from the 2-D FFT, straighten the swatch so
# a whole number of threads fits the tile exactly, then fade the wrap seams. The height map is
# derived from the albedo itself, so the normal's ridges sit under the painted threads.

def _grey(img):
    a = np.asarray(img, np.float32)
    if a.ndim == 3:
        a = a.mean(2)
    return a / 255.0 if a.max() > 1.0 else a


def _rgb(colour):
    """sRGB 0..1 or "#rrggbb" -> (3,) float."""
    if isinstance(colour, str):
        c = colour.lstrip("#")
        return np.array([int(c[i:i + 2], 16) for i in (0, 2, 4)], np.float32) / 255.0
    return np.asarray(colour, np.float32)[:3]


def even_lighting(img, radius=128):
    """Divide out slow lighting / vignetting so a swatch has one flat exposure.

    The generator bakes a soft light gradient into the swatch, which is what makes two opposite
    edges read as different brightness. Dividing by a very wide blur removes the gradient and
    keeps the global mean, leaving the weave untouched. Returns float 0..1.
    """
    a = np.asarray(img, np.float32)
    a = a / 255.0 if a.max() > 1.0 else a
    if a.ndim == 2:
        a = a[..., None]
    blur = np.maximum(np.stack([ndimage.gaussian_filter(a[..., c], radius)
                                for c in range(a.shape[2])], -1), 1e-4)
    gain = blur.mean((0, 1), keepdims=True) / blur
    return np.clip(a * gain, 0, 1)


def _spectrum(img):
    g = _grey(img)
    g = g - g.mean()
    win = np.outer(np.hanning(g.shape[0]), np.hanning(g.shape[1]))
    return np.abs(np.fft.fftshift(np.fft.fft2(g * win))) ** 2


def _top_peaks(P, count=2, guard=2, skip=4):
    """The `count` strongest peaks of a power spectrum, sub-bin refined, keeping `guard` quiet."""
    h, w = P.shape
    cy, cx = h // 2, w // 2
    Y, X = np.ogrid[:h, :w]
    P = P.copy()
    P[np.hypot(Y - cy, X - cx) < skip] = 0.0        # DC and the lighting cross
    out = []
    for _ in range(count):
        iy, ix = np.unravel_index(np.argmax(P), P.shape)
        fy, fx = float(iy - cy), float(ix - cx)

        def sub(a, b, c):
            tot = a + b + c
            return 0.0 if tot <= 0 else 0.5 * (a - c) / tot
        fy += sub(P[iy - 1, ix], P[iy, ix], P[iy + 1, ix])
        fx += sub(P[iy, ix - 1], P[iy, ix], P[iy, ix + 1])
        out.append((fx * 2 * np.pi / w, fy * 2 * np.pi / h))     # radians per px
        for (py0, px0) in ((iy, ix), ((2 * cy - iy) % h, (2 * cx - ix) % w)):
            P[max(0, py0 - guard):py0 + guard + 1, max(0, px0 - guard):px0 + guard + 1] = 0.0
    return out


def lattice(img):
    """Thread lattice of a woven swatch, from the 2-D FFT of its luminance.

    Returns dict(rotation, pitch, nx, ny, peak): `rotation` in radians (how far the weave runs
    off true), `pitch` in px per thread (mean of the axes), `nx`/`ny` threads across the image,
    `peak` the fundamental's power over the spectrum's mean (how well defined the weave is).

    A lattice repeats every 90 degrees, so the skew is the circular mean of the peaks' directions
    modulo a quarter turn, and conjugate halves of one fundamental cannot confuse it.
    """
    P = _spectrum(img)
    f1, f2 = _top_peaks(P, 2)
    q = np.pi / 2
    th = [(np.arctan2(f[1], f[0]) % q) for f in (f1, f2)]
    s = sum(np.sin(4 * t) for t in th)
    c = sum(np.cos(4 * t) for t in th)
    rot = 0.25 * np.arctan2(s, c)                    # in (-45, 45] degrees
    h, w = P.shape
    pitches = [2 * np.pi / max(np.hypot(*f), 1e-9) for f in (f1, f2)]
    # each peak is one fundamental; its wave vector is perpendicular to the thread row
    return dict(rotation=float(rot), pitch=float(np.mean(pitches)),
                nx=float(w / pitches[0]), ny=float(h / pitches[1]),
                peak=float(P.max() / max(P.mean(), 1e-9)))


def _sample_axis(img, axis, pos):
    """Bilinear sample along `axis` at fractional `pos` (wrapped), all other axes kept."""
    n = img.shape[axis]
    i0 = np.floor(pos).astype(int)
    f = pos - i0
    i1 = i0 + 1
    shape = [1] * img.ndim
    shape[axis] = -1
    f = f.reshape(shape)
    return np.take(img, i0 % n, axis=axis) * (1 - f) + np.take(img, i1 % n, axis=axis) * f


def _phase_profile(g, axis, pitch):
    """Unwrapped phase of the thread lattice along one axis, from the bandpassed luminance."""
    prof = g.mean(1 if axis == 0 else 0).astype(np.float64)
    n = len(prof)
    f = np.fft.rfftfreq(n, 1.0)
    F = np.fft.rfft(prof - prof.mean())
    f0 = 1.0 / pitch
    F = np.where(np.abs(f - f0) < 0.25 * f0, F, 0.0)
    band = np.fft.irfft(F, n)
    ph = np.unwrap(np.angle(signal.hilbert(band)))
    if ph[-1] < ph[0]:
        ph = -ph
    return ndimage.gaussian_filter1d(ph - ph[0], 8.0)


def phase_lock(img, n=None, size=None):
    """Resample each axis so the lattice's phase advances exactly `n` whole threads per tile.

    Straightening with a single global scale cannot tile a painted weave: the generator's
    lattice drifts, so the phase at the far edge is a fraction of a thread away from the phase
    at the near edge, and no scale fixes both ends. Locking the phase does: measure the phase
    drift along each axis, then resample so the tile holds exactly `n` cycles, cubic and
    wrapping. `n` defaults to the phase integral itself, which keeps the local stretch near a
    tenth of a percent, so the threads stay as sharp as they arrived while the tile comes out
    periodic by construction rather than papered over by a wide cross-fade.
    """
    a = np.asarray(img, np.float32)
    a = a / 255.0 if a.max() > 1.0 else a
    solo = a.ndim == 2
    if solo:
        a = a[..., None]
    size = size or a.shape[0]
    out = a
    counts = {}
    for axis, label in ((1, "x"), (0, "y")):
        g = _grey(out)
        lat = lattice(g)
        ph = _phase_profile(g, axis, lat["pitch"])
        have = (ph[-1] - ph[0]) / (2 * np.pi)
        n_ax = max(1, int(round(have))) if n is None else int(n)
        target = 2 * np.pi * n_ax * np.arange(size) / float(size)
        pos = np.interp(target, ph, np.arange(len(ph)))
        grid = [np.arange(out.shape[0], dtype=np.float64)[:, None] * np.ones((1, size)),
                np.ones((out.shape[0], 1)) * pos[None, :]] if axis == 1 else \
               [np.ones((1, out.shape[1])) * pos[:, None], np.arange(out.shape[1], dtype=np.float64)[None, :] * np.ones((size, 1))]
        out = np.stack([ndimage.map_coordinates(out[..., c], grid, order=3, mode="wrap")
                        for c in range(out.shape[2])], -1)
        counts[label] = (n_ax, round(float(have), 2))
    out = np.clip(out, 0, 1)
    return (out[..., 0] if solo else out), counts


def straighten(img, n=None, rotation=None, size=1024, order=3, refine=True, phase=True):
    """Rotate, rescale and phase-lock a swatch so exactly `n` threads fill the tile per axis.

    `n` defaults to round(size / lattice(img)["pitch"]). Any lattice skew is removed so the
    tile's threads run true to its axes, and with `phase` the lattice's drift is taken out per
    axis, which is what actually makes the tile periodic. Resampling uses mode="wrap" so the
    field stays continuous, and the affine scale is refined over +-5% for the sharpest lattice
    peak, because a pitch error of a few thousandths smears the threads by the far edge.
    """
    a = np.asarray(img, np.float32)
    a = a / 255.0 if a.max() > 1.0 else a
    if a.ndim == 2:
        a = a[..., None]
    lat = lattice(a)
    pitch = lat["pitch"]
    rot = lat["rotation"] if rotation is None else rotation
    user_n = n is not None
    n = int(round(size / pitch)) if n is None else int(n)
    h, w = a.shape[:2]
    base = (size / n) / pitch

    def warp(scale, order):
        th = -rot
        M = scale * np.array([[np.cos(th), np.sin(th)], [-np.sin(th), np.cos(th)]], np.float64)
        ctr = np.array([(h - 1) / 2.0, (w - 1) / 2.0])
        off = ctr - M @ ctr
        out = np.stack([ndimage.affine_transform(a[..., c], M, offset=off, output_shape=(size, size),
                                                 order=order, mode="wrap") for c in range(a.shape[2])], -1)
        return np.clip(out, 0, 1)

    scale = base
    if refine:
        def crisp(s):
            P = _spectrum(warp(s, 1))
            return float(P.max() / max(P.mean(), 1e-9))
        scale = max(((crisp(s), s) for s in np.linspace(base * 0.95, base * 1.05, 41)),
                    key=lambda t: t[0])[1]
    out = warp(scale, order)
    counts = {}
    if phase:
        out, counts = phase_lock(out, n=n if user_n else None, size=size)
    P = _spectrum(out)
    return (out if out.shape[2] > 1 else out[..., 0]), dict(
        n=n, scale=float(scale), rotation=float(rot), pitch_after=size / n,
        locked={k: (v[0], round(v[1], 2)) for k, v in counts.items()},
        peak=float(P.max() / max(P.mean(), 1e-9)))


def seam_fade(img, band=32):
    """Cosine cross-fade of the wrap seams over `band` px, per channel.

    Straightening should already leave the tile periodic; this mops up whatever slub and
    irregularity survives, over a band short enough to stay invisible on cloth.
    """
    a = np.asarray(img, np.float32)
    a = a / 255.0 if a.max() > 1.0 else a
    o = a.copy()
    h, w = o.shape[:2]
    b = max(1, min(int(band), min(h, w) // 2))
    ramp = 0.5 - 0.5 * np.cos(np.pi * (np.arange(b) + 1) / b)   # 0 at the seam -> 1 inside
    sh = (1,) * (o.ndim - 2)
    wx = ramp.reshape((1, b) + sh)
    wy = ramp.reshape((b, 1) + sh)
    left, right = o[:, :b].copy(), o[:, w - b:][:, ::-1].copy()
    mix = 0.5 * (left + right)
    o[:, :b] = left * wx + mix * (1 - wx)
    o[:, w - b:] = (right * wx + mix * (1 - wx))[:, ::-1]
    top, bot = o[:b].copy(), o[h - b:][::-1].copy()
    mix = 0.5 * (top + bot)
    o[:b] = top * wy + mix * (1 - wy)
    o[h - b:] = (bot * wy + mix * (1 - wy))[::-1]
    return np.clip(o, 0, 1)


def height_from_albedo(img, small=1.0, large=24.0):
    """Greyscale height map (0..1, white = raised) derived from a swatch's own luminance.

    Taking the height from the albedo means the normal's ridges cannot land anywhere except
    under the painted threads: no pitch mismatch, and no second generation to pay for.
    `small` keeps the thread crowns, `large` removes the broad shading between them.
    """
    g = _grey(img)
    h = ndimage.gaussian_filter(g, small) - ndimage.gaussian_filter(g, large)
    h = h - h.min()
    return h / max(float(h.max()), 1e-6)


def tint(albedo, colour):
    """Tint a neutral albedo to a palette colour, matching the target mean per channel.

    NOTE for the cloth/skin slots: `build.py` passes the garment colour as the glTF
    `baseColorFactor` *and* the fabric texture as `baseColorTexture`, so the shader already
    multiplies the two. A shared fabric texture must therefore stay near-white (that is what
    `cloth()` and `skin()` return). Use this for a material whose factor is white, or as the
    baked-tint variant of a swatch.
    """
    a = np.asarray(albedo, np.float32)
    a = a / 255.0 if a.max() > 1.0 else a
    if a.ndim == 2:
        a = a[..., None]
    c = _rgb(colour)
    mean = a.mean((0, 1))
    return np.clip(a * (c / np.maximum(mean, 1e-6)), 0, 1)
