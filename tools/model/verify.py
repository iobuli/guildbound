"""Verify the hero kit.

    python -I tools/model/verify.py [--frames N] [--libs ual1,ual2,sekiro] [--quick]

1. Every exported GLB (assets/characters/hero/**.glb, assets/anim/hero_styled.glb): joint
   names and order, inverse bind matrices and skeleton node TRS identical to ual2.glb; weights
   normalised; joint indices in range; morph target counts.
2. Penetration under real clips: ual1 / ual2 clips copy local rotations by name (the game's
   convention for UAL libraries, pelvis translation copied); sekiro (Mixamo) goes through the
   same world-space bind-delta retarget as `motion/retarget.rs`. Each layer pair (inner A under
   outer B) is measured as the signed distance of A's vertices to B's outer surface, for the A
   vertices that lie under B in the rest pose (more than 1 cm in from B's edges). A positive
   distance means A pokes through B. Hair is measured the other way round (hair inside a garment
   or the torso). Also run with morph extremes and a 1.3x exaggeration of the clip rotations
   (the F1 Exaggeration knob goes to 1.4).
Prints a JSON report (also written to assets/characters/hero/verify_report.json).
"""
from __future__ import annotations

import glob
import json
import os
import sys
import time

import numpy as np
from scipy.spatial import cKDTree

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import build  # noqa: E402
import gbmesh as gm  # noqa: E402
import surface as sf  # noqa: E402

REF, RS, NAMES = build.REF, build.RS, build.NAMES
ASSETS = build.ASSETS


# ------------------------------------------------------------------------------------------
# 1. Static checks

def static_checks():
    out = {}
    ref_nodes = {n.name: n for n in REF.g.nodes}
    files = sorted(glob.glob(os.path.join(ASSETS, "characters", "hero", "*", "*.glb"))) + [os.path.join(ASSETS, "anim", "hero_styled.glb")]
    worst = {"ibm": 0.0, "trs": 0.0, "wsum": 0.0}
    for f in files:
        d = gm.Doc(f)
        s = gm.Skin(d)
        rec = {"joints_equal": s.names == NAMES, "ibm_max_abs_diff": float(np.abs(s.ibm - RS.ibm).max())}
        trs = 0.0
        for j in s.joints:
            n = d.g.nodes[j]; r = ref_nodes[n.name]
            for a, b, dflt in ((n.translation, r.translation, [0, 0, 0]), (n.rotation, r.rotation, [0, 0, 0, 1]), (n.scale, r.scale, [1, 1, 1])):
                trs = max(trs, float(np.abs(np.array(a or dflt) - np.array(b or dflt)).max()))
        rec["skeleton_trs_max_abs_diff"] = trs
        rec["parents_equal"] = s.parent == RS.parent
        ws, jmax, verts, tris, targets = 0.0, 0, 0, 0, set()
        for m in d.g.meshes:
            for p in m.primitives:
                a = gm.prim_arrays(d, p)
                ws = max(ws, float(np.abs(a["weights"].sum(1) - 1).max()))
                jmax = max(jmax, int(a["joints"].max()))
                verts += len(a["pos"]); tris += len(a["idx"])
                targets.add(len(p.targets or []))
        rec.update(weight_sum_max_err=ws, max_joint_index=jmax, vertices=verts, triangles=tris, morph_targets=sorted(targets),
                   mounts=[n.name for n in d.g.nodes if n.name and n.name.startswith("sheath_")])
        worst["ibm"] = max(worst["ibm"], rec["ibm_max_abs_diff"]); worst["trs"] = max(worst["trs"], trs); worst["wsum"] = max(worst["wsum"], ws)
        ok = rec["joints_equal"] and rec["parents_equal"] and rec["ibm_max_abs_diff"] < 1e-6 and trs < 1e-6 and ws < 1e-4 and jmax < len(NAMES)
        rec["ok"] = bool(ok)
        out[os.path.relpath(f, ASSETS)] = rec
    return out, worst


# ------------------------------------------------------------------------------------------
# 2. Clips

def rot_of(m):
    r = m[:3, :3].copy()
    r /= np.linalg.norm(r, axis=0, keepdims=True)
    return r


