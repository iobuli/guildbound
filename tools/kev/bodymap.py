"""Kev body map: a rig's skinned mesh cut into Kev body parts, with anatomical frames, surface landmarks and facings,
so a pose can be asked "where is the heel, which way does the sole face" and answered in numbers.

    python3 -I tools/kev/bodymap.py build <rig.glb> [--out staging/kev/bodymap]
    python3 -I tools/kev/bodymap.py query <bodymap.json> <pose.json> [--ref ref.json --set NAME --phase PHASE]

build writes `<out>/<rig>.json` (summary) and `<out>/<rig>.npz` (every vertex: rest position and normal, both UV sets,
joints and weights, dominant part, primitive, triangles). Nothing is thrown away: the data is meant to be complete.
- Parts: each vertex belongs to its heaviest joint; each joint carries its Kev role / side / region (`nodes.py`).
  Per part: vertex count, surface area, centroid, PCA axes and spread, radius profile along the bone, UV bounds.
- Body frame (Kev body algebra): up = head minus the mean of the feet; forward = the toes' direction on the ground,
  checked against the nose; left = up x forward. Height = the crown (highest vertex), floor = the lowest vertex.
- Anatomical frame per joint, at rest, in world: `along` (head toward child), `front` (body forward made
  perpendicular to along; for the feet and toes, which point forward, `front` is up = the instep), `side`.
- Joint rings: the mannequin's ring primitive (M_Joints) cut by joint gives each joint's surface centre, ring axis
  (smallest spread) and radius.
- Landmarks: surface vertices (crown, nose, chest, upper back, belly, pelvis front, butt, and per side kneecap,
  heel, toe tip, instep, elbow tip, fingertip, shoulder top) and joints (hip, knee, ankle, ball, shoulder, elbow,
  wrist, knuckle = middle_01). A pose moves vertex landmarks by linear blend skinning and joint landmarks with
  their joint, so they are exact for any pose.
- Facings: named directions carried by a joint (face, chest, pelvis, kneecap, toes, sole, palm, back of hand),
  measured from the mesh where the mesh says it (sole = area-weighted normal of the downward foot faces, palm =
  of the downward hand faces in the T-pose, face = toward the nose) and from the anatomical frame otherwise.

Pose file (`query`): {"deltas": {joint: 16 floats, row-major}} = each joint's world transform now times its rest
world transform inverted, in glTF space (+Y up, the rig faces +Z). `tools/blender/karate.py --dump` writes them.
query prints every landmark (m, and in heights over the floor / along forward), every facing (a vector and the
nearest of up/down/forward/back/left/right in the reference view), and, with `--ref`, the comparison below.

Reference (`--ref`): 2D keypoints read off a side-view picture (pixels), per set and phase, with the picture's
floor and head-top rows and which image direction is the figure's forward. Both bodies are put in height units
(crown to floor); ours is projected on the same view (forward, up). Reported: each keypoint's error in heights,
each segment's angle error in degrees (hip-knee, knee-ankle, ankle-toe, hip-shoulder, shoulder-elbow,
elbow-fist, neck-crown), and each expected facing's angle from what is wanted.
"""
import argparse
import json
import math
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import nodes  # noqa: E402
import statue  # noqa: E402

OUT = "staging/kev/bodymap"
WORDS = ("up", "down", "forward", "back", "left", "right")


def fl(js, blob, i):
    """A float accessor as a float64 array (count, comps); normalized integer weights are scaled to 0..1."""
    arr, acc = statue.accessor_view(js, blob, i)
    if arr is None:
        raw = statue.int_accessor(js, blob, i).astype(float)
        return raw / {5121: 255.0, 5123: 65535.0}.get(acc["componentType"], 1.0)
    return arr.astype(float)


def unit(v):
    n = np.linalg.norm(v)
    return v / n if n > 1e-9 else v


def perp(v, axis):
    return unit(v - axis * np.dot(v, axis))


def node_worlds(js):
    """Rest world matrix of every node (column-vector convention)."""
    n = len(js["nodes"])
    parent = [None] * n
    for i, nd in enumerate(js["nodes"]):
        for c in nd.get("children", []):
            parent[c] = i
    world = [None] * n

    def get(i):
        if world[i] is None:
            local = statue.trs(js["nodes"][i])
            world[i] = local if parent[i] is None else get(parent[i]) @ local
        return world[i]

    return [get(i) for i in range(n)]


