"""Mesh surgery on a welded, skinned body surface: iso-line cuts with interpolated attributes,
smoothing, boundary loops, and shells with thickness (outer + inner + rim).

Every vertex carries one dense attribute row: position (3), normal (3), dense skin weights
(one column per ual2 joint) and morph deltas (3 per target).  A cut inserts vertices on the
edges it crosses with all attributes linearly interpolated, so new vertices get weights
interpolated from the original surface (renormalised to the 4 strongest when exported).
"""
from __future__ import annotations

import numpy as np


class Surf:
    """Welded triangle surface with dense attributes."""

    def __init__(self, pos, nrm, wdense, morph, tri):
        self.pos = np.asarray(pos, float)
        self.nrm = np.asarray(nrm, float)
        self.w = np.asarray(wdense, float)
        self.morph = np.asarray(morph, float)  # (n, K, 3)
        self.tri = np.asarray(tri, np.int64)

    def attrs(self):
        n = len(self.pos)
        return np.concatenate([self.pos, self.nrm, self.w, self.morph.reshape(n, -1)], 1)

    @staticmethod
    def from_attrs(a, tri, nj, nk):
        pos = a[:, :3]; nrm = a[:, 3:6]; w = a[:, 6:6 + nj]; m = a[:, 6 + nj:].reshape(len(a), nk, 3)
        nrm = nrm / np.maximum(np.linalg.norm(nrm, axis=1, keepdims=True), 1e-12)
        return Surf(pos, nrm, w, m, tri)

    def copy(self):
        return Surf(self.pos.copy(), self.nrm.copy(), self.w.copy(), self.morph.copy(), self.tri.copy())


def cut(s: Surf, f: np.ndarray, snap=2e-4) -> Surf:
    """Keep the part of the surface where f > 0, splitting triangles along f = 0."""
    f = np.where(np.abs(f) < snap, snap, f)  # keep cut vertices off existing ones
    nj, nk = s.w.shape[1], s.morph.shape[1]
    A = s.attrs()
    rows = [A]
    nrows = len(A)
    edge_new = {}

    def mid(a, b):
        nonlocal nrows
        key = (a, b) if a < b else (b, a)
        if key in edge_new:
            return edge_new[key]
        t = f[a] / (f[a] - f[b])
        rows.append((A[a] * (1 - t) + A[b] * t)[None, :])
        edge_new[key] = nrows
        nrows += 1
        return edge_new[key]

    out = []
    inside = f > 0
    for t in s.tri:
        ins = inside[t]
        c = int(ins.sum())
        if c == 3:
            out.append(t)
        elif c == 0:
            continue
        else:
            # rotate so the odd vertex comes first, preserving winding
            k = int(np.where(ins != (c == 2))[0][0])  # the vertex alone on its side
            a, b, cc = t[k], t[(k + 1) % 3], t[(k + 2) % 3]
            ab, ac = mid(a, b), mid(a, cc)
            if c == 1:  # a inside alone
                out.append([a, ab, ac])
            else:  # a outside alone: quad b, c, ac, ab
                out.append([ab, b, cc])
                out.append([ab, cc, ac])
    A2 = np.concatenate(rows, 0)
    tri = np.array(out, np.int64).reshape(-1, 3)
    used = np.unique(tri)
    remap = -np.ones(len(A2), np.int64)
    remap[used] = np.arange(len(used))
    return Surf.from_attrs(A2[used], remap[tri], nj, nk)


def select(s: Surf, keep_tri: np.ndarray) -> Surf:
    tri = s.tri[keep_tri]
    used = np.unique(tri)
    remap = -np.ones(len(s.pos), np.int64)
    remap[used] = np.arange(len(used))
    return Surf(s.pos[used], s.nrm[used], s.w[used], s.morph[used], remap[tri])


