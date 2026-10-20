"""Kev motion features: what a clip does, measured from its forward kinematics, for clustering.

    python3 tools/kev/motion.py <clip dir> [--out staging/kev/motion] [--kmax 8]

Each GLB in the folder is sampled with clip_fk at 30 fps; a file with several animations gives one row
per animation. Bones are found by Kev's role rules (roles.json via nodes.classify), not by name, so any
rig nodes.py can parse works: UE / UAL, Mixamo, Kevin, Rigify, Auto-Rig Pro. The pelvis is the parent of
the left thigh. Five features, all scale-free:

  hand_speed   95th-percentile speed of either hand relative to the pelvis, in arm lengths per second
               (a percentile, not the peak: one bad frame should not decide what a clip is)
  reach        largest shoulder-to-hand distance, in arm lengths (shoulder > elbow > wrist chain at
               the first frame; 1.0 = arm straight)
  travel       pelvis ground speed over the clip, in leg lengths per second (a treadmill clip's root
               motion counts; an in-place loop reads near 0)
  foot_lift    highest foot above the clip's lowest foot point, in leg lengths (jumps, kicks)
  extend       range of the shoulder-to-hand distance over the clip, in arm lengths: a punch or swing opens
               the arm from guard to straight, a stance, a walk or a fall barely does
  level        pelvis height change from first to last frame, in leg lengths (signed: a fall or a sit is
               negative, a get-up positive); falls, sits and get-ups move the hips a long way vertically, strikes do not
  returns      1 when the last pose matches the first (mean joint angle under 3 deg), else 0: a loop,
               or a one-shot that comes back to its stance

Each clip's `category` comes from rules on its own features (`name_cluster`: strike, fall-or-rise,
jump-or-kick, locomotion, transition, idle, gesture), so a label does not move when other clips are added.
The clips are also clustered with Kev's k-means on the standardised features (`cluster`, named from its
centroid by the same rules) to show which groups the data itself forms. Stability is checked the same way
survey.py does, so the result can be compared with the file-level clustering.
"""
import argparse
import json
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(REPO, "tools", "anim"))

import kev  # noqa: E402
import nodes  # noqa: E402
import survey  # noqa: E402

FEATURES = ["hand_speed", "reach", "extend", "travel", "foot_lift", "level", "returns"]
# Strike rules, read off the 84 ActionForge clips (tools/kev/results/motion/report.md):
# - a swing moves the hands fast with the arm already long: hook 44, sword swings 23-47, overhand 36,
#   shield bash 14 arm lengths / s; deaths, sits and recoveries stay at 2-11 (a knockback reaches 18 but
#   drops the hips, which LEVEL catches)
# - a thrust opens the arm a long way at modest speed: jab 0.53 and cross 0.57 arm lengths of extension
#   at 6-9 / s; idles and gestures open it 0.15-0.37, the mech's gun recoil 0.45
# - a run pumps the arms as fast as a swing (mech run 17.5) but lifts the feet 0.9 leg lengths; the
#   sword combos that hop stay under 0.6
SWING_SPEED = 14.0
SWING_FOOT_LIFT = 0.8
THRUST_EXTEND = 0.5
# Pelvis rise or fall (leg lengths) that marks a level change: a fall, a sit or a get-up. A lunge dips
# the hips up to about 0.4 and still counts as a strike.
LEVEL = 0.3
STRIKE_LEVEL = 0.4
LOOP_DEG = 3.0


# Canonical key -> (role, side) in nodes.py terms.
CANON = {
    "upperarm_l": ("upperarm", "l"), "lowerarm_l": ("forearm", "l"), "hand_l": ("hand", "l"),
    "upperarm_r": ("upperarm", "r"), "lowerarm_r": ("forearm", "r"), "hand_r": ("hand", "r"),
    "thigh_l": ("thigh", "l"), "calf_l": ("shin", "l"), "foot_l": ("foot", "l"),
    "thigh_r": ("thigh", "r"), "calf_r": ("shin", "r"), "foot_r": ("foot", "r"),
}


def bone_map(node_names, parent, rules=None):
    """Canonical key -> node index: the first node of each role and side, in file order (the first
    segment of a split limb). Raises KeyError naming what is missing."""
    rules = rules or nodes.load_rules()
    found = {}
    for i, name in enumerate(node_names):
        role_side = nodes.classify(name or "", rules)
        for key, want in CANON.items():
            if key not in found and role_side == want:
                found[key] = i
    missing = sorted(set(CANON) - set(found))
    if missing:
        raise KeyError(f"no bone for {missing}")
    found["pelvis"] = parent.get(found["thigh_l"], found["thigh_l"])
    return found


