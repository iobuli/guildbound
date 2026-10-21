"""Landmark body model shared by the physics pass: sole points, centre of mass, support, poles.

Masses are the Winter (2009) segment table used by bodymap.balance, so a pose this pass
calls balanced is the same pose Studio's readout calls balanced. Axes: +Y up, +Z forward,
+X character-left, metres.
"""
import math

import numpy as np

# (proximal, distal, COM fraction from proximal, mass share). "{s}" expands per side.
MASS = (
    ("head_joint", "crown", 0.5, 0.081),
    ("trunk_low", "trunk_high", 0.5, 0.497),
    ("shoulder_{s}", "elbow_{s}", 0.436, 0.028),
    ("elbow_{s}", "wrist_{s}", 0.43, 0.016),
    ("wrist_{s}", "knuckle_{s}", 0.506, 0.006),
    ("hip_{s}", "knee_{s}", 0.433, 0.100),
    ("knee_{s}", "ankle_{s}", 0.433, 0.0465),
    ("ankle_{s}", "ball_{s}", 0.5, 0.0145),
)

# Knee landmarks are poles for the solver, not skin contacts. Everything else shifts with the trunk.
LEG = {
    "knee_l", "knee_r", "ankle_l", "ankle_r", "ball_l", "ball_r",
    "heel_l", "heel_r", "toe_tip_l", "toe_tip_r", "toe_l", "toe_r", "instep_l", "instep_r",
}
G = 9.81
HEEL_BACK = 0.07
TOE_PAST = 0.04


def unit(v):
    n = float(np.linalg.norm(v))
    if n < 1e-9:
        return np.zeros(3)
    return np.asarray(v, float) / n


def as_lm(landmarks):
    return {k: np.array(v, float) for k, v in landmarks.items()}


def lists(lm, keys):
    return {k: [float(x) for x in lm[k]] for k in keys if k in lm}


def xz(v):
    return np.array([float(v[0]), float(v[2])])


def from_xz(v, y=0.0):
    return np.array([float(v[0]), float(y), float(v[1])])


def _horiz(v):
    h = np.array([float(v[0]), 0.0, float(v[2])])
    n = float(np.linalg.norm(h))
    if n < 1e-8:
        return np.array([0.0, 0.0, 1.0]), 1e-8
    return h / n, n


def _rest_pair(rest, side):
    if not rest:
        return None
    a, b = rest.get(f"ankle_{side}"), rest.get(f"ball_{side}")
    if a is None or b is None:
        return None
    return np.array(a, float), np.array(b, float)


def sole_pitch(ankle, ball, rest, side):
    """Bone pitch relative to the flat rest pose. 0 = the sole the rig stands on."""
    _, hlen = _horiz(ball - ankle)
    pitch = math.atan2(float((ball - ankle)[1]), hlen)
    pair = _rest_pair(rest, side)
    if pair is None:
        return 0.0
    ra, rb = pair
    _, rh = _horiz(rb - ra)
    return pitch - math.atan2(float((rb - ra)[1]), rh)


def _basis(ankle, ball):
    fwd, _ = _horiz(ball - ankle)
    side = np.array([fwd[2], 0.0, -fwd[0]])
    return fwd, side


def _place(ankle, ball, local_y, local_z, rest, side):
    """A sole point. local_z is forward of the ankle in the unpitched sole, local_y is up from the ankle."""
    fwd, lat = _basis(ankle, ball)
    th = -sole_pitch(ankle, ball, rest, side)
    c, s = math.cos(th), math.sin(th)
    y = local_y * c - local_z * s
    z = local_y * s + local_z * c
    return ankle + lat * 0.0 + np.array([0.0, y, 0.0]) + fwd * z


def _sole_locals(rest, side):
    """Flat-sole offsets (y from the ankle, z forward of the ankle) so the rest heel sits on the ball's height."""
    pair = _rest_pair(rest, side)
    if pair is None:
        return -0.09, HEEL_BACK, TOE_PAST, 0.16
    ra, rb = pair
    _, rh = _horiz(rb - ra)
    drop = float(rb[1] - ra[1])
    return drop, HEEL_BACK, TOE_PAST, rh


def heel_point(ankle, ball, rest, side):
    drop, back, _, _ = _sole_locals(rest, side)
    return _place(ankle, ball, drop, -back, rest, side)


def toe_point(ankle, ball, rest, side):
    drop, _, past, reach = _sole_locals(rest, side)
    return _place(ankle, ball, drop, reach + past, rest, side)