def neighbours(n, tri):
    nb = [set() for _ in range(n)]
    for a, b, c in tri:
        nb[a] |= {b, c}; nb[b] |= {a, c}; nb[c] |= {a, b}
    return [np.array(sorted(x), np.int64) for x in nb]


def boundary_edges(tri):
    from collections import Counter
    cnt = Counter()
    for a, b, c in tri:
        for e in ((a, b), (b, c), (c, a)):
            cnt[(min(e), max(e))] += 1
    directed = []
    for a, b, c in tri:
        for e in ((a, b), (b, c), (c, a)):
            if cnt[(min(e), max(e))] == 1:
                directed.append((e[0], e[1], c if e == (a, b) else (a if e == (b, c) else b)))
    return directed  # (from, to, opposite vertex)


def smooth_field(values, nb, iters=10, lam=0.5, fixed=None):
    v = values.copy()
    for _ in range(iters):
        avg = np.stack([v[x].mean(0) if len(x) else v[i] for i, x in enumerate(nb)])
        nv = v + lam * (avg - v)
        if fixed is not None:
            nv[fixed] = v[fixed]
        v = nv
    return v


def taubin(pos, nb, iters=8, lam=0.5, mu=-0.53, fixed=None):
    p = pos.copy()
    for _ in range(iters):
        for k in (lam, mu):
            avg = np.stack([p[x].mean(0) if len(x) else p[i] for i, x in enumerate(nb)])
            np_ = p + k * (avg - p)
            if fixed is not None:
                np_[fixed] = p[fixed]
            p = np_
    return p


def vertex_normals(pos, tri):
    n = np.zeros_like(pos)
    fn = np.cross(pos[tri[:, 1]] - pos[tri[:, 0]], pos[tri[:, 2]] - pos[tri[:, 0]])
    for k in range(3):
        np.add.at(n, tri[:, k], fn)
    return n / np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)


def dist_to_boundary(pos, tri):
    be = boundary_edges(tri)
    if not be:
        return np.full(len(pos), np.inf)
    bverts = np.unique(np.array([(a, b) for a, b, _ in be]).reshape(-1))
    from scipy.spatial import cKDTree
    d, _ = cKDTree(pos[bverts]).query(pos)
    return d