def features(path, name=None, gltf=None):
    """The five motion features of one clip (`name` defaults to the file's first animation)."""
    from clip_fk import Clip, GLTF2

    g = gltf or GLTF2().load(path)
    name = name or g.animations[0].name
    c = Clip(path, name, gltf=g)
    c.precompute()
    fps = c.fps
    bones = bone_map(c.node_names, c.parent)
    P = lambda w, b: np.asarray(w[bones[b]][:3, 3], float)  # noqa: E731
    w0 = c.world(0)
    arm = (np.linalg.norm(P(w0, "upperarm_l") - P(w0, "lowerarm_l")) + np.linalg.norm(P(w0, "lowerarm_l") - P(w0, "hand_l"))) or 1.0
    leg = (np.linalg.norm(P(w0, "thigh_l") - P(w0, "calf_l")) + np.linalg.norm(P(w0, "calf_l") - P(w0, "foot_l"))) or 1.0
    hands = {s: [] for s in "lr"}
    shoulder_reach, pelvis, feet = [], [], []
    reach_by_side = {s: [] for s in "lr"}
    for f in range(c.frames):
        w = c.world(f)
        pv = P(w, "pelvis")
        pelvis.append(pv)
        for s in "lr":
            h = P(w, f"hand_{s}")
            hands[s].append(h - pv)
            shoulder_reach.append(np.linalg.norm(h - P(w, f"upperarm_{s}")))
            reach_by_side[s].append(shoulder_reach[-1])
        feet.append(min(P(w, "foot_l")[1], P(w, "foot_r")[1]))
    speed = max(float(np.percentile(np.linalg.norm(np.diff(np.array(hands[s]), axis=0), axis=1), 95)) * fps if c.frames > 1 else 0.0 for s in "lr")
    pelvis = np.array(pelvis)
    path_len = float(np.sum(np.linalg.norm(np.diff(pelvis[:, [0, 2]], axis=0), axis=1))) if c.frames > 1 else 0.0
    duration = max(c.duration, 1e-3)
    feet = np.array(feet)
    high = max(P(c.world(f), "foot_l")[1] for f in range(c.frames))
    high = max(high, max(P(c.world(f), "foot_r")[1] for f in range(c.frames)))
    first, last = c.local(0), c.local(c.frames - 1)
    loop = mean_rotation_angle(first, last) < LOOP_DEG
    return {
        "clip": name,
        "duration_s": round(duration, 3),
        "hand_speed": round(speed / arm, 3),
        "reach": round(float(max(shoulder_reach)) / arm, 3),
        "extend": round(max(max(v) - min(v) for v in reach_by_side.values()) / arm, 3),
        "travel": round(path_len / duration / leg, 3),
        "foot_lift": round(float(high - feet.min()) / leg, 3),
        "level": round(float(pelvis[-1][1] - pelvis[0][1]) / leg, 3),
        "returns": 1.0 if loop else 0.0,
    }


def mean_rotation_angle(a, b):
    """Mean angle (deg) between two lists of local transforms' rotations (matrices or Transform-likes)."""
    angles = []
    for ma, mb in zip(a, b):
        ra, rb = np.asarray(ma)[:3, :3], np.asarray(mb)[:3, :3]
        r = ra.T @ rb
        cosv = (np.trace(r) - 1.0) / 2.0
        angles.append(np.degrees(np.arccos(np.clip(cosv, -1.0, 1.0))))
    return float(np.mean(angles)) if angles else 0.0


def name_cluster(c):
    """A readable label from features (one clip's, or a cluster centroid's, in raw units). Strikes are
    tested first: a lunge travels and dips, but its hands give it away."""
    level = abs(c.get("level", 0.0))
    swing = c["hand_speed"] >= SWING_SPEED and level <= STRIKE_LEVEL and c["foot_lift"] < SWING_FOOT_LIFT
    thrust = c.get("extend", 0.0) >= THRUST_EXTEND and level < LEVEL and c["foot_lift"] < 0.3
    if swing or thrust:
        return "strike"
    if level > LEVEL:
        return "fall-or-rise"
    if c["foot_lift"] > 0.5:
        return "jump-or-kick"
    if c["travel"] > 1.0:
        return "locomotion"
    if c["returns"] < 0.5:
        return "transition"
    if c["hand_speed"] < 1.0:
        return "idle"
    return "gesture"


def cluster(rows, kmax, seed=1):
    x = np.array([[r[f] for f in FEATURES] for r in rows], float)
    z = kev.standardise(x)
    labels, k = kev.choose_clusters(z, kmax, seed)
    names = {}
    for lab in sorted(set(labels)):
        members = x[labels == lab]
        centroid = dict(zip(FEATURES, members.mean(axis=0)))
        base = name_cluster(centroid)
        n = sum(1 for v in names.values() if v.split("-")[0] == base.split("-")[0])
        names[lab] = base if n == 0 else f"{base}-{n + 1}"
    for r, lab in zip(rows, labels):
        r["cluster"] = names[lab]
        r["category"] = name_cluster(r)
    stab = survey.stability(z, seed) if len(rows) >= 6 else None
    return k, names, stab


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("clips")
    ap.add_argument("--out", default=os.path.join(REPO, "staging", "kev", "motion"))
    ap.add_argument("--kmax", type=int, default=8)
    args = ap.parse_args()
    rows = []
    from clip_fk import GLTF2

    for f in sorted(os.listdir(args.clips)):
        if not f.endswith(".glb"):
            continue
        path = os.path.join(args.clips, f)
        g = GLTF2().load(path)
        for anim in g.animations:
            try:
                rows.append({"file": f, **features(path, anim.name, g)})
            except Exception as exc:  # a clip without the canonical bones is reported, not fatal
                print(f"skip {f}:{anim.name}: {type(exc).__name__} {exc}")
    k, names, stab = cluster(rows, args.kmax)
    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, "motion.json"), "w") as fh:
        json.dump({"features": FEATURES, "k": int(k), "stability": stab, "clips": rows}, fh, indent=1)
    counts = {}
    for r in rows:
        counts.setdefault(r["category"], []).append(r["clip"])
    print(f"{len(rows)} clips, k={k}")
    for cat, clips in sorted(counts.items()):
        print(f"  {cat:<16} {len(clips):>3}  e.g. {', '.join(clips[:5])}")
    if stab:
        print("stability (median adjusted Rand vs full run): " + ", ".join(f"{int(s['fraction'] * 100)}%={s['median_ari']}" for s in stab))


if __name__ == "__main__":
    main()
