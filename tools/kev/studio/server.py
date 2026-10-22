"""Kev Studio: a local web app to read and write a skeleton rig, with Kev's measurements live next to a 3D view.

    python3 -I tools/kev/studio/server.py [--port 8765] [--host 127.0.0.1]     then open http://localhost:8765

The browser (three.js) draws the rig and plays its clips; it streams the current bone rotations to this server several
times a second. Kev (this server, numpy only) turns them into world transforms with FK from the GLB's own hierarchy and
answers with the body map: landmark positions, facings, centre of mass against the support. Anything can read and write:

  MOVE   Kev solves for the rig: `reach arm_r 0.4 1.3 0.3` (IK to a world point), `aim`, and whole-body landmark frames:
         POST /api/control {"landmarks": {"wrist_r": [x, y, z], "pelvis": [...], ...}} -> commanded pose + reach errors back
  REFS   `ref capture` records Kev's landmarks while a clip plays; `ref play <name>` replays them onto the loaded model
  READ   GET /api/state  /api/rig  /api/models  /api/clips      (JSON)      or console / POST /api/exec {"line": "get LeftArm"}
  WRITE  POST /api/exec {"line": "set upperarm_l 0 0 60"}  or  POST /api/cmd {"cmd": "set", "bone": ..., "euler": [x, y, z]}
  LIVE   GET /api/events (server-sent events: "state" every update, "cmd" for commands bound for the viewer)

Console commands (also in the page): help | models | model <file> | clips | play <clip> | pause | resume | seek <s> |
speed <x> | bones [filter] | get <bone> | set <bone> <x> <y> <z> (degrees, in the bone's own rest frame) | reset |
landmarks [name] | facings | balance | rig | state |
hand <l|r> <open|fist|point|relaxed|0..1> | reach <arm_l|arm_r|leg_l|leg_r> x y z [pole x y z] (world metres) | aim <bone> x y z |
ref grab <model> <clip> [name] (60 fps, offline) | ref capture [name] (from the playing clip) | ref list |
check <name> (no-glitch report for a catalogue clip: frame ranges) |
ref play <name> (bake onto the loaded model, play at 60 fps) | ref bake <name> | ref baked <name> (play an existing bake, e.g. from tools/kev/grid) | ref live <name> [rate] | ref stop | ref status |
phys on|off|toggle|status|reset (optional ground/balance/momentum pass on /api/control targets).
Files are served only from tools/kev/studio/static and assets/anim (GLB). Nothing in them is executed.
"""
import argparse
import json
import math
import os
import queue
import re
import sys
import threading
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
KEV = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(KEV))
ANIM = os.path.join(REPO, "assets", "anim")
sys.path.insert(0, KEV)
import bodymap  # noqa: E402
import nodes as kev_nodes  # noqa: E402
import physics  # noqa: E402
import quality.core as quality  # noqa: E402
import statue  # noqa: E402

MIME = {".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".glb": "model/gltf-binary",
        ".txt": "text/plain", ".png": "image/png"}
LOCK = threading.Lock()
CLIENTS = []
STATE = {"model": None, "clip": None, "playing": False, "t": 0.0, "duration": 0.0, "speed": 1.0, "summary": {}, "stamp": 0.0}
PHYS = {"on": False, "sim": None, "last": 0.0, "model": None}
LATEST_QUATS = {}
LATEST_POS = {}
CACHE = {}


def quat_to_mat(q):
    x, y, z, w = q
    return np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                     [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                     [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])


def compose_trs(t, q, s):
    local = np.eye(4)
    local[:3, :3] = quat_to_mat(q) * np.asarray(s, float)
    local[:3, 3] = np.asarray(t, float)
    return local


def node_matrix(nd):
    return np.array(nd["matrix"], float).reshape(4, 4).T


FINGER_WORDS = {"index": "index", "middle": "middle", "ring": "ring", "pinky": "pinky", "thumb": "thumb"}
MIXAMO = {"Hips": "pelvis", "Spine": "spine_01", "Spine1": "spine_02", "Spine2": "spine_03", "Neck": "neck_01", "Head": "Head",
          "HeadTop_End": "Head_leaf", "Shoulder": "clavicle", "Arm": "upperarm", "ForeArm": "lowerarm", "Hand": "hand",
          "UpLeg": "thigh", "Leg": "calf", "Foot": "foot", "ToeBase": "ball", "Toe_End": "ball_leaf", "Root": "root"}
BRIG = {"hips": "pelvis", "spine": "spine_01", "chest": "spine_03", "neck": "neck_01", "head": "Head", "jaw": "jaw", "shoulder": "clavicle",
        "upperArm": "upperarm", "forearm": "lowerarm", "hand": "hand", "thigh": "thigh", "shin": "calf", "foot": "foot", "toe": "ball",
        "root": "root"}


def canonical(name):
    """Name in the UAL convention the body map and solvers use, or the name unchanged (UAL rigs pass through)."""
    m = re.match(r"^mixamorig:(Left|Right)?(.*)$", name)
    if m:
        side, rest = m.group(1), m.group(2)
        f = re.match(r"^Hand(Index|Middle|Ring|Pinky|Thumb)(\d)$", rest)
        sd = {"Left": "l", "Right": "r", None: None}[side]
        if f:
            k = int(f.group(2))
            base = f.group(1).lower()
            return f"{base}_{k:02d}_{sd}" if k < 4 else f"{base}_04_leaf_{sd}"
        base = MIXAMO.get(rest)
        if base:
            return f"{base}_{sd}" if sd else base
        return name
    m = re.match(r"^B-(.*?)(?:\.([LR]))?$", name)
    if m:
        rest, sd = m.group(1), (m.group(2) or "").lower()
        f = re.match(r"^(index|middle|ring)Finger(\d\d)$|^(pinky|thumb)(\d\d)$", rest)
        if f:
            base, k = (f.group(1), f.group(2)) if f.group(1) else (f.group(3), f.group(4))
            return f"{base}_{k}_{sd}"
        base = BRIG.get(rest)
        if base:
            return f"{base}_{sd}" if sd else base
    return name


def canonical_glb(path, name):
    """A copy of the GLB with UAL bone names when the rig uses another convention (only the JSON chunk changes)."""
    js, blob = statue.read_glb(path)
    new = [canonical(nd.get("name", "")) for nd in js["nodes"]]
    old = [nd.get("name", "") for nd in js["nodes"]]
    if new == old or len(set(new)) != len(new):
        return path
    for nd, nm in zip(js["nodes"], new):
        nd["name"] = nm
    d = os.path.join(REPO, "staging", "kev", "canon")
    os.makedirs(d, exist_ok=True)
    out = os.path.join(d, name)
    statue.write_glb(out, js, blob)
    return out


def model_info(name):
    """Per-model cache: glTF JSON, node hierarchy with rest TRS, Kev body map, clip list."""
    if name in CACHE:
        return CACHE[name]
    path = canonical_glb(os.path.join(ANIM, name), name)
    js, blob = statue.read_glb(path)
    n = len(js["nodes"])
    parent = [-1] * n
    for i, nd in enumerate(js["nodes"]):
        for c in nd.get("children", []):
            parent[c] = i
    order = []
    seen = set()

    def visit(i):
        if i in seen:
            return
        if parent[i] >= 0:
            visit(parent[i])
        seen.add(i)
        order.append(i)

    for i in range(n):
        visit(i)
    rest = [node_rest_trs(nd) for nd in js["nodes"]]
    rest_t = [r[0] for r in rest]
    rest_q = [r[1] for r in rest]
    rest_s = [r[2] for r in rest]
    names = [nd.get("name", f"node{i}") for i, nd in enumerate(js["nodes"])]
    skin = (js.get("skins") or [{}])[0]
    info = {"name": name, "js": js, "parent": parent, "order": order, "rest_q": rest_q, "rest_t": rest_t, "rest_s": rest_s, "names": names,
            "index": {nm: i for i, nm in enumerate(names)}, "clips": [a.get("name", f"clip{i}") for i, a in enumerate(js.get("animations", []))],
            "joints": skin.get("joints", []), "bm": None, "kev": None, "blob": blob}
    if skin:
        out = os.path.join(REPO, "staging", "kev", "bodymap")
        info["bm"] = bodymap.build(path, out)
        info["kev"] = kev_nodes.parse(path)
        info["rest_world"] = world_transforms(info, {})
    CACHE[name] = info
    return info


def world_transforms(info, quats, pos=None):
    """World 4x4 per node (glTF space): FK from the hierarchy, rest TRS with the given local rotations (and
    translations, `pos`) substituted."""
    n = len(info["names"])
    W = [None] * n
    for i in info["order"]:
        local = local_mat(info, quats, pos, i)
        p = info["parent"][i]
        W[i] = local if p < 0 else W[p] @ local
    return W


