"""Limb, hand, and foot checks, plus a cleanup that makes the same checks go quiet.

Landmarks are a (T, N, 3) array in metres. `hands` is a per-frame dict of joint world positions
(hand, thumb_01_l, ...). One solved frame uses the same code with T = 1. Quaternion tracks are
separate: hemisphere continuity and single-frame pops.

Cleanup is idempotent on the cases the checks know about: segment lengths, hinge limits, wrist
and ankle ranges, floor contact, planted-foot slide, toe-off bend, finger back-bend, palm facing,
hand-in-torso, and position or quaternion pops.
"""
import math

import numpy as np

SEGMENTS = (("shoulder_l", "elbow_l"), ("elbow_l", "wrist_l"), ("wrist_l", "knuckle_l"),
            ("shoulder_r", "elbow_r"), ("elbow_r", "wrist_r"), ("wrist_r", "knuckle_r"),
            ("hip_l", "knee_l"), ("knee_l", "ankle_l"), ("ankle_l", "ball_l"), ("ball_l", "toe_tip_l"),
            ("hip_r", "knee_r"), ("knee_r", "ankle_r"), ("ankle_r", "ball_r"), ("ball_r", "toe_tip_r"))
HINGES = (("hip_l", "knee_l", "ankle_l", "knee"), ("hip_r", "knee_r", "ankle_r", "knee"),
          ("shoulder_l", "elbow_l", "wrist_l", "elbow"), ("shoulder_r", "elbow_r", "wrist_r", "elbow"))
ANKLES = (("knee_l", "ankle_l", "ball_l"), ("knee_r", "ankle_r", "ball_r"))
WRISTS = (("elbow_l", "wrist_l", "knuckle_l"), ("elbow_r", "wrist_r", "knuckle_r"))
TOES = (("ankle_l", "ball_l", "toe_tip_l"), ("ankle_r", "ball_r", "toe_tip_r"))
FEET = ("ankle_l", "ankle_r", "ball_l", "ball_r", "toe_tip_l", "toe_tip_r", "heel_l", "heel_r")
FINGERS = ("thumb", "index", "middle", "ring", "pinky")
# Degrees of curl toward the palm at each joint, stacked along the finger (see curl_hand in the Studio).
SHAPES = {"open": (0.0, 0.0, 0.0), "blade": (8.0, 4.0, 0.0), "relaxed": (22.0, 30.0, 16.0), "fist": (48.0, 85.0, 70.0)}

LENGTH_TOL = 0.04
KNEE_MAX = 150.0
ELBOW_MAX = 155.0
WRIST_MAX = 85.0
ANKLE_MIN, ANKLE_MAX = 40.0, 135.0
TOE_MAX = 70.0
TOE_BACK = 12.0
PLANT_H = 0.05
SLIDE_MIN = 0.04
SLIDE_MAX = 0.7
FLOOR_EPS = 0.004
STILL_M = 0.002
POP_M = 0.06


def unit(v):
    n = float(np.linalg.norm(v))
    return v / n if n > 1e-9 else v


def rodrigues(axis, ang):
    a = unit(axis)
    x, y, z = a
    c, s = math.cos(ang), math.sin(ang)
    C = 1 - c
    return np.array([[c + x * x * C, x * y * C - z * s, x * z * C + y * s],
                     [y * x * C + z * s, c + y * y * C, y * z * C - x * s],
                     [z * x * C - y * s, z * y * C + x * s, c + z * z * C]])


def frame_ranges(mask):
    mask = np.asarray(mask, bool)
    if len(mask) == 0 or not mask.any():
        return []
    d = np.diff(mask.astype(np.int8))
    starts = list(np.where(d == 1)[0] + 1)
    ends = list(np.where(d == -1)[0])
    if mask[0]:
        starts = [0] + starts
    if mask[-1]:
        ends.append(len(mask) - 1)
    return [[int(a), int(b)] for a, b in zip(starts, ends)]


def _issue(kind, where, mask, peak, unit_name):
    frames = frame_ranges(mask)
    if not frames:
        return None
    return {"kind": kind, "where": where, "frames": frames, "peak": round(float(peak), 4), "unit": unit_name}


def counts(issues):
    out = {}
    for iss in issues:
        n = sum(b - a + 1 for a, b in iss["frames"])
        out[iss["kind"]] = out.get(iss["kind"], 0) + n
    return out


def motion_state(L, rot_deg, still_m=STILL_M):
    """'ok' when the sample moves, 'static' when the source is still, 'frozen' when the source
    moves and the sample does not (the matrix-track failure)."""
    travel = float(np.max(L.max(axis=0) - L.min(axis=0))) if len(L) else 0.0
    if travel >= still_m:
        return "ok"
    return "frozen" if rot_deg >= 2.0 else "static"


def lengths_from_body(bm):
    lm = bm["landmarks"]
    out = {}
    for a, b in SEGMENTS:
        if a in lm and b in lm and "rest" in lm[a] and "rest" in lm[b]:
            out[(a, b)] = float(np.linalg.norm(np.array(lm[b]["rest"]) - np.array(lm[a]["rest"])))
    return out


def _axes(body):
    up = np.array(body.get("up", [0, 1, 0]), float)
    up = unit(up)
    forward = np.array(body.get("forward", [0, 0, 1]), float)
    forward = unit(forward - up * float(forward @ up))
    left = np.array(body.get("left", np.cross(up, forward)), float)
    left = unit(left)
    return up, forward, left


def _height_of(P, up, floor):
    return P @ up - floor


def palm_normal(side, hand, middle, index, pinky):
    """Unit palm normal (out of the palm). Side flips the cross so both hands match the body map at rest."""
    across = (index - pinky) if side == "l" else (pinky - index)
    z = np.cross(middle - hand, across)
    n = float(np.linalg.norm(z))
    return z / n if n > 1e-8 else None


def palm_delta(forearm, nrm, up, inward, forward):
    """Radians about `forearm` that turns `nrm` toward a down / inward palm. 0 when the palm is already sensible."""
    axis = unit(forearm)
    n = nrm - axis * float(nrm @ axis)
    if np.linalg.norm(n) < 1e-6:
        return 0.0
    n = unit(n)
    down = -up - axis * float((-up) @ axis)
    if np.linalg.norm(down) > 1e-6 and float(n @ unit(down)) > math.cos(math.radians(40)):
        return 0.0
    desired = -up * 0.75 + inward * 0.35 + forward * 0.15
    d = desired - axis * float(desired @ axis)
    if np.linalg.norm(d) < 1e-6:
        return 0.0
    d = unit(d)
    if float(n @ d) > math.cos(math.radians(35)):
        return 0.0
    ang = math.atan2(float(np.cross(n, d) @ axis), float(n @ d))
    return float(np.clip(ang, math.radians(-100), math.radians(100)))