def triangles_area(pos, tri):
    a, b, c = pos[tri[:, 0]], pos[tri[:, 1]], pos[tri[:, 2]]
    area = 0.5 * np.linalg.norm(np.cross(b - a, c - a), axis=1)
    vert_area = np.zeros(len(pos))
    for k in range(3):
        np.add.at(vert_area, tri[:, k], area / 3)
    return vert_area


def build(path, out):
    js, blob = statue.read_glb(path)
    rep = nodes.parse(path)
    skin = js["skins"][0]
    joints = skin["joints"]
    jn = [js["nodes"][j].get("name", f"node{j}") for j in joints]
    table = {row[1]: row for row in rep["nodes"]}
    ibm = fl(js, blob, skin["inverseBindMatrices"]).reshape(-1, 4, 4).transpose(0, 2, 1)
    worlds = node_worlds(js)
    jworld = np.array([worlds[j] for j in joints])
    skinm = jworld @ ibm  # rest skinning matrices (identity when rest = bind)

    # Every skinned primitive, concatenated.
    P, N, UV0, UV1, J, W, PRIM, TRI = [], [], [], [], [], [], [], []
    base = 0
    prims = []
    for mi, mesh in enumerate(js["meshes"]):
        for pi, pr in enumerate(mesh["primitives"]):
            a = pr["attributes"]
            if "JOINTS_0" not in a:
                continue
            pos = fl(js, blob, a["POSITION"]).reshape(-1, 3)
            nrm = fl(js, blob, a["NORMAL"]).reshape(-1, 3)
            jj = statue.int_accessor(js, blob, a["JOINTS_0"]).reshape(-1, 4)
            ww = fl(js, blob, a["WEIGHTS_0"]).reshape(-1, 4)
            uv0 = fl(js, blob, a["TEXCOORD_0"]).reshape(-1, 2) if "TEXCOORD_0" in a else np.zeros((len(pos), 2))
            uv1 = fl(js, blob, a["TEXCOORD_1"]).reshape(-1, 2) if "TEXCOORD_1" in a else np.zeros((len(pos), 2))
            idx = statue.int_accessor(js, blob, pr["indices"]).reshape(-1, 3) if "indices" in pr else np.arange(len(pos)).reshape(-1, 3)
            mat = js["materials"][pr["material"]].get("name", str(pr["material"])) if "material" in pr else ""
            prims.append({"mesh": mesh.get("name", str(mi)), "primitive": pi, "material": mat, "first": base, "count": len(pos)})
            P.append(pos), N.append(nrm), UV0.append(uv0), UV1.append(uv1), J.append(jj), W.append(ww)
            PRIM.append(np.full(len(pos), len(prims) - 1)), TRI.append(idx + base)
            base += len(pos)
    P, N, UV0, UV1 = np.concatenate(P), np.concatenate(N), np.concatenate(UV0), np.concatenate(UV1)
    J, W, PRIM, TRI = np.concatenate(J), np.concatenate(W), np.concatenate(PRIM), np.concatenate(TRI)
    W = W / np.maximum(W.sum(1, keepdims=True), 1e-9)
    # Rest pose by linear blend skinning.
    M = np.einsum("vk,vkij->vij", W, skinm[J])
    rest = np.einsum("vij,vj->vi", M[:, :3, :3], P) + M[:, :3, 3]
    rnrm = np.array([unit(v) for v in np.einsum("vij,vj->vi", M[:, :3, :3], N)])
    part = J[np.arange(len(J)), W.argmax(1)]
    area = triangles_area(rest, TRI)

    head = {n: jworld[k][:3, 3] for k, n in enumerate(jn)}
    child = {}
    parents = [table[n][2] if n in table else None for n in jn]
    for k, p in enumerate(parents):
        if p is not None:
            child.setdefault(jn[p], []).append(jn[k])

    # Body frame from Kev's algebra, forward from the toes, height from the crown.
    up = unit(np.array(rep["up_axis"], dtype=float))
    floor = float((rest @ up).min())
    crown_i = int(np.argmax(rest @ up))
    height = float(rest[crown_i] @ up) - floor
    toes = [head["ball_" + s] - head["foot_" + s] for s in "lr" if "ball_" + s in head]
    forward = perp(np.mean(toes, axis=0), up)
    left = unit(np.cross(up, forward))

    def frame_of(n):
        h = head[n]
        kids = [c for c in child.get(n, []) if table.get(c, [0, 0, 0, "helper"])[3] != "helper"] or child.get(n, [])
        along = unit(np.mean([head[c] for c in kids], axis=0) - h) if kids else unit(jworld[jn.index(n)][:3, 1])
        if abs(np.dot(along, forward)) > 0.7:
            front = perp(up, along)
        else:
            front = perp(forward, along)
        return along, front, unit(np.cross(along, front))

    parts = {}
    for k, n in enumerate(jn):
        sel = np.where(part == k)[0]
        role, region = (table[n][3], table[n][4]) if n in table else ("", "")
        side = "l" if n.endswith("_l") else "r" if n.endswith("_r") else ""
        along, front, sidev = frame_of(n)
        entry = {"index": k, "role": role, "side": side, "region": region, "parent": jn[parents[k]] if parents[k] is not None else None,
                 "head": head[n].round(5).tolist(), "frame": {"along": along.round(5).tolist(), "front": front.round(5).tolist(), "side": sidev.round(5).tolist()},
                 "vertices": int(len(sel))}
        if len(sel):
            v = rest[sel]
            c = np.average(v, axis=0, weights=area[sel] + 1e-12)
            _, s, vt = np.linalg.svd(v - c, full_matrices=False)
            t = (v - head[n]) @ along
            r = np.linalg.norm((v - head[n]) - np.outer(t, along), axis=1)
            bins = np.linspace(t.min(), t.max(), 6)
            prof = [float(np.median(r[(t >= a) & (t <= b)])) if np.any((t >= a) & (t <= b)) else 0.0 for a, b in zip(bins[:-1], bins[1:])]
            entry.update({"area": round(float(area[sel].sum()), 6), "centroid": c.round(5).tolist(),
                          "pca_axes": vt.round(4).tolist(), "pca_spread": (s / max(len(sel), 1) ** 0.5).round(5).tolist(),
                          "extent_along": [round(float(t.min()), 4), round(float(t.max()), 4)], "radius_profile": [round(x, 4) for x in prof],
                          "uv0_bounds": [UV0[sel].min(0).round(4).tolist(), UV0[sel].max(0).round(4).tolist()]})
        parts[n] = entry

    # Joint rings: the ring primitive cut by joint.
    rings = {}
    ring_prims = [i for i, p in enumerate(prims) if "joint" in p["material"].lower()]
    for k, n in enumerate(jn):
        sel = np.where((part == k) & np.isin(PRIM, ring_prims))[0]
        if len(sel) < 12:
            continue
        v = rest[sel]
        c = v.mean(0)
        _, s, vt = np.linalg.svd(v - c, full_matrices=False)
        rings[n] = {"centre": c.round(5).tolist(), "axis": vt[2].round(4).tolist(), "radius": round(float(np.median(np.linalg.norm(v - c, axis=1))), 4), "vertices": int(len(sel))}

    def pick(names, score, where=None):
        ks = [jn.index(x) for x in names if x in jn]
        sel = np.where(np.isin(part, ks))[0]
        if where is not None:
            sel = sel[where(rest[sel])]
        return int(sel[np.argmax(score(rest[sel]))]) if len(sel) else None

    def near(j, r):
        return lambda v: np.linalg.norm(v - head[j], axis=1) < r

    lm = {"crown": {"vertex": crown_i}}
    hj = head.get("Head", head.get("head"))
    lm["nose"] = {"vertex": pick(["Head", "neck_01"], lambda v: v @ forward, lambda v: v @ up > hj @ up)}
    lm["chest"] = {"vertex": pick(["spine_03"], lambda v: v @ forward)}
    lm["upper_back"] = {"vertex": pick(["spine_03"], lambda v: -(v @ forward))}
    lm["belly"] = {"vertex": pick(["spine_01", "spine_02"], lambda v: v @ forward)}
    lm["pelvis_front"] = {"vertex": pick(["pelvis"], lambda v: v @ forward)}
    lm["butt"] = {"vertex": pick(["pelvis", "thigh_l", "thigh_r"], lambda v: -(v @ forward), lambda v: v @ up > head["thigh_l"] @ up - 0.12)}
    lm["neck"] = {"joint": "neck_01"}
    lm["head_joint"] = {"joint": "Head"}
    lm["pelvis"] = {"joint": "pelvis"}
    for s in "lr":
        sgn = 1 if s == "l" else -1
        lm[f"hip_{s}"] = {"joint": f"thigh_{s}"}
        lm[f"knee_{s}"] = {"joint": f"calf_{s}"}
        lm[f"ankle_{s}"] = {"joint": f"foot_{s}"}
        lm[f"ball_{s}"] = {"joint": f"ball_{s}"}
        lm[f"shoulder_{s}"] = {"joint": f"upperarm_{s}"}
        lm[f"elbow_{s}"] = {"joint": f"lowerarm_{s}"}
        lm[f"wrist_{s}"] = {"joint": f"hand_{s}"}
        lm[f"knuckle_{s}"] = {"joint": f"middle_01_{s}"}
        lm[f"kneecap_{s}"] = {"vertex": pick([f"thigh_{s}", f"calf_{s}"], lambda v: v @ forward, near(f"calf_{s}", 0.07))}
        lm[f"heel_{s}"] = {"vertex": pick([f"foot_{s}"], lambda v: -(v @ forward) - 0.3 * (v @ up))}
        lm[f"toe_tip_{s}"] = {"vertex": pick([f"ball_{s}", f"foot_{s}"], lambda v: v @ forward)}
        lm[f"instep_{s}"] = {"vertex": pick([f"foot_{s}"], lambda v: v @ up, lambda v: (v - head[f"foot_{s}"]) @ forward > 0.03)}
        lm[f"elbow_tip_{s}"] = {"vertex": pick([f"upperarm_{s}", f"lowerarm_{s}"], lambda v: -(v @ forward), near(f"lowerarm_{s}", 0.06))}
        lm[f"fingertip_{s}"] = {"vertex": pick([f"middle_03_{s}"], lambda v: sgn * (v @ left))}
        lm[f"shoulder_top_{s}"] = {"vertex": pick([f"clavicle_{s}", f"upperarm_{s}"], lambda v: v @ up, near(f"upperarm_{s}", 0.09))}
    for name, d in lm.items():
        if "vertex" in d and d["vertex"] is not None:
            i = d["vertex"]
            d["part"] = jn[part[i]]
            d["rest"] = rest[i].round(5).tolist()
            d["joints"] = [jn[j] for j in J[i]]
            d["weights"] = W[i].round(4).tolist()
            d["local"] = P[i].round(6).tolist()
        elif "joint" in d:
            d["rest"] = head[d["joint"]].round(5).tolist()

    # Facings, carried by a joint (rest world vectors).
    def mean_normal(names, cond):
        ks = [jn.index(x) for x in names if x in jn]
        sel = np.where(np.isin(part, ks))[0]
        sel = sel[cond(rnrm[sel])]
        return unit((rnrm[sel] * area[sel, None]).sum(0)) if len(sel) else None

    face = perp(rest[lm["nose"]["vertex"]] - hj, unit(np.array(parts["Head"]["frame"]["along"]))) if lm["nose"]["vertex"] is not None else forward
    fac = {"face": {"joint": "Head", "rest": face.tolist(), "from": "mesh"},
           "chest": {"joint": "spine_03", "rest": parts["spine_03"]["frame"]["front"]},
           "pelvis": {"joint": "pelvis", "rest": forward.tolist()}}
    for s in "lr":
        sgn = 1 if s == "l" else -1
        fac[f"kneecap_{s}"] = {"joint": f"calf_{s}", "rest": parts[f"calf_{s}"]["frame"]["front"]}
        fac[f"toes_{s}"] = {"joint": f"foot_{s}", "rest": forward.tolist()}
        sole = mean_normal([f"foot_{s}", f"ball_{s}"], lambda n: n @ up < -0.7)
        fac[f"sole_{s}"] = {"joint": f"foot_{s}", "rest": (sole if sole is not None else -up).tolist(), "from": "mesh"}
        palm = mean_normal([f"hand_{s}"], lambda n: n @ up < -0.5)
        fac[f"palm_{s}"] = {"joint": f"hand_{s}", "rest": (palm if palm is not None else -up).tolist(), "from": "mesh"}
        fac[f"back_of_hand_{s}"] = {"joint": f"hand_{s}", "rest": (-(palm if palm is not None else -up)).tolist()}
        fac[f"thumb_side_{s}"] = {"joint": f"hand_{s}", "rest": forward.tolist()}
        fac[f"elbow_point_{s}"] = {"joint": f"lowerarm_{s}", "rest": (-forward).tolist()}
    for f in fac.values():
        f["rest"] = [round(float(x), 5) for x in f["rest"]]

    rig = os.path.splitext(os.path.basename(path))[0]
    os.makedirs(out, exist_ok=True)
    summary = {"rig": rig, "path": path, "joints": jn, "inverse_bind": ibm.round(7).tolist(), "rest_world": jworld.round(7).tolist(),
               "body": {"up": up.round(5).tolist(), "forward": forward.round(5).tolist(), "left": left.round(5).tolist(),
                        "floor": round(floor, 5), "height": round(height, 5), "kev": {k: v for k, v in rep.items() if k not in ("nodes",)}},
               "primitives": prims, "parts": parts, "rings": rings, "landmarks": lm, "facings": fac,
               "vertices": int(len(rest)), "triangles": int(len(TRI))}
    with open(os.path.join(out, rig + ".json"), "w") as f:
        json.dump(summary, f, indent=1)
    np.savez_compressed(os.path.join(out, rig + ".npz"), mesh_position=P, rest_position=rest, rest_normal=rnrm, uv0=UV0, uv1=UV1,
                        joints=J, weights=W, part=part, primitive=PRIM, triangles=TRI, vertex_area=area)
    return summary


