"""Bake a skinned GLB's bind pose into a static mesh GLB (a statue): no skeleton, no skin, one root.

    python3 tools/kev/statue.py <in.glb> <out.glb> [--drop PREFIX ...]

At the bind pose every skinned vertex lands at joint world x inverse bind x v, the same matrix for every joint
of a skin, so each skinned mesh's POSITION is multiplied by that matrix (NORMAL and TANGENT by its rotation part)
and the JOINTS_* / WEIGHTS_* attributes (and COLOR_* unless --keep-colors: these exports carry dark
authoring channels there that engines multiply into the base colour), the skins and the skeleton nodes' influence are dropped. Every mesh node
is moved to the scene root with its baked world transform. `--drop` removes mesh nodes whose name starts with a
prefix (the mech's spare body sets Base_S_ / Base_L_). Materials and textures are kept as they are.

Engines scale a statue like any static mesh; a skinned mesh under a scaled root distorts in some (Bevy draws a
scaled Rigify character as spikes), which is why the region statues use these.
"""
import argparse
import json
import struct

import numpy as np

COMPONENTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def read_glb(path):
    with open(path, "rb") as f:
        data = f.read()
    n, _ = struct.unpack("<II", data[12:20])
    js = json.loads(data[20:20 + n])
    rest = data[20 + n:]
    blob = bytearray()
    if len(rest) >= 8:
        bl, _ = struct.unpack("<II", rest[:8])
        blob = bytearray(rest[8:8 + bl])
    return js, blob


def write_glb(path, js, blob):
    body = json.dumps(js, separators=(",", ":")).encode()
    body += b" " * ((4 - len(body) % 4) % 4)
    blob = bytes(blob) + b"\0" * ((4 - len(blob) % 4) % 4)
    total = 12 + 8 + len(body) + 8 + len(blob)
    with open(path, "wb") as f:
        f.write(struct.pack("<III", 0x46546C67, 2, total))
        f.write(struct.pack("<II", len(body), 0x4E4F534A))
        f.write(body)
        f.write(struct.pack("<II", len(blob), 0x004E4942))
        f.write(blob)


def trs(node):
    if "matrix" in node:
        return np.array(node["matrix"], float).reshape(4, 4).T
    t = np.array(node.get("translation", [0, 0, 0]), float)
    x, y, z, w = node.get("rotation", [0, 0, 0, 1])
    s = np.array(node.get("scale", [1, 1, 1]), float)
    r = np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])
    m = np.eye(4)
    m[:3, :3] = r * s
    m[:3, 3] = t
    return m


def accessor_view(js, blob, i):
    """(numpy float32 array view onto the blob (count, comps), accessor) for a float accessor."""
    acc = js["accessors"][i]
    view = js["bufferViews"][acc["bufferView"]]
    comps = COMPONENTS[acc["type"]]
    off = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = view.get("byteStride", 0) or comps * 4
    if acc.get("componentType") != 5126:
        return None, acc
    if stride != comps * 4:
        raise SystemExit("interleaved vertex buffers are not handled")
    arr = np.frombuffer(blob, "<f4", acc["count"] * comps, off).reshape(acc["count"], comps)
    return arr, acc


def bake(js, blob, drop=(), keep_colors=False):
    scene = js["scenes"][js.get("scene", 0)]
    world = {}

    def walk(i, parent):
        world[i] = parent @ trs(js["nodes"][i])
        for c in js["nodes"][i].get("children", []):
            walk(c, world[i])

    for i in scene["nodes"]:
        walk(i, np.eye(4))
    keep, done = [], set()
    for i, node in enumerate(js["nodes"]):
        if "mesh" not in node or i not in world:
            continue
        if any(node.get("name", "").startswith(p) for p in drop):
            continue
        if "skin" in node:
            skin = js["skins"][node["skin"]]
            ibm, _ = accessor_view(js, blob, skin["inverseBindMatrices"])
            m = world.get(skin["joints"][0], np.eye(4)) @ ibm[0].reshape(4, 4).T.astype(float)
        else:
            m = world[i]
        rot = m[:3, :3]
        nrm = np.linalg.inv(rot).T
        mesh = js["meshes"][node["mesh"]]
        if node["mesh"] not in done:
            done.add(node["mesh"])
            for prim in mesh["primitives"]:
                attrs = prim["attributes"]
                pos, acc = accessor_view(js, blob, attrs["POSITION"])
                out = pos @ rot.T + m[:3, 3]
                pos[:] = out.astype(np.float32)
                acc["min"] = out.min(axis=0).tolist()
                acc["max"] = out.max(axis=0).tolist()
                for key in ("NORMAL", "TANGENT"):
                    if key in attrs:
                        v, _ = accessor_view(js, blob, attrs[key])
                        if v is not None:
                            d = v[:, :3] @ nrm.T
                            d /= np.maximum(np.linalg.norm(d, axis=1, keepdims=True), 1e-9)
                            v[:, :3] = d.astype(np.float32)
                # Vertex colours from these exports are authoring channels (COLOR_0 averages 0.11 on the
                # Rigify characters): engines multiply them into the base colour and draw the statue black.
                drop_keys = ("JOINTS_", "WEIGHTS_") + (() if keep_colors else ("COLOR_",))
                for key in [k for k in attrs if k.startswith(drop_keys)]:
                    del attrs[key]
        keep.append({"name": node.get("name", f"mesh{i}"), "mesh": node["mesh"]})
    js["nodes"] = keep
    js["scenes"] = [{"nodes": list(range(len(keep)))}]
    js["scene"] = 0
    js.pop("skins", None)
    js.pop("animations", None)
    return js


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("dst")
    ap.add_argument("--drop", nargs="*", default=[])
    ap.add_argument("--keep-colors", action="store_true", help="keep COLOR_* vertex attributes")
    args = ap.parse_args()
    js, blob = read_glb(args.src)
    js = bake(js, blob, tuple(args.drop), args.keep_colors)
    write_glb(args.dst, js, blob)
    print(f"{args.dst}: {len(js['nodes'])} static mesh nodes")


if __name__ == "__main__":
    main()
