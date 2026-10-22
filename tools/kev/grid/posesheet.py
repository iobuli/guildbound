"""Pose sheet -> animation. A pose sheet holds joints read off a gridded image (grid.py) as fractional cell coordinates;
this lifts each pose to 3D landmarks for a rig, solves it with Kev Studio's controller, eases between the poses and bakes
the result as a track the Studio plays.

    python3 -I posesheet.py verify SHEET.json OUT.png        # draw the read joints on the grid image
    python3 -I posesheet.py bake   SHEET.json MODEL.glb NAME  # -> staging/kev/baked/MODEL__NAME.json

Sheet format (see sheets/*.json): image, box, cell, zoom, facing ("right"|"left"), floor_row, poses{name: joints},
sequence [[pose, time_s, ease]...]. Joints: crown nose neck shoulder hip (single points: the body's midline) and
elbow_l wrist_l elbow_r wrist_r knee_l ankle_l toe_l knee_r ankle_r toe_r. Side view: the image gives forward and
height, the left/right offsets come from a fixed body half-width (per-joint "lat" in the sheet overrides it).
"""
import importlib.util
import json
import math
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
STUDIO = os.path.join(HERE, "..", "studio")
sys.path.insert(0, os.path.join(HERE, ".."))
sys.path.insert(0, STUDIO)
sys.argv = sys.argv[:1] + sys.argv[1:]

LAT = {"shoulder": 0.115, "hip": 0.055, "elbow": 0.15, "wrist": 0.12, "knee": 0.07, "ankle": 0.065, "toe": 0.07}