def kev_read(info, quats, pos=None):
    """Kev's answer for a pose: landmarks (m), facings, balance, and the world position of every joint."""
    bm = info["bm"]
    if bm is None:
        return {}
    Wn = world_transforms(info, quats, pos)
    jn = bm["joints"]
    D = np.zeros((len(jn), 4, 4))
    for k, name in enumerate(jn):
        i = info["index"][name]
        D[k] = Wn[i] @ np.linalg.inv(info["rest_world"][i])
    L = bodymap.posed_landmarks(bm, D)
    F = bodymap.posed_facings(bm, D)
    bal = bodymap.balance(bm, L)
    up, fwd = np.array(bm["body"]["up"]), np.array(bm["body"]["forward"])
    H, floor = bm["body"]["height"], bm["body"]["floor"]
    left = np.cross(up, fwd)
    words = {}
    for name, v in F.items():
        w, d = bodymap.word(v, fwd, up, left)
        words[name] = {"vector": [round(float(x), 3) for x in v], "toward": w, "off_deg": round(d)}
    joints = {jn[k]: [round(float(x), 4) for x in Wn[info["index"][jn[k]]][:3, 3]] for k in range(len(jn))}
    return {"landmarks": {k: [round(float(x), 4) for x in v] for k, v in L.items()},
            "heights": {k: round(float((v @ up - floor) / H), 3) for k, v in L.items()},
            "facings": words,
            "balance": {"com": [round(float(x), 4) for x in bal["com"]], "support": bal["support"], "over_foot": bal["over_foot"],
                        "off_foot_heights": round(bal["off_foot"], 4), "forward": round(bal["forward"], 3), "left": round(bal["left"], 3)},
            "joints": joints}


def clip_index():
    """clip name -> [glb files that carry it] (names only: no full model parse)."""
    if "clips" not in CACHE:
        idx = {}
        for f in sorted(os.listdir(ANIM)):
            if f.endswith(".glb"):
                try:
                    js, _ = statue.read_glb(os.path.join(ANIM, f))
                except Exception:  # noqa: BLE001
                    continue
                for i, an in enumerate(js.get("animations", [])):
                    idx.setdefault(an.get("name", f"clip{i}"), []).append(f)
        CACHE["clips"] = idx
    return CACHE["clips"]


def resolve_clip(info, want):
    """Exact, then unique case-insensitive substring in this model's clips; else a message saying where it lives."""
    clips = info["clips"] if info else []
    if want in clips:
        return want, None
    low = [c for c in clips if want.lower() in c.lower()]
    if len(low) == 1:
        return low[0], None
    if len(low) > 1:
        return None, f"'{want}' matches {len(low)} clips here: " + ", ".join(low[:8])
    elsewhere = {c: f for c, f in clip_index().items() if want.lower() in c.lower()}
    if elsewhere:
        return None, "not in this model; found in: " + "; ".join(f"{c} -> {', '.join(f)}" for c, f in list(elsewhere.items())[:6]) + "  (use `model <file>` first)"
    return None, f"no clip like '{want}' in any model"


def local_mat(info, Q, P, i):
    """Rest matrix, or rest TRS with animated rotation / translation written over it.

    Some rigs (kev_melee) store a node matrix and also animate that node's TRS. glTF runtimes
    decompose the matrix and let the channels replace translation and rotation. Using the matrix
    as-is drops the clip and the pose stays at rest.
    """
    nd = info["js"]["nodes"][i]
    nm = info["names"][i]
    q_over = (Q or {}).get(nm)
    p_over = (P or {}).get(nm)
    if "matrix" in nd and q_over is None and p_over is None:
        return node_matrix(nd)
    t = info["rest_t"][i] if p_over is None else np.asarray(p_over, float)
    q = info["rest_q"][i] if q_over is None else np.asarray(q_over, float)
    return compose_trs(t, q, info["rest_s"][i])


def chain_world(info, Q, P, i):
    """World matrix of one node: only its ancestors are multiplied (the solvers ask for a handful of nodes per step)."""
    path = []
    while i >= 0:
        path.append(i)
        i = info["parent"][i]
    W = np.eye(4)
    for j in reversed(path):
        W = W @ local_mat(info, Q, P, j)
    return W


def child_pos(info, Q, P, bone_i, child_i, Wb):
    return (Wb @ local_mat(info, Q, P, child_i))[:3, 3]


# ---- Kev moves things: world-space solvers --------------------------------------------------------------------------
COMMANDED = {}      # local quaternions Kev has commanded (name -> xyzw), the solver's own state
COMMANDED_POS = {}  # local translation overrides (the pelvis)
CHAINS = {"arm_l": ("upperarm_l", "lowerarm_l", "hand_l", "wrist_l", "elbow_l"), "arm_r": ("upperarm_r", "lowerarm_r", "hand_r", "wrist_r", "elbow_r"),
          "leg_l": ("thigh_l", "calf_l", "foot_l", "ankle_l", "knee_l"), "leg_r": ("thigh_r", "calf_r", "foot_r", "ankle_r", "knee_r")}
CHILD = {"upperarm_l": "lowerarm_l", "lowerarm_l": "hand_l", "upperarm_r": "lowerarm_r", "lowerarm_r": "hand_r",
         "thigh_l": "calf_l", "calf_l": "foot_l", "thigh_r": "calf_r", "calf_r": "foot_r", "foot_l": "ball_l", "foot_r": "ball_r"}


def unit(v):
    n = np.linalg.norm(v)
    return v / n if n > 1e-9 else v


def mat_to_quat(R):
    t = np.trace(R)
    if t > 0:
        k = math.sqrt(t + 1) * 2
        q = [(R[2, 1] - R[1, 2]) / k, (R[0, 2] - R[2, 0]) / k, (R[1, 0] - R[0, 1]) / k, 0.25 * k]
    elif R[0, 0] > R[1, 1] and R[0, 0] > R[2, 2]:
        k = math.sqrt(1 + R[0, 0] - R[1, 1] - R[2, 2]) * 2
        q = [0.25 * k, (R[0, 1] + R[1, 0]) / k, (R[0, 2] + R[2, 0]) / k, (R[2, 1] - R[1, 2]) / k]
    elif R[1, 1] > R[2, 2]:
        k = math.sqrt(1 + R[1, 1] - R[0, 0] - R[2, 2]) * 2
        q = [(R[0, 1] + R[1, 0]) / k, 0.25 * k, (R[1, 2] + R[2, 1]) / k, (R[0, 2] - R[2, 0]) / k]
    else:
        k = math.sqrt(1 + R[2, 2] - R[0, 0] - R[1, 1]) * 2
        q = [(R[0, 2] + R[2, 0]) / k, (R[1, 2] + R[2, 1]) / k, 0.25 * k, (R[1, 0] - R[0, 1]) / k]
    q = np.array(q)
    return q / np.linalg.norm(q)


def decompose_trs(M):
    """Translation, rotation quaternion, scale of a glTF node matrix (column scaling, no shear)."""
    t = M[:3, 3].copy()
    A = M[:3, :3].copy()
    s = np.linalg.norm(A, axis=0)
    s = np.where(s < 1e-12, 1.0, s)
    R = A / s
    if np.linalg.det(R) < 0:
        s = s.copy()
        s[0] *= -1
        R[:, 0] *= -1
    U, _, Vt = np.linalg.svd(R)
    R = U @ Vt
    if np.linalg.det(R) < 0:
        U = U.copy()
        U[:, -1] *= -1
        R = U @ Vt
    return t, mat_to_quat(R), s


def node_rest_trs(nd):
    if "matrix" in nd:
        return decompose_trs(node_matrix(nd))
    t = np.array(nd.get("translation", [0, 0, 0]), float)
    q = np.array(nd.get("rotation", [0, 0, 0, 1]), float)
    s = np.array(nd.get("scale", [1, 1, 1]), float)
    return t, q, s


def arc_mat(a, b):
    """Rotation matrix taking unit vector a to unit vector b along the shortest arc."""
    a, b = unit(a), unit(b)
    v = np.cross(a, b)
    c = float(a @ b)
    if c < -0.999999:
        axis = unit(np.cross(a, [1, 0, 0]) if abs(a[0]) < 0.9 else np.cross(a, [0, 1, 0]))
        return 2 * np.outer(axis, axis) - np.eye(3)
    K = np.array([[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]])
    return np.eye(3) + K + K @ K / (1 + c)


def rot_of(W):
    R = W[:3, :3].copy()
    for i in range(3):
        R[:, i] = unit(R[:, i])
    return R


def slerp_mat(R0, R1, t):
    q0, q1 = mat_to_quat(R0), mat_to_quat(R1)
    if q0 @ q1 < 0:
        q1 = -q1
    d = min(1.0, float(q0 @ q1))
    th = math.acos(d)
    q = q0 if th < 1e-6 else (math.sin((1 - t) * th) * q0 + math.sin(t * th) * q1) / math.sin(th)
    x, y, z, w = q / np.linalg.norm(q)
    return np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                     [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                     [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])


def slerp_near(R0, R1, t, rest, near):
    """Blend two bases, then apply `rest`. A previous pose picks the quaternion sign that stays beside it.

    Shortest-arc slerp flips the neck a half turn when the head and the chest pass through opposite
    orientations: the two signs are the same rotation of each base, but the halfway point is not.
    """
    q0, q1 = mat_to_quat(R0), mat_to_quat(R1)
    signs = (q1, -q1) if near is not None else ((-q1 if float(q0 @ q1) < 0 else q1),)
    best, best_a = None, 1e9
    for qq in signs:
        d = float(np.clip(float(q0 @ qq), -1.0, 1.0))
        th = math.acos(d)
        # Opposite quaternions are the same rotation; slerping them is 0/0.
        if th < 1e-6 or th > math.pi - 1e-4:
            q = q0
        else:
            q = (math.sin((1.0 - t) * th) * q0 + math.sin(t * th) * qq) / math.sin(th)
        q = q / np.linalg.norm(q)
        if not np.isfinite(q).all():
            continue
        R = quat_to_mat(q) @ rest
        if near is None:
            return R
        ang = quat_angle(near, R)
        if ang < best_a:
            best, best_a = R, ang
    return best if best is not None else slerp_mat(R0, R1, t) @ rest


