"""Combo JSON -> 60 fps landmark samples.

A combo is a list of moves. Each move appends keys after the running pose, and one hermite
spline runs through the whole chain, so position and tangent meet at every join.
"""
import json
import os

import numpy as np

from .geom import rot_axis, sample_channel
from .moves import HAND, _seat_over_feet, build_move, hands_of
from .rig import DRIVEN

HERE = os.path.dirname(os.path.abspath(__file__))
LIB = os.path.join(HERE, "library")


def library_dir():
    return LIB


def list_combos():
    if not os.path.isdir(LIB):
        return []
    return sorted(f[:-5] for f in os.listdir(LIB) if f.endswith(".json"))


def load_combo(name_or_path):
    path = name_or_path
    if not os.path.isfile(path):
        path = os.path.join(LIB, name_or_path + ".json")
    with open(path) as f:
        combo = json.load(f)
    combo["_path"] = path
    combo.setdefault("name", os.path.splitext(os.path.basename(path))[0])
    combo.setdefault("model", "ual1.glb")
    combo.setdefault("fps", 60)
    return combo


def guard_key(rig):
    hands = hands_of(None, ("guard", "guard"))
    both = {f"{p}_{s}" for s in "lr" for p in ("ankle", "ball")}
    return {"t": 0.0, "pose": rig.copy(rig.guard), "hands": hands, "zero": None, "plant": both, "tag": "guard"}


def settle_key(rig, last):
    """Ease back to a balanced guard on whatever feet the combo finished on. Balls do not slide."""
    pose = rig.copy(rig.guard)
    entry = last["pose"]
    for s in "lr":
        ball = rig.set_up(entry[f"ball_{s}"], rig.up_of(rig.guard[f"ball_{s}"]))
        ankle = rig.ankle_from_ball(ball, s)
        pose[f"ball_{s}"] = ball
        pose[f"ankle_{s}"] = ankle
    pel = rig.pelvis_between(pose["ball_l"], pose["ball_r"], rig.up_of(rig.guard["pelvis"]), 0.5)
    settled = rig.shape_on(rig.guard, pel)
    for s in "lr":
        settled[f"ankle_{s}"] = pose[f"ankle_{s}"]
        settled[f"ball_{s}"] = pose[f"ball_{s}"]
        rig.seat_knee(settled, s, rig.fwd)
        rig.seat_elbow(settled, s, rig.guard[f"elbow_{s}"] - rig.guard[f"shoulder_{s}"])
        rig.clamp_wrist(settled, s, 0.96)
    rig.drop_to_fit(settled, ("l", "r"))
    for s in "lr":
        settled[f"ankle_{s}"] = pose[f"ankle_{s}"].copy()
        settled[f"ball_{s}"] = pose[f"ball_{s}"].copy()
    ankles = {f"ankle_{s}" for s in "lr"}
    feet = ankles | {f"ball_{s}" for s in "lr"}
    # Flat feet into the guard. A tangent on the closer bows the ankle while the segment is marked planted.
    if last.get("zero") is not None:
        last["zero"] = set(last["zero"]) | feet
    t = last["t"] + 0.26
    return {"t": round(t, 4), "pose": settled, "hands": {"l": HAND["guard"], "r": HAND["guard"]},
            "zero": None, "plant": ankles, "tag": "settle"}


def chain_keys(rig, combo):
    keys = [guard_key(rig)]
    for move in combo["moves"]:
        keys.extend(build_move(rig, move, keys[-1]))
    keys.append(settle_key(rig, keys[-1]))
    # times must strictly increase
    for a, b in zip(keys, keys[1:]):
        if b["t"] <= a["t"]:
            b["t"] = round(a["t"] + 1 / combo.get("fps", 60), 4)
    return keys


def _segment(keys, t):
    times = [k["t"] for k in keys]
    hi = 1
    while hi < len(times) - 1 and times[hi] < t:
        hi += 1
    return keys[hi - 1], keys[hi]


def _planted(a, b):
    names = []
    for n in ("ankle_l", "ankle_r", "ball_l", "ball_r"):
        if n in a["plant"] and n in b["plant"] and float(np.linalg.norm(a["pose"][n] - b["pose"][n])) < 0.004:
            names.append(n)
    return names


def _lerp_deg(a, b, u):
    """Shortest-angle blend. 180 toward 0 passes through 90, not through a full extra turn."""
    delta = (float(b) - float(a) + 180.0) % 360.0 - 180.0
    return float(a) + delta * float(u)


def _has_yaw(keys):
    for key in keys:
        if abs(float(key.get("yaw", 0.0) or 0.0)) > 1e-6 or abs(float(key.get("twist", 0.0) or 0.0)) > 1e-6:
            return True
    return False


