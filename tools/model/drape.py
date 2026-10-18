"""Hanging cloth: skirts, robe skirts, kilts, aprons, cloaks and tabard panels.

Body-surface shells (`surface.shell`) cannot bridge the gap between the legs or hang below the
body, so these garments are built as a ring grid around the body's vertical axis instead:

- For each ring height, the radius along each direction is where a ray from the axis leaves the
  2D convex hull of the body's cross-section in a thin band around that height (arms excluded),
  plus a clearance. Below the widest ring the cloth falls straight (a ring is never narrower than
  the one above, less `taper`), and `flare` widens the hem.
- Skin weights: the top of the garment follows the nearest body surface (spine / pelvis); lower
  down each vertex blends toward the leg on its side (thigh, then calf below the knee), with front
  and back centre lines shared between both legs, so the cloth swings with the stride without
  tearing between the legs.
- Morph targets: the nearest body vertex's body-shape deltas, so `bulk` / `heavy` widen it too.

`drape_surf` returns a `surface.Surf`, which `build.make_garment` turns into a shell with thickness
like every other garment.
"""
from __future__ import annotations

import numpy as np
from scipy.spatial import ConvexHull, cKDTree

import surface as sf

# The body's vertical axis (x = 0, between the hips).
AXIS_Z = -0.02


def _hull_radius(points2d: np.ndarray, angles: np.ndarray) -> np.ndarray:
    """Distance from the origin to the convex hull of `points2d` (x, z) along each angle
    (0 = +Z, increasing toward +X)."""
    if len(points2d) < 3:
        return np.full(len(angles), np.nan)
    hull = ConvexHull(points2d)
    eq = hull.equations  # a x + b z + c <= 0 inside
    d = np.stack([np.sin(angles), np.cos(angles)], 1)  # (x, z) direction
    # Ray t * d leaves the hull at the smallest t with a.d * t + c = 0, a.d > 0.
    ad = d @ eq[:, :2].T
    with np.errstate(divide="ignore", invalid="ignore"):
        t = np.where(ad > 1e-9, -eq[:, 2][None, :] / ad, np.inf)
    return t.min(1)


def _smoothstep(a, b, x):
    t = np.clip((x - a) / (b - a), 0.0, 1.0)
    return t * t * (3 - 2 * t)


def drape_surf(body, ji, joints, top, bottom, rings=22, segments=56, arc=(-np.pi, np.pi), clear=0.012,
               flare=0.06, taper=0.0, include_arms=False, follow_legs=True, band=0.035, max_x=None):
    """Ring-grid cloth from height `top` down to `bottom` (m), over the angular range `arc`
    (radians, 0 = front; a full circle closes the seam). `max_x` fits only body points within that
    distance of the centre line (cloaks: the torso, not the T-posed arms). Returns a `surface.Surf`."""
    surf = body.surf
    pos, w, morph = surf.pos, surf.w, surf.morph
    arms = sum(w[:, ji(f"{a}_{s}")] for a in ("upperarm", "lowerarm", "hand") for s in "lr")
    use = np.ones(len(pos), bool) if include_arms else arms < 0.3
    if max_x is not None:
        # Fit to the torso only (the bind pose holds the arms out; they hang inside when lowered).
        use &= np.abs(pos[:, 0]) < max_x
    full = np.isclose(arc[1] - arc[0], 2 * np.pi)
    cols = segments if full else segments + 1
    ang = np.linspace(arc[0], arc[1], cols, endpoint=not full)
    ys = np.linspace(top, bottom, rings)
    xz = np.stack([pos[:, 0], pos[:, 2] - AXIS_Z], 1)

    radius = np.zeros((rings, cols))
    widest = None
    for i, y in enumerate(ys):
        sel = use & (np.abs(pos[:, 1] - y) < band)
        r = _hull_radius(xz[sel], ang) if sel.sum() >= 3 else np.full(cols, np.nan)
        if np.isnan(r).all() or not np.isfinite(r).any():
            r = radius[i - 1] if i else np.full(cols, 0.15)
        r = np.where(np.isfinite(r), r, np.nanmax(r[np.isfinite(r)]))
        if widest is not None:
            # Cloth falls: never narrower than the ring above (less the taper).
            r = np.maximum(r, widest * (1.0 - taper))
        widest = r if widest is None else np.maximum(widest * (1.0 - taper), r)
        radius[i] = r
    # Smooth around and down, add clearance and the hem flare.
    for _ in range(3):
        radius = 0.5 * radius + 0.25 * (np.roll(radius, 1, 1) + np.roll(radius, -1, 1)) if full else radius
        radius[1:-1] = 0.5 * radius[1:-1] + 0.25 * (radius[:-2] + radius[2:])
    down = (top - ys) / max(top - bottom, 1e-6)
    radius = radius + clear + flare * down[:, None] ** 1.5

    P = np.zeros((rings, cols, 3))
    P[..., 0] = radius * np.sin(ang)[None, :]
    P[..., 2] = radius * np.cos(ang)[None, :] + AXIS_Z
    P[..., 1] = ys[:, None]
    P = P.reshape(-1, 3)

    tri = []
    for i in range(rings - 1):
        for j in range(cols if full else cols - 1):
            a, b = i * cols + j, i * cols + (j + 1) % cols
            c, d = a + cols, b + cols
            tri += [[a, c, b], [b, c, d]]
    tri = np.array(tri, np.int64)
    nrm = sf.vertex_normals(P, tri)
    # Make the normals face outward (away from the axis).
    out = np.stack([P[:, 0], np.zeros(len(P)), P[:, 2] - AXIS_Z], 1)
    if (np.einsum("ij,ij->i", nrm, out) < 0).mean() > 0.5:
        tri = tri[:, ::-1]
        nrm = -nrm

    # Weights and morphs from the nearest body surface, then the legs below the hips.
    tree = cKDTree(pos)
    d, nn = tree.query(P, k=4)
    k = 1.0 / np.maximum(d, 1e-4) ** 2
    k /= k.sum(1, keepdims=True)
    wd = np.einsum("nk,nkj->nj", k, w[nn])
    mo = np.einsum("nk,nkij->nij", k, morph[nn])
    if follow_legs:
        hip = joints["thigh_l"][1]
        knee = joints["calf_l"][1]
        leg = _smoothstep(hip + 0.02, hip - 0.12, P[:, 1])  # 0 at the hips .. 1 down the legs
        calf = _smoothstep(knee + 0.08, knee - 0.12, P[:, 1])
        left = _smoothstep(-0.07, 0.07, P[:, 0])  # +X is the body's left
        legs = np.zeros_like(wd)
        for side, share in (("l", left), ("r", 1 - left)):
            legs[:, ji(f"thigh_{side}")] += share * (1 - calf)
            legs[:, ji(f"calf_{side}")] += share * calf
        # Keep a little of the pelvis so the hem doesn't follow a kick all the way.
        legs = 0.8 * legs
        legs[:, ji("pelvis")] += 0.2
        wd = (1 - leg)[:, None] * wd + leg[:, None] * legs
    wd /= np.maximum(wd.sum(1, keepdims=True), 1e-12)
    return sf.Surf(P, nrm, wd, mo, tri)