def set_world_rot(info, Q, P, bone, R):
    """Command bone so its world rotation is R (the parents' current world rotation is read from FK)."""
    i = info["index"][bone]
    p = info["parent"][i]
    Pw = rot_of(chain_world(info, Q, P, p)) if p >= 0 else np.eye(3)
    Q[bone] = mat_to_quat(Pw.T @ R)


def aim(info, Q, P, bone, direction, child=None):
    """Swing `bone` so its direction to its child points along `direction` (world), from the current pose."""
    child = child or CHILD[bone]
    bi, ci = info["index"][bone], info["index"][child]
    Wb = chain_world(info, Q, P, bi)
    cur = child_pos(info, Q, P, bi, ci, Wb) - Wb[:3, 3]
    set_world_rot(info, Q, P, bone, arc_mat(cur, direction) @ rot_of(Wb))


def rot_about(axis, theta):
    a = unit(np.asarray(axis, float))
    x, y, z = a
    c, s = math.cos(theta), math.sin(theta)
    C = 1.0 - c
    return np.array([[c + x * x * C, x * y * C - z * s, x * z * C + y * s],
                     [y * x * C + z * s, c + y * y * C, y * z * C - x * s],
                     [z * x * C - y * s, z * y * C + x * s, c + z * z * C]])


def twist_closest(R, prev, axis):
    """Spin R around `axis` so a perpendicular bone axis stays as close as possible to `prev`.

    aim() only constrains the bone direction. The twist around it is free, and the 180-degree case of the
    shortest arc picks an arbitrary roll, which turns a hand or a foot inside out between two frames.
    """
    a = unit(np.asarray(axis, float))
    if np.linalg.norm(a) < 1e-6:
        return R
    rc = pc = None
    for col in range(3):
        rv = R[:, col] - a * float(a @ R[:, col])
        pv = prev[:, col] - a * float(a @ prev[:, col])
        if np.linalg.norm(rv) > 0.25 and np.linalg.norm(pv) > 0.25:
            rc, pc = unit(rv), unit(pv)
            break
    if rc is None:
        return R
    theta = math.atan2(float(a @ np.cross(rc, pc)), float(np.clip(rc @ pc, -1.0, 1.0)))
    if abs(theta) < 1e-4:
        return R
    return rot_about(a, theta) @ R


def bone_world_rot(info, Q, P, bone):
    if bone not in info["index"]:
        return None
    return rot_of(chain_world(info, Q, P, info["index"][bone]))


def pelvis_axis(info, Q, P, rest_world):
    """Where the rig's rest `rest_world` axis points now that the pelvis has been posed."""
    bi = info["index"]["pelvis"]
    local = rot_of(info["rest_world"][bi]).T @ unit(np.asarray(rest_world, float))
    return unit(bone_world_rot(info, Q, P, "pelvis") @ local)


def swing_towards(src, dst, max_deg):
    """Direction of `dst`, limited to `max_deg` away from `src`."""
    s, d = unit(src), unit(dst)
    c = float(np.clip(s @ d, -1.0, 1.0))
    ang = math.acos(c)
    cap = math.radians(max_deg)
    if ang < 1e-4 or ang <= cap:
        return d
    t = cap / ang
    return unit(math.sin((1.0 - t) * ang) / math.sin(ang) * s + math.sin(t * ang) / math.sin(ang) * d)


def quat_angle(R0, R1):
    q0, q1 = mat_to_quat(R0), mat_to_quat(R1)
    d = min(1.0, abs(float(q0 @ q1)))
    return math.degrees(2.0 * math.acos(d))


def reach(info, Q, P, chain, target, pole_pt=None, flex_dir=None):
    """Two-bone IK: put the chain's end joint at `target` (world), bending toward `pole_pt`. Returns the reach error (m).

    `flex_dir` is the anatomical bend (knees with the body's forward, elbows against it). A pole on the wrong
    side of the hip-ankle / shoulder-wrist line is reflected, so a bad sheet reading cannot hyperextend the hinge.
    """
    root, mid, end = CHAINS[chain][:3]
    pos = lambda n: chain_world(info, Q, P, info["index"][n])[:3, 3]  # noqa: E731
    r, m, e = pos(root), pos(mid), pos(end)
    l1, l2 = np.linalg.norm(m - r), np.linalg.norm(e - m)
    t = np.asarray(target, float) - r
    d = float(np.clip(np.linalg.norm(t), 1e-3, (l1 + l2) * 0.999))
    u = unit(t)
    pole = (np.asarray(pole_pt, float) - r) if pole_pt is not None else (m - r)
    pole = pole - u * float(np.asarray(pole, float) @ u)
    if np.linalg.norm(pole) < 1e-6:
        pole = (m - r) - u * float((m - r) @ u)
    pole = unit(pole)
    if flex_dir is not None:
        f = np.asarray(flex_dir, float)
        f = f - u * float(f @ u)
        if np.linalg.norm(f) > 1e-4 and np.linalg.norm(pole) > 1e-6 and float(pole @ unit(f)) < -0.35:
            pole = -pole
    a_ = (l1 * l1 - l2 * l2 + d * d) / (2 * d)
    h = math.sqrt(max(l1 * l1 - a_ * a_, 0.0))
    mid_new = r + u * a_ + pole * h
    aim(info, Q, P, root, mid_new - r)
    aim(info, Q, P, mid, (r + u * d) - pos(mid))
    return float(np.linalg.norm(pos(end) - np.asarray(target, float)))


def basis(x_dir, y_dir):
    x = unit(np.asarray(x_dir, float))
    z = unit(np.cross(x, y_dir))
    y = np.cross(z, x)
    return np.column_stack([x, y, z])


def rest_lm(info, name):
    return np.array(info["bm"]["landmarks"][name]["rest"], float)


FINGERS = ("thumb", "index", "middle", "ring", "pinky")


def hand_names(info, side):
    """Bones of one hand in chain order: {finger: [bone, ...child]} plus the hand bone itself."""
    out = {}
    for f in FINGERS:
        ch, i = [], 1
        while True:
            hit = [n for n in (f"{f}_{i:02d}_{side}", f"{f}_{i:02d}_leaf_{side}") if n in info["index"]]
            if not hit:
                break
            ch.append(hit[0])
            i += 1
        if ch:
            out[f] = ch
    return out


def hand_frame(pt):
    """Palm frame from positions of hand, middle_01, index_01, pinky_01: x along the fingers, z across the palm normal."""
    return basis(np.asarray(pt["middle_01"]) - np.asarray(pt["hand"]), np.asarray(pt["pinky_01"]) - np.asarray(pt["index_01"]))


def hand_pts(get, side):
    return {"hand": get(f"hand_{side}"), "middle_01": get(f"middle_01_{side}"), "index_01": get(f"index_01_{side}"), "pinky_01": get(f"pinky_01_{side}")}


def pose_hand(info, Q, P, side, hands):
    """Orient the hand from the reference palm frame, then aim every finger bone along the reference's
    segment direction taken in that frame. Returns the worst fingertip error (m) against the reference."""
    names = hand_names(info, side)
    if not names or f"hand_{side}" not in hands or not all(f"{k}_{side}" in hands for k in ("middle_01", "index_01", "pinky_01")):
        return None
    Wr = info["rest_world"]
    rpos = lambda n: Wr[info["index"][n]][:3, 3]  # noqa: E731
    Fref = hand_frame(hand_pts(lambda n: hands[n], side))
    Frest = hand_frame(hand_pts(rpos, side))
    hb = f"hand_{side}"
    set_world_rot(info, Q, P, hb, Fref @ Frest.T @ rot_of(Wr[info["index"][hb]]))
    wp = lambda n: chain_world(info, Q, P, info["index"][n])[:3, 3]  # noqa: E731
    Ft = hand_frame(hand_pts(wp, side))
    for f, ch in names.items():
        for b, c in zip(ch, ch[1:]):
            if b not in hands or c not in hands:
                continue
            aim(info, Q, P, b, Ft @ (Fref.T @ (np.asarray(hands[c]) - np.asarray(hands[b]))), c)
    tips = [ch[-1] for ch in names.values() if ch[-1] in hands]
    if not tips:
        return None
    return max(float(np.linalg.norm((wp(t) - wp(hb)) - (np.asarray(hands[t]) - np.asarray(hands[hb])))) for t in tips)


CURL = {"thumb": (35, 45, 35), "index": (80, 95, 70), "middle": (85, 100, 75), "ring": (85, 100, 75), "pinky": (85, 100, 70)}
POSES = {"open": 0.0, "fist": 1.0, "point": None, "relaxed": 0.3}