def _bone_angle(pa, pb, pc):
    u = pb - pa
    v = pc - pb
    nu, nv = np.linalg.norm(u), np.linalg.norm(v)
    if nu < 1e-8 or nv < 1e-8:
        return 0.0
    c = float(np.clip((u / nu) @ (v / nv), -1, 1))
    return math.degrees(math.acos(c))


def _pole(pa, pb, pc):
    axis = pc - pa
    n = float(np.linalg.norm(axis))
    if n < 1e-8:
        return pb - pa
    axis = axis / n
    return (pb - pa) - axis * float((pb - pa) @ axis)


def hinge_bad(pa, pb, pc, kind, forward):
    """True when a knee points backward or an elbow has popped forward, past a few degrees."""
    th = _bone_angle(pa, pb, pc)
    if th < 4.0:
        return False
    pole = _pole(pa, pb, pc)
    if np.linalg.norm(pole) < 1e-6:
        return False
    s = float(unit(pole) @ forward)
    if kind == "knee":
        return s < -0.25
    return s > 0.35


def _reflect(pa, pb, pc):
    axis = pc - pa
    d2 = float(axis @ axis)
    if d2 < 1e-12:
        return pb
    on = pa + axis * (float((pb - pa) @ axis) / d2)
    return on - (pb - on)


def _place_joint(pa, pc, l1, l2, pole_hint):
    """Knee or elbow position that keeps both bone lengths. The child stays put unless it is out of reach."""
    axis = pc - pa
    d = float(np.linalg.norm(axis))
    reach = l1 + l2
    gap = abs(l1 - l2)
    child = pc
    if d > reach - 1e-5:
        d = reach - 1e-4
        child = pa + unit(axis) * d
    elif d < gap + 1e-5:
        d = gap + 1e-4
        child = pa + unit(axis if np.linalg.norm(axis) > 1e-8 else pole_hint) * d
    axis_n = (child - pa) / d
    cos_a = float(np.clip((l1 * l1 + d * d - l2 * l2) / (2 * l1 * d), -1, 1))
    sin_a = math.sqrt(max(0.0, 1 - cos_a * cos_a))
    pole = pole_hint - axis_n * float(pole_hint @ axis_n)
    if np.linalg.norm(pole) < 1e-8:
        pole = np.cross(axis_n, [0, 1, 0] if abs(axis_n[1]) < 0.9 else [1, 0, 0])
    pole = unit(pole)
    joint = pa + axis_n * (l1 * cos_a) + pole * (l1 * sin_a)
    return joint, child


def _shift_side(hpts, side, delta):
    suf = "_" + side
    for n in list(hpts):
        if n.endswith(suf):
            hpts[n] = hpts[n] + delta


def _follow_wrist(pts, hpts):
    for side in "lr":
        w, h = f"wrist_{side}", f"hand_{side}"
        if w in pts and h in hpts:
            _shift_side(hpts, side, pts[w] - hpts[h])


def restore_lengths(pts, hpts, lengths, tol=LENGTH_TOL, only=None):
    for a, b in SEGMENTS:
        key = (a, b)
        if only is not None and key not in only:
            continue
        if a not in pts or b not in pts or key not in lengths:
            continue
        target = lengths[key]
        if target <= 1e-6:
            continue
        d = pts[b] - pts[a]
        n = float(np.linalg.norm(d))
        if n < 1e-8 or abs(n - target) / target <= tol:
            continue
        new_b = pts[a] + d / n * target
        delta = new_b - pts[b]
        pts[b] = new_b
        if b.startswith("wrist_"):
            _shift_side(hpts, b[-1], delta)
        if b.startswith("ankle_"):
            side = b[-1]
            for n in (f"ball_{side}", f"toe_tip_{side}", f"heel_{side}", f"instep_{side}"):
                if n in pts:
                    pts[n] = pts[n] + delta
        if b.startswith("ball_"):
            toe = f"toe_tip_{b[-1]}"
            if toe in pts:
                pts[toe] = pts[toe] + delta
    return pts


def clamp_hinges(pts, hpts, forward, up, lengths):
    for a, b, c, kind in HINGES:
        if a not in pts or b not in pts or c not in pts:
            continue
        pa, pb, pc = pts[a], pts[b], pts[c]
        if hinge_bad(pa, pb, pc, kind, forward):
            pts[b] = _reflect(pa, pb, pc)
            pa, pb, pc = pts[a], pts[b], pts[c]
        th = _bone_angle(pa, pb, pc)
        limit = KNEE_MAX if kind == "knee" else ELBOW_MAX
        if th <= limit:
            continue
        l1 = lengths.get((a, b), float(np.linalg.norm(pb - pa)))
        l2 = lengths.get((b, c), float(np.linalg.norm(pc - pb)))
        # Open the joint: the cosine law distance for `limit` degrees between the bones.
        cos_t = math.cos(math.radians(limit))
        d_need = math.sqrt(max(l1 * l1 + l2 * l2 + 2 * l1 * l2 * cos_t, 0.0))
        axis = pc - pa
        d = float(np.linalg.norm(axis))
        if d >= d_need - 1e-5:
            continue
        new_c = pa + unit(axis) * d_need
        delta = new_c - pc
        pts[c] = new_c
        if c.startswith("wrist_"):
            _shift_side(hpts, c[-1], delta)
        if c.startswith("ankle_"):
            side = c[-1]
            for n in (f"ball_{side}", f"toe_tip_{side}", f"heel_{side}", f"instep_{side}"):
                if n in pts:
                    pts[n] = pts[n] + delta
        pole = _pole(pa, pts[b], pts[c])
        if kind == "knee" and float(unit(pole if np.linalg.norm(pole) > 1e-6 else forward) @ forward) < 0:
            pole = forward
        if kind == "elbow" and float(unit(pole if np.linalg.norm(pole) > 1e-6 else -forward) @ forward) > 0:
            pole = -forward
        if np.linalg.norm(pole) < 1e-6:
            pole = up if kind == "knee" else -forward
        pts[b], pts[c] = _place_joint(pa, pts[c], l1, l2, pole)
    return pts


def _clamp_end(pts, a, b, c, lo, hi):
    if a not in pts or b not in pts or c not in pts:
        return
    pa, pb, pc = pts[a], pts[b], pts[c]
    th = _bone_angle(pa, pb, pc)
    if lo <= th <= hi:
        return
    target = min(hi, max(lo, th))
    u = pb - pa
    v = pc - pb
    if np.linalg.norm(u) < 1e-8 or np.linalg.norm(v) < 1e-8:
        return
    u = unit(u)
    axis = np.cross(u, v)
    if np.linalg.norm(axis) < 1e-8:
        axis = np.cross(u, [0, 1, 0] if abs(u[1]) < 0.9 else [1, 0, 0])
    R = rodrigues(axis, math.radians(target - th))
    pts[c] = pb + R @ v