def studio():
    spec = importlib.util.spec_from_file_location("kev_studio_server", os.path.join(STUDIO, "server.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def lift(sheet, pose, srv, info):
    """One pose's joints -> landmark targets (world metres) for this rig."""
    bm = info["bm"]
    up, fwd = np.array(bm["body"]["up"], float), np.array(bm["body"]["forward"], float)
    left = np.cross(up, fwd)
    H, floor = bm["body"]["height"], bm["body"]["floor"]
    d = 1.0 if sheet.get("facing", "right") == "right" else -1.0
    s = H / sheet.get("height_cells", sheet["floor_row"] - pose["crown"][1])  # fixed standing height when poses crouch or leave the ground
    hip = np.array(pose["hip"], float)
    pelvis = srv.rest_lm(info, "pelvis")
    base = pelvis - up * (pelvis @ up) + up * (floor + (sheet["floor_row"] - hip[1]) * s)

    def at(joint, lat=0.0, name=None):
        c, r = pose[joint]
        lat = pose.get("lat", {}).get(name or joint, lat)
        return base + fwd * ((c - hip[0]) * d * s) + up * ((hip[1] - r) * s) + left * (lat * H)

    lm = {"pelvis": at("hip"), "hip_l": at("hip", LAT["hip"]), "hip_r": at("hip", -LAT["hip"]),
          "neck": at("neck"), "shoulder_l": at("shoulder", LAT["shoulder"]), "shoulder_r": at("shoulder", -LAT["shoulder"]),
          "crown": at("crown"), "nose": at("nose")}
    lm["head_joint"] = lm["neck"] + (lm["crown"] - lm["neck"]) * 0.3
    for side, sg in (("l", 1), ("r", -1)):
        for j in ("elbow", "wrist"):
            lm[f"{j}_{side}"] = at(f"{j}_{side}", sg * LAT[j], j)
        lm[f"knee_{side}"] = at(f"knee_{side}", sg * LAT["knee"], "knee")
        lm[f"ankle_{side}"] = at(f"ankle_{side}", sg * LAT["ankle"], "ankle")
        toe = at(f"toe_{side}", sg * LAT["toe"], "toe")
        lm[f"ball_{side}"] = lm[f"ankle_{side}"] + (toe - lm[f"ankle_{side}"]) * 0.7
    return {k: [float(x) for x in v] for k, v in lm.items()}


def ease(kind, u):
    return {"smooth": u * u * (3 - 2 * u), "snap": 1 - (1 - u) ** 3, "out": 1 - (1 - u) ** 2, "linear": u}.get(kind, u)


def _torso_basis(srv, lm):
    """Hip line across, spine up. The matrix the solver will rebuild the pelvis from."""
    up = np.asarray(lm["neck"], float) - np.asarray(lm["pelvis"], float)
    across = np.asarray(lm["hip_l"], float) - np.asarray(lm["hip_r"], float)
    if np.linalg.norm(up) < 1e-6:
        up = np.array([0.0, 1.0, 0.0])
    return srv.basis(across, up)


def _flip_pair(lm_a, lm_b):
    """True when a straight blend of the spine would collapse or spin through more than a right angle."""
    ua = np.asarray(lm_a["neck"], float) - np.asarray(lm_a["pelvis"], float)
    ub = np.asarray(lm_b["neck"], float) - np.asarray(lm_b["pelvis"], float)
    na, nb = float(np.linalg.norm(ua)), float(np.linalg.norm(ub))
    if na < 1e-6 or nb < 1e-6:
        return True
    ang = math.degrees(math.acos(float(np.clip((ua / na) @ (ub / nb), -1.0, 1.0))))
    mn = min(float(np.linalg.norm((1 - u) * ua + u * ub)) for u in np.linspace(0.0, 1.0, 9))
    return ang > 100.0 or mn < 0.5 * min(na, nb)


def _retime(theta, max_deg):
    """Follow `theta` (radians-free degrees along the key path) with no step over `max_deg`.

    A fast stretch is started on earlier frames that have room, so the path still arrives on the
    last sample instead of clipping and catching up afterwards.
    """
    n = len(theta)
    out = np.empty(n)
    out[0] = theta[0]
    for i in range(1, n):
        step = float(theta[i] - out[i - 1])
        step = min(max(step, -max_deg), max_deg)
        out[i] = out[i - 1] + step
    lag = float(theta[-1] - out[-1])
    if abs(lag) < 1e-4:
        return out
    d = np.diff(out)
    remain = lag
    if lag > 0:
        for i in range(len(d)):
            room = max_deg - float(d[i])
            if room <= 0:
                continue
            take = min(room, remain)
            d[i] += take
            remain -= take
            if remain <= 1e-6:
                break
    else:
        for i in range(len(d)):
            room = float(d[i]) + max_deg
            if room <= 0:
                continue
            take = min(room, -remain)
            d[i] -= take
            remain += take
            if remain >= -1e-6:
                break
    c = float(theta[0])
    out[0] = c
    for i, s in enumerate(d):
        c += float(s)
        out[i + 1] = c
    return out


def _head_basis(srv, lm):
    """Same crown/nose frame the solver uses. A flat cross falls back to the chest, which is what a
    colinear nose reading would do, and that fallback is what flips the head 180 at the next key."""
    hj = np.asarray(lm["head_joint"], float)
    y = np.asarray(lm["crown"], float) - hj
    x = np.cross(y, np.asarray(lm["nose"], float) - hj)
    if np.linalg.norm(x) < 1e-6 or np.linalg.norm(y) < 1e-6:
        return _torso_basis(srv, lm)
    return srv.basis(x, y)


def _segment(theta, arcs):
    theta = min(max(float(theta), arcs[0]), arcs[-1])
    i = 0
    while i < len(arcs) - 2 and theta > arcs[i + 1]:
        i += 1
    span = arcs[i + 1] - arcs[i]
    u = 0.0 if span < 1e-6 else (theta - arcs[i]) / span
    return i, min(max(u, 0.0), 1.0)


def samples(sheet, lms, srv, fps=60, max_deg=16.0):
    """One landmark dict per frame. A flip is an orientation track, not a blend of positions.

    Linear blends of a backflip send the neck through the pelvis, so the spine direction whips on a
    single frame. Keys keep their bases; in between, the torso and the head slerp along that rotation
    and the angle is spread onto neighbouring frames so no step exceeds `max_deg`. Arms and legs stay
    on the straight blend: swinging them around the same arc is what made the flip's landmark jerk
    climb, and the spine does not read them. Clips that never flip stay on the straight blend.
    """
    seq = sheet["sequence"]
    total = seq[-1][1]
    n = int(round(total * fps)) + 1
    key_f, key_lm = [], []
    for item in seq:
        key_f.append(min(n - 1, int(round(item[1] * fps))))
        key_lm.append(lms[item[0]])
    key_f[0], key_f[-1] = 0, n - 1
    orient = any(_flip_pair(a, b) for a, b in zip(key_lm, key_lm[1:]))
    bases = heads = offs = hoffs = arcs = theta = None
    if orient:
        bases = [_torso_basis(srv, lm) for lm in key_lm]
        heads = [_head_basis(srv, lm) for lm in key_lm]
        offs, hoffs = [], []
        for lm, Rb, Rh in zip(key_lm, bases, heads):
            p = np.asarray(lm["pelvis"], float)
            hj = np.asarray(lm["head_joint"], float)
            offs.append({k: Rb.T @ (np.asarray(v, float) - p) for k, v in lm.items() if k not in ("pelvis", "crown", "nose")})
            hoffs.append({k: Rh.T @ (np.asarray(lm[k], float) - hj) for k in ("crown", "nose")})
        arcs = [0.0]
        for i in range(len(bases) - 1):
            turn = max(srv.quat_angle(bases[i], bases[i + 1]), srv.quat_angle(heads[i], heads[i + 1]))
            arcs.append(arcs[-1] + turn)
        theta = np.zeros(n)
        for i in range(len(seq) - 1):
            f0, f1 = key_f[i], key_f[i + 1]
            kind = seq[i + 1][2] if len(seq[i + 1]) > 2 else "smooth"
            for f in range(f0, f1 + 1):
                u = 0.0 if f1 == f0 else (f - f0) / (f1 - f0)
                u = ease(kind, u)
                theta[f] = arcs[i] * (1.0 - u) + arcs[i + 1] * u
        theta = _retime(theta, max_deg)
    out = []
    for k in range(n):
        t = k / fps
        i = max(j for j in range(len(seq)) if seq[j][1] <= t) if t < total else len(seq) - 2
        a, b = seq[i], seq[min(i + 1, len(seq) - 1)]
        u = 0.0 if b[1] == a[1] else min(1.0, (t - a[1]) / (b[1] - a[1]))
        u = ease(b[2] if len(b) > 2 else "smooth", u)
        la, lb = lms[a[0]], lms[b[0]]
        lm = {key: [x + (y - x) * u for x, y in zip(la[key], lb[key])] for key in la}
        if orient:
            i, u = _segment(theta[k], arcs)
            Rb = srv.slerp_mat(bases[i], bases[i + 1], u)
            Rh = srv.slerp_mat(heads[i], heads[i + 1], u)
            p = np.asarray(lm["pelvis"], float)
            for key in ("hip_l", "hip_r", "neck", "shoulder_l", "shoulder_r", "head_joint"):
                local = (1.0 - u) * offs[i][key] + u * offs[i + 1][key]
                lm[key] = [float(x) for x in (p + Rb @ local)]
            hj = np.asarray(lm["head_joint"], float)
            for key in ("crown", "nose"):
                local = (1.0 - u) * hoffs[i][key] + u * hoffs[i + 1][key]
                lm[key] = [float(x) for x in (hj + Rh @ local)]
        out.append(lm)
    return out


_ORIENT = ("pelvis", "hip_l", "hip_r", "neck", "shoulder_l", "shoulder_r", "head_joint", "crown", "nose")


def _basis_steps(L, ix, srv):
    """Per-frame torso and head basis, and the larger of the two steps into each frame (0 for frame 0)."""
    bases, heads, step = [], [], [0.0]
    for i in range(len(L)):
        lm = {n: L[i, ix[n]] for n in _ORIENT}
        bases.append(_torso_basis(srv, lm))
        heads.append(_head_basis(srv, lm))
        if i:
            step.append(max(srv.quat_angle(bases[-2], bases[-1]), srv.quat_angle(heads[-2], heads[-1])))
    return bases, heads, step


def cap_orientation(raw, smoothed, names, srv, max_deg=18.0):
    """Put a smoothed flip's head back on a limited turn.

    The position filter can kink a head basis the track had already spread. Only a clip whose raw
    steps already fit `max_deg` is repaired, so a strike that was fast before smoothing is left alone.
    The torso is rebuilt only when its own step exceeds the limit; otherwise just the crown and the nose.
    """
    ix = {n: i for i, n in enumerate(names)}
    if any(n not in ix for n in _ORIENT):
        return smoothed
    raw_b, raw_h, _ = _basis_steps(raw, ix, srv)
    bases, heads, _ = _basis_steps(smoothed, ix, srv)

    def steps(seq):
        return [0.0] + [srv.quat_angle(a, b) for a, b in zip(seq, seq[1:])]

    raw_over = max(max(steps(raw_b)), max(steps(raw_h)))
    if raw_over > max_deg + 0.5:
        return smoothed
    out = smoothed.copy()
    torso = ("hip_l", "hip_r", "neck", "shoulder_l", "shoulder_r", "head_joint")

    def apply(seq, keys, origin):
        st = steps(seq)
        if max(st) <= max_deg + 0.05:
            return
        arcs = np.cumsum(st)
        theta = _retime(np.array(arcs, float), max_deg)
        for k in range(len(out)):
            i, u = _segment(float(theta[k]), arcs)
            R = srv.slerp_mat(seq[i], seq[min(i + 1, len(seq) - 1)], u)
            p = out[k, ix[origin]]
            for key in keys:
                local = seq[k].T @ (smoothed[k, ix[key]] - smoothed[k, ix[origin]])
                out[k, ix[key]] = p + R @ local

    apply(bases, torso, "pelvis")
    apply(heads, ("crown", "nose"), "head_joint")
    return out


def bake(sheet, model, name, fps=60):
    srv = studio()
    info = srv.model_info(model)
    lms = {n: lift(sheet, p, srv, info) for n, p in sheet["poses"].items()}
    seq = sheet["sequence"]
    Q, P, frames, errs = {}, {}, [], []
    total = seq[-1][1]
    for lm in samples(sheet, lms, srv, fps):
        srv.apply_frame_into(info, Q, P, lm, None)
        for side in "lr":
            srv.curl_hand(info, Q, P, side, sheet.get("hands", 1.0))  # the sheet's figures hold fists
        frames.append({"q": {bn: [round(float(x), 5) for x in q] for bn, q in Q.items()}, "p": [round(float(x), 5) for x in P["pelvis"]]})
        m = srv.kev_read(info, Q, P)
        errs.append(float(np.mean([np.linalg.norm(np.array(m["landmarks"][key]) - np.array(lm[key])) for key in lm if key in m["landmarks"]])))
    out = {"model": model, "ref": name, "fps": fps, "duration": total, "frames": frames, "mean_cm": round(100 * float(np.mean(errs)), 2)}
    d = os.path.join(srv.REPO, "staging", "kev", "baked")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, f"{model[:-4]}__{name}.json"), "w") as f:
        json.dump(out, f)
    return out


