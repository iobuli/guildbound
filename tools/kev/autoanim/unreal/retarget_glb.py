"""Retarget decoded Sparking ZERO clips onto a Kev UAL mannequin GLB.

python retarget_glb.py --base assets/anim/ual2.glb --output assets/anim/sparking_preview.glb CLIP.json ...
"""
import argparse
import json
from pathlib import Path
import sys

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import statue
from export_glb import add_accessor

MAP = {"pelvis": "WAIST", "spine_01": "SPINE1", "spine_02": "SPINE2", "spine_03": "SPINE3",
       "neck_01": "NECK", "Head": "HEAD"}
for side in "lr":
    for target, source in (("thigh", "THIGH"), ("calf", "CLANK"), ("foot", "TOE1"), ("ball", "TOE2"),
                           ("clavicle", "CLAVICLE"), ("upperarm", "SHOULDER"),
                           ("lowerarm", "ELBOW"), ("hand", "WRIST")):
        MAP[f"{target}_{side}"] = f"{source}_{side.upper()}"
    for target, source in (("thumb", "THUMB"), ("index", "FORE"), ("middle", "MIDDLE"),
                           ("ring", "MEDICINAL"), ("pinky", "LITTLE")):
        for key in range(1, 4):
            MAP[f"{target}_{key:02d}_{side}"] = f"F_{source}{key}_{side.upper()}"

# This game's master skeleton uses +X anatomical left, +Y forward, +Z up.
# Preserve its authored basis rather than assuming UE's conventional +X forward.
BASIS = np.array([[1, 0, 0], [0, 0, 1], [0, 1, 0]], float)


def matrix(pose):
    return statue.trs({"rotation": pose["q"], "translation": pose["p"], "scale": pose["s"]})


def world(local, parents):
    output = [None] * len(local)
    def visit(i):
        if output[i] is None:
            parent = parents[i]
            output[i] = local[i] if parent < 0 else visit(parent) @ local[i]
        return output[i]
    for i in range(len(local)):
        visit(i)
    return output


def quaternion(m):
    # Eigenvector extraction works for every proper rotation, including 180 degrees.
    m = m[:3, :3]
    u, _, vt = np.linalg.svd(m)
    m = u @ vt
    x, y, z = m[0, 0], m[1, 1], m[2, 2]
    k = np.array([[x-y-z, m[1,0]+m[0,1], m[2,0]+m[0,2], m[2,1]-m[1,2]],
                  [m[1,0]+m[0,1], y-x-z, m[2,1]+m[1,2], m[0,2]-m[2,0]],
                  [m[2,0]+m[0,2], m[2,1]+m[1,2], z-x-y, m[1,0]-m[0,1]],
                  [m[2,1]-m[1,2], m[0,2]-m[2,0], m[1,0]-m[0,1], x+y+z]]) / 3
    _, vectors = np.linalg.eigh(k)
    return vectors[:, -1]