def curl_hand(info, Q, P, side, amount, point=False):
    """Curl a hand from rest: each finger joint bends toward the palm by its share of `amount` (0 open .. 1 fist)."""
    W = world_transforms(info, Q, P)
    Wr = info["rest_world"]
    names = hand_names(info, side)
    pt = hand_pts(lambda n: W[info["index"][n]][:3, 3], side)
    F = hand_frame(pt)
    axis = F[:, 1]
    rpos = lambda n: Wr[info["index"][n]][:3, 3]  # noqa: E731
    Frest = hand_frame(hand_pts(rpos, side))
    # Sign from the open hand. Reading it off the fingers already posed flips once they pass straight
    # and the next frame bends them backward.
    csgn = 0.0
    for f, ch in names.items():
        if f == "thumb" or len(ch) < 2:
            continue
        d = rpos(ch[1]) - rpos(ch[0])
        csgn += float(np.cross(Frest[:, 1], d) @ Frest[:, 2])
    sgn = 1.0 if csgn < 0 else -1.0
    for f, ch in names.items():
        amt = 0.0 if (point and f == "index") else amount
        d_rest = [rpos(c) - rpos(b) for b, c in zip(ch, ch[1:])]
        ang = 0.0
        for (b, c), dr, deg in zip(zip(ch, ch[1:]), d_rest, CURL[f]):
            ang += math.radians(deg) * amt
            rot = math.cos(ang / 2), axis * math.sin(sgn * ang / 2)
            v = F @ (Frest.T @ dr)
            w, u = rot
            v2 = v + 2 * np.cross(u, np.cross(u, v) + w * v)
            aim(info, Q, P, b, v2, c)
    return sgn


def sole_locals(info):
    """Bottom of each foot in that foot bone's rest frame, plus the skinning of the lowest sole vertices."""
    cached = info.get("sole_local")
    if cached is not None:
        return cached
    bm = info["bm"]
    z = np.load(os.path.join(REPO, "staging", "kev", "bodymap", bm["rig"] + ".npz"))
    rest, part = z["rest_position"], z["part"]
    bind, jidx, wts = z["mesh_position"], z["joints"], z["weights"]
    jn = bm["joints"]
    up = np.array(bm["body"]["up"], float)
    floor = float(bm["body"]["floor"])
    out, skin = {}, {}
    for side in "lr":
        ids = [jn.index(n) for n in (f"foot_{side}", f"ball_{side}") if n in jn]
        sel = np.where(np.isin(part, ids))[0]
        h = rest[sel] @ up
        idx = sel[np.argsort(h)[:16]]
        bi = info["index"][f"foot_{side}"]
        R = rot_of(info["rest_world"][bi])
        origin = info["rest_world"][bi][:3, 3]
        rows = [(rest[idx] - origin) @ R]
        for name in (f"ball_{side}", f"toe_tip_{side}", f"heel_{side}"):
            d = bm["landmarks"].get(name)
            if d:
                rows.append(((np.array(d["rest"], float) - origin) @ R)[None, :])
        out[side] = np.vstack(rows)
        skin[side] = {"local": bind[idx], "joints": jidx[idx], "weights": wts[idx]}
    info["sole_local"] = out
    info["sole_skin"] = skin
    return out


def _skin_matrices(info, Q, P):
    bm = info["bm"]
    Wn = world_transforms(info, Q, P)
    jn = bm["joints"]
    D = np.zeros((len(jn), 4, 4))
    for k, name in enumerate(jn):
        i = info["index"][name]
        D[k] = Wn[i] @ np.linalg.inv(info["rest_world"][i])
    return D @ np.array(bm["rest_world"]) @ np.array(bm["inverse_bind"])


def skinned_sole(info, Q, P, side):
    """Posed positions of the lowest sole vertices. A pelvis translation lifts these by the same amount."""
    sole_locals(info)
    data = info["sole_skin"][side]
    skinm = _skin_matrices(info, Q, P)
    sm = skinm[data["joints"]]
    M = np.einsum("nk,nkij->nij", data["weights"], sm)
    return np.einsum("nij,nj->ni", M[:, :3, :3], data["local"]) + M[:, :3, 3]


def plant_sole(info, Q, P, side, lm):
    """Pitch a foot that is down near the floor so the ball, the toe and the mesh sole stay on it.

    The foot bone is longer than the drawn ankle-to-ball, and the toe mesh hangs past the ball joint,
    so aiming the bone at a ball on the floor line drives the toe through the ground.
    """
    fb = f"foot_{side}"
    if fb not in info["index"] or not info.get("bm"):
        return
    floor = float(info["bm"]["body"]["floor"])
    up = np.array(info["bm"]["body"]["up"], float)
    ankle_lm = np.asarray(lm.get(f"ankle_{side}", [0.0, 1.0e9, 0.0]), float)
    origin = chain_world(info, Q, P, info["index"][fb])[:3, 3]
    if min(float(origin @ up) - floor, float(ankle_lm @ up) - floor) > 0.32:
        return
    local = sole_locals(info)[side]
    for _ in range(4):
        Rw = bone_world_rot(info, Q, P, fb)
        origin = chain_world(info, Q, P, info["index"][fb])[:3, 3]
        h = (local @ Rw.T + origin) @ up
        i = int(np.argmin(h))
        pen = floor - float(h[i])
        if pen <= 1e-4:
            return
        lever = (local[i] @ Rw.T + origin) - origin
        axis = np.cross(up, lever)
        if np.linalg.norm(axis) < 1e-4:
            return
        axis = unit(axis)
        dy = float(np.cross(axis, lever) @ up)
        if abs(dy) < 1e-5:
            return
        theta = float(np.clip(pen / dy, -0.6, 0.6))
        set_world_rot(info, Q, P, fb, rot_about(axis, theta) @ Rw)


def sole_gap(info, Q, P, side, lm, read=None):
    """Metres a grounded foot still sticks through the floor: mesh sole, toe tip, heel and ball. 0 if the foot is up."""
    fb = f"foot_{side}"
    if fb not in info["index"] or not info.get("bm"):
        return 0.0
    floor = float(info["bm"]["body"]["floor"])
    up = np.array(info["bm"]["body"]["up"], float)
    ankle_lm = np.asarray(lm.get(f"ankle_{side}", [0.0, 1.0e9, 0.0]), float)
    origin = chain_world(info, Q, P, info["index"][fb])[:3, 3]
    if min(float(origin @ up) - floor, float(ankle_lm @ up) - floor) > 0.32:
        return 0.0
    Rw = bone_world_rot(info, Q, P, fb)
    h = (sole_locals(info)[side] @ Rw.T + origin) @ up
    gap = floor - float(np.min(h))
    gap = max(gap, floor - float(np.min(skinned_sole(info, Q, P, side) @ up)))
    if read is not None:
        for n, src in ((f"ball_{side}", "joints"), (f"toe_tip_{side}", "landmarks"), (f"heel_{side}", "landmarks")):
            if n in read.get(src, {}):
                gap = max(gap, floor - float(np.array(read[src][n]) @ up))
    return max(0.0, gap)


def lift_skeleton(info, Q, P, pen):
    """Raise the whole pose so a sole that cannot pitch clear of the floor (the ankle itself is too low) sits on it."""
    if pen <= 0.0 or "pelvis" not in info["index"]:
        return
    pen += 2e-4  # a fifth of a millimetre, so rounding does not leave the sole on the wrong side of the floor
    pi = info["index"]["pelvis"]
    parent = info["parent"][pi]
    Wp = chain_world(info, Q, P, parent) if parent >= 0 else np.eye(4)
    world = chain_world(info, Q, P, pi)[:3, 3] + np.array(info["bm"]["body"]["up"], float) * pen
    P["pelvis"] = rot_of(Wp).T @ (world - Wp[:3, 3])


def _arm_extension(info, Q, P, side):
    need = [f"upperarm_{side}", f"lowerarm_{side}", f"hand_{side}"]
    if any(n not in info["index"] for n in need):
        return 0.5
    def wp(n):
        return chain_world(info, Q, P, info["index"][n])[:3, 3]
    upper = np.linalg.norm(wp(f"lowerarm_{side}") - wp(f"upperarm_{side}"))
    lower = np.linalg.norm(wp(f"hand_{side}") - wp(f"lowerarm_{side}"))
    reach = np.linalg.norm(wp(f"hand_{side}") - wp(f"upperarm_{side}"))
    return float(reach / max(upper + lower, 1e-6))


def orient_palm(info, Q, P, side):
    """Twist the hand about the forearm so the palm faces down / inward instead of following the forearm."""
    needed = [f"hand_{side}", f"lowerarm_{side}", f"middle_01_{side}", f"index_01_{side}", f"pinky_01_{side}"]
    if any(n not in info["index"] for n in needed) or not info.get("bm"):
        return
    def wp(n):
        return chain_world(info, Q, P, info["index"][n])[:3, 3]
    nrm = quality.palm_normal(side, wp(f"hand_{side}"), wp(f"middle_01_{side}"), wp(f"index_01_{side}"), wp(f"pinky_01_{side}"))
    if nrm is None:
        return
    forearm = wp(f"hand_{side}") - wp(f"lowerarm_{side}")
    body = info["bm"]["body"]
    up, fwd, left = np.array(body["up"], float), np.array(body["forward"], float), np.array(body["left"], float)
    inward = left if side == "l" else -left
    ang = quality.palm_delta(forearm, nrm, up, inward, fwd)
    if abs(ang) < math.radians(6):
        return
    hb = f"hand_{side}"
    Wb = chain_world(info, Q, P, info["index"][hb])
    set_world_rot(info, Q, P, hb, quality.rodrigues(forearm, ang) @ rot_of(Wb))


