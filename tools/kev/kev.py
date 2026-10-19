"""Kev: sort a folder of model files into categories by k-means on what the files contain.

    python3 tools/kev/kev.py <asset dir> <out dir> [--kmax 8] [--seed 1]

Reads each GLB's JSON header (no mesh decode): file size, node / mesh / clip counts, and the bone
count (joints of the first skin, else 0). Other formats (FBX, OBJ, BLEND, archives) are listed with
size only, plus bones when `assimp info` is available. Features are standardised and clustered with
k-means; k is picked from 2..kmax by silhouette score, so the number of categories is not fixed.

Writes, under <out dir>:
  kev_index.json   every asset with its features and category, plus the rule used (reproducible)
  notes/<asset>.txt   one text note per asset: name, path, bones, clips, size, category
  categories/<name>/  symlinks to the originals, one folder per category (originals never move)

Stdlib plus numpy only.
"""
import argparse
import collections
import hashlib
import json
import math
import os
import re
import shutil
import struct
import subprocess

import numpy as np

MODEL_EXT = {".glb", ".gltf", ".fbx", ".obj", ".blend"}
ARCHIVE_EXT = {".zip", ".rar"}
FEATURES = ["log_bytes", "log_bones", "log_clips", "log_meshes", "log_vertices"]


def read_glb_header(path):
    """JSON chunk of a GLB, or None if the file is not a GLB."""
    with open(path, "rb") as f:
        head = f.read(20)
        if len(head) < 20 or struct.unpack_from("<I", head, 0)[0] != 0x46546C67:
            return None
        length, kind = struct.unpack_from("<II", head, 12)
        if kind != 0x4E4F534A:
            return None
        return json.loads(f.read(length))


def glb_vertices(js):
    """Vertices across all mesh primitives: the POSITION accessor counts (no buffer read needed)."""
    accessors = js.get("accessors", [])
    return sum(
        accessors[prim["attributes"]["POSITION"]]["count"]
        for mesh in js.get("meshes", [])
        for prim in mesh.get("primitives", [])
        if "POSITION" in prim.get("attributes", {})
    )


def assimp_counts(path):
    """Bones, clips and meshes from `assimp info`, or None when assimp is missing or fails."""
    if shutil.which("assimp") is None:
        return None
    try:
        out = subprocess.run(["assimp", "info", path], capture_output=True, text=True, timeout=120).stdout
    except (subprocess.TimeoutExpired, OSError):
        return None
    found = {}
    for key, pattern in (("bones", r"^Bones:\s*(\d+)"), ("clips", r"^Animations:\s*(\d+)"), ("meshes", r"^Meshes:\s*(\d+)")):
        m = re.search(pattern, out, re.MULTILINE)
        found[key] = int(m.group(1)) if m else 0
    # An animation-only export reports no bones even when its hierarchy has them, so 0 means unknown.
    found["bones"] = found["bones"] or None
    return found


def file_sha1(path):
    h = hashlib.sha1()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def describe(path, rel):
    """One asset's facts and feature vector."""
    ext = os.path.splitext(path)[1].lower()
    info = {"path": rel, "name": os.path.splitext(os.path.basename(path))[0], "ext": ext, "bytes": os.path.getsize(path)}
    if ext in ARCHIVE_EXT:
        info.update(kind="archive", bones=None, clips=None, meshes=None)
    elif ext == ".glb":
        js = read_glb_header(path)
        if js is None:
            info.update(kind="unreadable", bones=None, clips=None, meshes=None)
        else:
            skins = js.get("skins", [])
            info.update(
                kind="model",
                bones=len(skins[0].get("joints", [])) if skins else 0,
                clips=len(js.get("animations", [])),
                meshes=len(js.get("meshes", [])),
                vertices=glb_vertices(js),
                clip_names=[a.get("name", "") for a in js.get("animations", [])],
            )
    else:
        counts = assimp_counts(path) if ext in MODEL_EXT else None
        info.update(kind="model" if ext in MODEL_EXT else "other", bones=None, clips=None, meshes=None)
        if counts:
            info.update(bones=counts["bones"], clips=counts["clips"], meshes=counts["meshes"])
    info["sha1"] = file_sha1(path)
    info["features"] = features_of(info)
    return info


