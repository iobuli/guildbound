"""Before/after numbers for a landmark sequence. Same thresholds the pass plants with."""
import numpy as np

from . import body

PLANT = 0.02
OFF_FOOT = 0.01
HALF_WIDTH = 0.045


def _series(frames, rest):
    out = []
    for fr in frames:
        lm, _ = body.enrich(fr, rest)
        out.append(lm)
    return out


def _speed(a, b, dt):
    return float(np.linalg.norm(body.xz(a) - body.xz(b))) / dt


def measure(frames, dt, floor, rest=None):
    """Slip, floor penetration, balance violations, jerk, root travel, required friction.

    Slip is the horizontal speed of a sole point while it is within 2 cm of the floor (a planted
    foot that is skating). Balance counts a grounded frame whose centre of mass misses the shoe
    outline by more than 1 cm. Friction is the horizontal COM acceleration, in g, while grounded.
    """
    dt = float(dt) if dt and dt > 0 else 1.0 / 30.0
    seq = _series(frames, rest)
    if len(seq) < 2:
        return {"frames": len(seq), "slip_mps": 0.0, "penetration_mm": 0.0, "balance_violations": 0,
                "ground_frames": 0, "com_off_mean_m": 0.0, "jerk": 0.0, "pelvis_travel_m": 0.0,
                "pelvis_path_m": 0.0, "mu_required": 0.0}
    slip_n, slip_sum, slip_max = 0, 0.0, 0.0
    pen_max, pen_sum, pen_n = 0.0, 0.0, 0
    violations, ground, off_sum = 0, 0, 0.0
    coms = []
    for i, lm in enumerate(seq):
        coms.append(body.centre_of_mass(lm))
        planted = []
        for s in "lr":
            key, p = body.lowest_sole(lm, s)
            if p is None:
                continue
            depth = floor - float(p[1])
            if depth > 0:
                pen_max = max(pen_max, depth)
                pen_sum += depth
                pen_n += 1
            if float(p[1]) <= floor + PLANT:
                planted.append(s)
            if i == 0:
                continue
            for key in body.sole_keys(s):
                if key not in lm or key not in seq[i - 1]:
                    continue
                if float(lm[key][1]) > floor + PLANT or float(seq[i - 1][key][1]) > floor + PLANT:
                    continue
                sp = _speed(lm[key], seq[i - 1][key], dt)
                slip_sum += sp
                slip_max = max(slip_max, sp)
                slip_n += 1
        if planted:
            ground += 1
            poly = body.support_polygon(lm, planted, HALF_WIDTH)
            _, dist, inside = body.closest_on_poly(poly, body.xz(coms[-1]))
            off = 0.0 if inside else dist
            off_sum += off
            if off > OFF_FOOT:
                violations += 1
    mu = 0.0
    for i in range(2, len(coms)):
        v0 = body.xz(coms[i - 1] - coms[i - 2]) / dt
        v1 = body.xz(coms[i] - coms[i - 1]) / dt
        mu = max(mu, float(np.linalg.norm(v1 - v0)) / dt / body.G)
    jerk = _jerk(seq, dt)
    pelvis = [lm["pelvis"] for lm in seq if "pelvis" in lm]
    if len(pelvis) >= 2:
        travel = float(np.linalg.norm(body.xz(pelvis[-1]) - body.xz(pelvis[0])))
        path = sum(float(np.linalg.norm(body.xz(b) - body.xz(a))) for a, b in zip(pelvis, pelvis[1:]))
    else:
        travel, path = 0.0, 0.0
    return {
        "frames": len(seq),
        "slip_mps": round(slip_sum / slip_n, 4) if slip_n else 0.0,
        "slip_max_mps": round(slip_max, 4),
        "penetration_mm": round(pen_max * 1000, 3),
        "penetration_mean_mm": round((pen_sum / pen_n) * 1000, 3) if pen_n else 0.0,
        "balance_violations": violations,
        "ground_frames": ground,
        "com_off_mean_m": round(off_sum / ground, 4) if ground else 0.0,
        "jerk": round(jerk, 1),
        "pelvis_travel_m": round(travel, 4),
        "pelvis_path_m": round(path, 4),
        "mu_required": round(mu, 3),
    }


def _jerk(seq, dt):
    names = ("wrist_l", "wrist_r", "head_joint", "ankle_l", "ankle_r")
    total, n = 0.0, 0
    for name in names:
        if any(name not in lm for lm in seq):
            continue
        p = np.array([lm[name] for lm in seq], float)
        if len(p) < 4:
            continue
        j = np.diff(p, n=3, axis=0) / dt ** 3
        total += float(np.linalg.norm(j, axis=1).mean())
        n += 1
    return total / n if n else 0.0
