"""Measure a skinned character mesh: counts, materials, skin weights, and the body split into
parts by dominant joint, with the radius of each part about its bone (for `motion/body.rs`
capsules) and the finger / palm sizes (for `motion/hand.rs`).

    python -I tools/model/analyse.py <model.glb|.gltf> [...]   (prints JSON)

The numbers in docs/MODEL_PARTS.md come from this script.
"""
from __future__ import annotations

import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gbmesh as gm  # noqa: E402

# Part of each canonical joint (dominant-joint split).
PART_OF = {}
for n in ["Head"]:
    PART_OF[n] = "head"
PART_OF["neck_01"] = "neck"
for n in ["spine_01", "spine_02", "spine_03", "clavicle_l", "clavicle_r"]:
    PART_OF[n] = "torso"
PART_OF["pelvis"] = "pelvis"
PART_OF["root"] = "pelvis"
for s in "lr":
    PART_OF[f"upperarm_{s}"] = f"upperarm_{s}"
    PART_OF[f"lowerarm_{s}"] = f"forearm_{s}"
    PART_OF[f"hand_{s}"] = f"hand_{s}"
    for f in ["index", "middle", "ring", "pinky", "thumb"]:
        for k in ["01", "02", "03", "04_leaf"]:
            PART_OF[f"{f}_{k}_{s}"] = f"fingers_{s}"
    PART_OF[f"thigh_{s}"] = f"thigh_{s}"
    PART_OF[f"calf_{s}"] = f"shin_{s}"
    PART_OF[f"foot_{s}"] = f"foot_{s}"
    PART_OF[f"ball_{s}"] = f"foot_{s}"
    PART_OF[f"ball_leaf_{s}"] = f"foot_{s}"

# Bone segment used for the radius of each part: (start joint, end joint)
SEGMENT = {"torso": ("spine_01", "neck_01"), "pelvis": ("pelvis", "spine_01"), "neck": ("neck_01", "Head"),
           "head": ("Head", None)}
for s in "lr":
    SEGMENT[f"upperarm_{s}"] = (f"upperarm_{s}", f"lowerarm_{s}")
    SEGMENT[f"forearm_{s}"] = (f"lowerarm_{s}", f"hand_{s}")
    SEGMENT[f"thigh_{s}"] = (f"thigh_{s}", f"calf_{s}")
    SEGMENT[f"shin_{s}"] = (f"calf_{s}", f"foot_{s}")
    SEGMENT[f"hand_{s}"] = (f"hand_{s}", f"middle_01_{s}")
    SEGMENT[f"foot_{s}"] = (f"foot_{s}", f"ball_{s}")
    SEGMENT[f"fingers_{s}"] = (None, None)


def seg_dist(p, a, b):
    ab = b - a
    t = np.clip(((p - a) @ ab) / max(ab @ ab, 1e-12), 0, 1)
    return np.linalg.norm(p - (a + t[:, None] * ab), axis=1)


def finger_radius(pos, dom, names, wpos, s):
    """Median distance of each finger's vertices to its own phalanx chain."""
    out = {}
    for f in ["index", "middle", "ring", "pinky", "thumb"]:
        chain = [names.index(f"{f}_{k}_{s}") for k in ["01", "02", "03", "04_leaf"]]
        sel = np.isin(dom, chain[:3])
        if not sel.any():
            continue
        d = np.min([seg_dist(pos[sel], wpos[chain[k]], wpos[chain[k + 1]]) for k in range(3)], axis=0)
        out[f] = {"verts": int(sel.sum()), "median_r": round(float(np.median(d)), 4), "p90_r": round(float(np.percentile(d, 90)), 4),
                  "length": round(float(sum(np.linalg.norm(wpos[chain[k + 1]] - wpos[chain[k]]) for k in range(3))), 4)}
    return out


