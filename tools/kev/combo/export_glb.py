"""Write assets/anim/kev.glb: UAL skeleton plus Kev strikes the game can retarget by bone name.

Strikes, combos and the generated sheets are solved on ual1. Run from the repo root:

    python3 -I tools/kev/combo/export_glb.py
"""
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
KEV = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(KEV))
sys.path.insert(0, KEV)

from combo.bake import solve, studio  # noqa: E402
from combo.compile import compile_combo, load_combo  # noqa: E402
from combo.metrics import measure  # noqa: E402
from combo.rig import Rig  # noqa: E402
from statue import read_glb, write_glb  # noqa: E402

OUT = os.path.join(REPO, "assets", "anim", "kev.glb")
UAL = os.path.join(REPO, "assets", "anim", "ual1.glb")


def _pad(blob):
    return blob + b"\0" * ((4 - len(blob) % 4) % 4)


def _one_move(move):
    return {"name": move["id"], "model": "ual1.glb", "fps": 60, "moves": [move]}


def _peak(solved, fps, names):
    """Fraction of the clip where a joint moves fastest. Hit windows are wrapped around it."""
    best = {}
    for name in names:
        if name not in solved[0]["joints"]:
            continue
        prev = np.array(solved[0]["joints"][name], float)
        peak, at = 0.0, 0
        for i, fr in enumerate(solved[1:], start=1):
            cur = np.array(fr["joints"][name], float)
            speed = float(np.linalg.norm(cur - prev)) * fps
            if speed > peak:
                peak, at = speed, i
            prev = cur
        best[name] = (peak, at / max(1, len(solved) - 1))
    return best


def _solved_clip(srv, rig, combo):
    compiled = compile_combo(rig, combo)
    solved, _ms = solve(srv, rig.info, compiled["samples"])
    report = measure(rig.info, compiled["samples"], solved, compiled["fps"])
    report["peak"] = _peak(solved, compiled["fps"], ("foot_l", "foot_r", "hand_l", "hand_r", "calf_l", "calf_r"))
    frames = []
    for fr in solved:
        frames.append({"q": fr["quats"], "p": fr["pelvis"]})
    return compiled["name"], compiled["fps"], float(compiled["duration"]), frames, report


def _collect():
    srv = studio()
    rig = Rig(srv, "ual1.glb")
    clips = []
    strikes = [
        {"id": "jab", "do": "jab", "clip": "Punch_Jab", "limb": "wrist_l", "plant": "l", "step": 0.16,
         "hands": {"l": "fist", "r": "guard"},
         "dur": {"antic": 0.13, "strike": 0.08, "hold": 0.045, "overshoot": 0.05, "follow": 0.12},
         "arc": 0.045, "ramp": 1.15},
        {"id": "cross", "do": "cross", "clip": "Punch_Cross", "limb": "wrist_r", "plant": "l", "step": 0.14, "pivot": 26,
         "hands": {"l": "guard", "r": "fist"},
         "dur": {"antic": 0.16, "strike": 0.1, "hold": 0.05, "overshoot": 0.05, "follow": 0.14},
         "arc": 0.04, "ramp": 1.0},
        {"id": "hook", "do": "hook", "hand": "l", "plant": "l", "step": 0.08, "pivot": 18,
         "hands": {"l": "fist", "r": "guard"},
         "dur": {"antic": 0.14, "strike": 0.09, "hold": 0.055, "overshoot": 0.05, "follow": 0.13},
         "arc": 0.08},
        {"id": "roundhouse", "do": "roundhouse", "foot": "r", "plant": "l", "step": 0.08, "pivot": 78,
         "hands": {"l": "guard", "r": "guard"},
         "dur": {"antic": 0.18, "strike": 0.11, "hold": 0.06, "overshoot": 0.055, "follow": 0.22}},
    ]
    for move in strikes:
        clips.append(_solved_clip(srv, rig, _one_move(move)))
    for name in ("jab_cross_hook_roundhouse", "rush_uppercut", "spin_back_jump_kick", "brawler_mage"):
        clips.append(_solved_clip(srv, rig, load_combo(name)))
    hands = {"l": "guard", "r": "guard"}
    cast = {"l": "cast", "r": "cast"}
    for sheet, plant, hs in (
        ("front_kick", "l", hands),
        ("jump_kick", "l", hands),
        ("palm_thrust_cast", "l", cast),
        ("backflip_evade", "l", hands),
        ("flying_side_kick", "r", hands),
        ("roundhouse_kick", "l", hands),
        ("axe_kick", "l", hands),
        ("jumping_knee", "l", hands),
        ("spinning_back_kick", "l", hands),
        ("aerial_dive_attack", "l", hands),
        ("crash_landing", "l", hands),
    ):
        clips.append(_solved_clip(srv, rig, {
            "name": sheet, "model": "ual1.glb", "fps": 60,
            "moves": [{"id": sheet, "do": "sheet", "sheet": sheet, "plant": plant,
                       "hands": hs,
                       "dur": {"antic": 0.14, "strike": 0.16, "hold": 0.05, "overshoot": 0.04, "follow": 0.16}}],
        }))
    return clips