def clamp_distal(pts):
    for a, b, c in WRISTS:
        _clamp_end(pts, a, b, c, 0.0, WRIST_MAX)
    for a, b, c in ANKLES:
        before = pts[c].copy() if c in pts else None
        _clamp_end(pts, a, b, c, ANKLE_MIN, ANKLE_MAX)
        if before is not None and c in pts:
            delta = pts[c] - before
            side = c[-1]
            for n in (f"toe_tip_{side}", f"heel_{side}", f"instep_{side}"):
                if n in pts:
                    pts[n] = pts[n] + delta
    for a, b, c in TOES:
        _clamp_end(pts, a, b, c, 0.0, TOE_MAX)
    return pts


def lift_floor(pts, hpts, up, floor):
    for n in FEET:
        if n not in pts:
            continue
        h = float(pts[n] @ up - floor)
        if h < -FLOOR_EPS:
            pts[n] = pts[n] + up * (-h)
    return pts


def lift_foot_rigid(pts, up, floor):
    """Raise a whole foot by its deepest penetration so the bones keep their length."""
    for side in "lr":
        names = [n for n in (f"ankle_{side}", f"ball_{side}", f"heel_{side}", f"toe_tip_{side}", f"instep_{side}") if n in pts]
        if not names:
            continue
        pen = min(float(pts[n] @ up - floor) for n in names)
        if pen < -FLOOR_EPS:
            delta = up * (-pen)
            for n in names:
                pts[n] = pts[n] + delta
    return pts


# Arms and the foot itself. The shin is owned by settle_legs, which keeps the planted ankle.
_DISTAL = {seg for seg in SEGMENTS if seg[0].split("_")[0] in ("shoulder", "elbow", "wrist", "ankle", "ball")}


def _snap_lengths(L, ix, lengths, tol=0.0, only=None):
    for t in range(len(L)):
        pts = {n: L[t, i].copy() for n, i in ix.items()}
        restore_lengths(pts, {}, lengths, tol=tol, only=only)
        for n, i in ix.items():
            L[t, i] = pts[n]


def _finger_chains(hpts):
    chains = []
    for side in "lr":
        for f in FINGERS:
            ch = []
            i = 1
            while f"{f}_{i:02d}_{side}" in hpts:
                ch.append(f"{f}_{i:02d}_{side}")
                i += 1
            if len(ch) >= 2:
                chains.append((side, f, ch))
    return chains


def _signed_flex(u, v, palm_n):
    axis = np.cross(u, palm_n)
    n = float(np.linalg.norm(axis))
    if n < 1e-8:
        return 0.0
    axis = axis / n
    return math.degrees(math.atan2(float(np.cross(u, v) @ axis), float(np.clip(u @ v, -1, 1))))


def clamp_fingers(hpts, pts):
    """Straighten finger joints that bend away from the palm. Lengths stay."""
    for side, _f, ch in _finger_chains(hpts):
        hand, mid, idx, pink = f"hand_{side}", f"middle_01_{side}", f"index_01_{side}", f"pinky_01_{side}"
        if not all(n in hpts for n in (hand, mid, idx, pink)):
            continue
        nrm = palm_normal(side, hpts[hand], hpts[mid], hpts[idx], hpts[pink])
        if nrm is None:
            continue
        for i in range(len(ch) - 1):
            parent = hpts[hand] if i == 0 else hpts[ch[i - 1]]
            u = hpts[ch[i]] - parent
            v = hpts[ch[i + 1]] - hpts[ch[i]]
            lu, lv = float(np.linalg.norm(u)), float(np.linalg.norm(v))
            if lu < 1e-8 or lv < 1e-8:
                continue
            flex = _signed_flex(u / lu, v / lv, nrm)
            if flex >= -TOE_BACK:
                continue
            axis = np.cross(u, nrm)
            if np.linalg.norm(axis) < 1e-8:
                continue
            R = rodrigues(axis, math.radians(-flex))
            pivot = hpts[ch[i]]
            for k in range(i + 1, len(ch)):
                hpts[ch[k]] = pivot + R @ (hpts[ch[k]] - pivot)
    for side in "lr":
        m, k = f"middle_01_{side}", f"knuckle_{side}"
        if m in hpts and k in pts:
            pts[k] = hpts[m].copy()
    return hpts


def push_hands(pts, hpts, up, forward, height):
    """Push a wrist or knuckle that sits inside the torso capsule or the head sphere back out."""
    if "pelvis" not in pts or "neck" not in pts:
        return pts
    radius = 0.09 * height
    deep = 0.62 * radius
    for side in "lr":
        for n in (f"wrist_{side}", f"knuckle_{side}"):
            if n not in pts:
                continue
            p = pts[n]
            ab = pts["neck"] - pts["pelvis"]
            d2 = float(ab @ ab)
            t = 0.0 if d2 < 1e-12 else float(np.clip(((p - pts["pelvis"]) @ ab) / d2, 0, 1))
            closest = pts["pelvis"] + ab * t
            delta = p - closest
            dist = float(np.linalg.norm(delta))
            # In front of the chest is a guard, not an intersection.
            if dist < deep and float((p - closest) @ forward) < 0.02 * height:
                direction = delta if dist > 1e-6 else forward
                new_p = closest + unit(direction) * radius
                diff = new_p - p
                pts[n] = new_p
                if n.startswith("wrist_"):
                    _shift_side(hpts, side, diff)
    if "head_joint" in pts and "crown" in pts:
        centre = 0.5 * (pts["head_joint"] + pts["crown"])
        hr = 0.075 * height
        for side in "lr":
            for n in (f"wrist_{side}", f"knuckle_{side}"):
                if n not in pts:
                    continue
                d = pts[n] - centre
                dist = float(np.linalg.norm(d))
                if dist < hr * 0.7:
                    new_p = centre + unit(d if dist > 1e-6 else forward) * hr
                    diff = new_p - pts[n]
                    pts[n] = new_p
                    if n.startswith("wrist_"):
                        _shift_side(hpts, side, diff)
    return pts


def clean_frame_pts(pts, hpts, body, lengths):
    up, forward, _left = _axes(body)
    floor, height = body["floor"], body["height"]
    pts = {k: np.array(v, float).copy() for k, v in pts.items()}
    hpts = {k: np.array(v, float).copy() for k, v in (hpts or {}).items()}
    for _ in range(2):
        restore_lengths(pts, hpts, lengths)
        clamp_hinges(pts, hpts, forward, up, lengths)
        clamp_distal(pts)
        lift_floor(pts, hpts, up, floor)
        _follow_wrist(pts, hpts)
    clamp_fingers(hpts, pts)
    push_hands(pts, hpts, up, forward, height)
    restore_lengths(pts, hpts, lengths)
    lift_floor(pts, hpts, up, floor)
    return pts, hpts


