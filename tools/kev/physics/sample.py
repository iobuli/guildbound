"""Joint landmarks from a GLB clip, without building a body-map mesh.

Enough for the physics pass and its report: pelvis, limbs, head, ball of the foot, middle finger.
Crown and nose are placed off the head joint so the head spring and the COM have something to hold.
"""
import os

import numpy as np

import statue

JOINTS = {
    "pelvis": "pelvis",
    "hip_l": "thigh_l", "hip_r": "thigh_r",
    "knee_l": "calf_l", "knee_r": "calf_r",
    "ankle_l": "foot_l", "ankle_r": "foot_r",
    "ball_l": "ball_l", "ball_r": "ball_r",
    "shoulder_l": "upperarm_l", "shoulder_r": "upperarm_r",
    "elbow_l": "lowerarm_l", "elbow_r": "lowerarm_r",
    "wrist_l": "hand_l", "wrist_r": "hand_r",
    "neck": "neck_01", "head_joint": "Head",
    "knuckle_l": "middle_01_l", "knuckle_r": "middle_01_r",
    "fingertip_l": "middle_04_leaf_l", "fingertip_r": "middle_04_leaf_r",
}

HERE = os.path.dirname(os.path.abspath(__file__))
ANIM = os.path.normpath(os.path.join(HERE, "..", "..", "..", "assets", "anim"))


def _unit(v):
    n = float(np.linalg.norm(v))
    return v / n if n > 1e-8 else np.array([0.0, 1.0, 0.0])


def _hierarchy(js):
    n = len(js["nodes"])
    parent = [-1] * n
    for i, nd in enumerate(js["nodes"]):
        for c in nd.get("children", []):
            parent[c] = i
    order, seen = [], set()

    def visit(i):
        if i in seen:
            return
        if parent[i] >= 0:
            visit(parent[i])
        seen.add(i)
        order.append(i)

    for i in range(n):
        visit(i)
    names = [nd.get("name", "") for nd in js["nodes"]]
    rest_t = [np.array(nd.get("translation", [0, 0, 0]), float) for nd in js["nodes"]]
    rest_q = [np.array(nd.get("rotation", [0, 0, 0, 1]), float) for nd in js["nodes"]]
    return parent, order, names, {nm: i for i, nm in enumerate(names)}, rest_t, rest_q


def _quat_mul_mat(q):
    x, y, z, w = q
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])


def _eval(t, v, kind, x):
    x = min(max(float(x), float(t[0])), float(t[-1]))
    hi = min(int(np.searchsorted(t, x)), len(t) - 1)
    if hi == 0:
        return v[0]
    lo = hi - 1
    u = (x - t[lo]) / max(t[hi] - t[lo], 1e-9)
    if kind == "STEP":
        return v[lo]
    if v.shape[1] == 4:
        a, b = v[lo], v[hi]
        if float(np.dot(a, b)) < 0:
            b = -b
        q = a * (1 - u) + b * u
        return q / np.linalg.norm(q)
    return v[lo] * (1 - u) + v[hi] * u


def _channels(js, blob, anim, index):
    out = {}
    for ch in anim["channels"]:
        path = ch["target"].get("path")
        if path not in ("rotation", "translation"):
            continue
        sm = anim["samplers"][ch["sampler"]]
        t, _ = statue.accessor_view(js, blob, sm["input"])
        v, _ = statue.accessor_view(js, blob, sm["output"])
        kind = sm.get("interpolation", "LINEAR")
        if kind == "CUBICSPLINE":
            v, kind = v[1::3], "LINEAR"
        node = ch["target"]["node"]
        out[(node, path)] = (np.asarray(t, float).reshape(-1), np.asarray(v, float), kind)
    return out


def _pose(js, parent, order, rest_t, rest_q, channels, time):
    T = list(rest_t)
    Q = list(rest_q)
    for (node, path), (t, v, kind) in channels.items():
        val = _eval(t, v, kind, time)
        if path == "translation":
            T[node] = np.asarray(val, float)
        else:
            Q[node] = np.asarray(val, float)
    world = [None] * len(parent)
    for i in order:
        local = np.eye(4)
        local[:3, :3] = _quat_mul_mat(Q[i])
        local[:3, 3] = T[i]
        p = parent[i]
        world[i] = local if p < 0 else world[p] @ local
    return world


def _landmarks(world, index):
    lm = {}
    for name, bone in JOINTS.items():
        i = index.get(bone)
        if i is None:
            continue
        lm[name] = [float(x) for x in world[i][:3, 3]]
    if "head_joint" in lm and "neck" in lm:
        head = np.array(lm["head_joint"])
        neck = np.array(lm["neck"])
        up = _unit(head - neck)
        lm["crown"] = [float(x) for x in head + up * 0.18]
        lm["nose"] = [float(x) for x in head + up * 0.06 + np.array([0.0, 0.0, 0.09])]
    return lm


def rest_landmarks(model):
    """Bind-pose landmarks for `model` (a file name in assets/anim, or a path)."""
    path = model if os.path.isabs(model) or model.endswith(".glb") and os.path.isfile(model) else os.path.join(ANIM, model)
    js, blob = statue.read_glb(path)
    parent, order, names, index, rest_t, rest_q = _hierarchy(js)
    world = _pose(js, parent, order, rest_t, rest_q, {}, 0.0)
    return _landmarks(world, index)


def clip_landmarks(model, clip, fps=60):
    """(frames, dt, duration) of joint landmarks for one clip."""
    path = model if os.path.isfile(model) else os.path.join(ANIM, model)
    js, blob = statue.read_glb(path)
    parent, order, names, index, rest_t, rest_q = _hierarchy(js)
    anim = next(a for a in js["animations"] if a.get("name") == clip)
    channels = _channels(js, blob, anim, index)
    dur = 0.0
    for t, _, _ in channels.values():
        dur = max(dur, float(t[-1]))
    n = int(round(dur * fps)) + 1
    frames = []
    for k in range(n):
        world = _pose(js, parent, order, rest_t, rest_q, channels, k / fps)
        frames.append(_landmarks(world, index))
    return frames, 1.0 / fps, dur