# --- Query ---------------------------------------------------------------------------------------------------------

def load_pose(path, bm):
    with open(path) as f:
        pose = json.load(f)
    jn = bm["joints"]
    D = np.repeat(np.eye(4)[None], len(jn), 0)
    for name, m in pose["deltas"].items():
        if name in jn:
            D[jn.index(name)] = np.array(m, dtype=float).reshape(4, 4)
    return D


def posed_landmarks(bm, D):
    jn = bm["joints"]
    ibm = np.array(bm["inverse_bind"])
    rw = np.array(bm["rest_world"])
    skinm = D @ rw @ ibm
    out = {}
    for name, d in bm["landmarks"].items():
        if d.get("vertex") is not None and "local" in d:
            m = sum(w * skinm[jn.index(j)] for j, w in zip(d["joints"], d["weights"]))
            out[name] = m[:3, :3] @ np.array(d["local"]) + m[:3, 3]
        elif "joint" in d and d["joint"] in jn:
            out[name] = (D[jn.index(d["joint"])] @ np.append(d["rest"], 1.0))[:3]
    return out


def posed_facings(bm, D):
    jn = bm["joints"]
    return {name: unit(D[jn.index(f["joint"])][:3, :3] @ np.array(f["rest"])) for name, f in bm["facings"].items() if f["joint"] in jn}