def clean_targets(lm, hands, body, lengths):
    """One solved frame. Temporal locks (plants, blends, pops) need the whole clip; see `clean`."""
    pts, hpts = clean_frame_pts(lm, hands or {}, body, lengths)
    out_h = {k: [round(float(x), 5) for x in v] for k, v in hpts.items()} if hands else hands
    return {k: [round(float(x), 5) for x in v] for k, v in pts.items()}, out_h


def _hands_arrays(hands, T):
    names = []
    if not hands:
        return {}, []
    keys = set()
    for h in hands:
        keys.update(h or {})
    arr = {}
    for k in sorted(keys):
        A = np.full((T, 3), np.nan)
        for t, h in enumerate(hands):
            if h and k in h:
                A[t] = h[k]
        if np.isfinite(A).all():
            arr[k] = A
            names.append(k)
    return arr, names


def _pin_deltas(P, mask, up, ramp=4):
    """Horizontal correction that pins masked runs. The same delta applied to every joint of a foot keeps bone lengths."""
    delta = np.zeros_like(P)
    T = len(P)
    t = 0
    while t < T:
        if not mask[t]:
            t += 1
            continue
        e = t
        while e + 1 < T and mask[e + 1]:
            e += 1
        if e - t >= 3:
            horiz = [P[k] - up * float(P[k] @ up) for k in range(t, e + 1)]
            mean = np.mean(horiz, axis=0)
            for k in range(max(t - ramp, 0), min(e + ramp, T - 1) + 1):
                w = 1.0 if t <= k <= e else 1.0 - (t - k if k < t else k - e) / (ramp + 1)
                target = mean + up * float(P[k] @ up)
                delta[k] += w * (target - P[k])
        t = e + 1
    return delta


def _plant_mask(P, up, floor, height):
    return _height_of(P, up, floor) < PLANT_H * height


def _slide_speed(P, up, fps):
    vel = np.gradient(P, axis=0) * fps
    horiz = vel - up * (vel @ up)[:, None]
    return np.linalg.norm(horiz, axis=1)


def lock_feet(L, ix, up, floor, height, fps):
    """Pin a planted foot as a rigid body: heel-strike locks the ankle, toe-off the ball, flat the ankle.
    One horizontal delta is added to the whole foot so the bones do not change length."""
    locked = 0
    for side in "lr":
        ankle, ball = f"ankle_{side}", f"ball_{side}"
        foot = [j for j in (ankle, ball, f"heel_{side}", f"toe_tip_{side}") if j in ix]
        if not foot:
            continue
        T = len(L)
        thr = PLANT_H * height
        ankle_low = _height_of(L[:, ix[ankle]], up, floor) < thr if ankle in ix else np.zeros(T, bool)
        ball_low = _height_of(L[:, ix[ball]], up, floor) < thr if ball in ix else np.zeros(T, bool)
        phases = []
        if ankle in ix:
            phases.append((ankle, ankle_low & ball_low))          # flat
            phases.append((ankle, ankle_low & ~ball_low))         # heel-strike
        if ball in ix:
            phases.append((ball, ball_low & ~ankle_low))          # toe-off
        for name, low in phases:
            if not np.any(low):
                continue
            speed = _slide_speed(L[:, ix[name]], up, fps)
            mask = low & (speed < SLIDE_MAX)
            delta = _pin_deltas(L[:, ix[name]], mask, up)
            if not np.any(np.linalg.norm(delta, axis=1) > 1e-5):
                continue
            for j in foot:
                L[:, ix[j]] = L[:, ix[j]] + delta
            locked += int(np.sum(np.linalg.norm(delta, axis=1) > 1e-5))
    return locked


def settle_legs(L, ix, lengths, forward):
    """Put the knee back on the legal side at the rest bone lengths, keeping the ankle where the plant left it."""
    for t in range(len(L)):
        for side in "lr":
            hip, knee, ankle = f"hip_{side}", f"knee_{side}", f"ankle_{side}"
            if hip not in ix or knee not in ix or ankle not in ix:
                continue
            l1, l2 = lengths.get((hip, knee)), lengths.get((knee, ankle))
            if not l1 or not l2:
                continue
            pa, pb, pc = L[t, ix[hip]], L[t, ix[knee]], L[t, ix[ankle]]
            pole = _pole(pa, pb, pc)
            if np.linalg.norm(pole) < 1e-6 or float(unit(pole) @ forward) < 0:
                pole = forward
            # Keep a planted ankle where it is. If the hip cannot reach it at rest length,
            # slide the hip along the leg instead of dragging the foot off its lock.
            axis = pc - pa
            dist = float(np.linalg.norm(axis))
            reach = l1 + l2
            gap = abs(l1 - l2)
            if dist > 1e-6:
                target = min(max(dist, gap + 1e-4), reach - 1e-4)
                if abs(target - dist) > 1e-5:
                    pa = pc - axis * (target / dist)
                    L[t, ix[hip]] = pa
            joint, _child = _place_joint(pa, pc, l1, l2, pole)
            L[t, ix[knee]] = joint


def bend_toes(L, ix, up, floor, height):
    """Heel-strike keeps the toe up; flat puts it on the floor; toe-off bends it at the ball."""
    for side in "lr":
        a, b, c = f"ankle_{side}", f"ball_{side}", f"toe_tip_{side}"
        if a not in ix or b not in ix or c not in ix:
            continue
        ankle, ball, toe = L[:, ix[a]], L[:, ix[b]], L[:, ix[c]]
        ha = _height_of(ankle, up, floor)
        hb = _height_of(ball, up, floor)
        thr = PLANT_H * height
        for t in range(len(L)):
            length = float(np.linalg.norm(toe[t] - ball[t]))
            if length < 1e-6:
                continue
            if hb[t] < thr and ha[t] >= thr:
                foot = ball[t] - ankle[t]
                horiz = foot - up * float(foot @ up)
                if np.linalg.norm(horiz) < 1e-6:
                    horiz = np.array([1.0, 0.0, 0.0])
                horiz = unit(horiz)
                drop = float(ball[t] @ up - floor)
                max_drop = length * math.sin(math.radians(TOE_MAX))
                drop = min(max(drop, 0.0), max_drop)
                along = math.sqrt(max(length * length - drop * drop, 0.0))
                L[t, ix[c]] = ball[t] + horiz * along - up * drop
            elif hb[t] < thr and ha[t] < thr:
                h = float(L[t, ix[c]] @ up - floor)
                if h < -FLOOR_EPS:
                    L[t, ix[c]] = L[t, ix[c]] + up * (-h)
                elif 0 <= h < 0.02:
                    L[t, ix[c]] = L[t, ix[c]] - up * h
            pts = {a: L[t, ix[a]].copy(), b: L[t, ix[b]].copy(), c: L[t, ix[c]].copy()}
            _clamp_end(pts, a, b, c, 0.0, TOE_MAX)
            L[t, ix[c]] = pts[c]


