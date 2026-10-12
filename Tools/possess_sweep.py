"""Possession sweep: drive the running game's Studio through gb.possess, step every clip of every model at a fixed
frame rate, and digest what the engine's final pose does (after corrections, life layers and authority rules).

python -I Tools/possess_sweep.py [--models packs/hero/fighter.glb,packs/npc/male_peasant.glb] [--fps 15] [--clips FILTER]

Writes collab/possess_sweep.json and prints a short digest: the life-layer checks (fists close into the palm, the
head turns to a target) and per clip: joint violations, frames out of balance with both feet down at low speed,
and momentum jumps between frames (pops a viewer feels as a hitch).
"""
import json
import math
import sys
import time

sys.path.insert(0, __file__.rsplit("\\", 1)[0].rsplit("/", 1)[0])
from gb import call  # noqa: E402

ROOT = __file__.replace("\\", "/").rsplit("/Tools/", 1)[0]


def possess(params=None):
    return call("gb.possess", params or {})


def settle(n=3):
    time.sleep(0.03 * n)


def arg(name, default):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else default


def life_checks():
    out = {}
    call("gb.studio", {"clip": "Unarmed-Idle", "time": 0.0, "hero_clips": True})
    for grip in ([0.0, 0.0], [1.0, 1.0]):
        possess({"grip": grip, "look": "off", "frame": 0})
        settle(30)
        b = {r["bone"]: r for r in possess({"bones": True})["bones"]}
        d = lambda x, y: math.dist(b[x]["world"], b[y]["world"])
        out[f"grip_{grip[0]}"] = {s: round(d(f"index_03_{s}", hand), 3) for s, hand in (("l", "B_L_Forearm.001"), ("r", "B_R_Hand"))}
    snap = possess({"grip": None})
    head = {r["bone"]: r for r in possess({"bones": True})["bones"]}["B_Head"]["world"]
    for side, dx in (("left", 1.0), ("right", -1.0)):
        target = [head[0] + dx * 3.0, head[1], head[2] + 0.3]
        possess({"look": target})
        settle(40)
        b = {r["bone"]: r for r in possess({"bones": True})["bones"]}
        q = b["B_Head"]["world_rot"]
        q0 = None
        possess({"look": "off"})
        settle(40)
        q0 = {r["bone"]: r for r in possess({"bones": True})["bones"]}["B_Head"]["world_rot"]
        dot = abs(sum(a * c for a, c in zip(q, q0)))
        out[f"look_{side}_turn_deg"] = round(math.degrees(2 * math.acos(min(1.0, dot))), 1)
    return out, snap


def sweep_clip(clip, frames, fps):
    rows = []
    possess({"frame": 0})
    settle()
    for k in range(frames):
        s = possess({"frame": k})
        settle(2)
        s = possess()
        ph = s["physics"]
        rows.append({"f": k, "speed": ph["speed_mps"], "mom": ph["momentum_kgmps"], "margin": ph["dynamic_margin_m"], "feet": ph["feet_down"], "g": ph["accel_g"],
                     "viol": s["numbers"].get("violations", []), "valid": ph["valid"]})
    viol = sorted({v for r in rows for v in r["viol"]})
    # falling: the extrapolated centre of mass leaves the support by more than 3 cm while something is down
    unbalanced = [r["f"] for r in rows if r["feet"] and r["margin"] is not None and r["margin"] < -0.03 and r["valid"]]
    # pops: centre-of-mass acceleration over 3 g between frames (a body cannot do that without an impact)
    jumps = [(r["f"], round(r["g"], 1)) for r in rows if r["valid"] and r["g"] > 3.0]
    return {"frames": frames, "violations": viol, "unbalanced_frames": unbalanced, "momentum_jumps": jumps,
            "peak_speed": round(max((r["speed"] for r in rows if r["valid"]), default=0.0), 2)}


def main():
    fps = float(arg("--fps", "15"))
    models = arg("--models", "packs/hero/fighter.glb").split(",")
    filt = arg("--clips", "")
    call("gb.control", {"state": "play"})
    time.sleep(1.0)
    call("gb.studio", {"open": True})
    time.sleep(0.5)
    report = {"fps": fps, "models": {}}
    for model in models:
        hero = "hero" in model
        call("gb.studio", {"spawn": model, "hero_clips": hero})
        time.sleep(2.0)
        possess({"enable": True, "fixed": True, "fps": fps, "clear": True, "grip": None, "look": "off", "lean": False, "breath": False})
        entry = {}
        if hero and "--life" in sys.argv:
            entry["life"], _ = life_checks()
            print("LIFE", json.dumps(entry["life"]))
        names = [n for n in sorted(call("gb.studio", {"hero_clips": hero}).get("clip_list", [])) if filt in n]
        if "--list" in sys.argv:
            wanted = arg("--list", "").split(",")
            names = [n for n in names if n in wanted]
        out = {}
        for clip in names:
            call("gb.studio", {"clip": clip, "hero_clips": hero})
            settle(4)
            s = possess()
            frames = max(2, int(round(s["duration"] * fps)))
            out[clip] = sweep_clip(clip, frames, fps)
            r = out[clip]
            if r["violations"] or r["unbalanced_frames"] or r["momentum_jumps"]:
                print("CLIP", model, clip, json.dumps(r))
        entry["clips"] = out
        report["models"][model] = entry
    with open(f"{ROOT}/collab/possess_sweep.json", "a") as fh:
        fh.write(json.dumps(report) + chr(10))
    print("SWEEP_DONE", sum(len(m.get("clips", {})) for m in report["models"].values()), "clips")


main()