def word(v, fwd, up, left):
    dirs = {"up": up, "down": -up, "forward": fwd, "back": -fwd, "left": left, "right": -left}
    best = max(dirs, key=lambda k: np.dot(v, dirs[k]))
    return best, math.degrees(math.acos(max(-1.0, min(1.0, float(np.dot(v, dirs[best]))))))


# Winter (2009) segment masses (share of body mass) and centre-of-mass fractions from the proximal landmark.
MASS = (("head_joint", "crown", 0.5, 0.081), ("trunk_low", "trunk_high", 0.5, 0.497),
        ("shoulder_{s}", "elbow_{s}", 0.436, 0.028), ("elbow_{s}", "wrist_{s}", 0.43, 0.016), ("wrist_{s}", "knuckle_{s}", 0.506, 0.006),
        ("hip_{s}", "knee_{s}", 0.433, 0.100), ("knee_{s}", "ankle_{s}", 0.433, 0.0465), ("ankle_{s}", "ball_{s}", 0.5, 0.0145))


def balance(bm, L):
    """Centre of mass (Winter segments on the posed landmarks) against the planted (lower) foot: offsets along
    forward and left in heights, and whether it lies over the foot (heel to toe tip, 0.03 heights each side)."""
    L = dict(L)
    L["trunk_low"] = (L["hip_l"] + L["hip_r"]) / 2
    L["trunk_high"] = (L["shoulder_l"] + L["shoulder_r"]) / 2
    com, total = np.zeros(3), 0.0
    for a, b, f, m in MASS:
        for s in ("l", "r") if "{s}" in a else ("",):
            pa, pb = L[a.format(s=s)], L[b.format(s=s)]
            com += m * (pa + f * (pb - pa))
            total += m
    com /= total
    up, fwd = np.array(bm["body"]["up"]), np.array(bm["body"]["forward"])
    left = np.cross(up, fwd)
    H = bm["body"]["height"]
    floor = bm["body"]["floor"]
    low = min(float(L[f"ankle_{x}"] @ up) for x in "lr")
    # Feet on the ground: both when both ankles are within 3 cm of the lower one.
    down = [x for x in "lr" if float(L[f"ankle_{x}"] @ up) - low < 0.03]
    pts = [L[f"{k}_{x}"] for x in down for k in ("heel", "toe_tip")]
    g = lambda v: np.array([float(v @ fwd), float(v @ left)])
    poly = hull([g(v) for v in pts])
    c2 = g(com)
    inside, dist = in_hull(poly, c2)
    centre = np.mean([g(v) for v in pts], axis=0)
    off = 0.0 if inside else dist
    return {"com": com, "support": "+".join(down), "forward": float(c2[0] - centre[0]) / H, "left": float(c2[1] - centre[1]) / H,
            "off_foot": off / H, "over_foot": bool(inside or off / H < 0.03)}


