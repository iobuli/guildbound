"""Small vector helpers and a cubic hermite sampler. Tangents are in units per second."""
import numpy as np


def unit(v):
    v = np.asarray(v, float)
    n = float(np.linalg.norm(v))
    return v / n if n > 1e-9 else v


def rot_axis(v, axis, deg):
    """Right-handed rotation of v around axis (Rodrigues)."""
    a = unit(axis)
    th = np.radians(deg)
    c, s = np.cos(th), np.sin(th)
    v = np.asarray(v, float)
    return v * c + np.cross(a, v) * s + a * np.dot(a, v) * (1 - c)


def hermite(p0, p1, v0, v1, dt, u):
    """Position at fraction u. v0 and v1 are velocities (units per second). dt is the segment length in seconds."""
    p0, p1 = np.asarray(p0, float), np.asarray(p1, float)
    m0, m1 = np.asarray(v0, float) * dt, np.asarray(v1, float) * dt
    u2, u3 = u * u, u * u * u
    h00 = 2 * u3 - 3 * u2 + 1
    h10 = u3 - 2 * u2 + u
    h01 = -2 * u3 + 3 * u2
    h11 = u3 - u2
    return h00 * p0 + h10 * m0 + h01 * p1 + h11 * m1


def _speed_cap(keys, i, get, v):
    """A tangent faster than the neighbouring segments makes one frame swallow the whole strike."""
    v = np.asarray(v, float)
    caps = []
    for j in (i - 1, i + 1):
        dt = abs(keys[j]["t"] - keys[i]["t"])
        if dt > 1e-6:
            dist = float(np.linalg.norm(np.asarray(get(keys[j]), float) - np.asarray(get(keys[i]), float)))
            caps.append(1.4 * dist / dt)
    if not caps:
        return v
    cap = max(caps)
    n = float(np.linalg.norm(v))
    if n > cap > 1e-8:
        v = v * (cap / n)
    return v if np.ndim(v) else float(v)


def catmull(keys, i, get):
    """Velocity at key i from the neighbouring values, capped so the spline cannot spike."""
    z = keys[i]["zero"]
    if z is None:
        return 0.0
    if i == 0 or i == len(keys) - 1:
        return 0.0
    dt = keys[i + 1]["t"] - keys[i - 1]["t"]
    if dt <= 1e-8:
        return 0.0
    return _speed_cap(keys, i, get, (get(keys[i + 1]) - get(keys[i - 1])) / dt)


def _held(key, zero_name):
    z = key["zero"]
    if z is None:
        return True
    return zero_name is not None and zero_name in z


def sample_channel(keys, get, zero_name, fps):
    """Sample one scalar or vector channel. A held key (zero is None, or zero_name is in the set) has a 0 tangent."""
    t1 = keys[-1]["t"]
    n = int(round(t1 * fps)) + 1
    times = [k["t"] for k in keys]
    vels = []
    for i, k in enumerate(keys):
        g = get(k)
        if _held(k, zero_name) or i == 0 or i == len(keys) - 1:
            vels.append(np.zeros(3) if np.ndim(g) else 0.0)
        else:
            vels.append(catmull(keys, i, get))
    out = []
    for f in range(n):
        t = min(t1, f / fps)
        hi = 1
        while hi < len(times) - 1 and times[hi] < t:
            hi += 1
        lo = hi - 1
        span = times[hi] - times[lo]
        u = 0.0 if span <= 1e-8 else (t - times[lo]) / span
        out.append(hermite(get(keys[lo]), get(keys[hi]), vels[lo], vels[hi], span, u))
    return out