def despike(P):
    out = P.copy()
    for t in range(1, len(out) - 1):
        a = float(np.linalg.norm(out[t] - out[t - 1]))
        b = float(np.linalg.norm(out[t + 1] - out[t]))
        c = float(np.linalg.norm(out[t + 1] - out[t - 1]))
        if a > POP_M and b > POP_M and c < 0.5 * min(a, b):
            out[t] = 0.5 * (out[t - 1] + out[t + 1])
    return out


def _blur(x, radius=5):
    if len(x) < 3 or radius < 1:
        return np.asarray(x, float)
    k = np.arange(-radius, radius + 1)
    w = np.exp(-0.5 * (k / max(radius / 2, 1)) ** 2)
    w /= w.sum()
    return np.convolve(np.asarray(x, float), w, mode="same")


def shape_weights(extension, speed):
    """Per-frame blend: fist on a fast extension, blade on a slow one, relaxed otherwise."""
    ext = np.asarray(extension, float)
    spd = np.asarray(speed, float)
    fist = np.clip((ext - 0.78) / 0.18, 0, 1) * np.clip(spd / 1.2, 0, 1)
    blade = np.clip((ext - 0.82) / 0.14, 0, 1) * np.clip(1.0 - spd / 1.2, 0, 1)
    fist, blade = _blur(fist), _blur(blade)
    over = np.clip(fist + blade, 0, 1)
    s = np.maximum(fist + blade, 1e-6)
    return {"fist": fist / s * over, "blade": blade / s * over, "relaxed": 1 - over, "open": np.zeros_like(ext)}


def _apply_shape_frame(hpts, weights):
    degs = np.zeros(3)
    for name, w in weights.items():
        degs = degs + w * np.array(SHAPES[name])
    for side, _f, ch in _finger_chains(hpts):
        hand, mid, idx, pink = f"hand_{side}", f"middle_01_{side}", f"index_01_{side}", f"pinky_01_{side}"
        if not all(n in hpts for n in (hand, mid, idx, pink)):
            continue
        nrm = palm_normal(side, hpts[hand], hpts[mid], hpts[idx], hpts[pink])
        if nrm is None:
            continue
        for i in range(len(ch) - 1):
            parent = hpts[hand] if i == 0 else hpts[ch[i - 1]]
            u = hpts[ch[i]] - parent
            v = hpts[ch[i + 1]] - hpts[ch[i]]
            lu = float(np.linalg.norm(u))
            if lu < 1e-8 or float(np.linalg.norm(v)) < 1e-8:
                continue
            cur = _signed_flex(u / lu, unit(v), nrm)
            target = float(degs[min(i, 2)])
            delta = target - cur
            if abs(delta) < 0.8:
                continue
            axis = np.cross(u, nrm)
            if np.linalg.norm(axis) < 1e-8:
                continue
            R = rodrigues(axis, math.radians(delta))
            pivot = hpts[ch[i]]
            for k in range(i + 1, len(ch)):
                hpts[ch[k]] = pivot + R @ (hpts[ch[k]] - pivot)


def _finger_flex_range(harr):
    """Degrees of finger motion already in the clip. A wide range is authored and left alone."""
    ranges = []
    T = len(next(iter(harr.values()))) if harr else 0
    if T < 2:
        return 0.0
    for side, _f, ch in _finger_chains({k: harr[k][0] for k in harr}):
        hand, mid, idx, pink = f"hand_{side}", f"middle_01_{side}", f"index_01_{side}", f"pinky_01_{side}"
        if not all(n in harr for n in (hand, mid, idx, pink)) or len(ch) < 2:
            continue
        flex = []
        for t in range(T):
            nrm = palm_normal(side, harr[hand][t], harr[mid][t], harr[idx][t], harr[pink][t])
            if nrm is None:
                continue
            u = harr[ch[0]][t] - harr[hand][t]
            v = harr[ch[1]][t] - harr[ch[0]][t]
            if np.linalg.norm(u) < 1e-8 or np.linalg.norm(v) < 1e-8:
                continue
            flex.append(_signed_flex(unit(u), unit(v), nrm))
        if flex:
            ranges.append(max(flex) - min(flex))
    return max(ranges) if ranges else 0.0


def restyle_fingers(harr, L, ix, fps, height):
    if not harr or "wrist_r" not in ix or "shoulder_r" not in ix:
        return False
    flex_range = _finger_flex_range(harr)
    if flex_range > 18.0:
        return False
    T = len(L)
    ext = []
    for side in "lr":
        if f"shoulder_{side}" not in ix or f"elbow_{side}" not in ix or f"wrist_{side}" not in ix:
            ext.append(np.zeros(T))
            continue
        arm = (np.linalg.norm(L[:, ix[f"elbow_{side}"]] - L[:, ix[f"shoulder_{side}"]], axis=1)
               + np.linalg.norm(L[:, ix[f"wrist_{side}"]] - L[:, ix[f"elbow_{side}"]], axis=1))
        reach = np.linalg.norm(L[:, ix[f"wrist_{side}"]] - L[:, ix[f"shoulder_{side}"]], axis=1)
        ext.append(reach / np.maximum(arm, 1e-6))
    extension = np.max(ext, axis=0)
    def full_speed(P):
        return np.linalg.norm(np.gradient(P, axis=0), axis=1) * fps / max(height, 1e-6)
    spd = max(float(full_speed(L[:, ix[f"wrist_{side}"]]).max()) if f"wrist_{side}" in ix else 0.0 for side in "lr")
    speed = np.maximum(
        full_speed(L[:, ix["wrist_r"]]) if "wrist_r" in ix else 0,
        full_speed(L[:, ix["wrist_l"]]) if "wrist_l" in ix else 0,
    )
    if float(np.max(extension)) < 0.8 or float(np.max(speed)) < 0.8:
        if spd < 0.8:
            return False
    weights = shape_weights(extension, speed)
    for t in range(T):
        hpts = {k: harr[k][t].copy() for k in harr}
        _apply_shape_frame(hpts, {name: float(weights[name][t]) for name in weights})
        for k in harr:
            harr[k][t] = hpts[k]
    return True


def _palm_swing(forearm, nrm, forward, up):
    """Rotation that swings a palm normal toward forward/down when a forearm twist cannot."""
    axis = np.cross(forearm, nrm)
    if float(np.linalg.norm(axis)) < 1e-6:
        axis = np.cross(forearm, up)
    if float(np.linalg.norm(axis)) < 1e-6:
        return None
    axis = unit(axis)
    best, score = None, float(nrm @ forward)
    for sgn in (1.0, -1.0):
        for deg in (20.0, 40.0, 60.0):
            R = rodrigues(axis * sgn, math.radians(deg))
            got = float((R @ nrm) @ forward)
            if got > score + 0.08:
                best, score = R, got
                if score > -0.2:
                    return best
    return best


