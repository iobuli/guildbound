"""Survey a folder of models in parallel: usability scores, outliers, and how much data Kev needs.

    python3 tools/kev/survey.py <asset dir> <out dir> [--reference assets/anim/ual2.glb] [--jobs 4]

Each GLB is analysed on its own process (one per CPU). For each file it reads:
  - bones and how many match the game's canonical skeleton (the reference GLB's joint names),
  - clips: duration, keyframes, and jitter (the p95 of per-key angular acceleration, in degrees),
  - the skinned mesh's height in metres (from POSITION bounds), vertices and triangles,
  - a content hash, so duplicates are found.
It then gives each file a verdict (usable, clip-only, rig-mismatch, static, check-units, jittery,
duplicate, unreadable) and checks how stable the Kev categories are on random subsamples: the
adjusted Rand index against the full run tells you how many assets are needed before the groups
stop changing.

Writes <out dir>/survey.json and prints a summary. Numpy and the standard library only.
"""
import argparse
import collections
import json
import math
import multiprocessing as mp
import os
import struct
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kev  # noqa: E402

CANON_MATCH = 0.8
HUMAN_HEIGHT = (0.5, 3.0)
JITTER_RATIO = 3.0


def read_glb(path):
    """(json, binary) for a GLB; the binary chunk is needed for keyframe values."""
    data = open(path, "rb").read()
    off, js, binary = 12, None, b""
    while off < len(data):
        length, kind = struct.unpack_from("<II", data, off)
        chunk = data[off + 8 : off + 8 + length]
        if kind == 0x4E4F534A:
            js = json.loads(chunk)
        elif kind == 0x004E4942:
            binary = chunk
        off += 8 + length
    return js, binary


def floats(js, binary, index):
    acc = js["accessors"][index]
    width = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}[acc["type"]]
    view = js["bufferViews"][acc["bufferView"]]
    base = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = view.get("byteStride", width * 4)
    arr = np.empty((acc["count"], width), dtype=np.float64)
    for i in range(acc["count"]):
        arr[i] = struct.unpack_from(f"<{width}f", binary, base + i * stride)
    return arr


def angle_steps(q):
    """Degrees between consecutive quaternions (a row per key)."""
    q = q / np.linalg.norm(q, axis=1, keepdims=True).clip(1e-9)
    d = np.abs(np.sum(q[:-1] * q[1:], axis=1)).clip(0, 1)
    return np.degrees(2 * np.arccos(d))


def jitter_p95(js, binary):
    """Jitter as a ratio: p95 of per-key angular acceleration divided by that track's mean step.

    A fast move has large acceleration in degrees, so the raw number mostly measures speed. Dividing
    by the track's own mean step makes a sprint and an idle comparable: a smooth curve stays near 1,
    a curve with spikes or seams goes well above it.
    """
    ratios = []
    for anim in js.get("animations", []):
        for ch in anim["channels"]:
            if ch["target"].get("path") != "rotation":
                continue
            q = floats(js, binary, anim["samplers"][ch["sampler"]]["output"])
            if len(q) < 4:
                continue
            steps = angle_steps(q)
            mean = float(np.mean(steps))
            if mean < 1e-3:
                continue
            ratios.append(np.abs(np.diff(steps)) / mean)
    if not ratios:
        return None
    return float(np.percentile(np.concatenate(ratios), 95))


def mesh_facts(js):
    """Vertices, triangles and height (m) of the mesh, from POSITION accessor bounds."""
    vertices = triangles = 0
    lo, hi = np.full(3, np.inf), np.full(3, -np.inf)
    for mesh in js.get("meshes", []):
        for prim in mesh.get("primitives", []):
            pos = prim.get("attributes", {}).get("POSITION")
            if pos is None:
                continue
            acc = js["accessors"][pos]
            vertices += acc["count"]
            triangles += acc["count"] // 3 if "indices" not in prim else js["accessors"][prim["indices"]]["count"] // 3
            if "min" in acc and "max" in acc:
                lo = np.minimum(lo, acc["min"])
                hi = np.maximum(hi, acc["max"])
    height = float(hi[1] - lo[1]) if vertices else None
    return vertices, triangles, height


def analyse(item):
    """One file's facts. Runs in a worker process."""
    path, rel = item
    try:
        info = kev.describe(path, rel)
        out = {k: info.get(k) for k in ("path", "name", "ext", "bytes", "sha1", "kind", "bones", "clips", "meshes", "vertices")}
        if info["ext"] != ".glb" or info["kind"] != "model":
            return out
        js, binary = read_glb(path)
        joints = [js["nodes"][j].get("name", "") for j in js.get("skins", [{}])[0].get("joints", [])] if js.get("skins") else []
        out["joint_names"] = joints
        out["clip_list"] = [
            {"name": a.get("name", ""), "keys": int(max((js["accessors"][s["input"]]["count"] for s in a["samplers"]), default=0))}
            for a in js.get("animations", [])
        ]
        out["jitter_p95"] = jitter_p95(js, binary) if js.get("animations") else None
        verts, tris, height = mesh_facts(js)
        out.update(vertices=verts, triangles=tris, height=height)
        return out
    except Exception as exc:  # one bad file must not stop the survey
        return {"path": rel, "name": os.path.basename(rel), "kind": "unreadable", "error": str(exc)}