def samples_from_keys(keys, fps):
    channels = {name: sample_channel(keys, lambda k, name=name: k["pose"][name], name, fps) for name in DRIVEN}
    hl = sample_channel(keys, lambda k: float(k["hands"]["l"]), None, fps)
    hr = sample_channel(keys, lambda k: float(k["hands"]["r"]), None, fps)
    has_yaw = _has_yaw(keys)
    t1 = keys[-1]["t"]
    n = int(round(t1 * fps)) + 1
    out = []
    for i in range(n):
        t = min(t1, i / fps)
        a, b = _segment(keys, t)
        lm = {name: [round(float(x), 5) for x in np.asarray(channels[name][i], float)] for name in DRIVEN}
        if has_yaw:
            span = b["t"] - a["t"]
            u = 0.0 if span <= 1e-8 else (t - a["t"]) / span
            u = min(1.0, max(0.0, u))
            lm["yaw"] = round(_lerp_deg(a.get("yaw", 0.0), b.get("yaw", 0.0), u), 4)
            lm["twist"] = round(_lerp_deg(a.get("twist", 0.0), b.get("twist", 0.0), u), 4)
        # safety: never ask the solver for a limb past its reach. Support feet are left alone
        # (clamping them would slide a plant); the arm and a raised foot are pulled back in.
        pose = {name: np.array(lm[name], float) for name in DRIVEN}
        out.append({"t": round(t, 4), "landmarks": lm, "hands": {"l": float(np.clip(hl[i], 0, 1)), "r": float(np.clip(hr[i], 0, 1))},
                    "plant": _planted(a, b), "tag": a["tag"] if (t - a["t"]) <= (b["t"] - t) else b["tag"],
                    "_pose": pose})
    return out


# How fast an elbow or knee pole may twist, and how fast a hand or airborne foot may change aim.
# Faster than this is a one-frame pop. Anime snaps still clear it; a flipped pole does not.
# The elbow hinge (shoulder to elbow) is capped separately: a bowed strike swings that
# bone harder than the wrist, and that swing is what reads as a joint pop.
POLE_STEP_DEG = 32.0
AIM_STEP_DEG = 40.0
HINGE_STEP_DEG = 32.0
POLES = (("shoulder_l", "elbow_l", "wrist_l"), ("shoulder_r", "elbow_r", "wrist_r"),
         ("hip_l", "knee_l", "ankle_l"), ("hip_r", "knee_r", "ankle_r"))


def _limit_pole(pose, prev, root, mid, end):
    """Cap twist of the elbow/knee around the current limb. Arm swing is not twist and is left alone."""
    sh, wr, el = pose[root], pose[end], pose[mid]
    u = wr - sh
    n = float(np.linalg.norm(u))
    if n < 1e-5:
        return
    u = u / n
    side = (el - sh) - u * float(np.dot(el - sh, u))
    sl = float(np.linalg.norm(side))
    if sl < 1e-4:
        return
    psh, pwr, pel = prev[root], prev[end], prev[mid]
    pu = pwr - psh
    pn = float(np.linalg.norm(pu))
    if pn < 1e-5:
        return
    pu = pu / pn
    pside = (pel - psh) - pu * float(np.dot(pel - psh, pu))
    if float(np.linalg.norm(pside)) < 1e-4:
        return
    # previous pole, projected into this frame's limb plane, is the twist reference
    a = pside - u * float(np.dot(pside, u))
    al = float(np.linalg.norm(a))
    if al < 1e-4:
        return
    a = a / al
    b = side / sl
    dot = float(np.clip(np.dot(a, b), -1.0, 1.0))
    ang = float(np.degrees(np.arccos(dot)))
    if ang <= POLE_STEP_DEG:
        return
    axis = np.cross(a, b)
    if float(np.linalg.norm(axis)) < 1e-6:
        axis = np.cross(a, u)
    turned = rot_axis(a, axis, POLE_STEP_DEG)
    turned = turned - u * float(np.dot(turned, u))
    tn = float(np.linalg.norm(turned))
    if tn < 1e-6:
        return
    turned = turned / tn
    para = u * float(np.dot(el - sh, u))
    pose[mid] = sh + para + turned * sl


def _limit_aim(pose, prev, root, end, step):
    cur = pose[end] - pose[root]
    old = prev[end] - prev[root]
    cn, on = float(np.linalg.norm(cur)), float(np.linalg.norm(old))
    if cn < 1e-5 or on < 1e-5:
        return
    a, b = old / on, cur / cn
    dot = float(np.clip(np.dot(a, b), -1.0, 1.0))
    ang = float(np.degrees(np.arccos(dot)))
    if ang <= step:
        return
    axis = np.cross(a, b)
    if float(np.linalg.norm(axis)) < 1e-6:
        return
    turned = rot_axis(a, axis, step)
    turned = turned / max(float(np.linalg.norm(turned)), 1e-9)
    pose[end] = pose[root] + turned * cn


def _swing_dir(src, dst, max_deg):
    s = src / max(float(np.linalg.norm(src)), 1e-9)
    d = dst / max(float(np.linalg.norm(dst)), 1e-9)
    ang = float(np.arccos(float(np.clip(np.dot(s, d), -1.0, 1.0))))
    cap = float(np.radians(max_deg))
    if ang <= cap or ang < 1e-4:
        return d
    t = cap / ang
    return (np.sin((1.0 - t) * ang) * s + np.sin(t * ang) * d) / np.sin(ang)