def mat_from_quat(q):
    return gm.quat_mat(q)


class Lib:
    def __init__(self, name):
        self.name = name
        self.doc = gm.Doc(os.path.join(ASSETS, "anim", f"{name}.glb"))
        self.clips = gm.clips(self.doc)
        self.by_name = {n.name: i for i, n in enumerate(self.doc.g.nodes)}
        self.mixamo = any((n.name or "").startswith("mixamorig:") for n in self.doc.g.nodes)
        if self.mixamo:
            from_names = {}
            src = open(os.path.join(build.ROOT, "crates", "game", "src", "motion", "retarget.rs")).read()
            import re
            for a, b in re.findall(r'\("([A-Za-z0-9]+)", "([A-Za-z0-9_]+)"\)', src.split("MIXAMO_MAP")[1].split("];")[0]):
                from_names[b] = "mixamorig:" + a
            self.map = from_names
            self.parents = self.doc.parents()
            self.rest_world = self.doc.world_rest()
            skin = gm.Skin(self.doc)
            self.bind_world = {skin.joints[i]: np.linalg.inv(skin.ibm[i]) for i in range(len(skin.joints))}

    def locals_at(self, clip, t):
        dur, ch = self.clips[clip]
        t = min(t, dur)
        if not self.mixamo:
            return gm.pose_locals(RS, self.doc, ch, t, self.by_name)
        # donor world rotations at t
        n = len(self.doc.g.nodes)
        loc = [None] * n
        for i, node in enumerate(self.doc.g.nodes):
            tr = np.array(node.translation or [0, 0, 0], float); r = np.array(node.rotation or [0, 0, 0, 1], float)
            sc = np.array(node.scale or [1, 1, 1], float)
            if i in ch:
                if "rotation" in ch[i]:
                    r = gm.sample(ch[i]["rotation"], t)
                if "translation" in ch[i]:
                    tr = gm.sample(ch[i]["translation"], t)
            loc[i] = gm.trs(tr, r, sc)
        world = [None] * n

        def w(i):
            if world[i] is None:
                p = self.parents[i]
                world[i] = loc[i] if p is None else w(p) @ loc[i]
            return world[i]
        rest_w = RS.rest_world()
        Wd = [None] * len(NAMES)
        for k, name in enumerate(NAMES):
            src = self.map.get(name)
            if src is None or src not in self.by_name:
                continue
            i = self.by_name[src]
            bind = self.bind_world.get(i, self.rest_world[i])
            Wd[k] = rot_of(w(i)) @ rot_of(bind).T @ rot_of(rest_w[k])
        out = RS.rest_local.copy()
        cur = [None] * len(NAMES)
        for k in range(len(NAMES)):
            p = RS.parent[k]
            pw = RS.base[:3, :3] if p is None else cur[p]
            if Wd[k] is None:
                cur[k] = pw @ RS.rest_local[k][:3, :3]
            else:
                cur[k] = Wd[k]
                lr = pw.T @ Wd[k]
                out[k][:3, :3] = lr
        return out


def exaggerate(loc, k):
    """Push each local rotation further from rest by factor k (like the F1 Exaggeration knob)."""
    if k == 1.0:
        return loc
    out = loc.copy()
    for i in range(len(loc)):
        r0 = RS.rest_local[i][:3, :3]; r = loc[i][:3, :3]
        d = r0.T @ r
        ang = np.arccos(np.clip((np.trace(d) - 1) / 2, -1, 1))
        if ang < 1e-6:
            continue
        axis = np.array([d[2, 1] - d[1, 2], d[0, 2] - d[2, 0], d[1, 0] - d[0, 1]]) / (2 * np.sin(ang))
        a = ang * k
        K = np.array([[0, -axis[2], axis[1]], [axis[2], 0, -axis[0]], [-axis[1], axis[0], 0]])
        rr = np.eye(3) + np.sin(a) * K + (1 - np.cos(a)) * K @ K
        out[i][:3, :3] = r0 @ rr
    return out


def frame_mats(loc):
    return np.einsum("nij,njk->nik", RS.fk(loc), RS.ibm)


# ------------------------------------------------------------------------------------------
# 3. Geometry per base