def enrich(landmarks, rest):
    """Copy plus heel and toe_tip where the pose only has ankle and ball. Returns (lm, added keys)."""
    out = as_lm(landmarks)
    added = []
    for s in "lr":
        ak, bk = f"ankle_{s}", f"ball_{s}"
        if ak not in out or bk not in out:
            continue
        if f"heel_{s}" not in out:
            out[f"heel_{s}"] = heel_point(out[ak], out[bk], rest, s)
            added.append(f"heel_{s}")
        if f"toe_tip_{s}" not in out:
            out[f"toe_tip_{s}"] = toe_point(out[ak], out[bk], rest, s)
            added.append(f"toe_tip_{s}")
    return out, added


def resole(lm, rest, added):
    """Recompute synthesized sole points after the ankle and ball moved."""
    for key in added:
        side = key[-1]
        if key.startswith("heel"):
            lm[key] = heel_point(lm[f"ankle_{side}"], lm[f"ball_{side}"], rest, side)
        elif key.startswith("toe"):
            lm[key] = toe_point(lm[f"ankle_{side}"], lm[f"ball_{side}"], rest, side)


def sole_keys(side):
    return (f"heel_{side}", f"ball_{side}", f"toe_tip_{side}")


def lowest_sole(lm, side):
    keys = [k for k in sole_keys(side) if k in lm]
    if not keys:
        return None, None
    key = min(keys, key=lambda k: float(lm[k][1]))
    return key, lm[key]


def infer_floor(frames, rest):
    """Standing sole height. Rest ball wins; otherwise the low end of the clip."""
    ys = []
    for s in "lr":
        pair = _rest_pair(rest, s)
        if pair is not None:
            ys.append(float(pair[1][1]))
            ys.append(float(heel_point(pair[0], pair[1], rest, s)[1]))
    if ys:
        return float(min(ys))
    samples = []
    for fr in frames:
        lm, _ = enrich(fr, rest)
        for s in "lr":
            _, p = lowest_sole(lm, s)
            if p is not None:
                samples.append(float(p[1]))
    if not samples:
        return 0.0
    samples.sort()
    return samples[max(0, int(len(samples) * 0.05))]


def centre_of_mass(lm):
    """Winter COM. Missing segments are skipped and the mass is renormalised."""
    P = dict(lm)
    if "hip_l" in P and "hip_r" in P:
        P["trunk_low"] = (P["hip_l"] + P["hip_r"]) / 2
    if "shoulder_l" in P and "shoulder_r" in P:
        P["trunk_high"] = (P["shoulder_l"] + P["shoulder_r"]) / 2
    elif "neck" in P:
        P["trunk_high"] = P["neck"]
    acc = np.zeros(3)
    total = 0.0
    for a, b, frac, mass in MASS:
        sides = ("l", "r") if "{s}" in a else ("",)
        for s in sides:
            ka, kb = a.format(s=s), b.format(s=s)
            if ka not in P or kb not in P:
                continue
            acc += mass * (P[ka] + frac * (P[kb] - P[ka]))
            total += mass
    if total < 1e-8:
        return P.get("pelvis", np.zeros(3)).copy()
    return acc / total


def shift_upper(lm, delta):
    d = np.asarray(delta, float)
    for k in list(lm):
        if k in LEG:
            continue
        lm[k] = lm[k] + d


def hull(points):
    """2D convex hull, counter-clockwise. Points are (x, z)."""
    pts = sorted({(round(float(p[0]), 6), round(float(p[1]), 6)) for p in points})
    if len(pts) < 3:
        return [np.array(p, float) for p in pts]

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return [np.array(p, float) for p in lower[:-1] + upper[:-1]]


def support_polygon(lm, planted, half_width):
    """Quad per planted foot (heel-toe, widened by the shoe), then the convex hull. (x, z) points."""
    raw = []
    for side in planted:
        hk, bk, tk = f"heel_{side}", f"ball_{side}", f"toe_tip_{side}"
        if hk not in lm or bk not in lm:
            continue
        heel, toe = lm[hk], lm.get(tk, lm[bk])
        fwd, _ = _horiz(toe - heel)
        if float(np.linalg.norm(toe - heel)) < 1e-4:
            fwd = np.array([0.0, 0.0, 1.0])
        lat = np.array([-fwd[2], 0.0, fwd[0]]) * half_width
        for p in (heel, toe):
            raw.append(xz(p + lat))
            raw.append(xz(p - lat))
    if not raw:
        return []
    return hull(raw)


