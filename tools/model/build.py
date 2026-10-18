"""Build the hero character kit from the Quaternius base characters, on ual2's skeleton.

    python -I tools/model/build.py            # writes assets/characters/hero/** and assets/anim/hero_styled.glb

Inputs (never modified): assets/anim/ual2.glb (skeleton, inverse bind matrices),
assets/characters/base/*.gltf (CC0 Quaternius base characters, hair, eyebrows, textures).

Outputs:
  assets/characters/hero/{male,female}/body.glb   skeleton + body split into `body` and
      `body_hide_<garments>` nodes (hidden by the game when one of those garments is worn) + eyes;
      morph targets `bulk heavy slim jaw` (body-shape parameters)
  assets/characters/hero/{male,female}/<part>.glb hair_*, brows_*, beard, and garments
      tunic shirt trousers boots shoes gloves belt baldric (each skinned to the same 65 joints,
      same inverse bind matrices, garments carry the same morph targets as the body)
  assets/characters/hero/tex/*.png                skin (neutral, multiplied by the skin tone),
      normal maps, hair, eye colours
  assets/anim/hero_styled.glb                     one self-contained file: the default male
      look (body + clothes + hair) for `GB_HERO=styled`

Every step is deterministic. See docs/MODEL_PARTS.md for the numbers and the design.
"""
from __future__ import annotations

import colorsys
import io
import json
import os
import sys

import numpy as np
from PIL import Image
from scipy.spatial import cKDTree

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gbmesh as gm  # noqa: E402
import rebind  # noqa: E402
import surface as sf  # noqa: E402
import drape  # noqa: E402
import textures as tx  # noqa: E402

ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
ASSETS = os.path.join(ROOT, "assets")
BASE_DIR = os.path.join(ASSETS, "characters", "base")
OUT = os.path.join(ASSETS, "characters", "hero")
MORPHS = ["bulk", "heavy", "slim", "jaw"]
HEAD_SCALE = 1.05  # stylised proportion: a slightly larger head reads better at game distance

REF = gm.Doc(os.path.join(ASSETS, "anim", "ual2.glb"))
RS = gm.Skin(REF)
NAMES = RS.names
NJ = len(NAMES)
JW = RS.rest_world()
J = {n: JW[i][:3, 3] for i, n in enumerate(NAMES)}


def ji(name):
    return NAMES.index(name)


def wsum(wd, names):
    return sum(wd[:, ji(n)] for n in names)