def verify(sheet, out, root):
    sys.path.insert(0, HERE)
    import grid
    from PIL import ImageDraw
    im = grid.overlay(os.path.join(root, sheet["image"]), sheet["cell"], sheet.get("box"), sheet["zoom"])
    d = ImageDraw.Draw(im)
    z = sheet["cell"] * sheet["zoom"]
    bones = [("crown", "neck"), ("neck", "shoulder"), ("shoulder", "hip"), ("shoulder", "elbow_l"), ("elbow_l", "wrist_l"),
             ("shoulder", "elbow_r"), ("elbow_r", "wrist_r"), ("hip", "knee_l"), ("knee_l", "ankle_l"), ("ankle_l", "toe_l"),
             ("hip", "knee_r"), ("knee_r", "ankle_r"), ("ankle_r", "toe_r")]
    for p in sheet["poses"].values():
        for a, b in bones:
            d.line([(p[a][0] * z, p[a][1] * z), (p[b][0] * z, p[b][1] * z)], fill=(255, 0, 255) if a.endswith("_r") or b.endswith("_r") or b == "knee_r" else (0, 200, 0), width=3)
        for k, v in p.items():
            if isinstance(v, list):
                d.ellipse([v[0] * z - 4, v[1] * z - 4, v[0] * z + 4, v[1] * z + 4], fill=(255, 200, 0))
    im.save(out)
    print("wrote", out)


if __name__ == "__main__":
    cmd, sheet_path = sys.argv[1], sys.argv[2]
    sheet = json.load(open(sheet_path))
    if cmd == "verify":
        verify(sheet, sys.argv[3], os.environ.get("GRID_IMAGES", "."))
    elif cmd == "bake":
        r = bake(sheet, sys.argv[3], sys.argv[4])
        print({k: r[k] for k in ("model", "ref", "duration", "mean_cm")}, len(r["frames"]), "frames")
