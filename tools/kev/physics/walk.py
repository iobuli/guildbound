"""The Studio walk cycle from kev_walk.py, as landmark frames. No HTTP.

Treadmill in place: the stance foot slides back at constant ground speed, the swing foot lifts and
comes forward, the pelvis bobs and shifts over the stance foot. World axes match /api/rig:
+Y up, +Z forward, +X character-left, metres. `rest` is {landmark: [x, y, z]} at the bind pose.
"""
import math


def _sub(a, b):
    return [a[i] - b[i] for i in range(3)]


def _add(a, b):
    return [a[i] + b[i] for i in range(3)]


def _rot_y(v, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return [c * v[0] + s * v[2], v[1], -s * v[0] + c * v[2]]


def _rot_x(v, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return [v[0], c * v[1] + s * v[2], -s * v[1] + c * v[2]]


def _rot_z(v, deg):
    c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
    return [c * v[0] - s * v[1], s * v[0] + c * v[1], v[2]]


def _smooth(x):
    return x * x * (3 - 2 * x)


def _lerp(a, b, t):
    return a + (b - a) * t


def frames(rest, seconds=2.2, hz=30.0, period=1.1, stride=0.30, lift=0.09, width=0.065, sway=0.045, duty=0.6, progress=0.0):
    """One landmark dict per frame, the same targets kev_walk.py posts to /api/control."""
    R = {k: list(v) for k, v in rest.items()}
    pel = R["pelvis"]
    off = {k: _sub(R[k], pel) for k in ("hip_l", "hip_r", "neck", "crown", "nose", "head_joint")}
    neck_sh = {k: _sub(R[k], R["neck"]) for k in ("shoulder_l", "shoulder_r")}
    foot_v = {s: _sub(R[f"ball_{s}"], R[f"ankle_{s}"]) for s in "lr"}
    n = int(round(seconds * hz))
    dt = 1.0 / hz
    out = []
    for i in range(n):
        out.append(_frame(i * dt, R, pel, off, neck_sh, foot_v, period, stride, lift, width, sway, duty, progress))
    return out


def _foot(side, phi, R, foot_v, stride, duty, lift, width):
    a0 = R[f"ankle_{side}"]
    z0 = a0[2]
    front, back = z0 + stride / 2, z0 - stride / 2
    if phi < duty:
        s = phi / duty
        z, up, stance = _lerp(front, back, s), 0.0, True
        pitch = 12 * (1 - s / 0.12) if s < 0.12 else (-22 * _smooth((s - 0.7) / 0.3) if s > 0.7 else 0.0)
    else:
        s = (phi - duty) / (1 - duty)
        z = _lerp(back, front, _smooth(s))
        up = lift * math.sin(math.pi * min(1.0, s * 1.1)) ** 1.2
        pitch = -22 * (1 - _smooth(min(1, s / 0.4))) + 12 * _smooth(max(0, (s - 0.6) / 0.4))
        stance = False
    delta = _rot_x(foot_v[side], pitch)
    y = a0[1] + up
    if pitch < 0:
        y = max(y, R[f"ball_{side}"][1] - delta[1])
    ankle = [math.copysign(width, a0[0]), y, z]
    return ankle, _add(ankle, delta), stance


def _frame(t, R, PEL, OFF, NECK_OFF_SH, FOOT, period, stride, lift, width, sway, duty, progress):
    phi = (t / period) % 1.0
    al, bl, _sl = _foot("l", phi, R, FOOT, stride, duty, lift, width)
    ar, br, _sr = _foot("r", (phi + 0.5) % 1.0, R, FOOT, stride, duty, lift, width)
    w = 2 * math.pi * phi
    bob = -0.035 + 0.014 * math.cos(2 * w - 2 * math.pi * duty)
    lat = sway * math.cos(w - math.pi * duty)
    yaw = 6 * math.cos(w)
    drop = 3 * math.cos(w - math.pi * duty)
    drift = (progress * t) % 1.5 if progress else 0.0
    pel = [PEL[0] + lat, PEL[1] + bob, PEL[2] + drift]

    def hip(k):
        return _add(pel, _rot_z(_rot_y(OFF[k], -yaw), drop))

    neck = _add(_add(pel, _rot_y(OFF["neck"], 0)), [lat * 0.25, 0, 0.025])
    lm = {"pelvis": pel, "hip_l": hip("hip_l"), "hip_r": hip("hip_r"), "neck": neck,
          "shoulder_l": _add(neck, _rot_y(NECK_OFF_SH["shoulder_l"], yaw * 0.8)),
          "shoulder_r": _add(neck, _rot_y(NECK_OFF_SH["shoulder_r"], yaw * 0.8))}
    for k in ("crown", "nose", "head_joint"):
        lm[k] = _add(neck, _sub(OFF[k], OFF["neck"]))
    for s, an, ba in (("l", al, bl), ("r", ar, br)):
        an = [an[0], an[1], an[2] + drift]
        ba = [ba[0], ba[1], ba[2] + drift]
        lm[f"ankle_{s}"], lm[f"ball_{s}"] = an, ba
        h = lm[f"hip_{s}"]
        lm[f"knee_{s}"] = [(h[0] + an[0]) / 2, (h[1] + an[1]) / 2, max(h[2], an[2]) + 0.35]
    for s, sign, ph in (("l", 1, 0.0), ("r", -1, math.pi)):
        sw = -math.cos(w + ph)
        z = -0.03 + (0.16 if sw > 0 else 0.11) * sw
        y = 0.935 + 0.035 * sw * sw + (0.03 * sw if sw > 0 else 0)
        lm[f"wrist_{s}"] = [sign * 0.205 + lat * 0.6, y + bob * 0.6, z + drift]
        lm[f"elbow_{s}"] = [sign * 0.24, 1.15, -0.45 + drift]
    return lm