def fix_palms(harr, L, ix, body):
    """Rotate finger joints about the forearm when an extended hand faces backward."""
    if not harr:
        return 0
    up, forward, left = _axes(body)
    fixed = 0
    T = len(L)
    for side in "lr":
        need = [f"hand_{side}", f"middle_01_{side}", f"index_01_{side}", f"pinky_01_{side}"]
        if not all(n in harr for n in need) or f"wrist_{side}" not in ix or f"elbow_{side}" not in ix:
            continue
        inward = left if side == "l" else -left
        for t in range(T):
            nrm = palm_normal(side, harr[f"hand_{side}"][t], harr[f"middle_01_{side}"][t],
                              harr[f"index_01_{side}"][t], harr[f"pinky_01_{side}"][t])
            if nrm is None:
                continue
            forearm = L[t, ix[f"wrist_{side}"]] - L[t, ix[f"elbow_{side}"]]
            if float(nrm @ forward) > -0.55:
                continue
            if f"shoulder_{side}" in ix:
                arm = np.linalg.norm(L[t, ix[f"elbow_{side}"]] - L[t, ix[f"shoulder_{side}"]]) + np.linalg.norm(forearm)
                reach = np.linalg.norm(L[t, ix[f"wrist_{side}"]] - L[t, ix[f"shoulder_{side}"]])
                if arm < 1e-6 or reach / arm < 0.8:
                    continue
            ang = palm_delta(forearm, nrm, up, inward, forward)
            R = rodrigues(forearm, ang) if abs(ang) >= math.radians(8) else np.eye(3)
            turned = R @ nrm
            if float(turned @ forward) <= float(nrm @ forward):
                R = np.eye(3)
                turned = nrm
            # A twist about the forearm cannot face a palm whose normal lies along the arm.
            # Swing the hand a short way off that axis until the palm looks forward or down.
            if float(turned @ forward) <= -0.55:
                swing = _palm_swing(forearm, turned, forward, up)
                if swing is not None:
                    R = swing @ R
                    turned = swing @ turned
            if float(turned @ forward) <= float(nrm @ forward) + 0.02:
                continue
            origin = harr[f"hand_{side}"][t]
            for n in harr:
                if n.endswith("_" + side):
                    harr[n][t] = origin + R @ (harr[n][t] - origin)
            fixed += 1
    return fixed


def _hpts_at(harr, t):
    return {k: harr[k][t].copy() for k in harr}


def clean(L, names, fps, floor, height, hands, body, lengths, restyle=True):
    """Whole clip. Returns (landmarks, hands, locked_frames)."""
    L = np.array(L, float).copy()
    ix = {n: i for i, n in enumerate(names)}
    up, _forward, _left = _axes(body)
    body = {**body, "floor": floor, "height": height}
    T = len(L)
    harr, _hnames = _hands_arrays(hands or [{}] * T, T)
    for t in range(T):
        pts = {n: L[t, i].copy() for i, n in enumerate(names)}
        hpts = _hpts_at(harr, t)
        pts, hpts = clean_frame_pts(pts, hpts, body, lengths)
        for n, i in ix.items():
            L[t, i] = pts[n]
        for k in harr:
            if k in hpts:
                harr[k][t] = hpts[k]
    if restyle:
        restyle_fingers(harr, L, ix, fps, height)
    fix_palms(harr, L, ix, body)
    for i in range(L.shape[1]):
        L[:, i] = despike(L[:, i])
    for k in harr:
        harr[k] = despike(harr[k])
    _snap_lengths(L, ix, lengths)
    locked = lock_feet(L, ix, up, floor, height, fps)
    bend_toes(L, ix, up, floor, height)
    for t in range(T):
        pts = {n: L[t, i].copy() for i, n in enumerate(names)}
        clamp_distal(pts)
        lift_foot_rigid(pts, up, floor)
        for n, i in ix.items():
            L[t, i] = pts[n]
    locked += lock_feet(L, ix, up, floor, height, fps)
    _up, forward, _left = _axes(body)
    settle_legs(L, ix, lengths, forward)
    # The plant and the despike move children. Put those bones back on their rest length
    # without dragging the ankle off the lock; then clear the floor as a rigid foot.
    _snap_lengths(L, ix, lengths, only=_DISTAL)
    for _ in range(2):
        for t in range(T):
            pts = {n: L[t, i].copy() for i, n in enumerate(names)}
            lift_foot_rigid(pts, up, floor)
            for n, i in ix.items():
                L[t, i] = pts[n]
        settle_legs(L, ix, lengths, forward)
    _snap_lengths(L, ix, lengths, only=_DISTAL)
    for t in range(T):
        pts = {n: L[t, i].copy() for i, n in enumerate(names)}
        lift_foot_rigid(pts, up, floor)
        for n, i in ix.items():
            L[t, i] = pts[n]
    hands_out = []
    for t in range(T):
        src = (hands[t] if hands and t < len(hands) and hands[t] else {})
        frame = {k: [round(float(x), 5) for x in harr[k][t]] for k in harr}
        for k, v in src.items():
            frame.setdefault(k, v)
        hands_out.append(frame)
    return L, hands_out, locked


def _mask_peak(mask, values):
    if not np.any(mask):
        return 0.0
    return float(np.max(np.abs(values[mask])))


