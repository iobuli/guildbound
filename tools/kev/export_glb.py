"""Append baked Kev animations to a rig GLB as glTF animations, so the game can load them as a donor library.

    python3 -I export_glb.py BASE.glb OUT.glb name=Clip_Name [name=Clip_Name ...] [--fps 30]

`name` is a baked track in staging/kev/baked (`<rig>__<name>.json`, the rig is the one named in BASE's sibling bake,
default karate): per-frame local rotations for each bone the solver drove, plus the pelvis translation. Only the JSON
chunk and the binary blob are rewritten; the base file's skin, mesh and existing animations are kept.
"""
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import statue  # noqa: E402

BAKED = os.path.join(HERE, "..", "..", "staging", "kev", "baked")


def add_accessor(js, blob, data, comps, kind, minmax=False):
    arr = np.ascontiguousarray(data, "<f4")
    while len(blob) % 4:
        blob.append(0)
    off = len(blob)
    blob.extend(arr.tobytes())
    js["bufferViews"].append({"buffer": 0, "byteOffset": off, "byteLength": arr.nbytes})
    acc = {"bufferView": len(js["bufferViews"]) - 1, "componentType": 5126, "count": int(arr.shape[0]), "type": kind}
    if minmax:
        acc["min"], acc["max"] = [float(arr.min())], [float(arr.max())]
    js["accessors"].append(acc)
    return len(js["accessors"]) - 1


def main():
    base, out = sys.argv[1], sys.argv[2]
    fps = 30
    pairs = []
    args = sys.argv[3:]
    if "--fps" in args:
        i = args.index("--fps")
        fps = int(args[i + 1])
        args = args[:i] + args[i + 2:]
    for a in args:
        k, v = a.split("=")
        pairs.append((k, v))
    js, blob = statue.read_glb(base)
    blob = bytearray(blob)
    idx = {nd.get("name"): i for i, nd in enumerate(js["nodes"])}
    for src, clip in pairs:
        b = json.load(open(os.path.join(BAKED, f"karate__{src}.json")))
        step = max(1, round(b["fps"] / fps))
        frames = b["frames"][::step]
        if (len(b["frames"]) - 1) % step:
            frames = frames + [b["frames"][-1]]
        times = np.array([k * step / b["fps"] for k in range(len(frames))], float)
        times[-1] = b["duration"]
        tacc = add_accessor(js, blob, times, 1, "SCALAR", minmax=True)
        chans, samps = [], []
        for bone in sorted({k for f in frames for k in f["q"]}):
            if bone not in idx:
                continue
            q = np.array([f["q"].get(bone, f["q"].get(bone)) for f in frames if bone in f["q"]], float)
            if len(q) != len(frames):
                continue
            q = q / np.linalg.norm(q, axis=1, keepdims=True)
            for i in range(1, len(q)):  # keep neighbouring quaternions in one hemisphere so slerp takes the short way
                if q[i] @ q[i - 1] < 0:
                    q[i] = -q[i]
            samps.append({"input": tacc, "output": add_accessor(js, blob, q, 4, "VEC4"), "interpolation": "LINEAR"})
            chans.append({"sampler": len(samps) - 1, "target": {"node": idx[bone], "path": "rotation"}})
        p = np.array([f["p"] for f in frames], float)
        samps.append({"input": tacc, "output": add_accessor(js, blob, p, 3, "VEC3"), "interpolation": "LINEAR"})
        chans.append({"sampler": len(samps) - 1, "target": {"node": idx["pelvis"], "path": "translation"}})
        js.setdefault("animations", []).append({"name": clip, "channels": chans, "samplers": samps})
        print(f"{clip}: {len(frames)} keys, {len(chans)} channels")
    js["buffers"][0]["byteLength"] = len(blob)
    statue.write_glb(out, js, blob)
    print("wrote", out, os.path.getsize(out) // 1024, "KB")


if __name__ == "__main__":
    main()