def features_of(info):
    """Standardised-later numeric features; missing counts become 0 so the file still clusters."""
    return [
        math.log10(max(info["bytes"], 1)),
        math.log10(1 + (info.get("bones") or 0)),
        math.log10(1 + (info.get("clips") or 0)),
        math.log10(1 + (info.get("meshes") or 0)),
        math.log10(1 + (info.get("vertices") or 0)),
    ]


def standardise(x):
    mu, sd = x.mean(axis=0), x.std(axis=0)
    sd[sd == 0] = 1.0
    return (x - mu) / sd


def kmeans(x, k, seed, iters=100):
    """Lloyd's algorithm with k-means++ seeding. Returns (labels, centres)."""
    rng = np.random.default_rng(seed)
    centres = [x[rng.integers(len(x))]]
    while len(centres) < k:
        d2 = np.min([((x - c) ** 2).sum(axis=1) for c in centres], axis=0)
        if d2.sum() == 0:
            centres.append(x[rng.integers(len(x))])
            continue
        centres.append(x[rng.choice(len(x), p=d2 / d2.sum())])
    centres = np.array(centres)
    labels = np.zeros(len(x), dtype=int)
    for _ in range(iters):
        dist = ((x[:, None, :] - centres[None, :, :]) ** 2).sum(axis=2)
        new = dist.argmin(axis=1)
        if np.array_equal(new, labels) and _ > 0:
            break
        labels = new
        for j in range(k):
            if np.any(labels == j):
                centres[j] = x[labels == j].mean(axis=0)
    return labels, centres


def silhouette(x, labels):
    """Mean silhouette score; higher means tighter, better-separated clusters."""
    if len(set(labels)) < 2:
        return -1.0
    dist = np.sqrt(((x[:, None, :] - x[None, :, :]) ** 2).sum(axis=2))
    scores = []
    for i in range(len(x)):
        same = labels == labels[i]
        same[i] = False
        a = dist[i, same].mean() if same.any() else 0.0
        b = min(dist[i, labels == j].mean() for j in set(labels) if j != labels[i])
        scores.append((b - a) / max(a, b, 1e-9))
    return float(np.mean(scores))


def choose_clusters(x, kmax, seed):
    """k in 2..kmax with the best silhouette; falls back to one cluster for very few assets."""
    if len(x) < 3:
        return np.zeros(len(x), dtype=int), 1
    best = None
    for k in range(2, min(kmax, len(x) - 1) + 1):
        labels, _ = kmeans(x, k, seed)
        score = silhouette(x, labels)
        if best is None or score > best[0]:
            best = (score, labels, k)
    return best[1], best[2]


def bone_band(bones):
    if bones is None:
        return "rig-unknown"
    if not bones:
        return "unrigged"
    if bones < 30:
        return "rigged-small"
    if bones < 80:
        return "rigged-medium"
    return "rigged-large"


GENERIC = {"hero", "male", "female", "base", "packs", "characters", "anim", "models", "fbx", "glb", "free", "standard", "the", "of", "and", "v", "a"}


def category_names(assets, labels):
    """Name each cluster from the words its file names share, plus its bone band: 'hair-buzzed-rigged-medium'."""
    names = {}
    for cluster in sorted(set(labels)):
        members = [a for a, l in zip(assets, labels) if l == cluster]
        words = collections.Counter()
        for a in members:
            stem = os.path.splitext(a["path"])[0].lower()
            tokens = {w for w in re.split(r"[/\\_\-. +]+", stem) if w and not w.isdigit() and w not in GENERIC}
            words.update(tokens)
        bands = collections.Counter(bone_band(a.get("bones")) for a in members)
        top = [w for w, _ in words.most_common(2)] or ["misc"]
        names[cluster] = f"{'-'.join(top)}-{bands.most_common(1)[0][0]}"
    seen = collections.Counter()
    for cluster in sorted(names):
        seen[names[cluster]] += 1
        if seen[names[cluster]] > 1:
            names[cluster] = f"{names[cluster]}-{seen[names[cluster]]}"
    return names