def validate(L, names, fps, floor, height, hands, body, lengths):
    """Issues with inclusive frame ranges. Empty list means the clip passes."""
    L = np.asarray(L, float)
    ix = {n: i for i, n in enumerate(names)}
    up, forward, left = _axes(body)
    T = len(L)
    issues = []
    for a, b in SEGMENTS:
        if a not in ix or b not in ix or (a, b) not in lengths:
            continue
        target = lengths[(a, b)]
        if target <= 1e-6:
            continue
        d = np.linalg.norm(L[:, ix[b]] - L[:, ix[a]], axis=1)
        rel = np.abs(d - target) / target
        iss = _issue("limb_stretch", f"{a}->{b}", rel > LENGTH_TOL + 0.005, _mask_peak(rel > LENGTH_TOL, rel), "ratio")
        if iss:
            issues.append(iss)
        if T > 2:
            jump = np.zeros(T)
            jump[1:] = np.abs(np.diff(d)) / target
            iss = _issue("limb_pop", f"{a}->{b}", jump > 0.03, _mask_peak(jump > 0.03, jump), "ratio")
            if iss:
                issues.append(iss)
    for a, b, c, kind in HINGES:
        if a not in ix or b not in ix or c not in ix:
            continue
        bad = np.zeros(T, bool)
        over = np.zeros(T, bool)
        peaks_b, peaks_o = [], []
        limit = KNEE_MAX if kind == "knee" else ELBOW_MAX
        for t in range(T):
            pa, pb, pc = L[t, ix[a]], L[t, ix[b]], L[t, ix[c]]
            th = _bone_angle(pa, pb, pc)
            if hinge_bad(pa, pb, pc, kind, forward):
                bad[t] = True
                peaks_b.append(th)
            if th > limit + 2:
                over[t] = True
                peaks_o.append(th - limit)
        name = "knee_hyperextension" if kind == "knee" else "elbow_hyperextension"
        iss = _issue(name, b, bad, max(peaks_b) if peaks_b else 0, "deg")
        if iss:
            issues.append(iss)
        iss = _issue("knee_limit" if kind == "knee" else "elbow_limit", b, over, max(peaks_o) if peaks_o else 0, "deg")
        if iss:
            issues.append(iss)
    for a, b, c in WRISTS:
        if a not in ix or b not in ix or c not in ix:
            continue
        th = np.array([_bone_angle(L[t, ix[a]], L[t, ix[b]], L[t, ix[c]]) for t in range(T)])
        mask = th > WRIST_MAX + 2
        iss = _issue("wrist_limit", b, mask, _mask_peak(mask, th - WRIST_MAX), "deg")
        if iss:
            issues.append(iss)
    for a, b, c in ANKLES:
        if a not in ix or b not in ix or c not in ix:
            continue
        th = np.array([_bone_angle(L[t, ix[a]], L[t, ix[b]], L[t, ix[c]]) for t in range(T)])
        mask = (th < ANKLE_MIN - 2) | (th > ANKLE_MAX + 2)
        iss = _issue("ankle_limit", b, mask, _mask_peak(mask, np.maximum(ANKLE_MIN - th, th - ANKLE_MAX)), "deg")
        if iss:
            issues.append(iss)
    for a, b, c in TOES:
        if a not in ix or b not in ix or c not in ix:
            continue
        th = np.array([_bone_angle(L[t, ix[a]], L[t, ix[b]], L[t, ix[c]]) for t in range(T)])
        # Backward: the toe drops through the floor side of a straight foot (angle past straight is 0,
        # so a negative flex is detected as the toe being below the ankle-ball line's ground side).
        # Folded back toward the ankle, not dorsiflex at toe-off (that bend points up and is wanted).
        back = np.zeros(T, bool)
        peak = 0.0
        for t in range(T):
            u = L[t, ix[b]] - L[t, ix[a]]
            v = L[t, ix[c]] - L[t, ix[b]]
            nu, nv = float(np.linalg.norm(u)), float(np.linalg.norm(v))
            if nu < 1e-8 or nv < 1e-8:
                continue
            cang = float((u / nu) @ (v / nv))
            if cang < 0.0:
                back[t] = True
                peak = max(peak, math.degrees(math.acos(np.clip(cang, -1, 1))))
        iss = _issue("toe_backbend", c, back, peak, "deg")
        if iss:
            issues.append(iss)
        mask = th > TOE_MAX + 3
        iss = _issue("toe_limit", c, mask, _mask_peak(mask, th - TOE_MAX), "deg")
        if iss:
            issues.append(iss)
    for n in FEET:
        if n not in ix:
            continue
        h = _height_of(L[:, ix[n]], up, floor)
        mask = h < -FLOOR_EPS
        iss = _issue("foot_floor", n, mask, _mask_peak(mask, -h), "m")
        if iss:
            issues.append(iss)
    for side in "lr":
        ankle, ball = f"ankle_{side}", f"ball_{side}"
        ankle_low = _plant_mask(L[:, ix[ankle]], up, floor, height) if ankle in ix else np.zeros(T, bool)
        ball_low = _plant_mask(L[:, ix[ball]], up, floor, height) if ball in ix else np.zeros(T, bool)
        # Flat and heel-strike plant through the ankle. The ball is the contact only at toe-off,
        # so a rocking ball under a still ankle is not a slide.
        checks = []
        if ankle in ix:
            checks.append((ankle, ankle_low))
        if ball in ix:
            checks.append((ball, ball_low & ~ankle_low))
        for n, low in checks:
            speed = _slide_speed(L[:, ix[n]], up, fps)
            mask = low & (speed > SLIDE_MIN) & (speed < SLIDE_MAX)
            keep = np.zeros(T, bool)
            for a, b in frame_ranges(mask):
                if b - a >= 3:
                    keep[a:b + 1] = True
            iss = _issue("foot_slide", n, keep, _mask_peak(keep, speed), "m/s")
            if iss:
                issues.append(iss)
    for i, n in enumerate(names):
        if T < 3:
            continue
        step = np.zeros(T)
        step[1:] = np.linalg.norm(np.diff(L[:, i], axis=0), axis=1)
        mask = np.zeros(T, bool)
        for t in range(1, T - 1):
            c = float(np.linalg.norm(L[t + 1, i] - L[t - 1, i]))
            if step[t] > POP_M and step[t + 1] > POP_M and c < 0.5 * min(step[t], step[t + 1]):
                mask[t] = True
        iss = _issue("discontinuity", n, mask, _mask_peak(mask, step), "m")
        if iss:
            issues.append(iss)
    if "pelvis" in ix and "neck" in ix:
        radius = 0.09 * height
        for side in "lr":
            for n in (f"wrist_{side}", f"knuckle_{side}"):
                if n not in ix:
                    continue
                mask = np.zeros(T, bool)
                depth = np.zeros(T)
                for t in range(T):
                    p = L[t, ix[n]]
                    ab = L[t, ix["neck"]] - L[t, ix["pelvis"]]
                    d2 = float(ab @ ab)
                    tt = 0.0 if d2 < 1e-12 else float(np.clip(((p - L[t, ix["pelvis"]]) @ ab) / d2, 0, 1))
                    closest = L[t, ix["pelvis"]] + ab * tt
                    dist = float(np.linalg.norm(p - closest))
                    if dist < 0.62 * radius and float((p - closest) @ forward) < 0.02 * height:
                        mask[t] = True
                        depth[t] = radius - dist
                iss = _issue("hand_intersection", n, mask, _mask_peak(mask, depth), "m")
                if iss:
                    issues.append(iss)
    harr, _ = _hands_arrays(hands, T) if hands else ({}, [])
    if harr:
        for side, f, ch in _finger_chains({k: harr[k][0] for k in harr}):
            hand, mid, idx, pink = f"hand_{side}", f"middle_01_{side}", f"index_01_{side}", f"pinky_01_{side}"
            if not all(n in harr for n in (hand, mid, idx, pink)):
                continue
            for i in range(len(ch) - 1):
                mask = np.zeros(T, bool)
                peak = 0.0
                for t in range(T):
                    nrm = palm_normal(side, harr[hand][t], harr[mid][t], harr[idx][t], harr[pink][t])
                    if nrm is None:
                        continue
                    parent = harr[hand][t] if i == 0 else harr[ch[i - 1]][t]
                    u = harr[ch[i]][t] - parent
                    v = harr[ch[i + 1]][t] - harr[ch[i]][t]
                    if np.linalg.norm(u) < 1e-8 or np.linalg.norm(v) < 1e-8:
                        continue
                    flex = _signed_flex(unit(u), unit(v), nrm)
                    if flex < -TOE_BACK:
                        mask[t] = True
                        peak = max(peak, -flex)
                iss = _issue("finger_backbend", ch[i], mask, peak, "deg")
                if iss:
                    issues.append(iss)
        for side in "lr":
            need = [f"hand_{side}", f"middle_01_{side}", f"index_01_{side}", f"pinky_01_{side}"]
            if not all(n in harr for n in need) or f"wrist_{side}" not in ix or f"elbow_{side}" not in ix:
                continue
            mask = np.zeros(T, bool)
            for t in range(T):
                nrm = palm_normal(side, harr[f"hand_{side}"][t], harr[f"middle_01_{side}"][t],
                                  harr[f"index_01_{side}"][t], harr[f"pinky_01_{side}"][t])
                if nrm is None:
                    continue
                if float(nrm @ forward) > -0.55:
                    continue
                if f"shoulder_{side}" in ix:
                    forearm = L[t, ix[f"wrist_{side}"]] - L[t, ix[f"elbow_{side}"]]
                    arm = np.linalg.norm(L[t, ix[f"elbow_{side}"]] - L[t, ix[f"shoulder_{side}"]]) + np.linalg.norm(forearm)
                    reach = np.linalg.norm(L[t, ix[f"wrist_{side}"]] - L[t, ix[f"shoulder_{side}"]])
                    if arm < 1e-6 or reach / arm < 0.8:
                        continue
                mask[t] = True
            iss = _issue("palm_backward", f"hand_{side}", mask, 1.0, "flag")
            if iss:
                issues.append(iss)
    return issues


