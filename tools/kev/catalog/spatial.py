"""Spatial smoothing for Kev references: landmark trajectories (T x N x 3, metres, fixed fps) in, smoother ones out.

Three passes, all offline so they can look both ways in time:
  1. adaptive zero-phase filter per landmark: a One-Euro low-pass whose cutoff rises with the landmark's own speed,
     run forward then backward. Slow drift (idle sway, keyframe seams) is flattened, fast strikes keep their peaks.
  2. foot lock: ankle/ball runs that are low and slow are pinned to their mean ground position (no slide under a plant).
  3. limb lengths: each limb segment is relaxed toward its median length (parent first), so the solver is not asked
     for a stretched arm.
"""
import numpy as np

LIMBS = [("shoulder_l", "elbow_l"), ("elbow_l", "wrist_l"), ("shoulder_r", "elbow_r"), ("elbow_r", "wrist_r"),
         ("hip_l", "knee_l"), ("knee_l", "ankle_l"), ("hip_r", "knee_r"), ("knee_r", "ankle_r"),
         ("ankle_l", "ball_l"), ("ankle_r", "ball_r")]


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


def foot_lock(L, names, fps, floor, height, speed=0.35, lift=0.08):
    """Pin low, slow feet in x/z. Returns the number of frames locked."""
    ix = {n: i for i, n in enumerate(names)}
    up_axis = 1
    locked = 0
    for side in "lr":
        for j in (f"ankle_{side}", f"ball_{side}"):
            if j not in ix:
                continue
            p = L[:, ix[j]]
            v = np.linalg.norm(np.gradient(p, axis=0)[:, [0, 2]], axis=1) * fps
            low = (p[:, up_axis] - floor) < lift * height
            still = low & (v < speed * height)
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
                    for k in range(t, e + 1):
                        L[k, ix[j]][[0, 2]] = mean
                    locked += e - t + 1
                t = e + 1
    return locked


def relax_limbs(L, names, strength=0.8):
    ix = {n: i for i, n in enumerate(names)}
    for a, b in LIMBS:
        if a not in ix or b not in ix:
            continue
        d = L[:, ix[b]] - L[:, ix[a]]
        n = np.linalg.norm(d, axis=1, keepdims=True)
        target = np.median(n)
        fixed = d / np.maximum(n, 1e-9) * target
        L[:, ix[b]] = L[:, ix[a]] + d + strength * (fixed - d)


def smooth(L, names, fps, floor, height, min_cut=4.0, beta=6.0):
    """L: T x N x 3. Returns (smoothed copy, frames locked)."""
    out = L.copy()
    for k in range(out.shape[1]):
        out[:, k] = smooth_track(out[:, k], fps, min_cut, beta)
    locked = foot_lock(out, names, fps, floor, height)
    relax_limbs(out, names)
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