def scan(root):
    found = []
    for dirpath, _, files in os.walk(root):
        for name in sorted(files):
            if os.path.splitext(name)[1].lower() in MODEL_EXT | ARCHIVE_EXT:
                full = os.path.join(dirpath, name)
                found.append((full, os.path.relpath(full, root)))
    return found


def run(root, out, kmax, seed):
    entries = [describe(full, rel) for full, rel in scan(root)]
    if not entries:
        raise SystemExit(f"no model or archive files under {root}")

    # Archives have no header to read, so they stay out of the clustering and get their own category.
    clustered = [e for e in entries if e["kind"] in ("model", "other", "unreadable")]
    x = standardise(np.array([e["features"] for e in clustered]))
    labels, k = choose_clusters(x, kmax, seed)
    names = category_names(clustered, labels)
    for e, l in zip(clustered, labels):
        e["category"] = names[l]
    for e in entries:
        e.setdefault("category", "archives" if e["kind"] == "archive" else names.get(0, "misc"))

    shutil.rmtree(os.path.join(out, "categories"), ignore_errors=True)
    shutil.rmtree(os.path.join(out, "notes"), ignore_errors=True)
    os.makedirs(os.path.join(out, "notes"), exist_ok=True)
    first_by_hash = {}
    for e in entries:
        e["duplicate_of"] = first_by_hash.get(e["sha1"])
        first_by_hash.setdefault(e["sha1"], e["path"])
    for e in entries:
        folder = os.path.join(out, "categories", e["category"])
        os.makedirs(folder, exist_ok=True)
        link = os.path.join(folder, os.path.basename(e["path"]))
        if not os.path.lexists(link):
            os.symlink(os.path.join(root, e["path"]), link)
        note = os.path.join(out, "notes", e["path"].replace(os.sep, "__") + ".txt")
        with open(note, "w") as f:
            f.write(f"name: {e['name']}\npath: {e['path']}\ncategory: {e['category']}\n")
            f.write(f"bones: {e.get('bones') if e.get('bones') is not None else 'unknown'}\n")
            f.write(f"clips: {e.get('clips') if e.get('clips') is not None else 'unknown'}\n")
            f.write(f"meshes: {e.get('meshes') if e.get('meshes') is not None else 'unknown'}\n")
            f.write(f"bytes: {e['bytes']}\n")
            if e["duplicate_of"]:
                f.write(f"same content as: {e['duplicate_of']}\n")
            for clip in e.get("clip_names", []):
                f.write(f"clip: {clip}\n")

    clusters = []
    for cluster in sorted(set(labels)):
        members = [e["path"] for e, l in zip(clustered, labels) if l == cluster]
        clusters.append({"id": int(cluster), "name": names[cluster], "size": len(members)})
    index = {
        "rule": {"features": FEATURES, "standardised": True, "k": int(k), "seed": seed, "kmax": kmax},
        "clusters": clusters,
        "assets": [{k_: v for k_, v in e.items() if k_ != "features"} for e in entries],
    }
    with open(os.path.join(out, "kev_index.json"), "w") as f:
        json.dump(index, f, indent=2)
    return index


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("root")
    ap.add_argument("out")
    ap.add_argument("--kmax", type=int, default=8)
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    index = run(args.root, args.out, args.kmax, args.seed)
    print(f"{len(index['assets'])} assets, k={index['rule']['k']} categories:")
    for c in index["clusters"]:
        print(f"  {c['name']:<40} {c['size']:>4}")


if __name__ == "__main__":
    main()