def apply_frame(info, lm, inplace=True, hands=None, Qp=None, Pp=None, cleanup=True):
    """One control frame: landmark targets (world, metres, this model's scale) -> commanded rotations and pelvis position.

    Pelvis: translated to the pelvis target; pelvis and chest oriented from the hip line and shoulder line with the
    spine direction as up (the spine bones share the twist between them); head from crown and nose; each arm and leg
    reaches wrist / ankle with the elbow / knee as the pole; feet aim at the ball. Missing targets are skipped.
    Returns {landmark: error_m} for the reach targets. With `cleanup`, targets pass the no-glitch
    frame pass (lengths, hinges, floor, finger back-bend) before the solve."""
    Q, P = (Qp, Pp) if Qp is not None else (COMMANDED, COMMANDED_POS)
    if cleanup and lm and info.get("bm"):
        lm, hands = quality.clean_targets(lm, hands, info["bm"]["body"], quality.lengths_from_body(info["bm"]))
    need = lambda *ns: all(n in lm for n in ns)  # noqa: E731
    Wr = info["rest_world"]
    rest_rot = lambda b: rot_of(Wr[info["index"][b]])  # noqa: E731
    if need("pelvis"):
        target = np.array(lm["pelvis"], float)
        if inplace:
            target[[0, 2]] = rest_lm(info, "pelvis")[[0, 2]]
        pi = info["index"]["pelvis"]
        p = info["parent"][pi]
        Wp = chain_world(info, Q, P, p) if p >= 0 else np.eye(4)
        Pw, pp = rot_of(Wp), Wp[:3, 3]
        P["pelvis"] = Pw.T @ (target - pp)
    prev_rot = {b: bone_world_rot(info, Q, P, b) for b in ("hand_l", "hand_r", "foot_l", "foot_r", "Head", "neck_01")}
    if need("hip_l", "hip_r", "neck", "pelvis", "shoulder_l", "shoulder_r"):
        up = np.array(lm["neck"]) - np.array(lm["pelvis"])
        # Linear landmark blends collapse a flip: the neck-pelvis vector whips through 100 degrees on one frame.
        # Cap the step. A real strike at 60 fps stays under this; the catch-up lands on the following frames.
        if "pelvis" in Q and np.linalg.norm(up) > 1e-6:
            prev_up = pelvis_axis(info, Q, P, info["bm"]["body"]["up"])
            up = swing_towards(prev_up, up, 48)
        up0 = rest_lm(info, "neck") - rest_lm(info, "pelvis")
        Dp = basis(np.array(lm["hip_l"]) - np.array(lm["hip_r"]), up) @ basis(rest_lm(info, "hip_l") - rest_lm(info, "hip_r"), up0).T
        Dc = basis(np.array(lm["shoulder_l"]) - np.array(lm["shoulder_r"]), up) @ basis(rest_lm(info, "shoulder_l") - rest_lm(info, "shoulder_r"), up0).T
        for bone, t in (("pelvis", 0.0), ("spine_01", 1 / 3), ("spine_02", 2 / 3), ("spine_03", 1.0)):
            if bone not in info["index"]:
                continue
            set_world_rot(info, Q, P, bone, slerp_mat(Dp, Dc, t) @ rest_rot(bone))
        if need("crown", "nose", "head_joint"):
            hj = np.array(lm["head_joint"])
            xraw = np.cross(np.array(lm["crown"]) - hj, np.array(lm["nose"]) - hj)
            yraw = np.array(lm["crown"]) - hj
            # Nose read onto the crown-neck line makes this cross a coin flip and the head pops 180 degrees.
            if np.linalg.norm(xraw) < 0.006 or np.linalg.norm(yraw) < 1e-4:
                Dh = Dc
            else:
                Dh = basis(xraw, yraw) @ \
                    basis(np.cross(rest_lm(info, "crown") - rest_lm(info, "head_joint"), rest_lm(info, "nose") - rest_lm(info, "head_joint")),
                          rest_lm(info, "crown") - rest_lm(info, "head_joint")).T
            Rh = Dh @ rest_rot("Head")
            if prev_rot.get("Head") is not None:
                ang = quat_angle(prev_rot["Head"], Rh)
                if ang > 60.0:
                    Rh = slerp_mat(prev_rot["Head"], Rh, min(1.0, 40.0 / ang))
                    Dh = Rh @ rest_rot("Head").T
            if "neck_01" in info["index"]:
                set_world_rot(info, Q, P, "neck_01", slerp_near(Dc, Dh, 0.5, rest_rot("neck_01"), prev_rot.get("neck_01")))
            set_world_rot(info, Q, P, "Head", Rh)
    err = {}
    flex = None
    if "pelvis" in info["index"] and info.get("bm"):
        fwd = pelvis_axis(info, Q, P, info["bm"]["body"]["forward"])
        flex = {"leg": fwd, "arm": -fwd}
    for chain, (root, mid, end, tgt, pole) in CHAINS.items():
        if need(tgt):
            kind = "leg" if chain.startswith("leg") else "arm"
            err[tgt] = reach(info, Q, P, chain, lm[tgt], lm.get(pole), None if flex is None else flex[kind])
    herr = {}
    posed = set()
    for side in "lr":
        if hands:
            e = pose_hand(info, Q, P, side, hands)
            if e is not None:
                herr[f"fingers_{side}"] = e
                posed.add(side)
    for side in "lr":
        hb = f"hand_{side}"
        child = f"middle_01_{side}"
        if side in posed or hb not in info["index"] or prev_rot.get(hb) is None:
            continue
        if child in info["index"]:
            Wb = chain_world(info, Q, P, info["index"][hb])
            axis = child_pos(info, Q, P, info["index"][hb], info["index"][child], Wb) - Wb[:3, 3]
        else:
            axis = bone_world_rot(info, Q, P, hb) @ np.array([0.0, 1.0, 0.0])
        set_world_rot(info, Q, P, hb, twist_closest(bone_world_rot(info, Q, P, hb), prev_rot[hb], axis))
    for side in "lr":
        if need(f"ball_{side}", f"ankle_{side}"):
            direction = np.array(lm[f"ball_{side}"]) - np.array(lm[f"ankle_{side}"])
            aim(info, Q, P, f"foot_{side}", direction)
            fb = f"foot_{side}"
            if prev_rot.get(fb) is not None:
                set_world_rot(info, Q, P, fb, twist_closest(bone_world_rot(info, Q, P, fb), prev_rot[fb], direction))
            plant_sole(info, Q, P, side, lm)
    if info.get("bm") and "pelvis" in info.get("index", {}):
        read = kev_read(info, Q, P)
        lift_skeleton(info, Q, P, max(sole_gap(info, Q, P, s, lm, read) for s in "lr"))
    if cleanup:
        _clamp_solved_hinges(info, Q, P)
    if not hands:
        for side in "lr":
            orient_palm(info, Q, P, side)
            ext = _arm_extension(info, Q, P, side)
            curl_hand(info, Q, P, side, float(np.clip(0.15 + max(ext - 0.55, 0) * 1.7, 0.15, 1.0)))
    err.update(herr)
    return err


def _clamp_solved_hinges(info, Q, P):
    """If IK left a knee backward or an elbow popped forward, re-aim with the pole on the legal side."""
    if not info.get("bm"):
        return
    fwd = np.array(info["bm"]["body"]["forward"], float)
    # Joint origins: thigh/calf/foot and upperarm/lowerarm/hand. Landmark names differ.
    spec = (("leg_l", "thigh_l", "calf_l", "foot_l", "knee"),
            ("leg_r", "thigh_r", "calf_r", "foot_r", "knee"),
            ("arm_l", "upperarm_l", "lowerarm_l", "hand_l", "elbow"),
            ("arm_r", "upperarm_r", "lowerarm_r", "hand_r", "elbow"))
    for chain, a, b, c, kind in spec:
        if any(n not in info["index"] for n in (a, b, c)):
            continue
        def wp(n, _Q=Q, _P=P):
            return chain_world(info, _Q, _P, info["index"][n])[:3, 3]
        pa, pb, pc = wp(a), wp(b), wp(c)
        if not quality.hinge_bad(pa, pb, pc, kind, fwd):
            continue
        reflected = quality._reflect(pa, pb, pc)
        pole = reflected if np.linalg.norm(reflected - pb) > 1e-4 else (pb + (fwd if kind == "knee" else -fwd))
        reach(info, Q, P, chain, pc, pole)


def apply_frame_into(info, Q, P, lm, hands):
    return apply_frame(info, lm, inplace=False, hands=hands, Qp=Q, Pp=P)


def pose_event():
    return {"cmd": "pose", "quats": {k: [round(float(x), 5) for x in v] for k, v in COMMANDED.items()},
            "pos": {k: [round(float(x), 5) for x in v] for k, v in COMMANDED_POS.items()}}


def apply_physics(info, landmarks):
    """Optional pass in front of the kinematic solve. Off unless `phys on`. Returns (landmarks, status)."""
    if not PHYS["on"]:
        return landmarks, {"on": False}
    if PHYS["sim"] is None or PHYS["model"] != STATE.get("model"):
        floor = None
        if info and info.get("bm"):
            floor = info["bm"].get("body", {}).get("floor")
        PHYS["sim"] = physics.PhysicsPass(floor=floor)
        PHYS["model"] = STATE.get("model")
        PHYS["last"] = time.time()
    now = time.time()
    dt = min(0.05, max(1.0 / 120.0, now - PHYS["last"] if PHYS["last"] else 1.0 / 30.0))
    PHYS["last"] = now
    step = PHYS["sim"].step(landmarks, dt)
    return step["landmarks"], {"on": True, **step["diag"]}