def closest_on_poly(poly, p):
    """(point, distance, inside) for p against a convex outline. Inside points have distance 0."""
    p = np.asarray(p, float)
    if not poly:
        return p, 1e9, False
    if len(poly) == 1:
        return poly[0], float(np.linalg.norm(p - poly[0])), False
    inside = len(poly) >= 3
    best, best_d = None, 1e9
    edges = list(zip(poly, poly[1:] + poly[:1]))
    for a, b in edges:
        ab = b - a
        t = float(np.dot(p - a, ab) / max(float(np.dot(ab, ab)), 1e-12))
        t = min(1.0, max(0.0, t))
        q = a + ab * t
        d = float(np.linalg.norm(p - q))
        if d < best_d:
            best, best_d = q, d
        if inside and (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]) < -1e-9:
            inside = False
    if inside:
        return p.copy(), 0.0, True
    return best, best_d, False


def rot_between(a, b):
    """Rotation matrix taking vector a onto b."""
    a, b = unit(a), unit(b)
    c = np.cross(a, b)
    d = float(np.dot(a, b))
    if float(np.linalg.norm(c)) < 1e-8:
        if d > 0:
            return np.eye(3)
        axis = unit(np.cross(a, np.array([1.0, 0.0, 0.0])))
        if float(np.linalg.norm(axis)) < 1e-6:
            axis = np.array([0.0, 0.0, 1.0])
        return rodrigues(axis, math.pi)
    K = np.array([[0, -c[2], c[1]], [c[2], 0, -c[0]], [-c[1], c[0], 0]], float)
    return np.eye(3) + K + K @ K * (1.0 / (1.0 + d))


def rodrigues(axis, theta):
    k = unit(axis)
    K = np.array([[0, -k[2], k[1]], [k[2], 0, -k[0]], [-k[1], k[0], 0]], float)
    return np.eye(3) + math.sin(theta) * K + (1 - math.cos(theta)) * (K @ K)


def transfer_pole(hip_old, hip_new, ankle_old, ankle_new, knee):
    """Keep the knee pole's offset from the hip-ankle line when that line moves."""
    a = np.asarray(ankle_old, float) - np.asarray(hip_old, float)
    b = np.asarray(ankle_new, float) - np.asarray(hip_new, float)
    if float(np.linalg.norm(a)) < 1e-6 or float(np.linalg.norm(b)) < 1e-6:
        return np.asarray(knee, float) + (np.asarray(hip_new, float) - np.asarray(hip_old, float))
    R = rot_between(a, b)
    return np.asarray(hip_new, float) + R @ (np.asarray(knee, float) - np.asarray(hip_old, float))


def leg_reach(rest, side):
    if not rest:
        return None
    h, k, a = rest.get(f"hip_{side}"), rest.get(f"knee_{side}"), rest.get(f"ankle_{side}")
    if h is None or k is None or a is None:
        return None
    h, k, a = np.array(h, float), np.array(k, float), np.array(a, float)
    return float(np.linalg.norm(k - h) + np.linalg.norm(a - k))


def clamp_vec(v, limit):
    n = float(np.linalg.norm(v))
    if n <= limit or n < 1e-12:
        return v
    return v * (limit / n)


class Spring:
    """World-space spring toward a moving target. Substeps so 30 Hz stays stable."""

    def __init__(self, hz, zeta, max_lag):
        self.hz = float(hz)
        self.zeta = float(zeta)
        self.max_lag = float(max_lag)
        self.x = None
        self.v = np.zeros(3)

    def reset(self):
        self.x = None
        self.v = np.zeros(3)

    def kick(self, vel):
        self.v = self.v + np.asarray(vel, float)

    def step(self, target, dt):
        target = np.asarray(target, float)
        if self.x is None or dt <= 0:
            self.x = target.copy()
            self.v = np.zeros(3)
            return self.x.copy()
        n = max(1, int(math.ceil(dt / (1.0 / 120.0))))
        h = dt / n
        w = 2 * math.pi * self.hz
        for _ in range(n):
            acc = w * w * (target - self.x) - 2 * self.zeta * w * self.v
            self.v = self.v + acc * h
            self.x = self.x + self.v * h
        lag = self.x - target
        dist = float(np.linalg.norm(lag))
        if dist > self.max_lag:
            self.x = target + lag * (self.max_lag / dist)
            self.v *= 0.5
        return self.x.copy()