def _skeleton():
    js, blob = read_glb(UAL)
    nodes = []
    for n in js["nodes"]:
        keep = {}
        for k in ("name", "translation", "rotation", "scale", "children"):
            if k in n:
                keep[k] = n[k]
        nodes.append(keep)
    skin = js["skins"][0]
    acc = js["accessors"][skin["inverseBindMatrices"]]
    view = js["bufferViews"][acc["bufferView"]]
    off = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    ibm = bytes(blob[off:off + view["byteLength"]])
    names = {n.get("name"): i for i, n in enumerate(nodes) if n.get("name")}
    return nodes, skin["joints"], ibm, names


def _anim(name, fps, duration, frames, nodes, blob, accessors, views):
    times = np.array([min(duration, i / fps) for i in range(len(frames))], np.float32)
    if len(times):
        times[-1] = np.float32(duration)
    bones = sorted(frames[0]["q"])
    channels = []
    samplers = []

    def push(array, acc_type, minimum, maximum):
        raw = _pad(array.astype(np.float32).tobytes())
        view_i = len(views)
        views.append({"buffer": 0, "byteOffset": len(blob), "byteLength": len(raw)})
        blob.extend(raw)
        acc_i = len(accessors)
        accessors.append({
            "bufferView": view_i,
            "componentType": 5126,
            "count": int(array.shape[0]),
            "type": acc_type,
            "min": minimum,
            "max": maximum,
        })
        return acc_i

    time_acc = push(times, "SCALAR", [float(times[0])], [float(times[-1])])

    def add_channel(node, path, values, acc_type):
        acc = push(values, acc_type, values.min(axis=0).tolist(), values.max(axis=0).tolist())
        samplers.append({"input": time_acc, "interpolation": "LINEAR", "output": acc})
        channels.append({"sampler": len(samplers) - 1, "target": {"node": node, "path": path}})

    for bone in bones:
        if bone not in nodes:
            continue
        vals = np.array([fr["q"][bone] for fr in frames], np.float32)
        # Keep the quaternion sign continuous so the game's slerp does not take the long way.
        for i in range(1, len(vals)):
            if float(np.dot(vals[i], vals[i - 1])) < 0:
                vals[i] = -vals[i]
        add_channel(nodes[bone], "rotation", vals, "VEC4")
    pelvis = np.array([fr["p"] for fr in frames], np.float32)
    if "pelvis" in nodes:
        add_channel(nodes["pelvis"], "translation", pelvis, "VEC3")
    return {"name": name, "channels": channels, "samplers": samplers}


def main():
    clips = _collect()
    nodes, joints, ibm, names = _skeleton()
    blob = bytearray(_pad(ibm))
    accessors = [{
        "bufferView": 0,
        "componentType": 5126,
        "count": len(joints),
        "type": "MAT4",
    }]
    views = [{"buffer": 0, "byteOffset": 0, "byteLength": len(ibm)}]
    animations = []
    failed = []
    print(f"{'clip':28} {'frames':6} {'dur':6}  report")
    for name, fps, duration, frames, report in clips:
        animations.append(_anim(name, fps, duration, frames, names, blob, accessors, views))
        if report["ok"]:
            peak = max(report["peak"].items(), key=lambda kv: kv[1][0])
            flag = (f"ok  jerk {report['jerk']:.0f} step {report['max_joint_step_deg']:.1f} "
                    f"slip {report['plant_slip_cm']:.2f}  peak {peak[0]} {peak[1][1]:.2f} {peak[1][0]:.1f}m/s")
        else:
            flag = "FAIL " + "; ".join(report["fails"])
            failed.append(name)
        print(f"{name:28} {len(frames):6} {duration:6.2f}  {flag}", flush=True)
    if failed:
        raise SystemExit("refusing to write kev.glb, failed: " + ", ".join(failed))
    gltf = {
        "asset": {"version": "2.0", "generator": "tools/kev/combo/export_glb.py"},
        "scene": 0,
        "scenes": [{"nodes": [len(nodes) - 1]}],
        "nodes": nodes,
        "skins": [{"name": "Armature", "joints": joints, "inverseBindMatrices": 0}],
        "animations": animations,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": len(blob)}],
    }
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    write_glb(OUT, gltf, bytes(blob))
    print(f"wrote {OUT}  {os.path.getsize(OUT)} bytes  {len(animations)} clips")


if __name__ == "__main__":
    main()