def shell(s: Surf, outer, inner, smooth_iters=6, bead=0.0, bead_width=0.012, extra=None, span=0.0):
    """Build a closed-edged garment shell over surface `s`.

    outer / inner: offsets (m) along the normal, scalars or per-vertex arrays.
    extra: optional per-vertex additional offset (flare), added to both.
    Returns dict with pos, nrm, wdense, morph, tri, and 'part' per vertex (0 outer, 1 inner, 2 rim).
    """
    n = len(s.pos)
    nb = neighbours(n, s.tri)
    be = boundary_edges(s.tri)
    bset = np.zeros(n, bool)
    for a, b, _ in be:
        bset[a] = bset[b] = True
    # drape: cloth spans hollows. Laplacian-smooth the surface and keep only the outward part of
    # the change along the normal (no sliding along the skin, so the skin weights still fit).
    nrm = smooth_field(s.nrm, nb, iters=4, lam=0.5)
    nrm /= np.maximum(np.linalg.norm(nrm, axis=1, keepdims=True), 1e-12)
    if span > 0:
        # Spanning height: how far nearby points of the garment stand above this vertex's tangent
        # plane, tapered to zero at `span` metres. Hollows (between the breasts, the small of the
        # back, between the buttocks) fill; convex areas get zero.
        from scipy.spatial import cKDTree
        tree = cKDTree(s.pos)
        lift = np.zeros(n)
        for i, nbrs in enumerate(tree.query_ball_point(s.pos, span)):
            d = s.pos[nbrs] - s.pos[i]
            h = d @ nrm[i]
            r = np.linalg.norm(d, axis=1)
            lift[i] = max(0.0, float(np.max(h * (1 - r / span))))
        lift = smooth_field(lift[:, None], nb, iters=smooth_iters)[:, 0]
    else:
        lift = np.zeros(n)
    ps = s.pos
    inward = lift
    outer = np.broadcast_to(np.asarray(outer, float), (n,)).copy()
    inner = np.broadcast_to(np.asarray(inner, float), (n,)).copy()
    if extra is not None:
        outer += extra; inner += extra
    if bead > 0:
        db = dist_to_boundary(s.pos, s.tri)
        outer += bead * np.clip(1 - db / bead_width, 0, 1) ** 2
    O = ps + nrm * (outer + inward)[:, None]
    I = ps + nrm * (inner + inward)[:, None]
    pos = np.concatenate([O, I])
    tri_o = s.tri
    tri_i = s.tri[:, ::-1] + n
    nrm_o = vertex_normals(O, tri_o)
    nrm_i = -vertex_normals(I, s.tri)
    # rim: duplicated vertices with their own normals
    rim_tris = []
    rim_src = []
    for a, b, c in be:
        ia = len(rim_src); rim_src.append(a)
        ib = len(rim_src); rim_src.append(b)
        rim_tris.append((ia, ib, c))
    rim_src = np.array(rim_src, np.int64)
    nr = len(rim_src)
    # rim vertex k: outer copy at 2n + k, inner copy at 2n + nr + k
    R_o = O[rim_src]; R_i = I[rim_src]
    rt = []
    rn = np.zeros((2 * nr, 3))
    P = np.concatenate([O, I, R_o, R_i])
    for k in range(0, nr, 2):
        a_o, b_o = 2 * n + k, 2 * n + k + 1
        a_i, b_i = 2 * n + nr + k, 2 * n + nr + k + 1
        q = [a_o, b_o, b_i, a_i]
        fnrm = np.cross(P[q[1]] - P[q[0]], P[q[3]] - P[q[0]])
        away = P[q[0]] - O[rim_tris[k // 2][2]]
        if fnrm @ away < 0:
            q = q[::-1]
        rt += [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]
        fn = fnrm if fnrm @ away >= 0 else -fnrm
        fn = fn / max(np.linalg.norm(fn), 1e-12)
        for v in (k, k + 1):
            rn[v] += fn; rn[nr + v] += fn
    # weld rim normals across shared source vertices
    acc = {}
    for k, src in enumerate(rim_src):
        acc.setdefault(src, []).append(k)
    for src, ks in acc.items():
        m = rn[ks].sum(0)
        m /= max(np.linalg.norm(m), 1e-12)
        rn[ks] = m; rn[[nr + k for k in ks]] = m
    pos = np.concatenate([O, I, R_o, R_i])
    nrm_all = np.concatenate([nrm_o, nrm_i, rn])
    src = np.concatenate([np.arange(n), np.arange(n), rim_src, rim_src])
    part = np.concatenate([np.zeros(n), np.ones(n), np.full(nr, 2), np.full(nr, 3)]).astype(int)
    tri = np.concatenate([tri_o, tri_i, np.array(rt, np.int64).reshape(-1, 3)])
    return {"pos": pos, "nrm": nrm_all, "w": s.w[src], "morph": s.morph[src], "tri": tri, "part": part,
            "src": src, "base": s.pos[src]}


def top4(wdense):
    idx = np.argsort(-wdense, axis=1)[:, :4]
    w = np.take_along_axis(wdense, idx, 1)
    w = np.maximum(w, 0)
    w /= np.maximum(w.sum(1, keepdims=True), 1e-12)
    w[w < 1e-4] = 0
    w /= w.sum(1, keepdims=True)
    idx[w == 0] = 0
    return idx.astype(np.int64), w


def dense_weights(joints, weights, nj):
    out = np.zeros((len(joints), nj))
    for k in range(joints.shape[1]):
        np.add.at(out, (np.arange(len(joints)), joints[:, k]), weights[:, k])
    return out
