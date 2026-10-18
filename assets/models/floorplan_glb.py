#!/usr/bin/env python3
"""floorplan_glb.py - turn guildhouse_layout.json into real 3D floor-plan GLBs.

Every object in the layout becomes a box at its own x, y, z, coloured by kind, and each floor is a
separate node so it can be toggled or moved in any viewer. Two scenes are written:

  guildhouse_3d_all.glb       floors stacked where they really are - a 3D section of the building
  guildhouse_3d_exploded.glb  the same floors lifted 6 m apart, so each plan reads on its own
  guildhouse_3d_<floor>.glb   one file per floor, in place

Front faces are wound outward by construction: each triangle's normal is checked against the
direction from the box centre and flipped when it faces inward, so nothing relies on the ordering
being hand-derived. Lights carry an emissive factor; fx-only objects are skipped, since the particle
system owns them.

    ../venv/bin/python tools/model/floorplan_glb.py --layout ../../guildhouse/assets/data/guildhouse_layout.json \
        --out ../../guildhouse/assets/models
"""
from __future__ import annotations

import argparse
import json
import os
import struct
from collections import defaultdict

import numpy as np
from pygltflib import (GLTF2, Accessor, Asset, Buffer, BufferView, Material, Mesh, Node,
                       PbrMetallicRoughness, Primitive, Scene)

COLOURS = {
    "structure": ([0.55, 0.55, 0.58, 1.0], None),
    "furniture": ([0.45, 0.33, 0.20, 1.0], None),
    "prop": ([0.60, 0.52, 0.38, 1.0], None),
    "banner": ([0.20, 0.30, 0.55, 1.0], None),
    "aperture": ([0.18, 0.90, 0.77, 1.0], [0.05, 0.25, 0.20]),
    "light": ([1.00, 0.85, 0.35, 1.0], [0.60, 0.45, 0.10]),
}
FACE = [(0, 1, 2, 3), (4, 5, 6, 7), (0, 1, 5, 4), (2, 3, 7, 6), (1, 2, 6, 5), (0, 3, 7, 4)]


def box(centre, size):
    """36 vertices and 12 triangles for one box, outward-wound."""
    c = np.asarray(centre, np.float64)
    h = np.asarray(size, np.float64) / 2.0
    corners = np.array([[c[0] + sx * h[0], c[1] + sy * h[1], c[2] + sz * h[2]]
                        for sx in (-1, 1) for sy in (-1, 1) for sz in (-1, 1)])
    verts, norms, tris = [], [], []
    for f in FACE:
        quad = corners[list(f)]
        for tri in ((0, 1, 2), (0, 2, 3)):
            p = [quad[i] for i in tri]
            n = np.cross(p[1] - p[0], p[2] - p[0])
            if np.dot(n, quad.mean(0) - c) < 0:      # faces inward: flip the winding
                p = [p[0], p[2], p[1]]
                n = -n
            n = n / max(np.linalg.norm(n), 1e-9)
            base = len(verts)
            verts.extend(p)
            norms.extend([n] * 3)
            tris.extend([base, base + 1, base + 2])
    return np.array(verts, np.float32), np.array(norms, np.float32), np.array(tris, np.uint32)


def default_size(kind):
    return {"structure": (1.0, 1.0, 1.0), "furniture": (1.2, 0.75, 0.6), "prop": (0.3, 0.3, 0.3),
            "banner": (0.1, 4.0, 1.2), "aperture": (0.4, 2.0, 1.2), "light": (0.25, 0.25, 0.25)}.get(kind, (0.3, 0.3, 0.3))