def hull(points):
    """2D convex hull (monotone chain), counter-clockwise."""
    pts = sorted({(round(float(p[0]), 6), round(float(p[1]), 6)) for p in points})
    if len(pts) < 3:
        return [np.array(p) for p in pts]
    cross = lambda o, a, b: (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return [np.array(p) for p in lower[:-1] + upper[:-1]]


def in_hull(poly, p):
    """(inside, distance to the outline) for a point against a convex outline (a segment or a point too)."""
    def seg(a, b):
        ab = b - a
        t = np.clip(np.dot(p - a, ab) / max(np.dot(ab, ab), 1e-12), 0, 1)
        return float(np.linalg.norm(p - (a + t * ab)))
    if len(poly) == 1:
        return False, float(np.linalg.norm(p - poly[0]))
    edges = list(zip(poly, poly[1:] + poly[:1]))
    dist = min(seg(a, b) for a, b in edges)
    if len(poly) < 3:
        return False, dist
    inside = all((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) >= 0 for a, b in edges)
    return inside, dist


SEGMENTS = (("hip", "knee"), ("knee", "ankle"), ("ankle", "toe"), ("hip", "shoulder"), ("shoulder", "elbow"), ("elbow", "fist"), ("neck", "crown"))


def compare(bm, L, F, ref, set_name, phase):
    """Our projected body against the reference keypoints, both in heights (crown to floor)."""
    r = ref["sets"][set_name]
    k = r["phases"][phase]
    view = ref.get("view", {})
    fwd = unit(np.array(view.get("forward", bm["body"]["forward"]), dtype=float))
    up = unit(np.array(bm["body"]["up"], dtype=float))
    H = bm["body"]["height"]
    floor = bm["body"]["floor"]
    # Reference keypoints in heights: x along the figure's forward, y up from the floor.
    top, bottom = k.get("top", r.get("top")), k.get("floor", r.get("floor"))
    # A leaning or crouching figure is shorter than it stands: `height_px` gives its standing height.
    hpx = k.get("height_px", r.get("height_px", bottom - top))
    sgn = -1 if r.get("forward", "left") == "left" else 1
    pts = {n: np.array([sgn * (p[0]) / hpx, (bottom - p[1]) / hpx]) for n, p in k["points"].items()}
    names = {n: v for n, v in r["landmarks"].items()}  # ref keypoint -> our landmark
    ours = {n: np.array([(L[m] @ fwd) / H, (L[m] @ up - floor) / H]) for n, m in names.items() if m in L}
    anchor = r.get("anchor", "ankle_s")
    off = ours[anchor] - pts[anchor] if anchor in ours and anchor in pts else np.zeros(2)
    rows = []
    for n, p in pts.items():
        if n in ours:
            o = ours[n] - off
            rows.append((n, p, o, float(np.linalg.norm(o - p))))
    segs = []
    for side in ("k", "s"):
        for a, b in SEGMENTS:
            na, nb = f"{a}_{side}" if f"{a}_{side}" in pts else a, f"{b}_{side}" if f"{b}_{side}" in pts else b
            if na in pts and nb in pts and na in ours and nb in ours and (na, nb) not in [(x[0], x[1]) for x in segs]:
                dp, do = pts[nb] - pts[na], ours[nb] - ours[na]
                ang = math.degrees(math.atan2(dp[1], dp[0]) - math.atan2(do[1], do[0]))
                ang = (ang + 180) % 360 - 180
                segs.append((na, nb, ang))
    left = unit(np.cross(up, fwd))
    facing = []
    for name, want in k.get("facing", {}).items():
        if name in F:
            got, off_deg = word(F[name], fwd, up, left)
            dirs = {"up": up, "down": -up, "forward": fwd, "back": -fwd, "left": left, "right": -left}
            err = math.degrees(math.acos(max(-1.0, min(1.0, float(np.dot(F[name], dirs[want]))))))
            facing.append((name, want, got, err))
    return rows, segs, facing


def track(bm, paths, fps, kick):
    """A motion, frame by frame: the kicking foot's speed (ball and toe tip), how far the support foot slides, and
    where the centre of mass sits whenever only one foot is down."""
    up = np.array(bm["body"]["up"])
    floor = bm["body"]["floor"]
    sup = "l" if kick == "r" else "r"
    frames = [posed_landmarks(bm, load_pose(p, bm)) for p in paths]
    start = {"ankle": frames[0][f"ankle_{sup}"]}
    fwd0 = frames[0][f"toe_tip_{sup}"] - frames[0][f"heel_{sup}"]
    rows = []
    for i, L in enumerate(frames):
        prev = frames[max(i - 1, 0)]
        nxt = frames[min(i + 1, len(frames) - 1)]
        span = (min(i + 1, len(frames) - 1) - max(i - 1, 0)) / fps
        speed = {k: float(np.linalg.norm(nxt[f"{k}_{kick}"] - prev[f"{k}_{kick}"]) / span) if span else 0.0 for k in ("ball", "toe_tip", "ankle")}
        # Slip = the support ankle sliding; turning the foot on the spot (a pivot) is reported apart.
        slip = float(np.linalg.norm(L[f"ankle_{sup}"] - start["ankle"]))
        f_now = L[f"toe_tip_{sup}"] - L[f"heel_{sup}"]
        a, b = fwd0 - up * (fwd0 @ up), f_now - up * (f_now @ up)
        pivot = math.degrees(math.atan2(float(np.cross(a, b) @ up), float(a @ b)))
        lifted = min(float(L[f"{k}_{kick}"] @ up) - floor for k in ("heel", "toe_tip", "ball")) > 0.03
        bal = balance(bm, L)
        rows.append({"frame": i + 1, "ball_mps": speed["ball"], "toe_mps": speed["toe_tip"], "ankle_mps": speed["ankle"],
                     "support_slip_m": slip, "pivot_deg": pivot, "single_support": lifted, "com_over_foot": bal["over_foot"], "com_off_h": bal["off_foot"],
                     "ankle_height_m": float(L[f"ankle_{kick}"] @ up) - floor})
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    t = sub.add_parser("track")
    t.add_argument("bodymap")
    t.add_argument("poses", nargs="+")
    t.add_argument("--fps", type=float, default=60.0)
    t.add_argument("--kick", default="r")
    b = sub.add_parser("build")
    b.add_argument("glb")
    b.add_argument("--out", default=OUT)
    q = sub.add_parser("query")
    q.add_argument("bodymap")
    q.add_argument("pose")
    q.add_argument("--ref")
    q.add_argument("--set")
    q.add_argument("--phase")
    q.add_argument("--json", action="store_true")
    a = ap.parse_args()
    if a.cmd == "build":
        s = build(a.glb, a.out)
        print(f"bodymap {s['rig']}: {s['vertices']} vertices, {s['triangles']} triangles, {len(s['parts'])} parts, {len(s['rings'])} rings, "
              f"{len(s['landmarks'])} landmarks, {len(s['facings'])} facings; height {s['body']['height']:.4f} m, forward {s['body']['forward']}")
        return
    with open(a.bodymap) as f:
        bm = json.load(f)
    if a.cmd == "track":
        rows = track(bm, sorted(a.poses), a.fps, a.kick)
        print("frame  ball m/s  toe m/s  ankle h  slip mm  1-foot  CoM")
        for r in rows:
            print(f"{r['frame']:5d}  {r['ball_mps']:8.2f}  {r['toe_mps']:7.2f}  {r['ankle_height_m']:7.3f}  {r['support_slip_m'] * 1000:7.1f}  "
                  f"{'yes' if r['single_support'] else '   '}     {'over' if r['com_over_foot'] else 'OFF %.3f' % r['com_off_h']}")
        peak = max(rows, key=lambda r: r["ball_mps"])
        single = [r for r in rows if r["single_support"]]
        print(f"peak ball-of-foot speed {peak['ball_mps']:.1f} m/s at frame {peak['frame']}; support foot slip max "
              f"{max(r['support_slip_m'] for r in rows) * 1000:.1f} mm (pivot up to {max(abs(r['pivot_deg']) for r in rows):.0f} deg); one-foot frames {len(single)}, CoM off the foot in "
              f"{sum(1 for r in single if not r['com_over_foot'])}")
        return
    D = load_pose(a.pose, bm)
    L, F = posed_landmarks(bm, D), posed_facings(bm, D)
    fwd, up = np.array(bm["body"]["forward"]), np.array(bm["body"]["up"])
    left = np.cross(up, fwd)
    if a.ref:
        with open(a.ref) as f:
            ref = json.load(f)
        rows, segs, facing = compare(bm, L, F, ref, a.set, a.phase)
        if a.json:
            bal = balance(bm, L)
            bal["com"] = bal["com"].tolist()
            print(json.dumps({"points": [(n, p.tolist(), o.tolist(), e) for n, p, o, e in rows], "segments": segs, "facing": facing,
                              "balance": bal, "landmarks": {k: v.tolist() for k, v in L.items()}}))
            return
        print(f"{a.set} / {a.phase}: keypoints (heights; x forward, y up)")
        for n, p, o, e in rows:
            print(f"  {n:<12} ref {p[0]:+.3f},{p[1]:.3f}  ours {o[0]:+.3f},{o[1]:.3f}  err {e:.3f}")
        print("  mean keypoint error %.3f heights" % (sum(r[3] for r in rows) / max(len(rows), 1)))
        print("segments (ref minus ours, degrees):")
        for na, nb, d in segs:
            print(f"  {na:>11} -> {nb:<11} {d:+6.1f}")
        print("facings:")
        for name, want, got, err in facing:
            print(f"  {name:<14} want {want:<8} got {got:<8} off {err:5.1f} deg")
        bal = balance(bm, L)
        print(f"balance: centre of mass {'OVER' if bal['over_foot'] else 'OFF'} the {bal['support']} foot; from the foot's centre "
              f"{bal['forward']:+.3f} forward, {bal['left']:+.3f} left; {bal['off_foot']:.3f} heights from the foot line")
        return
    H, floor = bm["body"]["height"], bm["body"]["floor"]
    for n, p in L.items():
        print(f"{n:<16} {p.round(4).tolist()}  height {(p @ up - floor) / H:.3f}  forward {(p @ fwd) / H:+.3f}")
    for n, v in F.items():
        w, d = word(v, fwd, up, left)
        print(f"facing {n:<14} {v.round(3).tolist()}  ~{w} ({d:.0f} deg)")


if __name__ == "__main__":
    main()