class Piece:
    """Skinnable geometry with an 'outer surface' (triangles) for distance queries."""

    def __init__(self, name, pos, joints, weights, morph, tri_outer, probe=None):
        self.name = name
        self.pos = pos; self.joints = joints; self.weights = weights; self.morph = morph
        self.tri = tri_outer
        self.probe = probe if probe is not None else np.arange(len(pos))
        self.boundary = np.array([], np.int64)

    def posed(self, mats, morph_w=None):
        p = self.pos
        if morph_w is not None and self.morph is not None:
            p = p + np.einsum("k,nkj->nj", morph_w, self.morph)
        return gm.skin_points(p, self.joints, self.weights, mats)


def piece_from_shell(name, sh):
    joints, weights = sf.top4(sh["w"])
    n = (sh["part"] == 0).sum()
    outer_tri = sh["tri"][np.all(sh["tri"] < n, axis=1)]
    probe = np.where(sh["part"] != 1)[0]  # outer + rim vertices are what could show
    pc = Piece(name, sh["pos"], joints, weights, sh["morph"], outer_tri, probe)
    be = sf.boundary_edges(outer_tri)
    pc.boundary = np.unique(np.array([(a, b) for a, b, _ in be]).reshape(-1)) if be else np.array([], int)
    return pc


def closest_on_tris(p, a, b, c):
    """Closest points on triangles (vectorised, Ericson's method). Returns points."""
    ab = b - a; ac = c - a; ap = p - a
    d1 = np.einsum("ij,ij->i", ab, ap); d2 = np.einsum("ij,ij->i", ac, ap)
    bp = p - b; d3 = np.einsum("ij,ij->i", ab, bp); d4 = np.einsum("ij,ij->i", ac, bp)
    cp = p - c; d5 = np.einsum("ij,ij->i", ab, cp); d6 = np.einsum("ij,ij->i", ac, cp)
    va = d3 * d6 - d5 * d4; vb = d5 * d2 - d1 * d6; vc = d1 * d4 - d3 * d2
    denom = va + vb + vc
    denom = np.where(np.abs(denom) < 1e-18, 1e-18, denom)
    v = vb / denom; w = vc / denom
    res = a + ab * v[:, None] + ac * w[:, None]
    # regions
    m = (d1 <= 0) & (d2 <= 0); res[m] = a[m]
    m = (d3 >= 0) & (d4 <= d3); res[m] = b[m]
    m = (d6 >= 0) & (d5 <= d6); res[m] = c[m]
    m = (vc <= 0) & (d1 >= 0) & (d3 <= 0)
    t = d1 / np.where(np.abs(d1 - d3) < 1e-18, 1e-18, d1 - d3); res[m] = (a + ab * t[:, None])[m]
    m = (vb <= 0) & (d2 >= 0) & (d6 <= 0)
    t = d2 / np.where(np.abs(d2 - d6) < 1e-18, 1e-18, d2 - d6); res[m] = (a + ac * t[:, None])[m]
    m = (va <= 0) & ((d4 - d3) >= 0) & ((d5 - d6) >= 0)
    t = (d4 - d3) / np.where(np.abs((d4 - d3) + (d5 - d6)) < 1e-18, 1e-18, (d4 - d3) + (d5 - d6))
    res[m] = (b + (c - b) * t[:, None])[m]
    return res


def signed_distance(points, verts, tri, vtri, k=4, want_tri=False):
    """Signed distance from points to the triangle surface (sign by face normal).
    With want_tri, also returns the closest triangle and the closest point."""
    used = np.unique(tri)
    tree = cKDTree(verts[used])
    _, nn = tree.query(points, k=k)
    nn = used[nn]
    best_d = np.full(len(points), np.inf); best_s = np.zeros(len(points))
    best_t = np.zeros(len(points), np.int64); best_q = np.zeros_like(points)
    fn = np.cross(verts[tri[:, 1]] - verts[tri[:, 0]], verts[tri[:, 2]] - verts[tri[:, 0]])
    fn /= np.maximum(np.linalg.norm(fn, axis=1, keepdims=True), 1e-12)
    for col in range(k):
        cand = vtri[nn[:, col]]  # (P, maxdeg) with -1 padding
        for j in range(cand.shape[1]):
            t = cand[:, j]
            ok = t >= 0
            if not ok.any():
                continue
            idx = np.where(ok)[0]
            tt = t[idx]
            q = closest_on_tris(points[idx], verts[tri[tt, 0]], verts[tri[tt, 1]], verts[tri[tt, 2]])
            d = np.linalg.norm(points[idx] - q, axis=1)
            s = np.einsum("ij,ij->i", points[idx] - q, fn[tt])
            better = d < best_d[idx]
            bi = idx[better]
            best_d[bi] = d[better]
            best_s[bi] = np.where(s[better] >= 0, d[better], -d[better])
            best_t[bi] = tt[better]; best_q[bi] = q[better]
    if want_tri:
        return best_s, best_t, best_q
    return best_s