# ---- Kev references: capture measurements while a clip plays, replay them onto any model --------------------------
HAND_RE = re.compile(r"^(hand|thumb|index|middle|ring|pinky)(_\d\d)?(_leaf)?_[lr]$")
HERE_TOOLS = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REFDIR = os.path.join(REPO, "staging", "kev", "refs")
CAPTURE = {"on": False, "name": None, "samples": [], "last_t": -1.0, "model": None, "clip": None}
REFPLAY = {"thread": None, "stop": False, "status": {}}


def capture_sample(model, clip, t, summary):
    c = CAPTURE
    if not c["on"]:
        return
    if c["clip"] is None:
        c["clip"], c["model"] = clip, model
    if t < c["last_t"] - 0.05 and len(c["samples"]) > 5:  # the clip looped: one full cycle recorded
        return finish_capture()
    c["last_t"] = t
    if not c["samples"] or t > c["samples"][-1]["t"] + 1e-4:
        hands = {k: v for k, v in summary["joints"].items() if HAND_RE.match(k)}
        c["samples"].append({"t": round(t, 4), "landmarks": summary["landmarks"], "hands": hands, "com": summary["balance"]["com"]})


def finish_capture():
    c = CAPTURE
    c["on"] = False
    info = model_info(c["model"])
    bm = info["bm"]
    ref = {"name": c["name"], "model": c["model"], "clip": c["clip"], "height": bm["body"]["height"], "floor": bm["body"]["floor"],
           "duration": c["samples"][-1]["t"] if c["samples"] else 0.0, "samples": c["samples"]}
    os.makedirs(REFDIR, exist_ok=True)
    with open(os.path.join(REFDIR, c["name"] + ".json"), "w") as f:
        json.dump(ref, f)
    broadcast("log", {"line": f"(capture finished) {c['name']}", "out": f"{len(c['samples'])} samples, {ref['duration']:.2f} s, from {c['model']} / {c['clip']}", "from": "kev"})
    return ref


def load_ref(name):
    with open(os.path.join(REFDIR, name + ".json")) as f:
        return json.load(f)


def sample_ref(ref, tau):
    ss = ref["samples"]
    tau = min(max(tau, ss[0]["t"]), ss[-1]["t"])
    hi = next((i for i, s in enumerate(ss) if s["t"] >= tau), len(ss) - 1)
    lo = max(hi - 1, 0)
    a, b = ss[lo], ss[hi]
    u = 0.0 if b["t"] == a["t"] else (tau - a["t"]) / (b["t"] - a["t"])
    mixd = lambda da, db: {k: [x + (y - x) * u for x, y in zip(da[k], db[k])] for k in da if k in db}  # noqa: E731
    return mixd(a["landmarks"], b["landmarks"]), mixd(a.get("hands", {}), b.get("hands", {}))


def scale_to(info, ref, lm):
    """Reference landmarks (its model's metres) -> this model: uniform height ratio about the floor."""
    s = info["bm"]["body"]["height"] / ref["height"]
    f0, f1 = ref["floor"], info["bm"]["body"]["floor"]
    first = ref["samples"][0]["landmarks"]["pelvis"]
    return {k: [(v[0] - first[0]) * s, (v[1] - f0) * s + f1, (v[2] - first[2]) * s] for k, v in lm.items()}


def scale_hands(info, ref, hands):
    s = info["bm"]["body"]["height"] / ref["height"]
    return {k: [v[0] * s, v[1] * s, v[2] * s] for k, v in hands.items()}  # directions only: no offset needed


def clip_tracks(info, clip):
    """{bone: (times, values, path)} for one animation of the loaded GLB, read straight from the buffer."""
    js, blob = info["js"], info["blob"]
    anim = next(a for a in js["animations"] if a.get("name") == clip)
    out = {}
    for ch in anim["channels"]:
        path = ch["target"].get("path")
        if path not in ("rotation", "translation"):
            continue
        sm = anim["samplers"][ch["sampler"]]
        t, _ = statue.accessor_view(js, blob, sm["input"])
        v, _ = statue.accessor_view(js, blob, sm["output"])
        kind = sm.get("interpolation", "LINEAR")
        if kind == "CUBICSPLINE":  # (in-tangent, value, out-tangent) per key: keep the values, read them linearly
            v, kind = v[1::3], "LINEAR"
        out[(info["names"][ch["target"]["node"]], path)] = (t[:, 0], v, kind)
    return out


def clip_motion_span(model, clip):
    """Largest rotation (degrees) of any bone away from its first key. Used to tell a real static clip
    from one whose tracks never reached the skeleton."""
    info = model_info(model)
    tracks = clip_tracks(info, clip)
    max_rot = 0.0
    for (_bone, path), (_t, v, _kind) in tracks.items():
        if path != "rotation" or len(v) < 2:
            continue
        dots = np.clip(np.abs(v @ v[0]), 0.0, 1.0)
        ang = float(np.degrees(2 * np.arccos(dots)).max())
        if ang > max_rot:
            max_rot = ang
    return max_rot


def eval_track(t, v, kind, x):
    x = min(max(x, t[0]), t[-1])
    hi = min(int(np.searchsorted(t, x)), len(t) - 1)
    if hi == 0:
        return v[0]
    lo = hi - 1
    u = (x - t[lo]) / max(t[hi] - t[lo], 1e-9)
    if kind == "STEP":
        return v[lo]
    if v.shape[1] == 4:
        a, b = v[lo], v[hi]
        if a @ b < 0:
            b = -b
        q = a * (1 - u) + b * u
        return q / np.linalg.norm(q)
    return v[lo] * (1 - u) + v[hi] * u


def grab(model, clip, name, fps=60):
    """Kev reference straight from a clip: every frame at `fps`, no browser, no real-time wait."""
    info = model_info(model)
    tracks = clip_tracks(info, clip)
    dur = max(t[-1] for t, _, _ in tracks.values())
    samples = []
    for k in range(int(round(dur * fps)) + 1):
        x = k / fps
        Q, P = {}, {}
        for (bone, path), (t, v, kind) in tracks.items():
            (Q if path == "rotation" else P)[bone] = eval_track(t, v, kind, x)
        r = kev_read(info, Q, P)
        hands = {kk: vv for kk, vv in r["joints"].items() if HAND_RE.match(kk)}
        samples.append({"t": round(x, 4), "landmarks": r["landmarks"], "hands": hands, "com": r["balance"]["com"]})
    bm = info["bm"]
    ref = {"name": name, "model": model, "clip": clip, "height": bm["body"]["height"], "floor": bm["body"]["floor"],
           "duration": samples[-1]["t"], "samples": samples}
    os.makedirs(REFDIR, exist_ok=True)
    with open(os.path.join(REFDIR, name + ".json"), "w") as f:
        json.dump(ref, f)
    return ref


def solve_ref(info, ref, tau):
    lm0, hd0 = sample_ref(ref, tau)
    lm = scale_to(info, ref, lm0)
    hd = scale_hands(info, ref, hd0)
    rp = rest_lm(info, "pelvis")
    for k in lm:
        lm[k][0] += rp[0]
        lm[k][2] += rp[2]
    return lm, hd


def bake(model, name, fps=60):
    """Solve a whole reference onto `model` and keep the numbers: per-frame local rotations (+ pelvis position)."""
    info = model_info(model)
    ref = load_ref(name)
    Q, P = {}, {}
    frames, errs, tips = [], [], []
    t0 = time.time()
    n = int(round(ref["duration"] * fps)) + 1
    for k in range(n):
        lm, hd = solve_ref(info, ref, k / fps)
        err = apply_frame_into(info, Q, P, lm, hd)
        if not hd:  # sheet figures hold fists; a clip reference brings its own finger tracks
            for side in "lr":
                if f"hand_{side}" in info["index"]:
                    curl_hand(info, Q, P, side, 1.0)
        frames.append({"q": {b: [round(float(x), 5) for x in q] for b, q in Q.items()}, "p": [round(float(x), 5) for x in P["pelvis"]]})
        m = kev_read(info, Q, P)
        errs.append(float(np.mean([np.linalg.norm(np.array(m["landmarks"][kk]) - np.array(lm[kk])) for kk in lm if kk in m["landmarks"]])))
        tips.append(max(err.get("fingers_l", 0), err.get("fingers_r", 0)))
    pops = quality.repair_baked(frames)
    out = {"model": model, "ref": name, "fps": fps, "duration": ref["duration"], "frames": frames,
           "mean_cm": round(100 * float(np.mean(errs)), 2), "max_mean_cm": round(100 * float(np.max(errs)), 2),
           "finger_cm": round(100 * float(np.max(tips)), 2), "solve_ms_per_frame": round((time.time() - t0) / n * 1000, 1),
           "quat_pops_fixed": pops}
    os.makedirs(os.path.join(REPO, "staging", "kev", "baked"), exist_ok=True)
    with open(os.path.join(REPO, "staging", "kev", "baked", f"{model[:-4]}__{name}.json"), "w") as f:
        json.dump(out, f)
    return out


def play_baked(model, name):
    with open(os.path.join(REPO, "staging", "kev", "baked", f"{model[:-4]}__{name}.json")) as f:
        b = json.load(f)
    stop_ref()
    send_cmd({"cmd": "baked", "fps": b["fps"], "frames": b["frames"], "loop": True})
    return b