def smoothstep(a, b, x):
    t = np.clip((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


FINGER_SETS = {s: {"01": [f"{f}_01_{s}" for f in ["index", "middle", "ring", "pinky"]],
                   "far": [f"{f}_{k}_{s}" for f in ["index", "middle", "ring", "pinky"] for k in ["02", "03", "04_leaf"]]
                   + [f"thumb_{k}_{s}" for k in ["03", "04_leaf"]],
                   "thumb2": [f"thumb_02_{s}"]} for s in "lr"}


# ------------------------------------------------------------------------------------------
# Sources

def material_kind(doc, prim):
    name = doc.g.materials[prim.material].name
    if "Eye" in name:
        return "eyes"
    if "Hair" in name:
        return "hair"
    return "body"


def tex_bytes(doc, mat_index, slot="base"):
    m = doc.g.materials[mat_index]
    ti = m.pbrMetallicRoughness.baseColorTexture if slot == "base" else m.normalTexture
    if ti is None:
        return None
    return doc.image_bytes(doc.g.images[doc.g.textures[ti.index].source])[0]


def head_scale(arr):
    """Polygon edit: grow everything weighted to the head about the head joint."""
    wd = sf.dense_weights(arr["joints"], arr["weights"], NJ)
    k = wd[:, ji("Head")] * (HEAD_SCALE - 1.0)
    out = dict(arr)
    out["pos"] = arr["pos"] + k[:, None] * (arr["pos"] - J["Head"][None, :])
    return out


def load_rebound(path):
    doc = gm.Doc(path)
    skin = gm.Skin(doc)
    rb = rebind.Rebinder(skin, RS)
    prims = []
    for m in doc.g.meshes:
        for p in m.primitives:
            a = rebind.rebind_prim(rb, gm.prim_arrays(doc, p))
            a = head_scale(a)
            a["kind"] = material_kind(doc, p)
            a["mat"] = p.material
            prims.append(a)
    return doc, skin, prims


# ------------------------------------------------------------------------------------------
# Body: welded surface, morph targets, hide groups

class Body:
    def __init__(self, sex):
        self.sex = sex
        fname = "Superhero_Male_FullBody.gltf" if sex == "male" else "Superhero_Female_FullBody.gltf"
        self.doc, self.src_skin, prims = load_rebound(os.path.join(BASE_DIR, fname))
        self.body = next(p for p in prims if p["kind"] == "body")
        self.eyes = next(p for p in prims if p["kind"] == "eyes")
        self.tex_base = tex_bytes(self.doc, self.body["mat"], "base")
        self.tex_normal = tex_bytes(self.doc, self.body["mat"], "normal")
        self.tex_eye = tex_bytes(self.doc, self.eyes["mat"], "base")
        b = self.body
        self.wd = sf.dense_weights(b["joints"], b["weights"], NJ)
        ids = rebind.weld_ids(b["pos"])
        self.ids = ids
        n = ids.max() + 1
        first = np.zeros(n, np.int64)
        first[ids[::-1]] = np.arange(len(ids))[::-1]
        tri = ids[b["idx"]]
        tri = tri[(tri[:, 0] != tri[:, 1]) & (tri[:, 1] != tri[:, 2]) & (tri[:, 0] != tri[:, 2])]
        pos = b["pos"][first]
        nrm = sf.vertex_normals(pos, tri)
        self.nb = sf.neighbours(n, tri)
        morph = self.morph_targets(pos, nrm, self.wd[first])
        self.surf = sf.Surf(pos, nrm, self.wd[first], morph, tri)
        self.morph_unwelded = morph[ids]
        self.sole_y = float(b["pos"][:, 1].min())
        self.head_verts = pos[self.wd[first][:, ji("Head")] > 0.5]

    def morph_targets(self, pos, nrm, wd):
        """Body-shape parameters as morph targets (deltas along the smoothed normal)."""
        def joint_mask(table):
            m = np.zeros(len(pos))
            for name, v in table.items():
                for s in "lr":
                    nm = name.replace("_S", "_" + s)
                    if nm in NAMES:
                        m += wd[:, ji(nm)] * v
            return m

        bulk = joint_mask({"upperarm_S": 1.0, "lowerarm_S": 0.7, "spine_03": 0.9, "spine_02": 0.6, "clavicle_S": 0.9,
                           "spine_01": 0.3, "thigh_S": 0.9, "calf_S": 0.7, "neck_01": 0.6, "pelvis": 0.4, "hand_S": 0.15})
        front = smoothstep(-0.4, 0.6, nrm[:, 2])
        heavy = joint_mask({"spine_01": 1.0, "pelvis": 0.8, "spine_02": 0.6, "thigh_S": 0.45, "upperarm_S": 0.35,
                            "spine_03": 0.25, "calf_S": 0.15, "lowerarm_S": 0.15, "neck_01": 0.3})
        belly = joint_mask({"spine_01": 1.0, "spine_02": 0.7, "pelvis": 0.5}) * front
        slim = joint_mask({"spine_01": 1.0, "spine_02": 0.8, "spine_03": 0.6, "pelvis": 0.8, "thigh_S": 0.8, "calf_S": 0.5,
                           "upperarm_S": 0.8, "lowerarm_S": 0.5, "clavicle_S": 0.4, "neck_01": 0.4})
        hy = J["Head"][1]
        head = wd[:, ji("Head")]
        jaw = head * smoothstep(hy + 0.075, hy + 0.02, pos[:, 1]) * smoothstep(hy - 0.02, hy + 0.01, pos[:, 1]) \
            * smoothstep(J["Head"][2] - 0.01, J["Head"][2] + 0.06, pos[:, 2])
        amounts = np.stack([0.012 * bulk, 0.012 * heavy + 0.028 * belly, -0.008 * slim, 0.007 * jaw], 1)
        amounts = sf.smooth_field(amounts, self.nb, iters=6, lam=0.5)
        nrm_s = sf.smooth_field(nrm, self.nb, iters=4)
        nrm_s /= np.maximum(np.linalg.norm(nrm_s, axis=1, keepdims=True), 1e-12)
        out = amounts[:, :, None] * nrm_s[:, None, :]
        # jaw: widen sideways and forward (not up)
        out[:, 3, 1] *= 0.2
        return out


# ------------------------------------------------------------------------------------------
# Garments

def neckline(depth, width, height=1.505, radius=0.115):
    zc = J["neck_01"][2]

    def f(p, wd):
        rho = np.hypot(p[:, 0], p[:, 2] - zc)
        v = depth * np.clip(1 - np.abs(p[:, 0]) / width, 0, 1) * np.clip((p[:, 2] - zc) / 0.03, 0, 1)
        return np.maximum(height - v - p[:, 1], rho - radius)
    return f


def glove_fields(p, wd):
    out = []
    for s, sign in (("l", 1), ("r", -1)):
        fs = FINGER_SETS[s]
        fing = wsum(wd, fs["far"]) + 0.55 * wsum(wd, fs["01"]) + 0.6 * wsum(wd, fs["thumb2"])
        out.append(np.minimum(sign * p[:, 0] - 0.685, 0.5 - fing))
    return np.maximum(out[0], out[1])


def not_head(p, wd):
    """> 0 away from the head and the upper neck (keeps collars off the face)."""
    return 0.5 - wd[:, ji("Head")] - 0.6 * wd[:, ji("neck_01")]


def armness(p, wd):
    return wsum(wd, [f"{a}_{s}" for a in ["upperarm", "lowerarm", "hand"] for s in "lr"])


BALDRIC_C = np.array([-0.14, 1.50, 0.0])
_bd = np.array([0.30, -0.47, 0.0]) / np.linalg.norm([0.30, -0.47, 0.0])
BALDRIC_DIR = _bd
BALDRIC_N = np.array([_bd[1] * -1, _bd[0], 0.0])  # perpendicular in the frontal plane

GARMENTS = {
    # name: dict(fields=[f(p, wd) > 0 kept], hide=[(f, margin)], outer, inner, flare(p, wd), bead, smooth, colour,
    #            slot, material)
    "tunic": dict(
        slot="top", colour=(0.56, 0.17, 0.14), rough=0.92,
        fields=[lambda p, wd: p[:, 1] - 0.90, lambda p, wd: 0.715 - np.abs(p[:, 0]), neckline(0.075, 0.075), not_head],
        hide=[(lambda p, wd: p[:, 1] - 0.90, 0.035), (lambda p, wd: 0.715 - np.abs(p[:, 0]), 0.03),
              (neckline(0.075, 0.075), 0.02), (not_head, 0.2)],
        outer=0.015, inner=0.011, bead=0.0025, smooth=0, span=0.11,
        flare=lambda p, wd: 0.030 * smoothstep(0.985, 0.90, p[:, 1]) + 0.006 * smoothstep(0.64, 0.715, np.abs(p[:, 0]))),
    "shirt": dict(
        slot="top", colour=(0.86, 0.80, 0.66), rough=0.95,
        fields=[lambda p, wd: p[:, 1] - 0.95, lambda p, wd: 0.37 - np.abs(p[:, 0]), neckline(0.03, 0.05), not_head],
        hide=[(lambda p, wd: p[:, 1] - 0.95, 0.03), (lambda p, wd: 0.37 - np.abs(p[:, 0]), 0.025),
              (neckline(0.03, 0.05), 0.02), (not_head, 0.2)],
        outer=0.014, inner=0.011, bead=0.002, smooth=0, span=0.11,
        flare=lambda p, wd: 0.004 * smoothstep(0.31, 0.37, np.abs(p[:, 0])) + 0.008 * smoothstep(1.0, 0.95, p[:, 1])),
    "trousers": dict(
        slot="legs", colour=(0.33, 0.26, 0.19), rough=0.95,
        fields=[lambda p, wd: 1.035 - p[:, 1], lambda p, wd: p[:, 1] - 0.075],
        hide=[(lambda p, wd: 1.035 - p[:, 1], 0.03), (lambda p, wd: p[:, 1] - 0.075, 0.03)],
        outer=0.008, inner=0.004, bead=0.002, smooth=0, span=0.08, flare=None),
    "boots": dict(
        slot="feet", colour=(0.26, 0.17, 0.11), rough=0.75, sole=True,
        fields=[lambda p, wd: 0.36 - p[:, 1]],
        hide=[(lambda p, wd: 0.36 - p[:, 1], 0.025)],
        outer=0.016, inner=0.012, bead=0.003, smooth=0, span=0.06,
        flare=lambda p, wd: 0.008 * smoothstep(0.30, 0.36, p[:, 1])),
    "shoes": dict(
        slot="feet", colour=(0.30, 0.20, 0.12), rough=0.75, sole=True,
        fields=[lambda p, wd: 0.125 - 0.45 * np.maximum(0, p[:, 2] - J["foot_l"][2]) - p[:, 1]],
        hide=[(lambda p, wd: 0.125 - 0.45 * np.maximum(0, p[:, 2] - J["foot_l"][2]) - p[:, 1], 0.02)],
        outer=0.016, inner=0.012, bead=0.0025, smooth=0, span=0.06, flare=None),
    "gloves": dict(
        slot="hands", colour=(0.45, 0.31, 0.19), rough=0.8,
        fields=[glove_fields],
        hide=[(glove_fields, 0.02)],
        outer=0.005, inner=0.002, bead=0.0015, smooth=0,
        flare=lambda p, wd: 0.003 * smoothstep(0.71, 0.685, np.abs(p[:, 0]))),
    "belt": dict(
        slot="belt", colour=(0.24, 0.15, 0.09), rough=0.7, buckle=True,
        fields=[lambda p, wd: p[:, 1] - 0.995, lambda p, wd: 1.055 - p[:, 1]],
        hide=[], outer=0.030, inner=0.023, bead=0.0015, smooth=0, span=0.06, flare=None),
    "baldric": dict(
        slot="strap", colour=(0.30, 0.19, 0.11), rough=0.7,
        fields=[lambda p, wd: 0.024 - (p - BALDRIC_C) @ BALDRIC_N, lambda p, wd: 0.024 + (p - BALDRIC_C) @ BALDRIC_N,
                lambda p, wd: p[:, 1] - 1.03, lambda p, wd: 0.45 - armness(p, wd),
                lambda p, wd: 0.5 - wd[:, ji("Head")] - wd[:, ji("neck_01")]],
        hide=[], outer=0.020, inner=0.016, bead=0.001, smooth=0, span=0.06, flare=None),
}


def front_open(gap):
    """> 0 except in a strip `gap` wide down the front centre (open jackets and vests)."""
    return lambda p, wd: np.maximum(np.abs(p[:, 0]) - gap, -p[:, 2])


def face_hole(p, wd):
    """> 0 outside the face opening of a hood."""
    inside = np.minimum.reduce([p[:, 2] - 0.035, 0.08 - np.abs(p[:, 0]), p[:, 1] - 1.55, 1.765 - p[:, 1]])
    return -inside


def lowerarm_band(p, wd):
    return np.minimum.reduce([wd[:, ji("lowerarm_l")] + wd[:, ji("lowerarm_r")] + wd[:, ji("hand_l")] + wd[:, ji("hand_r")] - 0.5,
                              np.abs(p[:, 0]) - 0.50, 0.668 - np.abs(p[:, 0])])


TAU = 2 * np.pi
GARMENTS.update({
    # Class, area and world-place clothing (2026-10-18). Shells on the body surface first.
    "haori": dict(
        slot="coat", colour=(0.20, 0.22, 0.30), rough=0.9,
        fields=[lambda p, wd: p[:, 1] - 0.80, lambda p, wd: 0.69 - np.abs(p[:, 0]), neckline(0.13, 0.085), not_head, front_open(0.035)],
        hide=[], outer=0.026, inner=0.021, bead=0.003, smooth=0, span=0.11,
        flare=lambda p, wd: 0.06 * smoothstep(0.30, 0.64, np.abs(p[:, 0])) * armness(p, wd) + 0.02 * smoothstep(0.95, 0.80, p[:, 1])),
    "robe_top": dict(
        slot="top", colour=(0.22, 0.24, 0.40), rough=0.9,
        fields=[lambda p, wd: p[:, 1] - 0.93, lambda p, wd: 0.665 - np.abs(p[:, 0]), neckline(0.06, 0.06), not_head],
        hide=[(lambda p, wd: p[:, 1] - 0.93, 0.035), (lambda p, wd: 0.665 - np.abs(p[:, 0]), 0.03),
              (neckline(0.06, 0.06), 0.02), (not_head, 0.2)],
        outer=0.016, inner=0.012, bead=0.0025, smooth=0, span=0.11,
        flare=lambda p, wd: 0.03 * smoothstep(0.45, 0.66, np.abs(p[:, 0])) * armness(p, wd)),
    "vest": dict(
        slot="coat", colour=(0.36, 0.24, 0.14), rough=0.7,
        fields=[lambda p, wd: p[:, 1] - 0.97, lambda p, wd: 0.25 - armness(p, wd), neckline(0.15, 0.10), not_head, front_open(0.04)],
        hide=[], outer=0.027, inner=0.022, bead=0.0025, smooth=0, span=0.11,
        flare=lambda p, wd: 0.01 * smoothstep(1.05, 0.97, p[:, 1])),
    "mantle": dict(
        slot="cape", colour=(0.30, 0.18, 0.30), rough=0.9,
        fields=[lambda p, wd: p[:, 1] - 1.20, lambda p, wd: 0.36 - np.abs(p[:, 0]), neckline(0.04, 0.06), not_head],
        hide=[], outer=0.034, inner=0.029, bead=0.003, smooth=0, span=0.11,
        flare=lambda p, wd: 0.035 * smoothstep(1.36, 1.20, p[:, 1])),
    "hood": dict(
        slot="head", colour=(0.24, 0.28, 0.20), rough=0.92,
        fields=[lambda p, wd: p[:, 1] - 1.40, lambda p, wd: 0.21 - np.abs(p[:, 0]), lambda p, wd: 0.25 - armness(p, wd), face_hole],
        hide=[], outer=0.036, inner=0.031, bead=0.004, smooth=0, span=0.12,
        flare=lambda p, wd: 0.025 * smoothstep(1.62, 1.78, p[:, 1]) * smoothstep(0.05, -0.08, p[:, 2])),
    "bracers": dict(
        slot="wrists", colour=(0.36, 0.24, 0.14), rough=0.65,
        fields=[lowerarm_band], hide=[], outer=0.011, inner=0.007, bead=0.002, smooth=0, flare=None),
    "hakama": dict(
        slot="legs", colour=(0.18, 0.18, 0.22), rough=0.92,
        fields=[lambda p, wd: 1.06 - p[:, 1], lambda p, wd: p[:, 1] - 0.08],
        hide=[(lambda p, wd: 1.06 - p[:, 1], 0.03), (lambda p, wd: p[:, 1] - 0.08, 0.03)],
        outer=0.012, inner=0.008, bead=0.002, smooth=0, span=0.08,
        flare=lambda p, wd: 0.11 * smoothstep(0.88, 0.12, p[:, 1])),
    "leg_wraps": dict(
        slot="shins", colour=(0.62, 0.56, 0.44), rough=0.95,
        fields=[lambda p, wd: 0.47 - p[:, 1], lambda p, wd: p[:, 1] - 0.13],
        hide=[], outer=0.013, inner=0.009, bead=0.0015, smooth=0, span=0.04, flare=None),
    # Hanging cloth (`drape.py`): ring grids around the body, not body-surface shells.
    "robe_skirt": dict(slot="skirt", colour=(0.22, 0.24, 0.40), rough=0.9, hide=[], outer=0.006, inner=0.0, bead=0.002,
                       drape=dict(top=1.03, bottom=0.20, clear=0.022, flare=0.11)),
    "kilt": dict(slot="skirt", colour=(0.30, 0.26, 0.16), rough=0.92, hide=[], outer=0.006, inner=0.0, bead=0.002,
                 drape=dict(top=1.03, bottom=0.55, clear=0.02, flare=0.05)),
    "apron": dict(slot="apron", colour=(0.48, 0.36, 0.22), rough=0.8, hide=[], outer=0.005, inner=0.0, bead=0.002,
                  drape=dict(top=1.04, bottom=0.46, arc=(-1.05, 1.05), clear=0.035, flare=0.02)),
    "cloak": dict(slot="cape", colour=(0.20, 0.24, 0.18), rough=0.92, hide=[], outer=0.006, inner=0.0, bead=0.003,
                  drape=dict(top=1.45, bottom=0.30, arc=(0.85, TAU - 0.85), clear=0.075, flare=0.13, rings=26, max_x=0.23)),
    "tabard": dict(slot="tabard", colour=(0.22, 0.30, 0.52), rough=0.88, hide=[], outer=0.005, inner=0.0, bead=0.002,
                   drape=[dict(top=1.40, bottom=0.50, arc=(-0.6, 0.6), clear=0.04, flare=0.01),
                          dict(top=1.40, bottom=0.50, arc=(np.pi - 0.6, np.pi + 0.6), clear=0.04, flare=0.01)]),
})
HIDING = [g for g, d in GARMENTS.items() if d["hide"]]


def hide_mask(body: Body):
    """Per body triangle: bitmask of garments that hide it (all three vertices well inside)."""
    p = body.body["pos"]
    wd = body.wd
    tri = body.body["idx"]
    mask = np.zeros(len(tri), np.int64)
    for bit, g in enumerate(HIDING):
        ok = np.ones(len(p), bool)
        for f, margin in GARMENTS[g]["hide"]:
            ok &= f(p, wd) > margin
        mask |= (ok[tri].all(1).astype(np.int64) << bit)
    return mask


def merge_surfs(surfs):
    pos, nrm, w, morph, tri, off = [], [], [], [], [], 0
    for s in surfs:
        pos.append(s.pos); nrm.append(s.nrm); w.append(s.w); morph.append(s.morph); tri.append(s.tri + off)
        off += len(s.pos)
    return sf.Surf(np.concatenate(pos), np.concatenate(nrm), np.concatenate(w), np.concatenate(morph), np.concatenate(tri))


def make_garment(body: Body, name: str):
    g = GARMENTS[name]
    if "drape" in g:
        specs = g["drape"] if isinstance(g["drape"], list) else [g["drape"]]
        s = merge_surfs([drape.drape_surf(body, ji, J, **spec) for spec in specs])
        flare = None
    else:
        s = body.surf.copy()
        for f in g["fields"]:
            s = sf.cut(s, f(s.pos, s.w))
        # drop tiny islands (fewer than 12 triangles) left by the cuts
        s = drop_islands(s, 12)
        flare = g["flare"](s.pos, s.w) if g.get("flare") else None
    sh = sf.shell(s, g["outer"], g["inner"], smooth_iters=4, bead=g["bead"], extra=flare, span=g.get("span", 0.0))
    if g.get("sole"):
        floor = body.sole_y - 0.005
        low = sh["pos"][:, 1] < floor + 0.004
        sh["pos"][low, 1] = floor  # flat sole and heel
        sh["sole"] = sh["pos"][:, 1] < floor + 0.012
    return s, sh


def drop_islands(s: sf.Surf, min_tris):
    n = len(s.pos)
    parent = np.arange(n)

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a
    for a, b, c in s.tri:
        for x, y in ((a, b), (b, c)):
            rx, ry = find(x), find(y)
            if rx != ry:
                parent[rx] = ry
    roots = np.array([find(t[0]) for t in s.tri])
    ids, counts = np.unique(roots, return_counts=True)
    big = set(ids[counts >= min_tris])
    keep = np.array([r in big for r in roots])
    return sf.select(s, keep)


def garment_colors(sh, g, seed):
    """Painterly vertex colours: soft low-frequency variation, darker inside, sole and hem shading."""
    rng = np.random.default_rng(seed)
    k = rng.normal(size=(6, 3))
    ph = rng.uniform(0, 6.28, 6)
    p = sh["base"]
    var = sum(np.sin(p @ k[i] * 9.0 + ph[i]) for i in range(6)) / 6.0
    c = 1.0 + 0.07 * var
    c = np.where(sh["part"] == 1, 0.55, c)
    c = np.where(sh["part"] >= 2, 0.8, c)
    c = c * (0.9 + 0.1 * smoothstep(0.2, 1.3, p[:, 1]))  # a touch darker low down
    if "sole" in sh:
        c = np.where(sh["sole"], 0.35, c)
    col = np.stack([c, c, c, np.ones_like(c)], 1)
    return np.clip(col, 0, 1.2)


def buckle(sh, wd_src):
    """A small brass buckle box at the front of the belt (weights of the nearest belt vertex)."""
    p = sh["pos"]
    score = np.abs(p[:, 0]) * 4 + np.abs(p[:, 1] - 1.025) * 4 - p[:, 2] + (sh["part"] != 0) * 100.0
    i = int(np.argmin(score))
    c = sh["pos"][i] + sh["nrm"][i] * 0.004
    n = sh["nrm"][i]; n = np.array([n[0], 0, n[2]]); n /= np.linalg.norm(n)
    up = np.array([0, 1.0, 0]); side = np.cross(up, n)
    hx, hy, hz = 0.028, 0.022, 0.005
    corners, normals, idx = [], [], []
    for axis, sgn in [(n, 1), (n, -1), (side, 1), (side, -1), (up, 1), (up, -1)]:
        a = axis * sgn
        others = [v for v in (n, side, up) if abs(v @ axis) < 0.5]
        u, v = others
        size = {id(n): hz, id(side): hx, id(up): hy}
        du = u * size[id(u)]; dv = v * size[id(v)]
        center = c + a * size[id(axis)]
        base = len(corners)
        quad = [center - du - dv, center + du - dv, center + du + dv, center - du + dv]
        corners += quad; normals += [a] * 4
        t1, t2 = [base, base + 1, base + 2], [base, base + 2, base + 3]
        if np.cross(quad[1] - quad[0], quad[2] - quad[0]) @ a < 0:
            t1, t2 = t1[::-1], t2[::-1]
        idx += [t1, t2]
    pos = np.array(corners)
    w = np.repeat(sh["w"][i][None], len(pos), 0)
    m = np.repeat(sh["morph"][i][None], len(pos), 0)
    return {"pos": pos, "nrm": np.array(normals), "w": w, "morph": m, "tri": np.array(idx)}


def frame_node(pos, y_axis, z_axis, parent):
    """Mount node TRS (local to joint `parent`) from a world frame: +Y toward the hilt, +Z out."""
    y = y_axis / np.linalg.norm(y_axis)
    z = z_axis - (z_axis @ y) * y
    z /= np.linalg.norm(z)
    x = np.cross(y, z)
    m = np.eye(4)
    m[:3, 0], m[:3, 1], m[:3, 2], m[:3, 3] = x, y, z, pos
    local = np.linalg.inv(JW[ji(parent)]) @ m
    r = local[:3, :3]
    q = mat_quat(r)
    return {"translation": [float(v) for v in local[:3, 3]], "rotation": [float(v) for v in q]}


def mat_quat(r):
    t = np.trace(r)
    if t > 0:
        s = np.sqrt(t + 1.0) * 2
        q = [(r[2, 1] - r[1, 2]) / s, (r[0, 2] - r[2, 0]) / s, (r[1, 0] - r[0, 1]) / s, 0.25 * s]
    elif r[0, 0] > r[1, 1] and r[0, 0] > r[2, 2]:
        s = np.sqrt(1.0 + r[0, 0] - r[1, 1] - r[2, 2]) * 2
        q = [0.25 * s, (r[0, 1] + r[1, 0]) / s, (r[0, 2] + r[2, 0]) / s, (r[2, 1] - r[1, 2]) / s]
    elif r[1, 1] > r[2, 2]:
        s = np.sqrt(1.0 + r[1, 1] - r[0, 0] - r[2, 2]) * 2
        q = [(r[0, 1] + r[1, 0]) / s, 0.25 * s, (r[1, 2] + r[2, 1]) / s, (r[0, 2] - r[2, 0]) / s]
    else:
        s = np.sqrt(1.0 + r[2, 2] - r[0, 0] - r[1, 1]) * 2
        q = [(r[0, 2] + r[2, 0]) / s, (r[1, 2] + r[2, 1]) / s, 0.25 * s, (r[1, 0] - r[0, 1]) / s]
    q = np.array(q)
    return q / np.linalg.norm(q)


def mounts_for(name, sh):
    outer = sh["part"] == 0
    P = sh["pos"]; N = sh["nrm"]
    out = []
    if name == "belt":
        for s, sign in (("l", 1), ("r", -1)):
            want = np.array([sign * 0.17, 1.025, J["pelvis"][2] - 0.03])
            d = np.linalg.norm(P - want, axis=1) + (~outer) * 10
            i = int(np.argmin(d))
            n = N[i]; out_dir = np.array([n[0], 0, n[2]])
            hilt = np.array([0, 1.0, 0.55]) if s == "l" else np.array([0, 1.0, 0.25])
            node = frame_node(P[i] + out_dir / np.linalg.norm(out_dir) * 0.012, hilt, out_dir, "pelvis")
            out.append((f"sheath_hip_{s}", "pelvis", node))
    if name == "baldric":
        back = outer & (N[:, 2] < -0.6)
        d = np.abs(P[:, 0]) + (~back) * 10
        i = int(np.argmin(d))
        node = frame_node(P[i] + N[i] * 0.015, -BALDRIC_DIR, N[i], "spine_03")
        out.append(("sheath_back", "spine_03", node))
    return out


# ------------------------------------------------------------------------------------------
# Hair, brows, beard

HAIR = {  # name: (file, native sex)
    "hair_buzzed": ("Hair_Buzzed.gltf", "male"),
    "hair_parted": ("Hair_SimpleParted.gltf", "male"),
    "hair_buzzed_f": ("Hair_BuzzedFemale.gltf", "female"),
    "hair_long": ("Hair_Long.gltf", "female"),
    "hair_buns": ("Hair_Buns.gltf", "female"),
    "brows_regular": ("Eyebrows_Regular.gltf", "male"),
    "brows_soft": ("Eyebrows_Female.gltf", "female"),
    "beard": ("Hair_Beard.gltf", "male"),
}


def bbox(v):
    return v.min(0), v.max(0)


def transfer_weights(p, dst: Body, k=4):
    """Skin weights from the nearest body surface (inverse-distance blend of k vertices)."""
    d, nn = cKDTree(dst.surf.pos).query(p, k=k)
    w = 1.0 / np.maximum(d, 1e-4) ** 2
    w /= w.sum(1, keepdims=True)
    dense = np.einsum("nk,nkj->nj", w, dst.surf.w[nn])
    return sf.top4(dense)


def fit_hair(arr, src: Body, dst: Body, clear_head=0.0015, clear_torso=0.026, native=True, transfer=False, cloth=None):
    p = arr["pos"].copy()
    if not native:
        # map through the two heads' bounding boxes (per axis), then push out of the new head
        a0, a1 = bbox(src.head_verts); b0, b1 = bbox(dst.head_verts)
        p = b0 + (p - a0) * (b1 - b0) / (a1 - a0)
    surf = dst.surf
    tree = cKDTree(surf.pos)
    head_w = surf.w[:, ji("Head")] + surf.w[:, ji("neck_01")]
    for _ in range(3):
        d, k = tree.query(p, k=6)
        best = np.full(len(p), np.inf); push = np.zeros(len(p))
        nn = k[:, 0]
        n = surf.nrm[nn]
        s = np.einsum("ij,ij->i", p - surf.pos[nn], n)
        on_head = head_w[nn] > 0.5
        want = np.where(on_head, clear_head, clear_torso)
        if native:
            want = np.where(on_head, -1.0, clear_torso)  # keep authored roots in the scalp
        need = (s < want) & (d[:, 0] < 0.08)
        push = np.where(need, want - s, 0)
        p += push[:, None] * n
    if cloth is not None:
        # also clear the outermost cloth over the shoulders and back (tunic, shirt, baldric)
        cpos, cnrm = cloth
        ctree = cKDTree(cpos)
        for _ in range(3):
            d, nn = ctree.query(p)
            s = np.einsum("ij,ij->i", p - cpos[nn], cnrm[nn])
            need = (s < 0.004) & (d < 0.06) & (p[:, 1] < J["Head"][1] + 0.02)
            p += np.where(need, 0.004 - s, 0)[:, None] * cnrm[nn]
    out = dict(arr)
    if transfer:
        out["joints"], out["weights"] = transfer_weights(p, dst)
    ids = rebind.weld_ids(arr["pos"])
    before = rebind.welded_normals(arr["pos"], arr["idx"], ids)
    after = rebind.welded_normals(p, arr["idx"], ids)
    out["nrm"] = rebind.rotate_normals(arr["nrm"], before, after)
    out["pos"] = p
    return out


# ------------------------------------------------------------------------------------------
# Textures

def png(im: Image.Image) -> bytes:
    b = io.BytesIO()
    im.save(b, format="PNG", optimize=True)
    return b.getvalue()


def neutral_skin(data: bytes, size=1024) -> Image.Image:
    """Divide out the average skin colour so the game's skin tone (base colour) sets the hue."""
    im = Image.open(io.BytesIO(data)).convert("RGB").resize((size, size), Image.LANCZOS)
    a = np.asarray(im, float) / 255.0
    lum = a @ [0.2126, 0.7152, 0.0722]
    sat = a.max(2) - a.min(2)
    skin = (lum > 0.25) & (sat > 0.08)
    avg = a[skin].mean(0)
    out = np.clip(a / avg * 0.92, 0, 1)
    return Image.fromarray((out * 255).astype(np.uint8))


def neutral_hair(data: bytes, size=512) -> Image.Image:
    im = Image.open(io.BytesIO(data)).convert("RGB").resize((size, size), Image.LANCZOS)
    a = np.asarray(im, float) / 255.0
    lum = a @ [0.2126, 0.7152, 0.0722]
    lum = lum / lum.mean() * 0.85
    out = np.clip(np.stack([lum] * 3, 2), 0, 1)
    return Image.fromarray((out * 255).astype(np.uint8))


# Iris colours (sRGB) for our eye textures.
IRIS_COLOURS = {"brown": (0.36, 0.20, 0.09), "blue": (0.30, 0.48, 0.70), "green": (0.28, 0.48, 0.30), "grey": (0.50, 0.54, 0.58),
                "hazel": (0.48, 0.38, 0.18), "amber": (0.70, 0.46, 0.12)}
EYE_COLOURS = {"brown": None, "blue": (0.58, 0.55), "green": (0.30, 0.55), "grey": (0.58, 0.08), "hazel": (0.14, 0.55),
               "amber": (0.10, 0.85)}


def eye_variant(data: bytes, colour) -> Image.Image:
    im = Image.open(io.BytesIO(data)).convert("RGBA")
    if colour is None:
        return im
    a = np.asarray(im, float) / 255.0
    h, w = a.shape[:2]
    yy, xx = np.mgrid[0:h, 0:w]
    rgb = a[..., :3]
    mx = rgb.max(2); mn = rgb.min(2)
    sat = (mx - mn) / np.maximum(mx, 1e-6)
    # the iris: a saturated, darker disc around the centre of the eye texture
    cy, cx = np.argwhere((sat > 0.45) & (mx < 0.7)).mean(0)
    r = np.hypot(yy - cy, xx - cx)
    iris = (r < h * 0.11) & (mx < 0.8)
    hue, s_mul = colour
    out = rgb.copy()
    for y, x in np.argwhere(iris):
        hh, ll, ss = colorsys.rgb_to_hls(*rgb[y, x])
        out[y, x] = colorsys.hls_to_rgb(hue, min(ll * 1.25, 0.9), ss * s_mul)
    a[..., :3] = out
    return Image.fromarray((a * 255).astype(np.uint8))


# ------------------------------------------------------------------------------------------
# Writing

def prim_out(pos, nrm, wd_or_joints, morph, idx, material, uv=None, color=None, joints=None, weights=None):
    if joints is None:
        joints, weights = sf.top4(wd_or_joints)
    d = {"pos": pos, "nrm": nrm, "joints": joints, "weights": weights, "idx": idx, "material": material,
         "uv": uv, "color": color}
    if morph is not None:
        d["targets"] = [morph[:, k, :] for k in range(morph.shape[1])]
    return d


class Out:
    """One GLB on ual2's skeleton."""

    def __init__(self, embed=False, tex_prefix="../tex/"):
        self.w = gm.Writer()
        self.embed = embed
        self.tex_prefix = tex_prefix
        self.textures = {}
        self.meshes = []  # (node name, mesh index)
        self.mounts = {}

    def texture(self, name):
        if name in self.textures:
            return self.textures[name]
        path = os.path.join(OUT, "tex", name)
        if self.embed:
            with open(path, "rb") as f:
                t = self.w.image(f.read(), name=name)
        else:
            img = {"uri": self.tex_prefix + name, "mimeType": "image/png", "name": name}
            self.w.g["images"].append(img)
            if not self.w.g["samplers"]:
                self.w.g["samplers"].append({"magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497})
            self.w.g["textures"].append({"sampler": 0, "source": len(self.w.g["images"]) - 1})
            t = len(self.w.g["textures"]) - 1
        self.textures[name] = t
        return t

    def material(self, name, colour, rough=0.85, base=None, normal=None, metal=0.0):
        """`colour` is sRGB (like `Color::srgb` in the game); glTF stores linear factors."""
        lin = [float(c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4) for c in colour[:3]]
        return self.w.material("hero_" + name, lin + [1.0], rough=rough, metal=metal,
                               base_tex=None if base is None else self.texture(base),
                               normal_tex=None if normal is None else self.texture(normal))

    def add(self, node_name, prims):
        mi = mesh_with_targets(self.w, node_name, prims)
        self.meshes.append((node_name, mi))

    def mount(self, name, parent, node):
        self.mounts.setdefault(parent, []).append((name, node))

    def save(self, path):
        extra = {}
        for parent, items in self.mounts.items():
            extra[parent] = [self.w.node(name=n, **t) for n, t in items]
        mesh_nodes = []
        top, mapping, skin = None, None, None
        top, mapping, skin = gm.copy_skeleton(self.w, REF, RS, extra_children=extra)
        for name, mi in self.meshes:
            node = {"name": name, "mesh": mi, "skin": skin}
            mesh_nodes.append(self.w.node(**node))
        # The Armature node holds the mesh nodes and the skeleton, like ual2.glb.
        arm = self.w.g["nodes"][top]
        arm["children"] = mesh_nodes + arm.get("children", [])
        self.w.g["scenes"][0]["nodes"] = [top]
        os.makedirs(os.path.dirname(path), exist_ok=True)
        self.w.save(path)


def mesh_with_targets(w: gm.Writer, name, prims):
    out = []
    ntargets = 0
    for p in prims:
        attrs = {
            "POSITION": w.accessor(p["pos"], "VEC3", target=34962, minmax=True),
            "NORMAL": w.accessor(p["nrm"], "VEC3", target=34962),
            "JOINTS_0": w.accessor(p["joints"].astype(np.uint16), "VEC4", comp=5123, target=34962),
            "WEIGHTS_0": w.accessor(p["weights"], "VEC4", target=34962),
        }
        if p.get("uv") is not None:
            attrs["TEXCOORD_0"] = w.accessor(p["uv"], "VEC2", target=34962)
        if p.get("color") is not None:
            attrs["COLOR_0"] = w.accessor(p["color"], "VEC4", target=34962)
        prim = {"attributes": attrs, "indices": w.accessor(np.asarray(p["idx"]).reshape(-1).astype(np.uint32), "SCALAR", comp=5125, target=34963),
                "material": p["material"], "mode": 4}
        if p.get("targets"):
            prim["targets"] = [{"POSITION": w.accessor(t, "VEC3", target=34962, minmax=True)} for t in p["targets"]]
            ntargets = len(p["targets"])
        out.append(prim)
    mesh = {"name": name, "primitives": out}
    if ntargets:
        mesh["weights"] = [0.0] * ntargets
        mesh["extras"] = {"targetNames": MORPHS[:ntargets]}
    w.g["meshes"].append(mesh)
    return len(w.g["meshes"]) - 1


# ------------------------------------------------------------------------------------------
# Assembly

def body_prims(body: Body, out: Out, skin_colour=(1, 1, 1)):
    """Body split into hide groups + eyes. Returns {node name: [prims]}."""
    b = body.body
    mask = hide_mask(body)
    mat_skin = out.material("skin", skin_colour, rough=0.75, base=f"{body.sex}_skin.png", normal=f"{body.sex}_skin_normal.png")
    groups = {}
    for m in np.unique(mask):
        sel = mask == m
        tri = b["idx"][sel]
        used = np.unique(tri)
        remap = -np.ones(len(b["pos"]), np.int64); remap[used] = np.arange(len(used))
        name = "body" if m == 0 else "body_hide_" + "+".join(g for bit, g in enumerate(HIDING) if m >> bit & 1)
        groups[name] = [prim_out(b["pos"][used], b["nrm"][used], None, body.morph_unwelded[used], remap[tri], mat_skin,
                                 uv=b["uv"][used], joints=b["joints"][used], weights=b["weights"][used])]
    e = body.eyes
    mat_eye = out.material("eyes", (1, 1, 1), rough=0.3, base="eye_brown.png")
    groups["eyes"] = [prim_out(e["pos"], e["nrm"], None, np.zeros((len(e["pos"]), len(MORPHS), 3)), e["idx"], mat_eye,
                               uv=e["uv"], joints=e["joints"], weights=e["weights"])]
    return groups, mask


# Which generated weave each garment wears (`textures.cloth`).
FABRIC = {"tunic": "wool", "shirt": "linen", "trousers": "twill", "boots": "leather", "shoes": "leather", "gloves": "leather",
          "belt": "leather", "baldric": "leather", "haori": "twill", "vest": "leather", "mantle": "wool", "hood": "wool",
          "bracers": "leather", "hakama": "twill", "leg_wraps": "linen", "robe_top": "twill", "robe_skirt": "twill",
          "kilt": "wool", "apron": "leather", "cloak": "wool", "tabard": "twill"}


def garment_prims(body: Body, name: str, out: Out, colour=None):
    g = GARMENTS[name]
    s, sh = make_garment(body, name)
    colour = colour or g["colour"]
    fabric = FABRIC.get(name, "linen")
    mat = out.material(f"cloth_{name}", colour, rough=g["rough"], base=f"cloth_{fabric}.png", normal=f"cloth_{fabric}_normal.png")
    col = garment_colors(sh, g, seed=sum(map(ord, name)))
    uv = tx.cloth_uv(sh["pos"], armness(sh["pos"], sh["w"]), tile=0.09 if fabric == "leather" else 0.12)
    prims = [prim_out(sh["pos"], sh["nrm"], sh["w"], sh["morph"], sh["tri"], mat, uv=uv, color=col.astype(np.float32))]
    if g.get("buckle"):
        bk = buckle(sh, None)
        mm = out.material(f"metal_{name}", (0.74, 0.60, 0.32), rough=0.35, metal=0.9)
        prims.append(prim_out(bk["pos"], bk["nrm"], bk["w"], bk["morph"], bk["tri"], mm))
    mounts = mounts_for(name, sh)
    return prims, mounts, sh


def hair_prims(arr, out: Out, matname, colour, tex):
    mat = out.material(matname, colour, rough=0.85, base=f"{tex}_base.png", normal=f"{tex}_normal.png")
    return [prim_out(arr["pos"], arr["nrm"], None, None, arr["idx"], mat, uv=arr["uv"], joints=arr["joints"], weights=arr["weights"])]


def load_hair(name):
    fname, native = HAIR[name]
    doc, skin, prims = load_rebound(os.path.join(BASE_DIR, fname))
    tex_name = None
    for img in doc.g.images:
        if "Hair_1" in (img.uri or ""):
            tex_name = "hair_1"
        if "Hair_2" in (img.uri or ""):
            tex_name = "hair_2"
    return prims[0], native, tex_name, doc


DEFAULT_COLOURS = {"skin": (0.80, 0.60, 0.46), "hair": (0.30, 0.20, 0.12)}


def main():
    os.makedirs(os.path.join(OUT, "tex"), exist_ok=True)
    bodies = {s: Body(s) for s in ("male", "female")}
    report = {"bodies": {}, "garments": {}, "hair": {}}
    # textures
    # Our own textures (`textures.py`): generated from the meshes, no source pixels copied.
    for s, b in bodies.items():
        base, normal = tx.skin(b, J)
        base.save(os.path.join(OUT, "tex", f"{s}_skin.png"), optimize=True)
        normal.save(os.path.join(OUT, "tex", f"{s}_skin_normal.png"), optimize=True)
    centre, radius = tx.find_iris(Image.open(io.BytesIO(bodies["male"].tex_eye)))
    for c, srgb in IRIS_COLOURS.items():
        tx.iris(srgb, centre, radius, seed=sum(map(ord, c))).save(os.path.join(OUT, "tex", f"eye_{c}.png"), optimize=True)
    for k, seed in (("1", 11), ("2", 12)):
        hb, hn = tx.hair(seed=seed)
        hb.save(os.path.join(OUT, "tex", f"hair_{k}_base.png"), optimize=True)
        hn.save(os.path.join(OUT, "tex", f"hair_{k}_normal.png"), optimize=True)
    for kind in tx.CLOTH_KINDS:
        cb, cn = tx.cloth(kind, seed=sum(map(ord, kind)))
        # A processed AI swatch (`ai_texture.py`, ART_PROMPTS T-runs) replaces the procedural weave.
        ai = os.path.join(HERE, "ai_tex", f"{kind}_albedo.png")
        if os.path.exists(ai):
            cb, cn = Image.open(ai).convert("RGB"), Image.open(ai.replace("_albedo", "_normal")).convert("RGB")
        cb.save(os.path.join(OUT, "tex", f"cloth_{kind}.png"), optimize=True)
        cn.save(os.path.join(OUT, "tex", f"cloth_{kind}_normal.png"), optimize=True)

    hairs = {n: load_hair(n) for n in HAIR}
    styled = Out(embed=True)
    for sex, body in bodies.items():
        d = os.path.join(OUT, sex)
        out = Out()
        groups, mask = body_prims(body, out, DEFAULT_COLOURS["skin"])
        for name, prims in groups.items():
            out.add(name, prims)
        out.save(os.path.join(d, "body.glb"))
        report["bodies"][sex] = {"groups": {k: int(sum(len(p["idx"]) for p in v)) for k, v in groups.items()},
                                 "vertices": int(len(body.body["pos"])), "triangles": int(len(body.body["idx"])),
                                 "sole_y": body.sole_y}
        if sex == "male":
            sg, _ = body_prims(body, styled, DEFAULT_COLOURS["skin"])
            for name, prims in sg.items():
                styled.add(name, prims)
        for gname in GARMENTS:
            out = Out()
            prims, mounts, sh = garment_prims(body, gname, out)
            out.add(gname, prims)
            for mname, parent, node in mounts:
                out.mount(mname, parent, node)
            out.save(os.path.join(d, f"{gname}.glb"))
            report["garments"].setdefault(gname, {})[sex] = {
                "vertices": int(sum(len(p["pos"]) for p in prims)), "triangles": int(sum(len(p["idx"]) for p in prims)),
                "mounts": [m[0] for m in mounts]}
            if sex == "male" and gname in ("tunic", "trousers", "boots", "gloves", "belt", "baldric"):
                sp, sm, _ = garment_prims(body, gname, styled)
                styled.add(gname, sp)
                for mname, parent, node in sm:
                    styled.mount(mname, parent, node)
        cloth_p, cloth_n = [], []
        for gname in ("tunic", "shirt", "baldric"):
            _, sh = make_garment(body, gname)
            outer = sh["part"] == 0
            # hair has no morph targets: clear the cloth at its fullest (bulk + heavy)
            cloth_p.append((sh["pos"] + sh["morph"][:, 0] + sh["morph"][:, 1])[outer]); cloth_n.append(sh["nrm"][outer])
        cloth = (np.concatenate(cloth_p), np.concatenate(cloth_n))
        for hname, (arr, native, tex, _) in hairs.items():
            if hname == "beard" and sex == "female":
                continue
            src = bodies[native]
            fitted = fit_hair(arr, src, body, native=(native == sex), transfer=not hname.startswith("brows"),
                              cloth=cloth if hname.startswith("hair") else None)
            out = Out()
            kind = "brows" if hname.startswith("brows") else ("beard" if hname == "beard" else "hair")
            out.add(hname, hair_prims(fitted, out, kind, DEFAULT_COLOURS["hair"], tex))
            out.save(os.path.join(d, f"{hname}.glb"))
            report["hair"].setdefault(hname, {})[sex] = {"vertices": int(len(fitted["pos"])), "triangles": int(len(fitted["idx"])),
                                                         "native": native == sex,
                                                         "moved_max": float(np.linalg.norm(fitted["pos"] - arr["pos"], axis=1).max())}
            if sex == "male" and hname in ("hair_parted", "brows_regular", "beard"):
                styled.add(hname, hair_prims(fitted, styled, kind, DEFAULT_COLOURS["hair"], tex))
    styled.save(os.path.join(ASSETS, "anim", "hero_styled.glb"))
    with open(os.path.join(OUT, "build_report.json"), "w") as f:
        json.dump(report, f, indent=1)
    print(json.dumps(report, indent=1))


if __name__ == "__main__":
    main()