def quat_angle_deg(a, b):
    d = abs(float(np.dot(a, b)))
    return math.degrees(2 * math.acos(min(1.0, max(0.0, d))))


def slerp(a, b, u):
    a = np.asarray(a, float)
    b = np.asarray(b, float)
    a = a / np.linalg.norm(a)
    b = b / np.linalg.norm(b)
    if float(a @ b) < 0:
        b = -b
    d = float(np.clip(a @ b, -1, 1))
    th = math.acos(d)
    if th < 1e-6:
        return a
    return (math.sin((1 - u) * th) * a + math.sin(u * th) * b) / math.sin(th)


def repair_quats(q, max_step_deg=110.0):
    """Flip hemisphere, then replace a one-frame pop with the slerp of its neighbours."""
    q = np.array(q, float).copy()
    for t in range(1, len(q)):
        if float(q[t] @ q[t - 1]) < 0:
            q[t] = -q[t]
    if len(q) < 3:
        return q
    for t in range(1, len(q) - 1):
        a = quat_angle_deg(q[t - 1], q[t])
        b = quat_angle_deg(q[t], q[t + 1])
        c = quat_angle_deg(q[t - 1], q[t + 1])
        if a > max_step_deg and b > max_step_deg and c < max_step_deg * 0.5:
            q[t] = slerp(q[t - 1], q[t + 1], 0.5)
    return q


def validate_quats(quats, max_step_deg=110.0):
    """`quats` is {bone: (T, 4)}. Flags pops, not hemisphere flips (those are the same rotation)."""
    issues = []
    for bone, q in quats.items():
        q = np.asarray(q, float)
        if len(q) < 3:
            continue
        fixed = q.copy()
        for t in range(1, len(fixed)):
            if float(fixed[t] @ fixed[t - 1]) < 0:
                fixed[t] = -fixed[t]
        mask = np.zeros(len(q), bool)
        peak = 0.0
        for t in range(1, len(q) - 1):
            a = quat_angle_deg(fixed[t - 1], fixed[t])
            b = quat_angle_deg(fixed[t], fixed[t + 1])
            c = quat_angle_deg(fixed[t - 1], fixed[t + 1])
            if a > max_step_deg and b > max_step_deg and c < max_step_deg * 0.5:
                mask[t] = True
                peak = max(peak, a, b)
        iss = _issue("quat_pop", bone, mask, peak, "deg")
        if iss:
            issues.append(iss)
    return issues


def repair_baked(frames, max_step_deg=110.0):
    """In-place hemisphere + pop repair on baked `{q, p}` frames. Returns how many samples changed."""
    if not frames or "q" not in frames[0]:
        return 0
    changed = 0
    for bone in frames[0]["q"]:
        q = np.array([fr["q"][bone] for fr in frames], float)
        fixed = repair_quats(q, max_step_deg)
        for fr, a, b in zip(frames, q, fixed):
            if float(np.linalg.norm(a - b)) > 1e-6 and abs(float(a @ b)) < 0.999:
                changed += 1
            fr["q"][bone] = [round(float(x), 5) for x in b]
    return changed


def apply_tracks(ref, names, L, hands):
    out = dict(ref)
    samples = []
    for i, s in enumerate(ref["samples"]):
        lm = {n: [round(float(x), 5) for x in L[i, k]] for k, n in enumerate(names)}
        hd = hands[i] if hands else s.get("hands") or {}
        samples.append({**s, "landmarks": lm, "hands": hd})
    out["samples"] = samples
    return out


def ref_arrays(ref):
    names = sorted(ref["samples"][0]["landmarks"])
    L = np.array([[s["landmarks"][n] for n in names] for s in ref["samples"]], float)
    hands = [s.get("hands") or {} for s in ref["samples"]]
    return names, L, hands


def format_issues(name, issues, motion=None):
    head = f"{name}: motion {motion}" if motion else name
    if not issues:
        return head + ": no issues"
    lines = [f"{head}: {len(issues)} issue(s)"]
    for iss in issues:
        rng = ", ".join(f"{a}-{b}" if a != b else str(a) for a, b in iss["frames"])
        lines.append(f"  {iss['kind']:<22} {iss.get('where', ''):<16} frames {rng}  peak {iss['peak']} {iss['unit']}")
    return "\n".join(lines)


def check_text(ref, body, lengths=None):
    names, L, hands = ref_arrays(ref)
    fps = 60.0
    if len(ref["samples"]) > 1:
        dt = ref["samples"][1]["t"] - ref["samples"][0]["t"]
        fps = 1.0 / dt if dt > 1e-6 else 60.0
    lengths = lengths if lengths is not None else {}
    issues = validate(L, names, fps, ref["floor"], ref["height"], hands, body, lengths)
    return format_issues(ref.get("name") or ref.get("clip") or "clip", issues)