def barycentric(q, a, b, c):
    v0, v1, v2 = b - a, c - a, q - a
    d00 = np.einsum("ij,ij->i", v0, v0); d01 = np.einsum("ij,ij->i", v0, v1); d11 = np.einsum("ij,ij->i", v1, v1)
    d20 = np.einsum("ij,ij->i", v2, v0); d21 = np.einsum("ij,ij->i", v2, v1)
    den = np.maximum(d00 * d11 - d01 * d01, 1e-18)
    v = (d11 * d20 - d01 * d21) / den; w = (d00 * d21 - d01 * d20) / den
    return np.stack([1 - v - w, v, w], 1)


def vert_tris(nv, tri):
    deg = np.bincount(tri.reshape(-1), minlength=nv)
    out = -np.ones((nv, max(int(deg.max()), 1)), np.int64)
    fill = np.zeros(nv, np.int64)
    for t, (a, b, c) in enumerate(tri):
        for v in (a, b, c):
            out[v, fill[v]] = t; fill[v] += 1
    return out


class Pair:
    """A under B: A's probe vertices that lie under B's interior in the rest pose."""

    def __init__(self, a: Piece, b: Piece, hair=False, below=None):
        self.a, self.b, self.hair = a, b, hair
        self.b_vtri = vert_tris(len(b.pos), b.tri)
        rest = np.eye(4)[None].repeat(len(NAMES), 0)
        pa = a.posed(rest)[a.probe]; pb = b.posed(rest)
        sd, tt, qq = signed_distance(pa, pb, b.tri, self.b_vtri, want_tri=True)
        used = np.unique(b.tri)
        tb = cKDTree(pb[used])
        dist_any, _ = tb.query(pa)
        if len(b.boundary):
            dbound, _ = cKDTree(pb[b.boundary]).query(pa)
        else:
            dbound = np.full(len(pa), np.inf)
        if hair:
            sel = (np.abs(sd) < 0.05) & (dbound > 0.01)
            if below is not None:
                sel &= pa[:, 1] < below
        else:
            sel = (sd < 0.002) & (sd > -0.06) & (dist_any < 0.06) & (dbound > 0.01)
        self.idx = a.probe[sel]
        self.rest_sd = sd[sel]
        # rest-pose correspondence: the point of B each A vertex lies under, tracked through poses
        self.t = tt[sel]
        tb_ = b.tri[self.t]
        self.bary = barycentric(qq[sel], pb[tb_[:, 0]], pb[tb_[:, 1]], pb[tb_[:, 2]])
        # keep only pairs that ride the same bones (else it is limb-on-limb contact, not layering)
        wa = sf.dense_weights(a.joints[self.idx], a.weights[self.idx], len(NAMES))
        wb = sum(self.bary[:, k:k + 1] * sf.dense_weights(b.joints[tb_[:, k]], b.weights[tb_[:, k]], len(NAMES)) for k in range(3))
        same = np.abs(wa - wb).sum(1) < 0.5
        self.dropped = int((~same).sum())
        self.idx, self.t, self.bary, self.rest_sd = self.idx[same], self.t[same], self.bary[same], self.rest_sd[same]

    def measure(self, mats, morph_w=None):
        if len(self.idx) == 0:
            return np.zeros(0)
        pa = self.a.posed(mats, morph_w)[self.idx]
        pb = self.b.posed(mats, morph_w)
        tri = self.b.tri[self.t]
        A, B, C = pb[tri[:, 0]], pb[tri[:, 1]], pb[tri[:, 2]]
        q = A * self.bary[:, :1] + B * self.bary[:, 1:2] + C * self.bary[:, 2:]
        n = np.cross(B - A, C - A)
        n /= np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)
        sd = np.einsum("ij,ij->i", pa - q, n)
        return -sd if self.hair else sd


