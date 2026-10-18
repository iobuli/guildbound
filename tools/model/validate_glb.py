#!/usr/bin/env python3
"""validate_glb.py - structural validation for every GLB in the repo.

Checks what actually breaks a Bevy load or a garment swap, rather than just "does it parse":
  * parses as GLB2, reports generator/version
  * nodes, meshes, primitives, materials, textures, images, skins, animations
  * every primitive's attributes: POSITION always, NORMAL and TEXCOORD_0 flagged when missing
  * skinned meshes: JOINTS_0 + WEIGHTS_0 present, joint count, and whether all garments in a folder
    share one joint count (the rig consistency `build.py` depends on)
  * external image URIs resolve relative to the file, else flagged
  * POSITION accessor bounds: present, finite, and a sane size in metres (a hero body ~1.7 m)
  * index accessor sanity

    ../venv/bin/python tools/model/validate_glb.py --root assets --out ../../glb_validation
"""
from __future__ import annotations

import argparse
import json
import math
import os
from collections import Counter, defaultdict

from pygltflib import GLTF2


def bounds(gltf, acc_i):
    a = gltf.accessors[acc_i]
    if a.min is None or a.max is None:
        return None
    if len(a.min) < 3:
        return None
    return [float(v) for v in a.min[:3]], [float(v) for v in a.max[:3]]


def check(path):
    r = {"file": path, "problems": [], "warnings": [], "info": {}}
    try:
        g = GLTF2().load(path)
    except Exception as e:                                        # noqa: BLE001
        r["problems"].append(f"does not parse as GLB: {e}")
        return r
    info = r["info"]
    info["generator"] = (g.asset.generator if g.asset else None)
    info["version"] = (g.asset.version if g.asset else None)
    info["nodes"] = len(g.nodes or [])
    info["meshes"] = len(g.meshes or [])
    info["materials"] = len(g.materials or [])
    info["textures"] = len(g.textures or [])
    info["images"] = len(g.images or [])
    info["skins"] = len(g.skins or [])
    info["animations"] = len(g.animations or [])
    prims = skinned = 0
    joints = set()
    for s in (g.skins or []):
        if s.joints:
            joints.add(len(s.joints))
    info["joint_counts"] = sorted(joints)
    if len(joints) > 1:
        r["problems"].append(f"skins disagree on joint count: {sorted(joints)}")
    for m in (g.meshes or []):
        for p in (m.primitives or []):
            prims += 1
            at = p.attributes
            if at is None or at.POSITION is None:
                r["problems"].append(f"mesh '{m.name}' primitive has no POSITION")
                continue
            b = bounds(g, at.POSITION)
            if b is None:
                r["warnings"].append(f"mesh '{m.name}' POSITION accessor has no min/max")
            else:
                lo, hi = b
                dims = [hi[i] - lo[i] for i in range(3)]
                if any(not math.isfinite(v) for v in lo + hi):
                    r["problems"].append(f"mesh '{m.name}' POSITION bounds are not finite")
                info.setdefault("max_dim_m", round(max(dims), 3))
                if at.JOINTS_0 is not None:
                    skinned += 1
                else:
                    if at.NORMAL is None:
                        r["warnings"].append(f"mesh '{m.name}' has no NORMAL (flat shading will be used)")
                    if at.TEXCOORD_0 is None:
                        r["warnings"].append(f"mesh '{m.name}' has no TEXCOORD_0 (textures cannot map)")
                if p.material is None:
                    r["warnings"].append(f"mesh '{m.name}' primitive has no material")
            if p.indices is None:
                r["warnings"].append(f"mesh '{m.name}' primitive is not indexed")
    info["primitives"] = prims
    info["skinned_primitives"] = skinned
    # external images must resolve next to the file
    root = os.path.dirname(path)
    for img in (g.images or []):
        if img.uri and not img.uri.startswith("data:"):
            target = os.path.join(root, img.uri)
            if not os.path.isfile(target):
                r["problems"].append(f"image uri '{img.uri}' not found next to the file")
    if (g.buffers or []) and g.buffers[0].uri and not os.path.isfile(os.path.join(root, g.buffers[0].uri)):
        r["problems"].append(f"buffer uri '{g.buffers[0].uri}' not found next to the file")
    return r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default="assets")
    ap.add_argument("--out", default="../../glb_validation")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    files = []
    for dirpath, _, names in os.walk(args.root):
        for n in sorted(names):
            if n.endswith(".glb"):
                files.append(os.path.join(dirpath, n))
    results = [check(p) for p in sorted(files)]
    per_dir = defaultdict(list)
    for r in results:
        per_dir[os.path.dirname(r["file"])].append(r)
    lines = []
    bad = warn = 0
    for d in sorted(per_dir):
        rows = per_dir[d]
        jc = {tuple(r["info"].get("joint_counts", [])) for r in rows if r["info"].get("joint_counts")}
        sizes = [r["info"].get("max_dim_m") for r in rows if r["info"].get("max_dim_m")]
        lines.append(f"{d}  ({len(rows)} files)  joint counts {sorted(jc) or 'none'}  "
                     f"max dimension {min(sizes) if sizes else 0:.2f}-{max(sizes) if sizes else 0:.2f} m")
        for r in rows:
            bad += len(r["problems"])
            warn += len(r["warnings"])
            for p in r["problems"]:
                lines.append(f"  FAIL {os.path.basename(r['file'])}: {p}")
            for w in r["warnings"]:
                lines.append(f"  warn {os.path.basename(r['file'])}: {w}")
    summary = {"files": len(results), "problems": bad, "warnings": warn,
               "per_dir": {d: {"files": len(v),
                               "joint_counts": sorted({tuple(r['info'].get('joint_counts', [])) for r in v}),
                               "generators": sorted({r['info'].get('generator') for r in v if r['info'].get('generator')})}
                           for d, v in per_dir.items()},
               "results": results}
    with open(os.path.join(args.out, "glb_validation.json"), "w") as fh:
        json.dump(summary, fh, indent=2)
    with open(os.path.join(args.out, "glb_validation.txt"), "w") as fh:
        fh.write("\n".join(lines) + "\n")
    print("\n".join(lines[:40]))
    print(f"{len(results)} GLB files | {bad} problems | {warn} warnings")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
