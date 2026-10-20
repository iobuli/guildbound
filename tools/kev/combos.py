"""Kev combos: find clips that chain well, by how well one clip's last pose matches another's first.

    python3 tools/kev/combos.py <clip dir> <out dir> [--length 4] [--top 5]

Every GLB is read at its first and last two keys: each joint's rotation there, and how it is moving
(the rotation between the two keys, as an axis-angle vector). A transition from clip A to clip B
scores as two terms, both over the joints both clips drive:
  pose      mean angle (deg) between A's last pose and B's first pose
  velocity  mean difference (deg per key) between A's end motion and B's start motion
Low on both is a clean cut that keeps the momentum; the score is what a crossfade would have to hide.

Writes <out dir>/combos.json: for each clip its best next clips, and a greedy chain of `--length`
clips starting from each clip with the lowest total transition score. Stdlib plus numpy only.
"""
import argparse
import json
import math
import os
import re
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from survey import floats, read_glb  # noqa: E402


def unit(q):
    return q / max(np.linalg.norm(q), 1e-9)


def quat_mul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return np.array([
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ])


def rotvec(q_from, q_to):
    """Axis-angle of the rotation that takes q_from to q_to (vector length = angle, rad)."""
    a, b = unit(q_from), unit(q_to)
    if np.dot(a, b) < 0:
        b = -b
    rel = quat_mul(b, np.array([-a[0], -a[1], -a[2], a[3]]))
    w = min(1.0, abs(rel[3]))
    s = math.sqrt(max(1e-12, 1 - w * w))
    return rel[:3] / s * (2 * math.acos(w))


def anim_ends(js, binary, anim, names):
    """(first, last) of one animation: per joint, (pose quaternion, motion vector in rad per key)."""
    first, last = {}, {}
    for ch in anim["channels"]:
        if ch["target"].get("path") != "rotation":
            continue
        sampler = anim["samplers"][ch["sampler"]]
        q = floats(js, binary, sampler["output"])
        if len(q) < 2:
            continue
        joint = names[ch["target"]["node"]]
        first[joint] = (unit(q[0]), rotvec(q[0], q[1]))
        last[joint] = (unit(q[-1]), rotvec(q[-2], q[-1]))
    return first, last


def clip_ends(path):
    """{animation name: (first, last)} for every animation in a GLB (a library holds many clips)."""
    js, binary = read_glb(path)
    names = [n.get("name", f"node{i}") for i, n in enumerate(js["nodes"])]
    return {a.get("name", f"anim{i}"): anim_ends(js, binary, a, names) for i, a in enumerate(js.get("animations", []))}


def pose_ends(path):
    """(first, last) of a one-clip GLB; with several clips, later clips overwrite earlier joints."""
    first, last = {}, {}
    for f, l in clip_ends(path).values():
        first.update(f)
        last.update(l)
    return first, last


# Arm joints across rig families (UE / UAL, Mixamo, Kevin).
ARM = re.compile(r"(upperarm|lowerarm|hand_|clavicle|Arm|ForeArm|Hand$|hand\.|forearm|shoulder)")
# Mean arm motion (deg per key) at a clip's end above which it ends mid-swing. Measured: clips that
# ship with a `_Rec` recovery (Melee_Hook 4.0, Sword_Regular_A 3.6) sit far above it; settled ends
# (Punch_Jab 0.15, the _Rec clips, idles) stay under 0.6.
MID_SWING_DEG = 2.0


def mid_swing(last):
    """True when a clip ends with its arms still moving fast: a strike that wants its recovery clip,
    not a place to cut into the next clip."""
    v = [np.linalg.norm(last[j][1]) for j in last if ARM.search(j)]
    return bool(v) and math.degrees(float(np.mean(v))) > MID_SWING_DEG


def angle(a, b):
    d = abs(float(np.dot(a, b)))
    return math.degrees(2 * math.acos(min(1.0, d)))


def transition(last_a, first_b):
    """(pose, velocity) of a cut from A's last to B's first, over the joints both drive. Degrees."""
    common = [j for j in last_a if j in first_b]
    if not common:
        return None
    pose = float(np.mean([angle(last_a[j][0], first_b[j][0]) for j in common]))
    velocity = float(np.mean([math.degrees(np.linalg.norm(last_a[j][1] - first_b[j][1])) for j in common]))
    return pose, velocity


def total(t):
    """One number for a cut: pose error plus motion mismatch."""
    return t[0] + t[1]


def build_matrix(clips):
    names = sorted(clips)
    first = {n: clips[n][0] for n in names}
    last = {n: clips[n][1] for n in names}
    score = {}
    for a in names:
        for b in names:
            if a != b:
                t = transition(last[a], first[b])
                if t is not None:
                    score[(a, b)] = total(t)
    return names, score


def best_next(names, score, top):
    out = {}
    for a in names:
        options = sorted(((score[(a, b)], b) for b in names if (a, b) in score))[:top]
        out[a] = [{"next": b, "degrees": round(s, 2)} for s, b in options]
    return out


def chain(start, names, score, length, stops=frozenset()):
    """Greedy: at each step take the cheapest unused next clip. A clip in `stops` (ends mid-swing)
    may close a chain but nothing is cut onto its end."""
    path, total, used = [start], 0.0, {start}
    while len(path) < length and path[-1] not in stops:
        options = [(score[(path[-1], b)], b) for b in names if b not in used and (path[-1], b) in score]
        if not options:
            break
        s, b = min(options)
        path.append(b)
        used.add(b)
        total += s
    return {"clips": path, "total_degrees": round(total, 2)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("clips")
    ap.add_argument("out")
    ap.add_argument("--length", type=int, default=4)
    ap.add_argument("--top", type=int, default=5)
    args = ap.parse_args()

    clips = {}
    for name in sorted(os.listdir(args.clips)):
        if name.endswith(".glb"):
            first, last = pose_ends(os.path.join(args.clips, name))
            if first:
                clips[os.path.splitext(name)[0]] = (first, last)
    if len(clips) < 2:
        raise SystemExit(f"need at least two clips with rotation keys under {args.clips}")

    names, score = build_matrix(clips)
    stops = frozenset(n for n in names if mid_swing(clips[n][1]))
    chains = sorted((chain(n, names, score, args.length, stops) for n in names), key=lambda c: c["total_degrees"])
    result = {
        "rule": {"score": "mean joint angle (deg), last pose of A vs first pose of B", "length": args.length},
        "clips": len(names),
        "best_next": best_next(names, score, args.top),
        "chains": chains[: args.top * 2],
        "mid_swing_ends": sorted(stops),
    }
    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, "combos.json"), "w") as f:
        json.dump(result, f, indent=1)
    print(f"{len(names)} clips scored, {len(score)} transitions")
    for c in result["chains"][: args.top]:
        print(f"  {c['total_degrees']:7.1f} deg  " + " > ".join(c["clips"]))


if __name__ == "__main__":
    main()