def reference_names(path):
    js, _ = read_glb(path)
    return {js["nodes"][j].get("name", "") for j in js["skins"][0]["joints"]}


def donor_family(names):
    """Which naming family a foreign rig uses: the game retargets each family with its own map."""
    joined = " ".join(names)
    if "mixamorig:" in joined:
        return "mixamo"
    if "B-hips" in joined:
        return "kevin"
    if "Hips" in names or "Spine" in names:
        return "cmu-or-generic"
    return "other"


def verdict(a, ref):
    if a.get("kind") == "unreadable":
        return "unreadable"
    if a.get("duplicate_of"):
        return "duplicate"
    if a.get("kind") == "archive":
        return "archive (extract first)"
    if a.get("ext") != ".glb":
        # Only GLB headers are read here; FBX/OBJ bones come from assimp and are not reliable.
        return "unverified (not GLB)"
    if not a.get("bones"):
        return "clip-only" if a.get("clip_list") else "static"
    names = a.get("joint_names", [])
    match = len(set(names) & ref) / max(len(ref), 1)
    a["canon_match"] = round(match, 3)
    height = a.get("height")
    if height is not None and height > HUMAN_HEIGHT[1]:
        return "check-units"
    if match < CANON_MATCH:
        return f"donor-{donor_family(names)}"
    if height is not None and height < HUMAN_HEIGHT[0]:
        return "outfit-piece"
    if a.get("jitter_p95") is not None and a["jitter_p95"] > JITTER_RATIO:
        return "jittery"
    return "usable"


def adjusted_rand(a, b):
    """Adjusted Rand index between two labellings of the same items (1 = identical, ~0 = chance)."""
    pairs = collections.Counter(zip(a, b))
    rows = collections.Counter(a)
    cols = collections.Counter(b)
    n = len(a)
    choose = lambda x: x * (x - 1) / 2  # noqa: E731
    index = sum(choose(v) for v in pairs.values())
    expected = sum(choose(v) for v in rows.values()) * sum(choose(v) for v in cols.values()) / max(choose(n), 1)
    best = (sum(choose(v) for v in rows.values()) + sum(choose(v) for v in cols.values())) / 2
    return 1.0 if best == expected else (index - expected) / (best - expected)


def stability(x, seed, fractions=(0.25, 0.5, 0.75, 1.0), repeats=5):
    """Median adjusted Rand index of Kev's clustering on subsamples, against the full run."""
    full_labels, _ = kev.choose_clusters(x, kmax=8, seed=seed)
    rows = []
    for frac in fractions:
        scores = []
        for r in range(repeats if frac < 1 else 1):
            rng = np.random.default_rng(seed + r)
            idx = np.sort(rng.choice(len(x), size=max(3, int(frac * len(x))), replace=False))
            sub_labels, _ = kev.choose_clusters(x[idx], kmax=8, seed=seed)
            scores.append(adjusted_rand(sub_labels, full_labels[idx]))
        rows.append({"fraction": frac, "items": int(max(3, frac * len(x))), "median_ari": round(float(np.median(scores)), 3)})
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("out")
    ap.add_argument("--reference", default="assets/anim/ual2.glb")
    ap.add_argument("--jobs", type=int, default=os.cpu_count())
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)

    ref = reference_names(args.reference)
    items = [(full, rel) for full, rel in kev.scan(args.root)]
    with mp.Pool(args.jobs) as pool:
        facts = list(pool.imap_unordered(analyse, items, chunksize=2))
    facts.sort(key=lambda a: a["path"])

    first = {}
    for a in facts:
        a["duplicate_of"] = first.get(a.get("sha1"))
        first.setdefault(a.get("sha1"), a["path"])
        a["verdict"] = verdict(a, ref)

    clustered = [a for a in facts if a.get("kind") in ("model", "unreadable") and a.get("verdict") != "unreadable"]
    feats = np.array([kev.features_of(a) for a in clustered])
    stab = None
    if len(clustered) >= 6:
        stab = stability(kev.standardise(feats), args.seed)

    summary = {
        "assets": len(facts),
        "cpus": args.jobs,
        "verdicts": dict(collections.Counter(a["verdict"] for a in facts)),
        "bones": _pct([a["bones"] for a in facts if a.get("bones")]),
        "height_m": _pct([a["height"] for a in facts if a.get("height")]),
        "jitter_p95_deg": _pct([a["jitter_p95"] for a in facts if a.get("jitter_p95") is not None]),
        "stability": stab,
    }
    with open(os.path.join(args.out, "survey.json"), "w") as f:
        json.dump({"summary": summary, "assets": facts}, f, indent=2, default=float)

    print(f"{summary['assets']} assets on {args.jobs} processes")
    for v, n in sorted(summary["verdicts"].items(), key=lambda kv: -kv[1]):
        print(f"  {v:<26} {n:>4}")
    print("bones   ", summary["bones"])
    print("height m", summary["height_m"])
    print("jitter  ", summary["jitter_p95_deg"])
    if stab:
        print("stability (median adjusted Rand vs full run):")
        for row in stab:
            print(f"  {int(row['fraction'] * 100):>3}% = {row['items']:>3} assets  ARI {row['median_ari']}")


def _pct(values):
    if not values:
        return None
    v = np.array(values, dtype=float)
    return {p: round(float(np.percentile(v, q)), 3) for p, q in (("p5", 5), ("median", 50), ("p95", 95))}


if __name__ == "__main__":
    main()