def run_ref(name, rate, loop):
    info = model_info(STATE["model"])
    ref = load_ref(name)
    broadcast("cmd", {"cmd": "stopclip"})
    COMMANDED.clear()
    COMMANDED_POS.clear()
    t0 = time.time()
    n = 0
    while not REFPLAY["stop"]:
        tau = (time.time() - t0) * rate
        if tau > ref["duration"]:
            if not loop:
                break
            t0 = time.time()
            tau = 0.0
        lm0, hd0 = sample_ref(ref, tau)
        lm = scale_to(info, ref, lm0)
        hd = scale_hands(info, ref, hd0)
        # place the reference pelvis x,z at this model's rest pelvis (in place); keep heights
        for k in lm:
            lm[k][0] += rest_lm(info, "pelvis")[0]
            lm[k][2] += rest_lm(info, "pelvis")[2]
        err = apply_frame(info, lm, inplace=False, hands=hd)
        measured = kev_read(info, COMMANDED, COMMANDED_POS)
        track = [float(np.linalg.norm(np.array(measured["landmarks"][k]) - np.array(lm[k]))) for k in lm if k in measured["landmarks"]]
        REFPLAY["status"] = {"name": name, "tau": round(tau, 2), "duration": ref["duration"], "rate": rate, "frames": n,
                             "tracking_error_m": {"mean": round(float(np.mean(track)), 4), "max": round(float(np.max(track)), 4)},
                             "reach_error_m": {k: round(v, 4) for k, v in err.items()},
                             "worst": sorted(((k, round(v, 3)) for k, v in zip([k for k in lm if k in measured["landmarks"]], track)), key=lambda x: -x[1])[:4]}
        broadcast("cmd", pose_event())
        n += 1
        time.sleep(0.05)
    REFPLAY["status"] = {**REFPLAY["status"], "running": False}


def start_ref(name, rate=1.0, loop=True):
    stop_ref()
    REFPLAY["stop"] = False
    th = threading.Thread(target=run_ref, args=(name, rate, loop), daemon=True)
    REFPLAY["thread"] = th
    th.start()


def stop_ref():
    REFPLAY["stop"] = True
    th = REFPLAY["thread"]
    if th and th.is_alive():
        th.join(timeout=1.0)


def broadcast(event, data):
    msg = f"event: {event}\ndata: {json.dumps(data)}\n\n"
    with LOCK:
        for q in list(CLIENTS):
            try:
                q.put_nowait(msg)
            except queue.Full:
                pass


def send_cmd(cmd):
    broadcast("cmd", cmd)
    return {"sent": cmd}


def table(rows):
    return "\n".join(rows)


def execute(line):
    """One console line -> text. Writes go to the viewer as commands; reads answer from the latest streamed pose."""
    t = line.strip().split()
    if not t:
        return ""
    c, a = t[0].lower(), t[1:]
    info = model_info(STATE["model"]) if STATE["model"] else None
    s = STATE["summary"]
    if c == "help":
        return __doc__.split("Console commands")[1].split("Files are served")[0].strip()
    if c == "models":
        return table(sorted(f for f in os.listdir(ANIM) if f.endswith(".glb")))
    if c == "model" and a:
        return json.dumps(send_cmd({"cmd": "model", "file": a[0]}))
    if c == "clips":
        return table(f"{i:2d} {n}" for i, n in enumerate(info["clips"])) if info else "(no model loaded)"
    if c in ("play", "pause", "resume", "seek", "speed", "reset"):
        cmd = {"cmd": c}
        if c == "play" and a:
            name, err = resolve_clip(info, " ".join(a))
            if err:
                return err
            cmd["clip"] = name
        if c in ("seek", "speed") and a:
            cmd["value"] = float(a[0])
        return json.dumps(send_cmd(cmd))
    if c == "ref":
        sub = a[0] if a else "status"
        if sub == "capture":
            if not STATE.get("clip"):
                return "play a clip first (ref capture records one loop of it)"
            name = a[1] if len(a) > 1 else f"{STATE['model'].replace('.glb', '')}__{STATE['clip']}"
            CAPTURE.update({"on": True, "name": name, "samples": [], "last_t": -1.0, "model": None, "clip": None})
            send_cmd({"cmd": "play", "clip": STATE["clip"]})  # restart from t=0 so one full loop is recorded
            return f"capturing '{name}' while {STATE['clip']} plays one full loop..."
        if sub == "list":
            return table(sorted(f[:-5] for f in os.listdir(REFDIR) if f.endswith(".json"))) if os.path.isdir(REFDIR) else "(none)"
        if sub == "grab" and len(a) > 2:
            t0 = time.time()
            r = grab(a[1], a[2], a[3] if len(a) > 3 else a[2])
            return f"grabbed '{r['name']}' from {a[1]} / {a[2]}: {len(r['samples'])} frames @60 fps in {time.time() - t0:.1f} s"
        if sub in ("play", "bake") and len(a) > 1:
            if not info or not info["bm"]:
                return "no skinned model streamed"
            b = bake(STATE["model"], a[1])
            if sub == "play":
                play_baked(STATE["model"], a[1])
            return (f"baked '{a[1]}' -> {STATE['model']}: {len(b['frames'])} frames, {b['solve_ms_per_frame']} ms/frame solve, "
                    f"mean error {b['mean_cm']} cm, fingers {b['finger_cm']} cm" + ("; playing in the viewer at 60 fps" if sub == "play" else ""))
        if sub == "baked" and len(a) > 1:
            b = play_baked(STATE["model"], a[1])
            return f"playing baked '{a[1]}' ({len(b['frames'])} frames)"
        if sub == "live" and len(a) > 1:
            start_ref(a[1], float(a[2]) if len(a) > 2 else 1.0)
            return f"live controller (per-frame solve over SSE): '{a[1]}'"
        if sub == "stop":
            stop_ref()
            return "controller stopped"
        return json.dumps(REFPLAY["status"] or {"running": False}, indent=1)
    if c == "cat":
        ixp = os.path.join(HERE_TOOLS, "catalog", "index.json")
        cat = json.load(open(ixp))["animations"] if os.path.exists(ixp) else {}
        sub = a[0] if a else "list"
        if sub == "list":
            return table([f"{n:16} {e['source']['type']:5} {e['duration_s']:5.2f}s  jerk {e['metrics']['jerk']:>7}  reach {e['metrics']['wrist_reach_h']}  {','.join(e['tags'])}" for n, e in sorted(cat.items())]) or "(empty: tools/kev/catalog/catalog.py add-clip ...)"
        if sub == "info" and len(a) > 1 and a[1] in cat:
            return json.dumps(cat[a[1]], indent=1)
        if sub == "play" and len(a) > 1 and a[1] in cat:
            if not info or not info["bm"]:
                return "no skinned model streamed"
            if not os.path.exists(os.path.join(REFDIR, a[1] + ".json")):
                return "reference data missing: python3 -I tools/kev/catalog/catalog.py rebuild"
            b = bake(STATE["model"], a[1])
            play_baked(STATE["model"], a[1])
            return f"'{a[1]}' (smoothed) -> {STATE['model']}: {len(b['frames'])} frames, mean error {b['mean_cm']} cm"
        return "cat list | cat info <name> | cat play <name>"
    if c == "check" and a:
        path = os.path.join(REFDIR, a[0] + ".json")
        if not os.path.exists(path):
            return "reference missing: python3 -I tools/kev/catalog/catalog.py rebuild"
        ref = json.load(open(path))
        info = model_info(ref["model"])
        if not info.get("bm"):
            return "no body map for " + ref["model"]
        text = quality.check_text(ref, info["bm"]["body"], quality.lengths_from_body(info["bm"]))
        ixp = os.path.join(HERE_TOOLS, "catalog", "index.json")
        if os.path.exists(ixp):
            motion = json.load(open(ixp))["animations"].get(a[0], {}).get("motion")
            if motion:
                text = f"motion {motion.get('state')} (source rotation {motion.get('source_rot_deg')} deg)\n" + text
        return text
    if c == "phys":
        sub = a[0] if a else "status"
        if sub in ("on", "off", "toggle"):
            PHYS["on"] = sub == "on" or (sub == "toggle" and not PHYS["on"])
            PHYS["sim"] = None
            send_cmd({"cmd": "phys", "on": PHYS["on"]})
            return f"physics {'on' if PHYS['on'] else 'off'}"
        if sub == "reset":
            PHYS["sim"] = None
            return "physics reset"
        if PHYS["sim"] is None:
            return "physics on (no frame yet)" if PHYS["on"] else "physics off"
        return json.dumps({"on": PHYS["on"], **PHYS["sim"].status()})
    if c == "hand" and len(a) >= 2:
        if not info or not info["bm"]:
            return "no skinned model streamed"
        side = {"l": "l", "left": "l", "r": "r", "right": "r"}.get(a[0])
        if not side:
            return "hand <l|r> <open|fist|point|relaxed|0..1>"
        stop_ref()
        if not COMMANDED:
            send_cmd({"cmd": "stopclip"})
        amt = POSES.get(a[1], None) if a[1] in POSES else float(a[1])
        curl_hand(info, COMMANDED, COMMANDED_POS, side, 1.0 if amt is None else amt, point=a[1] == "point")
        send_cmd(pose_event())
        return f"hand_{side}: {a[1]}"
    if c in ("reach", "aim") and len(a) >= 4:
        if not info or not info["bm"]:
            return "no skinned model streamed"
        stop_ref()
        if not COMMANDED:
            send_cmd({"cmd": "stopclip"})
        vec = lambda i: np.array([float(x) for x in a[i:i + 3]])  # noqa: E731
        if c == "aim":
            aim(info, COMMANDED, COMMANDED_POS, a[0], vec(1))
            out = f"aimed {a[0]}"
        else:
            chain = a[0]
            if chain not in CHAINS:
                return "chain: " + ", ".join(CHAINS)
            e = reach(info, COMMANDED, COMMANDED_POS, chain, vec(1), vec(4) if len(a) >= 7 else None)
            out = f"reached {chain}: error {e * 100:.1f} cm"
        send_cmd(pose_event())
        return out
    if c == "set" and len(a) >= 4:
        cmd = {"cmd": "set", "bone": a[0], "euler": [float(x) for x in a[1:4]]}
        out = json.dumps(send_cmd(cmd))
        time.sleep(0.25)
        return out + "\n" + describe_bone(a[0])
    if not info:
        return "(no model streamed yet: open the page)"
    if c == "bones":
        flt = a[0].lower() if a else ""
        rows = []
        for r in info["kev"]["nodes"]:
            if flt in r[1].lower() or flt in str(r[3]).lower() or flt in str(r[4]).lower():
                rows.append(f"{r[0]:3d} {r[1]:<22} parent {r[2]!s:<4} role {r[3]:<9} region {r[4]}")
        return table(rows) or "no match"
    if c == "get" and a:
        return describe_bone(a[0])
    if c == "landmarks":
        lm = s.get("landmarks", {})
        if a:
            return f"{a[0]}: {lm.get(a[0])}  height {s.get('heights', {}).get(a[0])}"
        return table(f"{k:<16} {v}  h={s['heights'][k]}" for k, v in lm.items())
    if c == "facings":
        return table(f"{k:<16} toward {v['toward']:<8} ({v['off_deg']} deg off)  {v['vector']}" for k, v in s.get("facings", {}).items())
    if c == "balance":
        return json.dumps(s.get("balance", {}), indent=1)
    if c == "rig":
        b = info["bm"]["body"]
        k = info["kev"]
        return json.dumps({"model": info["name"], "joints": k["joints"], "height_m": b["height"], "units": k.get("units_guess"),
                           "limbs": k["limbs"], "regions": {r: v["span_h"] for r, v in k["regions"].items()},
                           "landmarks": len(info["bm"]["landmarks"]), "facings": len(info["bm"]["facings"])}, indent=1)
    if c == "state":
        return json.dumps({k: STATE[k] for k in ("model", "clip", "playing", "t", "duration", "speed")}, indent=1)
    return f"unknown command: {c} (try help)"


