"""Convert a Kevin Iglesias animation pack (FBX, `HumanM@<Clip>.fbx`) into one GLB library.

Each FBX is exported to glTF by assimp (`assimp export in.fbx out.glb -fglb2`), then the clips are
merged into a single GLB: the first file's skeleton, every file's animation (named after the clip,
`HumanM@` dropped), all sharing one node list (the packs use one rig, `B-hips`, `B-spine`, ...; the
merge refuses files whose node names differ). The FBX scene is Z-up in centimetres, so the root
node gets a -90 deg turn about X and a 0.01 scale: Y-up metres, facing +Z, the way the game's
retarget expects (both binds T-pose, +Z forward; see `motion::retarget`, `KEVIN_MAP`).

Root-motion copies (`[RM]`) and the female rig are skipped; the male rig drives the canonical
skeleton like every other donor.

    python3 tools/model/kev_pack.py <unpacked pack dir> assets/anim/kev_melee.glb

Needs `assimp` on PATH (Ubuntu: assimp-utils).
"""
import glob
import json
import math
import os
import struct
import subprocess
import sys
import tempfile


def read_glb(path):
    b = open(path, "rb").read()
    magic, _, _ = struct.unpack_from("<III", b, 0)
    assert magic == 0x46546C67, f"{path}: not a GLB"
    off, js, binary = 12, None, b""
    while off < len(b):
        length, kind = struct.unpack_from("<II", b, off)
        chunk = b[off + 8 : off + 8 + length]
        if kind == 0x4E4F534A:
            js = json.loads(chunk)
        elif kind == 0x004E4942:
            binary = chunk
        off += 8 + length
    return js, binary


def write_glb(path, js, binary):
    text = json.dumps(js, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    binary += b"\0" * (-len(binary) % 4)
    out = struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(text) + 8 + len(binary))
    out += struct.pack("<II", len(text), 0x4E4F534A) + text
    out += struct.pack("<II", len(binary), 0x004E4942) + binary
    open(path, "wb").write(out)


def clip_name(fbx):
    name = os.path.splitext(os.path.basename(fbx))[0]
    return name.split("@", 1)[1] if "@" in name else name


def merge(clips):
    """clips: [(name, (json, bin))] -> (json, bin) with every animation on the first skeleton."""
    (_, (base, binary)) = clips[0]
    base = json.loads(json.dumps(base))
    names = [n.get("name") for n in base["nodes"]]
    base["animations"] = []
    binary = bytearray(binary)
    for name, (js, b) in clips:
        if [n.get("name") for n in js["nodes"]] != names:
            raise SystemExit(f"{name}: a different rig (node names differ from the first clip)")
        for anim in js.get("animations", []):
            pad = -len(binary) % 4
            binary += b"\0" * pad
            shift_bytes = len(binary)
            binary += b
            view0, acc0 = len(base["bufferViews"]), len(base["accessors"])
            for v in js["bufferViews"]:
                v = dict(v)
                v["byteOffset"] = v.get("byteOffset", 0) + shift_bytes
                v["buffer"] = 0
                base["bufferViews"].append(v)
            for a in js["accessors"]:
                a = dict(a)
                if "bufferView" in a:
                    a["bufferView"] += view0
                base["accessors"].append(a)
            anim = json.loads(json.dumps(anim))
            for s in anim["samplers"]:
                s["input"] += acc0
                s["output"] += acc0
            anim["name"] = name
            base["animations"].append(anim)
    base["buffers"] = [{"byteLength": len(binary)}]
    return base, bytes(binary)


def y_up_metres(js):
    """Turn the root Z-up centimetres into Y-up metres (root node matrix, column-major)."""
    c, s, k = math.cos(-math.pi / 2), math.sin(-math.pi / 2), 0.01
    rot = [[1, 0, 0], [0, c, -s], [0, s, c]]
    scene = js["scenes"][js.get("scene", 0)]
    for r in scene["nodes"]:
        node = js["nodes"][r]
        if "matrix" in node:
            m = node.pop("matrix")
            cols = [m[0:4], m[4:8], m[8:12], m[12:16]]
            old = [[cols[j][i] for j in range(4)] for i in range(4)]
        else:
            old = [[1 if i == j else 0 for j in range(4)] for i in range(4)]
            for key in ("translation", "rotation", "scale"):
                if key in node:
                    raise SystemExit("root node has TRS; expected an identity/matrix root from assimp")
        new = [[sum(rot[i][t] * k * old[t][j] for t in range(3)) for j in range(4)] for i in range(3)] + [old[3]]
        node["matrix"] = [new[i][j] for j in range(4) for i in range(4)]


def main():
    src, out = sys.argv[1], sys.argv[2]
    fbxs = sorted(f for f in glob.glob(os.path.join(src, "**", "HumanM@*.fbx"), recursive=True) if "[RM]" not in f)
    if not fbxs:
        raise SystemExit(f"no HumanM@*.fbx under {src}")
    clips = []
    with tempfile.TemporaryDirectory() as tmp:
        for i, f in enumerate(fbxs):
            g = os.path.join(tmp, f"{i}.glb")
            subprocess.run(["assimp", "export", f, g, "-fglb2"], check=True, capture_output=True)
            js, b = read_glb(g)
            if js.get("animations"):
                clips.append((clip_name(f), (js, b)))
    js, b = merge(clips)
    y_up_metres(js)
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    write_glb(out, js, b)
    print(f"{out}: {len(js['animations'])} clips: {', '.join(a['name'] for a in js['animations'])}")


if __name__ == "__main__":
    main()