def _keep_spine(rig, pose, prev):
    """A hermite tuck shortens the neck through zero and the torso basis flips.

    The raw sample is not the previous output, so a short neck has to be rebuilt from the last
    good spine. The rebuild steps a few centimetres a frame, which keeps the correction off the pop gate.
    """
    want = float(np.linalg.norm(rig.guard["neck"] - rig.guard["pelvis"]))
    up = pose["neck"] - pose["pelvis"]
    dist = float(np.linalg.norm(up))
    upper = ("neck", "crown", "nose", "head_joint", "shoulder_l", "shoulder_r",
             "elbow_l", "elbow_r", "wrist_l", "wrist_r")
    if prev is None:
        if 1e-4 < dist < want * 0.85:
            delta = up * (want / dist - 1.0)
            for k in upper:
                pose[k] = pose[k] + delta
        return
    prev_up = prev["neck"] - prev["pelvis"]
    if float(np.linalg.norm(prev_up)) < want * 0.5:
        prev_up = rig.guard["neck"] - rig.guard["pelvis"]
    if dist >= want * 0.85:
        direction = _swing_dir(prev_up, up, 32.0)
        goal = direction * dist
    else:
        direction = prev_up / max(float(np.linalg.norm(prev_up)), 1e-9) if dist < 0.08 else _swing_dir(prev_up, up, 32.0)
        goal = direction * want
    step = goal - prev_up
    n = float(np.linalg.norm(step))
    if n > 0.04:
        step = step * (0.04 / n)
    delta = (prev_up + step) - up
    if float(np.linalg.norm(delta)) < 1e-4:
        return
    for k in upper:
        pose[k] = pose[k] + delta


def clamp_samples(rig, samples):
    """Keep wrists and airborne feet inside the limb, and stop a limb twisting or aiming in one frame."""
    clipped = 0
    prev = None
    for s in samples:
        pose = s.pop("_pose")
        _keep_spine(rig, pose, prev)
        planted = set(s["plant"])
        # A yawed ball already on the floor is the pivot. Later aim limits must not pick it up.
        floored = {}
        if "yaw" in s["landmarks"] or "twist" in s["landmarks"]:
            for side in "lr":
                if rig.up_of(pose[f"ball_{side}"]) - rig.floor <= 0.03:
                    floored[side] = pose[f"ball_{side}"].copy()
        for side in "lr":
            before = pose[f"wrist_{side}"].copy()
            rig.clamp_wrist(pose, side, 0.93)
            if float(np.linalg.norm(pose[f"wrist_{side}"] - before)) > 0.01:
                clipped += 1
            if f"ankle_{side}" not in planted:
                before = pose[f"ankle_{side}"].copy()
                rig.clamp_ankle(pose, side, 0.95)
                if float(np.linalg.norm(pose[f"ankle_{side}"] - before)) > 0.01:
                    clipped += 1
        if prev is not None:
            for side in "lr":
                _limit_aim(pose, prev, f"shoulder_{side}", f"wrist_{side}", AIM_STEP_DEG)
                if f"ankle_{side}" not in planted:
                    before = pose[f"ankle_{side}"].copy()
                    _limit_aim(pose, prev, f"hip_{side}", f"ankle_{side}", AIM_STEP_DEG)
                    if side not in floored:
                        pose[f"ball_{side}"] = pose[f"ball_{side}"] + (pose[f"ankle_{side}"] - before)
                        # The foot bone is ankle to ball. A kick that snaps it past the gate
                        # reads as a flipped foot even when the ankle itself only stepped a few centimetres.
                        _limit_aim(pose, prev, f"ankle_{side}", f"ball_{side}", 28.0)
            for root, mid, end in POLES:
                _limit_pole(pose, prev, root, mid, end)
            # Knees stay on the pole cap only. Pulling a knee off a planted shin slides the foot.
            for side in "lr":
                _limit_aim(pose, prev, f"shoulder_{side}", f"elbow_{side}", HINGE_STEP_DEG)
        for side, ball in floored.items():
            pose[f"ball_{side}"] = ball
        support = [side for side in "lr" if f"ankle_{side}" in planted]
        _seat_over_feet(rig, pose, support)
        prev = {k: np.array(v, float).copy() for k, v in pose.items()}
        yaw = s["landmarks"].get("yaw")
        twist = s["landmarks"].get("twist")
        s["landmarks"] = {k: [round(float(x), 5) for x in pose[k]] for k in DRIVEN}
        # Scalars, not positions. Rotating the positions here would turn the body a second time.
        if yaw is not None:
            s["landmarks"]["yaw"] = yaw
        if twist is not None:
            s["landmarks"]["twist"] = twist
    return clipped


def compile_combo(rig, combo):
    keys = chain_keys(rig, combo)
    samples = samples_from_keys(keys, combo.get("fps", 60))
    clipped = clamp_samples(rig, samples)
    return {"name": combo["name"], "model": combo.get("model", rig.model), "fps": combo.get("fps", 60),
            "duration": samples[-1]["t"], "samples": samples, "keys": keys, "clipped": clipped}