def build(objects, y_offset=0.0):
    """One mesh with a primitive per kind."""
    by_kind = defaultdict(lambda: ([], [], []))
    for o in objects:
        if o.get("fx_only"):
            continue
        size = (o.get("sx") or default_size(o["kind"])[0],
                o.get("sy") or default_size(o["kind"])[1],
                o.get("sz") or default_size(o["kind"])[2])
        v, n, t = box((o["x"], o["y"] + y_offset, o["z"]), size)
        acc = by_kind[o["kind"]]
        acc[0].append(v)
        acc[1].append(n)
        acc[2].append(t + (sum(len(x) for x in acc[0][:-1]) if acc[0] else 0))
    return {k: (np.concatenate(v), np.concatenate(n), np.concatenate(t)) for k, (v, n, t) in by_kind.items()}


def write_glb(path, per_kind, generator="Guildbound guildhouse floorplan from guildhouse_layout.json"):
    gltf = GLTF2(asset=Asset(generator=generator), scene=0)
    blob = bytearray()
    accessors, views, prims, materials = [], [], [], []

    def add_view(data, target):
        off = len(blob)
        blob.extend(data.tobytes())
        while len(blob) % 4:
            blob.append(0)
        views.append(BufferView(buffer=0, byteOffset=off, byteLength=int(data.nbytes), target=target))
        return len(views) - 1

    for kind, (v, n, t) in sorted(per_kind.items()):
        colour, emissive = COLOURS.get(kind, COLOURS["prop"])
        materials.append(Material(name=kind, pbrMetallicRoughness=PbrMetallicRoughness(baseColorFactor=colour),
                                  emissiveFactor=emissive))
        vv, nv, tv = add_view(v, 34962), add_view(n, 34962), add_view(t.astype(np.uint16), 34963)
        acc_v = len(accessors)
        accessors.append(Accessor(bufferView=vv, componentType=5126, count=len(v), type="VEC3",
                                  min=[float(x) for x in v.min(0)], max=[float(x) for x in v.max(0)]))
        accessors.append(Accessor(bufferView=nv, componentType=5126, count=len(n), type="VEC3"))
        acc_t = len(accessors)
        accessors.append(Accessor(bufferView=tv, componentType=5123, count=len(t), type="SCALAR"))
        prims.append(Primitive(attributes={"POSITION": acc_v, "NORMAL": acc_v + 1}, indices=acc_t,
                               material=len(materials) - 1))
    gltf.meshes.append(Mesh(name="floorplan", primitives=prims))
    gltf.nodes.append(Node(name="floorplan", mesh=0))
    gltf.scenes.append(Scene(nodes=[0]))
    gltf.accessors = accessors
    gltf.bufferViews = views
    gltf.materials = materials
    gltf.buffers = [Buffer(byteLength=len(blob))]
    gltf.set_binary_blob(bytes(blob))
    gltf.save_binary(path)
    return len(blob), sum(a.count for a in accessors if a.type == "VEC3") // 2


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--layout", default="../../guildhouse/assets/data/guildhouse_layout.json")
    ap.add_argument("--out", default="../../guildhouse/assets/models")
    ap.add_argument("--gap", type=float, default=6.0)
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    doc = json.load(open(args.layout))
    objs = doc["objects"]
    floors = sorted({o["floor"] for o in objs})
    written = []

    def emit(name, subset):
        per = build(subset)
        path = os.path.join(args.out, name)
        size, verts = write_glb(path, per)
        written.append((name, len(subset), verts, size, sorted(per)))

    emit("guildhouse_3d_all.glb", objs)
    # exploded: lift each floor by its index, so the plans read apart
    lifted = []
    for o in objs:
        o2 = dict(o)
        o2["y"] = o["y"] + floors.index(o["floor"]) * args.gap
        lifted.append(o2)
    emit("guildhouse_3d_exploded.glb", lifted)
    for f in floors:
        emit(f"guildhouse_3d_{f}.glb", [o for o in objs if o["floor"] == f])
    for name, n, verts, size, kinds in written:
        print(f"  {name:34s} {n:3d} objects | {verts:5d} verts | {size / 1024:6.1f} KiB | kinds {','.join(kinds)}")
    print(f"wrote {len(written)} GLBs to {args.out}")
    print("floors:", floors)


if __name__ == "__main__":
    main()
