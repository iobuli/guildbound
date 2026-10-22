"""Spatial smoothing for Kev references: landmark trajectories (T x N x 3, metres, fixed fps) in, smoother ones out.

Two passes, all offline so they can look both ways in time:
  1. adaptive zero-phase filter per landmark: a One-Euro low-pass whose cutoff rises with the landmark's own speed,
     run forward then backward. Slow drift (idle sway, keyframe seams) is flattened, fast strikes keep their peaks.
     A landmark the filter would make rougher (higher jerk) is left raw. That is what was roughening backflip_evade,
     aerial_dive_attack and axe_kick: stretching short segments up to a median length, then filtering that invented
     motion. The solver already refuses a reach past the bone length, so landmarks are not stretched here.
  2. foot lock: ankle runs that are low and slow are pinned in x/z, and the ball keeps its offset from the ankle so
     the toe does not spin. The pin eases in and out with a smoothstep.
Feet the filter pulled through the floor are put back on it.
"""
import numpy as np


def one_euro(x, fps, min_cut, beta, d_cut=1.0):
    """One pass of the One-Euro filter over x (T x 3)."""
    def alpha(c):
        tau = 1.0 / (2 * np.pi * c)
        return 1.0 / (1.0 + tau * fps)
    y = x.copy()
    dx = np.zeros(3)
    for t in range(1, len(x)):
        raw = (x[t] - x[t - 1]) * fps
        dx = dx + alpha(d_cut) * (raw - dx)
        cut = min_cut + beta * np.linalg.norm(dx)
        y[t] = y[t - 1] + alpha(cut) * (x[t] - y[t - 1])
    return y


def smooth_track(x, fps, min_cut=4.0, beta=6.0):
    fwd = one_euro(x, fps, min_cut, beta)
    return one_euro(fwd[::-1], fps, min_cut, beta)[::-1]


def _smoothstep(x):
    x = np.clip(x, 0.0, 1.0)
    return x * x * (3.0 - 2.0 * x)


def foot_lock(L, names, fps, floor, height, speed=0.35, lift=0.08):
    """Pin low, slow ankles in x/z and move the ball by the same delta. Returns frames locked."""
    ix = {n: i for i, n in enumerate(names)}
    up_axis = 1
    locked = 0
    ramp = 4
    for side in "lr":
        ankle, ball = f"ankle_{side}", f"ball_{side}"
        if ankle not in ix:
            continue
        p = L[:, ix[ankle]].copy()
        v = np.linalg.norm(np.gradient(p, axis=0)[:, [0, 2]], axis=1) * fps
        low = (p[:, up_axis] - floor) < lift * height
        still = low & (v < speed * height)
        weight = np.zeros(len(p))
        target = np.zeros((len(p), 2))
        t = 0
        while t < len(p):
            if not still[t]:
                t += 1
                continue
            e = t
            while e + 1 < len(p) and still[e + 1]:
                e += 1
            if e - t >= 3:
                mean = p[t:e + 1][:, [0, 2]].mean(axis=0)
                for k in range(max(t - ramp, 0), min(e + ramp, len(p) - 1) + 1):
                    if k < t:
                        x = (k - (t - ramp)) / ramp
                    elif k > e:
                        x = ((e + ramp) - k) / ramp
                    else:
                        x = 1.0
                    w = float(_smoothstep(x))
                    if w >= weight[k]:
                        weight[k] = w
                        target[k] = mean
                locked += e - t + 1
            t = e + 1
        for k in range(len(p)):
            if weight[k] <= 0.0:
                continue
            old = p[k][[0, 2]]
            new = (1.0 - weight[k]) * old + weight[k] * target[k]
            delta = new - L[k, ix[ankle]][[0, 2]]
            L[k, ix[ankle]][[0, 2]] = new
            if ball in ix:
                L[k, ix[ball]][[0, 2]] = L[k, ix[ball]][[0, 2]] + delta
    return locked


def _track_jerk(x):
    if len(x) < 4:
        return 0.0
    return float(np.linalg.norm(np.diff(x, 3, axis=0), axis=1).mean())


def smooth(L, names, fps, floor, height, min_cut=4.0, beta=6.0):
    """L: T x N x 3. Returns (smoothed copy, frames locked)."""
    out = L.copy()
    for k in range(out.shape[1]):
        y = smooth_track(out[:, k], fps, min_cut, beta)
        if _track_jerk(y) <= _track_jerk(out[:, k]):
            out[:, k] = y
    locked = foot_lock(out, names, fps, floor, height)
    ix = {n: i for i, n in enumerate(names)}
    for j in ("ankle_l", "ankle_r", "ball_l", "ball_r"):
        if j not in ix:
            continue
        y = out[:, ix[j], 1]
        y[y < floor] = floor
    return out, locked


def metrics(L, names, fps, floor, height):
    """Numbers a catalogue can sort and filter by (all body-height normalised so rigs compare)."""
    ix = {n: i for i, n in enumerate(names)}
    T = len(L)
    jerk = np.mean([np.linalg.norm(np.diff(L[:, k], 3, axis=0), axis=1).mean() * fps ** 3 for k in range(L.shape[1])]) / height
    pelvis = L[:, ix["pelvis"]]
    reach = max(np.linalg.norm(L[:, ix[f"wrist_{s}"]] - pelvis, axis=1).max() for s in "lr") / height
    speed = max(np.linalg.norm(np.gradient(L[:, ix[f"wrist_{s}"]], axis=0), axis=1).max() * fps for s in "lr") / height
    kick = max(np.linalg.norm(np.gradient(L[:, ix[f"ankle_{s}"]], axis=0), axis=1).max() * fps for s in "lr") / height
    slips = []
    for s in "lr":
        p = L[:, ix[f"ankle_{s}"]]
        low = (p[:, 1] - floor) < 0.08 * height
        v = np.linalg.norm(np.gradient(p, axis=0)[:, [0, 2]], axis=1) * fps / height
        if low.any():
            slips.append(v[low].mean())
    return {"frames": T, "duration_s": round(T / fps, 3), "jerk": round(float(jerk), 1), "wrist_reach_h": round(float(reach), 3),
            "wrist_speed_hs": round(float(speed), 2), "ankle_speed_hs": round(float(kick), 2),
            "foot_slip_hs": round(float(np.mean(slips)) if slips else 0.0, 3)}