def convert(base, output, sources, sequence=None, animation_only=False):
    js, blob = statue.read_glb(base)
    js["animations"] = []
    if animation_only:
        # The game donor loader reads skin 0's joint order even when there is no mesh.
        # Keep that skeleton metadata; inverse-bind accessors refer to buffers discarded below.
        js["skins"] = [{key: skin[key] for key in ("name", "skeleton", "joints") if key in skin}
                       for skin in js.get("skins", [])]
        for node in js["nodes"]:
            node.pop("mesh", None)
            node.pop("skin", None)
        for key in ("meshes", "materials", "textures", "images", "samplers", "extensionsUsed", "extensionsRequired"):
            js.pop(key, None)
        js["accessors"], js["bufferViews"], js["buffers"] = [], [], [{"byteLength": 0}]
        blob = bytearray()
    dst_index = {n.get("name"): i for i, n in enumerate(js["nodes"])}
    parents = [-1] * len(js["nodes"])
    for i, node in enumerate(js["nodes"]):
        for child in node.get("children", []):
            parents[child] = i
    rest = [statue.trs(n) for n in js["nodes"]]
    dst_world = world(rest, parents)
    reports = []
    inputs = [json.loads(Path(source).read_text()) for source in sources] if sequence else None
    if sequence:
        merged = dict(inputs[0])
        merged["name"] = sequence
        merged["source"] = " + ".join(data["source"] for data in inputs)
        frames, offset = [], 0.0
        for data in inputs:
            if data["bones"] != merged["bones"]:
                raise ValueError("sequence sections use different skeletons")
            for frame in data["frames"]:
                t = offset + frame["t"]
                if not frames or t > frames[-1]["t"] + 1e-7:
                    frames.append({**frame, "t": t})
            offset += data["duration"]
        merged["frames"], merged["duration"] = frames, offset
        inputs.append(merged)
    data_stream = inputs if inputs is not None else (json.loads(Path(source).read_text()) for source in sources)
    for data in data_stream:
        src_index = {b["name"]: i for i, b in enumerate(data["bones"])}
        src_parents = [b["parent"] for b in data["bones"]]
        # Only source bones that drive the destination and their ancestors are needed.
        needed = set()
        for name in MAP.values():
            i = src_index.get(name, -1)
            while i >= 0 and i not in needed:
                needed.add(i)
                i = src_parents[i]
        order = sorted(needed)
        compact = {original: i for i, original in enumerate(order)}
        cp = [compact.get(src_parents[i], -1) for i in order]
        src_rest = world([matrix(data["bones"][i]["rest"]) for i in order], cp)
        mapping = {dst_index[d]: compact[src_index[s]] for d, s in MAP.items() if d in dst_index and s in src_index}
        rotations = {i: [] for i in mapping}
        pelvis = []
        source_hip = compact[src_index["WAIST"]]
        for frame in data["frames"]:
            sw = world([matrix(frame["pose"][i]) for i in order], cp)
            local = [r.copy() for r in rest]
            posed = [None] * len(local)
            def visit(i):
                if posed[i] is not None:
                    return posed[i]
                parent = visit(parents[i]) if parents[i] >= 0 else np.eye(4)
                if i in mapping:
                    si = mapping[i]
                    delta = sw[si][:3, :3] @ np.linalg.inv(src_rest[si][:3, :3])
                    desired = BASIS @ delta @ BASIS.T @ dst_world[i][:3, :3]
                    local[i][:3, :3] = np.linalg.inv(parent[:3, :3]) @ desired
                if i == dst_index["pelvis"]:
                    delta_p = BASIS @ (sw[source_hip][:3, 3] - src_rest[source_hip][:3, 3]) * 0.01
                    if data.get("in_place"):
                        # Rush sections retain accumulated arena travel across combo stages.
                        # The game controller owns attack steps; keep the body over its capsule.
                        delta_p[[0, 2]] = 0.0
                    target = dst_world[i][:3, 3] + delta_p
                    local[i][:3, 3] = (np.linalg.inv(parent) @ np.r_[target, 1])[:3]
                posed[i] = parent @ local[i]
                return posed[i]
            for i in range(len(local)):
                visit(i)
            for i in rotations:
                rotations[i].append(quaternion(local[i]))
            pelvis.append(local[dst_index["pelvis"]][:3, 3].tolist())
        times = np.array([f["t"] for f in data["frames"]], float)
        time_acc = add_accessor(js, blob, times, 1, "SCALAR", minmax=True)
        channels, samplers = [], []
        def channel(node, path, values, comps, kind):
            index = add_accessor(js, blob, np.asarray(values), comps, kind)
            channels.append({"sampler": len(samplers), "target": {"node": node, "path": path}})
            samplers.append({"input": time_acc, "output": index, "interpolation": "LINEAR"})
        for i, qs in rotations.items():
            for k in range(1, len(qs)):
                if np.dot(qs[k], qs[k-1]) < 0:
                    qs[k] *= -1
            channel(i, "rotation", qs, 4, "VEC4")
        channel(dst_index["pelvis"], "translation", pelvis, 3, "VEC3")
        js["animations"].append({"name": data["name"], "channels": channels, "samplers": samplers,
                                  "extras": {"source": data["source"], "retargeted": True}})
        reports.append({"clip": data["name"], "frames": len(times), "mapped_bones": len(mapping), "duration": data["duration"]})
    js["buffers"][0]["byteLength"] = len(blob)
    if Path(output).exists():
        raise FileExistsError(output)
    statue.write_glb(output, js, blob)
    print(json.dumps(reports, indent=2))
    return reports


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("sources", nargs="+")
    parser.add_argument("--sequence", help="also concatenate sections in source order")
    parser.add_argument("--animation-only", action="store_true", help="small clip library sharing the viewer's mannequin")
    args = parser.parse_args()
    convert(args.base, args.output, args.sources, args.sequence, args.animation_only)