class HairPair:
    """Hair over a garment, measured along the skin normal: at rest each hair vertex (below the
    skull, within 6 cm of the skin) takes its nearest skin vertex b and the garment's outer vertex
    built from b. In a pose: pen = garment height - hair height over b along b's normal."""

    def __init__(self, hair: Piece, body: Piece, body_nrm, shell, garment: Piece):
        self.hair, self.body, self.garment = hair, body, garment
        self.body_nrm = body_nrm
        d, b = cKDTree(body.pos).query(hair.pos)
        sel = (d < 0.06) & (hair.pos[:, 1] < build.J["Head"][1])
        outer = np.where(shell["part"] == 0)[0]
        dg, gi = cKDTree(shell["base"][outer]).query(body.pos[b])
        sel &= dg < 0.004
        self.idx = np.where(sel)[0]
        self.b = b[sel]
        self.g = outer[gi[sel]]
        self.dropped = 0

    def measure(self, mats, morph_w=None):
        if len(self.idx) == 0:
            return np.zeros(0)
        h = self.hair.posed(mats)[self.idx]
        bp = self.body.posed(mats, morph_w)[self.b]
        bn = gm.skin_vectors(self.body_nrm[self.b], self.body.joints[self.b], self.body.weights[self.b], mats)
        bn /= np.maximum(np.linalg.norm(bn, axis=1, keepdims=True), 1e-12)
        gp = self.garment.posed(mats, morph_w)[self.g]
        return np.einsum("ij,ij->i", gp - bp, bn) - np.einsum("ij,ij->i", h - bp, bn)


def pieces_for(sex):
    body = build.Body(sex)
    b = body.body
    tri = b["idx"]
    mask = build.hide_mask(body)
    pieces = {}
    pieces["body"] = Piece("body", b["pos"], b["joints"], b["weights"], body.morph_unwelded, tri)
    pieces["body"].hide_mask = mask
    shells = {}
    for g in build.GARMENTS:
        _, sh = build.make_garment(body, g)
        shells[g] = sh
        pieces[g] = piece_from_shell(g, sh)
    cp, cn = [], []
    for g in ("tunic", "shirt", "baldric"):
        o = shells[g]["part"] == 0
        cp.append((shells[g]["pos"] + shells[g]["morph"][:, 0] + shells[g]["morph"][:, 1])[o]); cn.append(shells[g]["nrm"][o])
    cloth = (np.concatenate(cp), np.concatenate(cn))
    for name, (fname, native) in build.HAIR.items():
        if name.startswith("brows") or name == "beard":
            continue
        arr, nat, tex, _ = build.load_hair(name)
        fitted = build.fit_hair(arr, None if nat == sex else build.Body(nat), body, native=(nat == sex), transfer=True, cloth=cloth)
        pieces[name] = Piece(name, fitted["pos"], fitted["joints"], fitted["weights"], None, fitted["idx"])
    return body, pieces, shells


PAIRS = [  # (inner, outer)
    ("trousers", "tunic"), ("trousers", "shirt"), ("trousers", "boots"), ("trousers", "shoes"), ("trousers", "belt"),
    ("gloves", "tunic"), ("tunic", "belt"), ("shirt", "belt"), ("tunic", "baldric"), ("shirt", "baldric"),
    ("baldric", "belt"),
]
HAIR_OVER = ["tunic", "shirt", "baldric", "body"]


