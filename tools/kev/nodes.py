"""Kev nodes: parse a rigged model into a compact hierarchy of nodes and measure its body areas.

    python3 tools/kev/nodes.py <model.glb> [<model.glb> ...] [--json out.json] [--llm]

Fast path: only the GLB's JSON header and the skin's inverse-bind matrices are read (no mesh decode).
Each joint becomes a node [index, name, parent, role, region, length], where role and region come from
the rules in roles.json (the extension point: a new rig family is a new prefix or pattern there, not new
code). The parent is the nearest ancestor that is also a joint.

The body is then measured with linear algebra, all lengths divided by the skeletal height so a rig in
centimetres and one in metres compare directly:
  up axis      unit vector from the feet to the head (no assumption that the file is Y-up)
  height       head minus lowest foot/toe joint along the up axis, in model units, plus a units guess
  limbs        arm (shoulder > elbow > wrist) and leg (hip > knee > ankle) lengths per side, their
               left/right asymmetry, and the arm's angle from the up axis (90 = T-pose, ~180 = arms down)
  regions      per region: joint count, centroid, and a PCA of the joint positions (principal axis,
               its angle from up, and elongation = largest / second singular value)

`--llm` prints one short line per model and region: the form meant for a language model's context.

How this fits the rest of Kev, and how to extend it: tools/kev/ARCHITECTURE.md.
"""
import argparse
import json
import math
import os
import re
import struct
import sys
import time

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
RULES_PATH = os.path.join(HERE, "roles.json")


def load_rules(path=RULES_PATH):
    with open(path) as f:
        rules = json.load(f)
    rules["_roles"] = [(role, re.compile(pat)) for role, pat in rules["roles"]]
    rules["_side"] = {s: [re.compile(p) for p in pats] for s, pats in rules["side"].items()}
    rules["_suffix"] = [re.compile(p) for p in rules.get("suffix", [])]
    return rules


def read_header_and_ibm(path):
    """(json header, inverse-bind matrices as (n,4,4) row-major) for skin 0, reading only what is needed."""
    with open(path, "rb") as f:
        head = f.read(12)
        if struct.unpack_from("<I", head, 0)[0] != 0x46546C67:
            raise ValueError(f"{path}: not a GLB")
        length, kind = struct.unpack("<II", f.read(8))
        js = json.loads(f.read(length))
        skins = js.get("skins") or []
        if not skins or "inverseBindMatrices" not in skins[0]:
            return js, None
        acc = js["accessors"][skins[0]["inverseBindMatrices"]]
        view = js["bufferViews"][acc["bufferView"]]
        bin_len, _ = struct.unpack("<II", f.read(8))
        start = f.tell()
        f.seek(start + view.get("byteOffset", 0) + acc.get("byteOffset", 0))
        raw = f.read(acc["count"] * 64)
    # glTF matrices are column-major: transpose each to row-major.
    return js, np.frombuffer(raw, dtype="<f4").reshape(acc["count"], 4, 4).transpose(0, 2, 1).astype(np.float64)


def classify(name, rules):
    """(role, side) for a joint name."""
    core = name.lower()
    for prefix in rules["strip"]:
        if core.startswith(prefix):
            core = core[len(prefix):]
    # Segment numbers after the side (Rigify "upper_arm.L.001") go first, so the side marker ends the name.
    for p in rules.get("_suffix", []):
        core = p.sub("", core)
    side = ""
    for s, pats in rules["_side"].items():
        for p in pats:
            if p.search(core):
                side = s
                core = p.sub("", core).strip("_. ")
                break
        if side:
            break
    for role, pat in rules["_roles"]:
        if pat.search(core):
            return role, side
    return "other", side


def region_of(role, side, rules, parent_region=None):
    if role == "twist":
        return parent_region or "other"
    base = rules["regions"].get(role, "other")
    return f"{base}_{side}" if side and base in ("arm", "hand", "leg", "foot") else base


def parse(path, rules=None):
    """The node table and body measurements for one rigged GLB."""
    rules = rules or load_rules()
    t0 = time.perf_counter()
    js, ibm = read_header_and_ibm(path)
    skin = (js.get("skins") or [{}])[0]
    joints = skin.get("joints", [])
    if not joints or ibm is None:
        return {"model": os.path.basename(path), "joints": 0, "error": "no skin with bind matrices"}
    nodes = js["nodes"]
    parent_of_node = {}
    for i, n in enumerate(nodes):
        for c in n.get("children", []):
            parent_of_node[c] = i
    joint_index = {node: k for k, node in enumerate(joints)}

    def joint_parent(node):
        p = parent_of_node.get(node)
        while p is not None and p not in joint_index:
            p = parent_of_node.get(p)
        return joint_index.get(p) if p is not None else None

    pos = np.array([np.linalg.inv(m)[:3, 3] for m in ibm])
    names = [nodes[j].get("name", f"node{j}") for j in joints]
    parents = [joint_parent(j) for j in joints]
    roles, sides, regions = [], [], [None] * len(joints)
    for k, name in enumerate(names):
        role, side = classify(name, rules)
        roles.append(role)
        sides.append(side)
    # Twist bones take their parent's region, so resolve in hierarchy order.
    order = sorted(range(len(joints)), key=lambda k: depth(k, parents))
    for k in order:
        p = parents[k]
        regions[k] = region_of(roles[k], sides[k], rules, regions[p] if p is not None else None)

    lengths = [float(np.linalg.norm(pos[k] - pos[parents[k]])) if parents[k] is not None else 0.0 for k in range(len(joints))]
    body = measure(pos, names, roles, sides, regions)
    scale = body.get("height") or 1.0
    table = [[k, names[k], parents[k], roles[k], regions[k], round(lengths[k] / scale, 4)] for k in range(len(joints))]
    body["parse_ms"] = round((time.perf_counter() - t0) * 1000, 2)
    return {"model": os.path.basename(path), "path": path, "joints": len(joints), "nodes": table, **body}


