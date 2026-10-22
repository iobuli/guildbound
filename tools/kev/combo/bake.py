"""Solve a compiled combo with Kev Studio's landmark IK and write the baked replay the viewer already plays."""
import json
import math
import os
import time

import numpy as np

from .compile import compile_combo, list_combos, load_combo
from .metrics import measure
from .render import render_sheet, render_strip
from .rig import Rig

HERE = os.path.dirname(os.path.abspath(__file__))
RESULTS = os.path.join(HERE, "results")


def studio():
    import importlib.util
    path = os.path.join(HERE, "..", "studio", "server.py")
    spec = importlib.util.spec_from_file_location("kev_studio_server", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _quat_step(a, b):
    a = np.asarray(a, float)
    b = np.asarray(b, float)
    a = a / max(float(np.linalg.norm(a)), 1e-9)
    b = b / max(float(np.linalg.norm(b)), 1e-9)
    d = min(1.0, abs(float(np.dot(a, b))))
    return math.degrees(2 * math.acos(d))


def _slerp_quat(a, b, u):
    a = np.asarray(a, float)
    b = np.asarray(b, float)
    a = a / max(float(np.linalg.norm(a)), 1e-9)
    b = b / max(float(np.linalg.norm(b)), 1e-9)
    d = float(np.dot(a, b))
    if d < 0.0:
        b = -b
        d = -d
    d = min(1.0, d)
    if d > 0.9995:
        out = (1.0 - u) * a + u * b
        return out / max(float(np.linalg.norm(out)), 1e-9)
    th = math.acos(d)
    return (math.sin((1.0 - u) * th) * a + math.sin(u * th) * b) / math.sin(th)


# One frame may not swing a bone through a pop. The IK still catches up on the next frames.
MAX_BONE_STEP_DEG = 48.0


def solve(srv, info, samples):
    """One stateful IK pass. Q and P carry forward so a frame starts from the pose it will barely leave."""
    Q, P = {}, {}
    info.pop("_body_yaw", None)
    info.pop("_body_twist", None)
    solved = []
    prev_hand = {}
    prev_q = {}
    t0 = time.time()
    for s in samples:
        err = srv.apply_frame_into(info, Q, P, s["landmarks"], None)
        for side in "lr":
            srv.curl_hand(info, Q, P, side, float(s["hands"][side]))
            # Curl aims the fingers and can roll the hand a half turn. Put that roll back.
            hb = f"hand_{side}"
            child = f"middle_01_{side}"
            if hb in info["index"] and child in info["index"] and hb in prev_hand:
                now = srv.bone_world_rot(info, Q, P, hb)
                # Curl can roll a hand through a half turn. A small roll is the strike; only unwind a flip.
                if srv.quat_angle(prev_hand[hb], now) > 90.0:
                    Wb = srv.chain_world(info, Q, P, info["index"][hb])
                    axis = srv.child_pos(info, Q, P, info["index"][hb], info["index"][child], Wb) - Wb[:3, 3]
                    srv.set_world_rot(info, Q, P, hb, srv.twist_closest(now, prev_hand[hb], axis))
            if hb in info["index"]:
                prev_hand[hb] = srv.bone_world_rot(info, Q, P, hb)
        for bone, q in list(Q.items()):
            if bone not in prev_q:
                continue
            ang = _quat_step(prev_q[bone], q)
            if ang > MAX_BONE_STEP_DEG:
                Q[bone] = _slerp_quat(prev_q[bone], q, MAX_BONE_STEP_DEG / ang)
        prev_q = {b: np.array(q, float) for b, q in Q.items()}
        m = srv.kev_read(info, Q, P)
        solved.append({
            "landmarks": m["landmarks"],
            "joints": m["joints"],
            "balance": m["balance"],
            "reach": {k: float(v) for k, v in err.items()},
            "quats": {b: [float(x) for x in q] for b, q in Q.items()},
            "pelvis": [float(x) for x in P.get("pelvis", np.zeros(3))],
        })
    ms = (time.time() - t0) / max(1, len(samples)) * 1000
    return solved, ms


def baked_payload(model, name, fps, samples, solved, ms):
    frames = []
    for fr in solved:
        frames.append({"q": {b: [round(x, 5) for x in q] for b, q in fr["quats"].items()},
                       "p": [round(x, 5) for x in fr["pelvis"]]})
    return {"model": model, "ref": name, "fps": fps, "duration": samples[-1]["t"], "frames": frames,
            "loop": False, "solve_ms_per_frame": round(ms, 2), "source": "combo"}


def write_baked(srv, payload):
    d = os.path.join(srv.REPO, "staging", "kev", "baked")
    os.makedirs(d, exist_ok=True)
    model = payload["model"]
    path = os.path.join(d, f"{model[:-4]}__{payload['ref']}.json")
    with open(path, "w") as f:
        json.dump(payload, f)
    return path


def bake_combo(srv, name_or_path, model=None, render=True):
    combo = load_combo(name_or_path)
    if model:
        combo["model"] = model
    rig = Rig(srv, combo["model"])
    compiled = compile_combo(rig, combo)
    solved, ms = solve(srv, rig.info, compiled["samples"])
    report = measure(rig.info, compiled["samples"], solved, compiled["fps"])
    report["name"] = compiled["name"]
    report["model"] = combo["model"]
    report["clipped_targets"] = compiled["clipped"]
    report["solve_ms"] = round(ms, 2)
    payload = baked_payload(combo["model"], compiled["name"], compiled["fps"], compiled["samples"], solved, ms)
    baked_path = write_baked(srv, payload)
    strip = None
    if render:
        os.makedirs(RESULTS, exist_ok=True)
        try:
            strip = render_strip(rig.info, compiled["samples"], solved,
                                 os.path.join(RESULTS, compiled["name"] + ".png"),
                                 f"{compiled['name']}  {combo['model']}  {report['duration_s']}s")
        except ImportError:
            print("pillow is not installed; skipped the contact strip")
    return {"report": report, "baked": baked_path, "strip": strip, "payload": payload}


def bake_all(srv, render=True):
    rows = []
    strips = []
    for name in list_combos():
        row = bake_combo(srv, name, render=render)
        rows.append(row["report"])
        if row["strip"]:
            strips.append(row["strip"])
        flag = "ok" if row["report"]["ok"] else "FAIL " + "; ".join(row["report"]["fails"])
        print(f"{name:28} {flag}  jerk {row['report']['jerk']:8.1f}  step {row['report']['max_joint_step_deg']:5.1f} deg  "
              f"slip {row['report']['plant_slip_cm']:5.2f} cm  drift {row['report']['segment_drift_cm']:5.2f} cm  "
              f"reach {row['report']['reach_mean_cm']:4.2f}/{row['report']['reach_max_cm']:4.1f} cm")
    if strips:
        render_sheet(strips, os.path.join(RESULTS, "contact_sheet.png"))
    os.makedirs(RESULTS, exist_ok=True)
    with open(os.path.join(RESULTS, "metrics.json"), "w") as f:
        json.dump(rows, f, indent=1)
    return rows