def run(libs, frames_per_clip, quick):
    t0 = time.time()
    report = {"static": None, "penetration": {}}
    report["static"], worst = static_checks()
    report["static_worst"] = worst
    libs = [Lib(n) for n in libs]
    poses = []
    for lib in libs:
        for clip, (dur, _) in lib.clips.items():
            if clip in ("A_TPose", "T-Pose"):
                continue
            n = frames_per_clip if dur > 0.05 else 1
            for t in np.linspace(0, dur, n):
                poses.append((lib, clip, float(t)))
    if quick:
        poses = poses[::7]
    report["clips"] = {lib.name: len(lib.clips) for lib in libs}
    report["frames"] = len(poses)
    mats_all = []
    for lib, clip, t in poses:
        loc = lib.locals_at(clip, t)
        mats_all.append((f"{lib.name}/{clip}@{t:.2f}", frame_mats(loc), frame_mats(exaggerate(loc, 1.3))))
    morph_sets = {"none": None, "bulk+heavy": np.array([1.0, 1.0, 0, 0]), "slim": np.array([0, 0, 1.0, 0])}
    for sex in ("male", "female"):
        body, pieces, shells = pieces_for(sex)
        res = {}
        pairs = {}
        for g in build.GARMENTS:
            if g in ("belt", "baldric"):
                continue  # over the clothes; the body under them is never hidden by them
            pairs[f"body>{g}"] = Pair(pieces["body"], pieces[g])
        for a, b in PAIRS:
            pairs[f"{a}>{b}"] = Pair(pieces[a], pieces[b])
        for h in [k for k in pieces if k.startswith("hair_")]:
            for g in HAIR_OVER:
                if g == "body":
                    # hair against the torso and neck (not the scalp, where the roots sit by design)
                    pairs[f"{h}<{g}"] = Pair(pieces[h], pieces["body"], hair=True, below=float(build.J["Head"][1]))
                else:
                    pairs[f"{h}<{g}"] = HairPair(pieces[h], pieces["body"], body.body["nrm"], shells[g], pieces[g])
        hidden_by = {}
        mask = pieces["body"].hide_mask
        for bit, g in enumerate(build.HIDING):
            v = np.zeros(len(pieces["body"].pos), bool)
            v[np.unique(pieces["body"].tri[(mask >> bit & 1) == 1])] = True
            hidden_by[g] = v
        for key, pr in pairs.items():
            stats = {"probe_vertices": int(len(pr.idx)), "dropped_other_bones": pr.dropped}
            for label, morph in morph_sets.items():
                for ex in ((False, True) if label == "none" else (False,)):
                    vals = []
                    worst = (0.0, "")
                    step = 1 if label == "none" else 3
                    for name, mats, mats_ex in mats_all[::step]:
                        d = pr.measure(mats_ex if ex else mats, morph)
                        if key.startswith("body>"):
                            g = key.split(">")[1]
                            vis = ~hidden_by[g][pr.idx]
                            d_vis = d[vis] if len(d) else d
                            stats.setdefault("_vis", []).append(float(d_vis.max()) if len(d_vis) else 0.0)
                        if len(d):
                            m = float(d.max())
                            vals.append(d)
                            if m > worst[0]:
                                worst = (m, name)
                    allv = np.concatenate(vals) if vals else np.zeros(1)
                    tag = label + ("+exag1.3" if ex else "")
                    stats[tag] = {"worst_mm": round(worst[0] * 1000, 2), "worst_pose": worst[1],
                                  "p99_mm": round(float(np.percentile(allv, 99)) * 1000, 2),
                                  "frac_through": round(float((allv > 0).mean()), 5)}
            if "_vis" in stats:
                stats["visible_body_worst_mm"] = round(max(stats.pop("_vis")) * 1000, 2)
            res[key] = stats
            print(sex, key, stats.get("none"), stats.get("visible_body_worst_mm", ""), f"{time.time() - t0:.0f}s", flush=True)
        report["penetration"][sex] = res
    return report


def main():
    args = sys.argv[1:]
    frames = int(args[args.index("--frames") + 1]) if "--frames" in args else 5
    libs = args[args.index("--libs") + 1].split(",") if "--libs" in args else ["ual1", "ual2", "sekiro"]
    rep = run(libs, frames, "--quick" in args)
    path = os.path.join(ASSETS, "characters", "hero", "verify_report.json")
    with open(path, "w") as f:
        json.dump(rep, f, indent=1)
    print(json.dumps({"static_worst": rep["static_worst"], "all_static_ok": all(v["ok"] for v in rep["static"].values()),
                      "frames": rep["frames"], "clips": rep["clips"]}, indent=1))


if __name__ == "__main__":
    main()