def depth(k, parents):
    d = 0
    while parents[k] is not None:
        k = parents[k]
        d += 1
    return d


def first(roles, sides, role, side=""):
    for k, (r, s) in enumerate(zip(roles, sides)):
        if r == role and s == side:
            return k
    return None


def measure(pos, names, roles, sides, regions):
    """Up axis, height, limb lengths and symmetry, arm pose angle, per-region PCA. Lengths are / height."""
    head = first(roles, sides, "head")
    feet = [k for k, r in enumerate(roles) if r in ("foot", "toe")]
    if head is None or not feet:
        return {"height": None, "error": "no head or feet found"}
    foot_mid = pos[feet].mean(axis=0)
    up = pos[head] - foot_mid
    up /= max(np.linalg.norm(up), 1e-9)
    heights = (pos - foot_mid) @ up
    height = float(heights[head] - heights[feet].min())
    units = "cm" if height > 20 else "m"

    def chain_len(*ks):
        if any(k is None for k in ks):
            return None
        return float(sum(np.linalg.norm(pos[b] - pos[a]) for a, b in zip(ks, ks[1:]))) / height

    out = {"height": round(height, 4), "units_guess": units, "up_axis": [round(float(v), 3) for v in up], "limbs": {}}
    for s in ("l", "r"):
        arm = chain_len(first(roles, sides, "upperarm", s), first(roles, sides, "forearm", s), first(roles, sides, "hand", s))
        leg = chain_len(first(roles, sides, "thigh", s), first(roles, sides, "shin", s), first(roles, sides, "foot", s))
        ua, fa = first(roles, sides, "upperarm", s), first(roles, sides, "forearm", s)
        angle = None
        if ua is not None and fa is not None:
            v = pos[fa] - pos[ua]
            angle = math.degrees(math.acos(max(-1.0, min(1.0, float(v @ up) / max(np.linalg.norm(v), 1e-9)))))
        out["limbs"][s] = {"arm": rnd(arm), "leg": rnd(leg), "arm_angle_deg": rnd(angle, 1)}
    l, r = out["limbs"]["l"], out["limbs"]["r"]
    out["asymmetry"] = {
        "arm": rnd(abs(l["arm"] - r["arm"]) / ((l["arm"] + r["arm"]) / 2)) if l["arm"] and r["arm"] else None,
        "leg": rnd(abs(l["leg"] - r["leg"]) / ((l["leg"] + r["leg"]) / 2)) if l["leg"] and r["leg"] else None,
    }
    out["regions"] = {}
    for reg in sorted(set(regions)):
        ks = [k for k, g in enumerate(regions) if g == reg]
        pts = pos[ks]
        hs = ((pts - foot_mid) @ up) / height
        entry = {"joints": len(ks), "centroid_h": rnd(float(hs.mean())), "span_h": [rnd(float(hs.min())), rnd(float(hs.max()))]}
        if len(ks) >= 3:
            centred = pts - pts.mean(axis=0)
            _, sv, vt = np.linalg.svd(centred, full_matrices=False)
            axis = vt[0]
            entry["axis_from_up_deg"] = rnd(math.degrees(math.acos(min(1.0, abs(float(axis @ up))))), 1)
            entry["elongation"] = rnd(float(sv[0] / max(sv[1], 1e-9)), 2)
        out["regions"][reg] = entry
    return out


def rnd(v, d=4):
    return None if v is None else round(float(v), d)


def llm_lines(rep):
    """One terse line per model and per region: compact context for a language model."""
    if rep.get("error"):
        return [f"{rep['model']}: {rep['error']}"]
    l, r = rep["limbs"]["l"], rep["limbs"]["r"]
    lines = [
        f"{rep['model']}: {rep['joints']} joints, height {rep['height']} {rep['units_guess']}, "
        f"arm {l['arm']}/{r['arm']} leg {l['leg']}/{r['leg']} (x height, L/R), "
        f"arm angle {l['arm_angle_deg']}/{r['arm_angle_deg']} deg, asym arm {rep['asymmetry']['arm']} leg {rep['asymmetry']['leg']}"
    ]
    for reg, e in rep["regions"].items():
        extra = f" axis {e['axis_from_up_deg']} deg elong {e['elongation']}" if "elongation" in e else ""
        lines.append(f"  {reg}: {e['joints']} j, at {e['centroid_h']} h{extra}")
    return lines


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("models", nargs="+")
    ap.add_argument("--json")
    ap.add_argument("--llm", action="store_true")
    args = ap.parse_args()
    rules = load_rules()
    reports = [parse(p, rules) for p in args.models]
    if args.json:
        with open(args.json, "w") as f:
            json.dump(reports, f, indent=1)
    for rep in reports:
        if args.llm:
            print("\n".join(llm_lines(rep)))
        else:
            print(f"{rep['model']:<28} joints {rep['joints']:>3}  height {rep.get('height')} {rep.get('units_guess', '')}  {rep.get('parse_ms', '-')} ms")


if __name__ == "__main__":
    sys.exit(main())