def analyse(path: str) -> dict:
    doc = gm.Doc(path)
    skin = gm.Skin(doc)
    rest = skin.rest_world()
    wpos = rest[:, :3, 3]
    bind_err = float(np.abs(np.einsum("nij,njk->nik", rest, skin.ibm) - np.eye(4)).max())
    rep = {"file": os.path.basename(path), "joints": len(skin.names), "bind_equals_rest_fk_max_err": bind_err,
           "materials": [], "meshes": []}
    for m in doc.g.materials or []:
        pb = m.pbrMetallicRoughness
        rep["materials"].append({"name": m.name, "base_color": [round(c, 3) for c in (pb.baseColorFactor or [1, 1, 1, 1])],
                                 "texture": pb.baseColorTexture is not None, "metallic": pb.metallicFactor, "roughness": pb.roughnessFactor})
    allpos, alldom, allw, alltri = [], [], [], []
    base = 0
    for mi, m in enumerate(doc.g.meshes):
        for p in m.primitives:
            a = gm.prim_arrays(doc, p)
            w = a["weights"]
            nz = (w > 1e-4).sum(1)
            dom = a["joints"][np.arange(len(w)), np.argmax(w, 1)]
            welded = len(np.unique(np.round(a["pos"], 5), axis=0))
            rep["meshes"].append({"mesh": m.name, "material": (doc.g.materials[p.material].name if p.material is not None else None),
                                  "vertices": len(a["pos"]), "welded_vertices": welded, "triangles": len(a["idx"]),
                                  "bbox_min": a["pos"].min(0).round(3).tolist(), "bbox_max": a["pos"].max(0).round(3).tolist(),
                                  "weight_sum_max_err": float(np.abs(w.sum(1) - 1).max()),
                                  "influences_hist": {int(k): int((nz == k).sum()) for k in range(1, 5)},
                                  "joints_used": int(len(np.unique(a["joints"][w > 1e-4])))})
            allpos.append(a["pos"]); alldom.append(dom); allw.append(w); alltri.append(a["idx"] + base)
            base += len(a["pos"])
    pos = np.concatenate(allpos); dom = np.concatenate(alldom); tri = np.concatenate(alltri)
    parts = {}
    part_of_vertex = np.array([PART_OF.get(skin.names[j], "other") for j in dom])
    tri_part = part_of_vertex[tri[:, 0]]
    for part in sorted(set(part_of_vertex)):
        sel = part_of_vertex == part
        pp = pos[sel]
        info = {"vertices": int(sel.sum()), "triangles": int((tri_part == part).sum()),
                "bbox_min": pp.min(0).round(3).tolist(), "bbox_max": pp.max(0).round(3).tolist()}
        a, b = SEGMENT.get(part, (None, None))
        if a and b:
            d = seg_dist(pp, wpos[skin.names.index(a)], wpos[skin.names.index(b)])
            info["radius_median"] = round(float(np.median(d)), 4)
            info["radius_p90"] = round(float(np.percentile(d, 90)), 4)
            info["radius_max"] = round(float(d.max()), 4)
        elif a:
            d = np.linalg.norm(pp - wpos[skin.names.index(a)], axis=1)
            info["dist_from_joint_median"] = round(float(np.median(d)), 4)
            info["dist_from_joint_max"] = round(float(d.max()), 4)
        parts[part] = info
    rep["parts"] = parts
    rep["fingers_l"] = finger_radius(pos, dom, skin.names, wpos, "l")
    # Palm half-thickness: hand-dominant vertices, spread along the palm normal (approx. -Y in T-pose).
    for s in "l":
        sel = dom == skin.names.index(f"hand_{s}")
        if sel.any():
            y = pos[sel][:, 1] - wpos[skin.names.index(f"hand_{s}")][1]
            rep[f"palm_{s}_thickness_y"] = [round(float(y.min()), 4), round(float(y.max()), 4)]
    rep["height"] = round(float(pos[:, 1].max() - pos[:, 1].min()), 4)
    rep["sole_y"] = round(float(pos[:, 1].min()), 4)
    return rep


if __name__ == "__main__":
    print(json.dumps([analyse(p) for p in sys.argv[1:]], indent=1))