def describe_bone(name):
    info = model_info(STATE["model"])
    if name not in info["index"]:
        return f"no bone named {name}"
    i = info["index"][name]
    q = LATEST_QUATS.get(name, info["rest_q"][i].tolist())
    p = info["parent"][i]
    W = world_transforms(info, LATEST_QUATS, LATEST_POS)
    row = next((r for r in (info["kev"] or {}).get("nodes", []) if r[1] == name), None)
    return json.dumps({"bone": name, "parent": info["names"][p] if p >= 0 else None, "role": row[3] if row else None,
                       "region": row[4] if row else None, "local_quat_xyzw": [round(float(x), 4) for x in q],
                       "rest_quat_xyzw": [round(float(x), 4) for x in info["rest_q"][i]],
                       "world_position": [round(float(x), 4) for x in W[i][:3, 3]]})


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def reply(self, code, body, ctype="application/json"):
        data = body if isinstance(body, bytes) else (body if isinstance(body, str) else json.dumps(body)).encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def serve_file(self, root, rel):
        p = os.path.normpath(os.path.join(root, rel))
        if not p.startswith(root + os.sep) or not os.path.isfile(p):
            return self.reply(404, {"error": "not found"})
        ext = os.path.splitext(p)[1]
        with open(p, "rb") as f:
            self.reply(200, f.read(), MIME.get(ext, "application/octet-stream"))

    def do_GET(self):
        u = urllib.parse.urlparse(self.path)
        path = u.path
        if path == "/":
            return self.serve_file(os.path.join(HERE, "static"), "index.html")
        if path.startswith("/static/"):
            return self.serve_file(os.path.join(HERE, "static"), path[8:])
        if path.startswith("/assets/anim/") and path.endswith(".glb"):
            return self.serve_file(ANIM, path[13:])
        if path == "/api/models":
            return self.reply(200, sorted(f for f in os.listdir(ANIM) if f.endswith(".glb")))
        if path == "/api/clips":
            return self.reply(200, model_info(STATE["model"])["clips"] if STATE["model"] else [])
        if path == "/api/rig":
            q = urllib.parse.parse_qs(u.query)
            info = model_info(q.get("model", [STATE["model"]])[0])
            return self.reply(200, {"model": info["name"], "kev": info["kev"], "landmarks": info["bm"]["landmarks"] if info["bm"] else {},
                                    "facings": info["bm"]["facings"] if info["bm"] else {}})
        if path == "/api/state":
            return self.reply(200, {**{k: STATE[k] for k in ("model", "clip", "playing", "t", "duration", "speed", "stamp")}, **STATE["summary"],
                                    "quats": LATEST_QUATS})
        if path == "/api/events":
            q = queue.Queue(maxsize=200)
            with LOCK:
                CLIENTS.append(q)
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-store")
            self.send_header("Connection", "keep-alive")
            self.end_headers()
            try:
                self.wfile.write(b": hello\n\n")
                self.wfile.flush()
                while True:
                    try:
                        self.wfile.write(q.get(timeout=15).encode())
                    except queue.Empty:
                        self.wfile.write(b": ping\n\n")
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError, OSError):
                pass
            finally:
                with LOCK:
                    if q in CLIENTS:
                        CLIENTS.remove(q)
            return
        self.reply(404, {"error": "not found"})

    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        try:
            body = json.loads(self.rfile.read(n) or b"{}")
        except ValueError:
            return self.reply(400, {"error": "bad json"})
        path = urllib.parse.urlparse(self.path).path
        if path == "/api/telemetry":
            return self.telemetry(body)
        if path == "/api/cmd":
            return self.reply(200, send_cmd(body))
        if path == "/api/control":
            info = model_info(STATE["model"]) if STATE["model"] else None
            if not info or not info["bm"]:
                return self.reply(400, {"error": "no skinned model streamed"})
            stop_ref()
            if not COMMANDED:
                send_cmd({"cmd": "stopclip"})
            incoming = {k: v for k, v in body.get("landmarks", {}).items()}
            solved, phys = apply_physics(info, incoming)
            err = apply_frame(info, solved, inplace=body.get("inplace", False), hands=body.get("hands"))
            measured = kev_read(info, COMMANDED, COMMANDED_POS)
            ev = pose_event()
            if phys.get("on"):
                ev["ghost"] = {k: [round(float(x), 4) for x in v] for k, v in incoming.items() if isinstance(v, (list, tuple)) and len(v) == 3}
                ev["phys"] = {k: phys[k] for k in ("plants", "slip_mps", "penetration_mm", "com_off_m", "compression_m") if k in phys}
            send_cmd(ev)
            return self.reply(200, {"reach_error_m": {k: round(v, 4) for k, v in err.items()},
                                    "landmarks": measured["landmarks"], "balance": measured["balance"], "physics": phys})
        if path == "/api/physics":
            frames = body.get("frames", body.get("landmarks"))
            if isinstance(frames, dict):
                frames = [frames]
            if not isinstance(frames, list) or not frames:
                return self.reply(400, {"error": "frames or landmarks required"})
            return self.reply(200, physics.correct(frames, dt=body.get("dt", 1.0 / 30.0), floor=body.get("floor"),
                                                    rest=body.get("rest"), config=body.get("config"), events=body.get("events")))
        if path == "/api/exec":
            try:
                out = execute(body.get("line", ""))
            except Exception as e:  # noqa: BLE001
                out = f"error: {e}"
            broadcast("log", {"line": body.get("line", ""), "out": out, "from": body.get("from", "api")})
            return self.reply(200, {"out": out})
        self.reply(404, {"error": "not found"})

    def telemetry(self, body):
        global LATEST_QUATS, LATEST_POS
        model = body.get("model")
        if not model or not os.path.isfile(os.path.join(ANIM, model)):
            return self.reply(400, {"error": "unknown model"})
        info = model_info(model)
        quats = {k: v for k, v in body.get("quats", {}).items() if k in info["index"]}
        pos = {k: v for k, v in (body.get("pos") or {}).items() if k in info["index"]}
        LATEST_QUATS = quats
        LATEST_POS = pos
        STATE.update({"model": model, "clip": body.get("clip"), "playing": body.get("playing"), "t": body.get("t", 0.0),
                      "duration": body.get("duration", 0.0), "speed": body.get("speed", 1.0), "stamp": time.time()})
        STATE["summary"] = kev_read(info, quats, pos)
        capture_sample(model, body.get("clip"), body.get("t", 0.0), STATE["summary"])
        broadcast("state", {**{k: STATE[k] for k in ("model", "clip", "playing", "t", "duration", "speed")}, **STATE["summary"],
                            "ref": REFPLAY["status"], "capturing": CAPTURE["on"]})
        self.reply(200, {"ok": True})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8765)
    ap.add_argument("--host", default="127.0.0.1")
    a = ap.parse_args()
    srv = ThreadingHTTPServer((a.host, a.port), Handler)
    srv.daemon_threads = True
    print(f"Kev Studio on http://{a.host}:{a.port}  (assets: {ANIM})", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
