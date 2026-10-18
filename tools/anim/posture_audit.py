#!/usr/bin/env python3
"""posture_audit.py - section 1: what the clip library's posture already does.

Every clip in ual1.glb and ual2.glb (and sekiro.glb where its bones map) is FK'd on its own skeleton
at 30 fps, and per frame the numbers in docs/HIGGS_REQUEST_ANIMATION.md section 1 are computed in
engine model space (+X the body's left, +Y up, +Z forward):

  lean, pitch          chest line (pelvis -> spine_03) off vertical, signed as asked
  shoulder_diff        left minus right shoulder height (m)
  com_offset           the pelvis/spine/head mean against the foot midpoint (lateral, forward)
  feet                 foot and ball height above the clip's lowest point, and contact flags
  hands                each hand's height and distance from the chest
  root_speed, turn     from the root joint, both per second

    ../venv/bin/python tools/anim/posture_audit.py --anim assets/anim --out docs/anim
"""
from __future__ import annotations

import argparse
import csv
import json
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from clip_fk import BONES, Clip, detect_axes, engine, metrics  # noqa: E402

FPS = 30.0
CONTACT_H = 0.02
CONTACT_V = 0.15


def audit(path, name, fps=FPS, gltf=None):
    clip = Clip(path, name, fps=fps, gltf=gltf)
    clip.precompute()                     # FK for every frame, once
    ax = detect_axes(clip)
    lowest = None
    first = metrics(clip, 0, ax)
    # lowest point of the whole clip, from the feet, so heights are relative to the ground the clip uses
    lows = []
    for f in range(clip.frames):
        w = clip.world(f)
        lows.append(min(clip.pos(w, "foot_l")[1], clip.pos(w, "foot_r")[1],
                        clip.pos(w, "ball_l")[1], clip.pos(w, "ball_r")[1]))
    lowest = min(lows)
    rows, prev = [], None
    for f in range(clip.frames):
        m = metrics(clip, f, ax, lowest=lowest)
        if prev is not None:
            dt = 1.0 / fps
            d = np.asarray(m["root"]["pos"], float) - np.asarray(prev["root"]["pos"], float)
            m["root"]["speed"] = float(np.linalg.norm(d) / dt)
            dy = (m["root"]["yaw"] - prev["root"]["yaw"] + np.pi) % (2 * np.pi) - np.pi
            m["root"]["turn"] = float(dy / dt)
        else:
            m["root"]["speed"] = 0.0
            m["root"]["turn"] = 0.0
        rows.append(m)
        prev = m
    out = {"duration": round(clip.duration, 3), "frames": clip.frames,
           "lean": _stat([r["lean"] for r in rows]),
           "pitch": _stat([r["pitch"] for r in rows]),
           "shoulder_diff": _stat([r["shoulder_diff"] for r in rows]),
           "com_offset": {"lat": _stat([r["com_off"][0] for r in rows]),
                          "fwd": _stat([r["com_off"][1] for r in rows])},
           "contacts": _contacts(clip, rows, ax, lowest),
           "planted": _planted(clip, lowest),
           "feet": {"l_ankle": _stat([r["feet"]["l"]["foot_h"] for r in rows]),
                    "r_ankle": _stat([r["feet"]["r"]["foot_h"] for r in rows]),
                    "l_clearance": _stat([min(r["feet"]["l"]["foot_h"], r["feet"]["l"]["ball_h"]) for r in rows]),
                    "r_clearance": _stat([min(r["feet"]["r"]["foot_h"], r["feet"]["r"]["ball_h"]) for r in rows])},
           "hands": {"l": _stat([r["hands"]["l"]["h"] for r in rows]),
                     "r": _stat([r["hands"]["r"]["h"] for r in rows])},
           "root_speed": _stat([r["root"]["speed"] for r in rows]),
           "turn_rate": _stat([r["root"]["turn"] for r in rows]),
           "_axes_detected": {"left_sign": ax["left_sign"],
                              "forward": [round(float(v), 3) for v in ax["forward"]],
                              "head_above_pelvis": round(ax["head_up"], 3)}}
    return out, clip, ax, lowest


def _stat(vals):
    a = np.asarray(vals, float)
    return [round(float(a.min()), 4), round(float(a.mean()), 4), round(float(a.max()), 4)]


def _contacts(clip, rows, ax, lowest):
    """Per frame: is each foot planted? Ground clearance < 2 cm and its speed < 0.15 m/s.

    Clearance is the LOWER of ankle and ball for that foot. Using the ankle alone reads every
    locomotion clip as airborne, because in this rig the ball sits 9 cm below the ankle and is what
    actually touches the floor.
    """
    out, last = [], None
    for f in range(clip.frames):
        w = clip.world(f)
        cur = {}
        for s in ("l", "r"):
            f_pos, b_pos = clip.pos(w, "foot_" + s), clip.pos(w, "ball_" + s)
            cur[s] = (f_pos, b_pos)
        pair = []
        for s in ("l", "r"):
            f_pos, b_pos = cur[s]
            h = min(f_pos[1], b_pos[1]) - lowest
            if last is None:
                v = 0.0
            else:
                lf, lb = last[s]
                v = float(min(np.linalg.norm(f_pos - lf), np.linalg.norm(b_pos - lb)) * FPS)
            pair.append(bool(h < CONTACT_H and v < CONTACT_V))
        out.append(pair)
        last = cur
    return out


def _planted(clip, lowest):
    """Ground clearance only: the foot is on the floor. Reported beside `contacts` because these
    libraries are TREADMILL clips - the planted foot slides at the locomotion speed in model space
    (about 0.98 m/s in Walk_Loop), so the brief's speed < 0.15 m/s test can only fire when the clip
    is stationary. The literal flag is kept as asked; this is the reading that works for walk/jog."""
    out = []
    for f in range(clip.frames):
        w = clip.world(f)
        pair = []
        for s in ("l", "r"):
            h = min(clip.pos(w, "foot_" + s)[1], clip.pos(w, "ball_" + s)[1]) - lowest
            pair.append(bool(h < CONTACT_H))
        out.append(pair)
    return out


def skeleton(axm, clip, frame, ax, lowest, plane):
    w = clip.world(frame)
    pts = {}
    for k in BONES:
        p = clip.pos(w, k)
        v = engine(np.array([p[0], p[1] - lowest, p[2]]), ax)
        pts[k] = (v[0], v[1], v[2])
    def xy(k):
        x, y, z = pts[k]
        return (x, y) if plane == "front" else (z, y)
    bones = [("root", "pelvis"), ("pelvis", "spine1"), ("spine1", "spine2"), ("spine2", "spine3"),
             ("spine3", "neck"), ("neck", "head"),
             ("spine3", "clavicle_l"), ("clavicle_l", "upperarm_l"), ("upperarm_l", "lowerarm_l"),
             ("lowerarm_l", "hand_l"),
             ("spine3", "clavicle_r"), ("clavicle_r", "upperarm_r"), ("upperarm_r", "lowerarm_r"),
             ("lowerarm_r", "hand_r"),
             ("pelvis", "thigh_l"), ("thigh_l", "calf_l"), ("calf_l", "foot_l"), ("foot_l", "ball_l"),
             ("pelvis", "thigh_r"), ("thigh_r", "calf_r"), ("calf_r", "foot_r"), ("foot_r", "ball_r")]
    return [(xy(a), xy(b)) for a, b in bones], pts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--anim", default="assets/anim")
    ap.add_argument("--out", default="docs/anim")
    ap.add_argument("--top", type=int, default=20)
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    sources = [("ual1.glb", "ual1"), ("ual2.glb", "ual2"), ("sekiro.glb", "sekiro")]
    data, rows = {}, []
    kept = {}
    for fname, tag in sources:
        path = os.path.join(args.anim, fname)
        if not os.path.isfile(path):
            print(f"  {fname}: not present, skipped")
            continue
        from pygltflib import GLTF2
        shared = GLTF2().load(path)                      # parsed once per file, not per clip
        names = [a.name for a in shared.animations]
        for nm in names:
            key = f"{tag}/{nm}"
            try:
                res, clip, ax, lowest = audit(path, nm, gltf=shared)
            except Exception as e:                                   # noqa: BLE001
                print(f"  {key}: skipped ({type(e).__name__}: {e})")
                continue
            data[key] = res
            kept[key] = (clip, ax, lowest)
            rows.append({"clip": key, "duration_s": res["duration"], "frames": res["frames"],
                         "lean_min": res["lean"][0], "lean_mean": res["lean"][1], "lean_max": res["lean"][2],
                         "pitch_min": res["pitch"][0], "pitch_mean": res["pitch"][1], "pitch_max": res["pitch"][2],
                         "shoulder_min": res["shoulder_diff"][0], "shoulder_mean": res["shoulder_diff"][1],
                         "shoulder_max": res["shoulder_diff"][2],
                         "com_lat_mean": res["com_offset"]["lat"][1], "com_fwd_mean": res["com_offset"]["fwd"][1],
                         "root_speed_mean": res["root_speed"][1], "turn_rate_mean": res["turn_rate"][1],
                         "contact_frames_l": sum(1 for c in res["contacts"] if c[0]),
                         "contact_frames_r": sum(1 for c in res["contacts"] if c[1]),
                         "planted_frames_l": sum(1 for c in res["planted"] if c[0]),
                         "planted_frames_r": sum(1 for c in res["planted"] if c[1])})
    with open(os.path.join(args.out, "posture_audit.json"), "w") as fh:
        json.dump(data, fh, indent=1)
    if not rows:
        raise SystemExit("no clips audited")
    with open(os.path.join(args.out, "posture_audit.csv"), "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(sorted(rows, key=lambda r: r["clip"]))
    # the note the section asks for
    lean_over = sorted([(k, v["lean"][1]) for k, v in data.items() if abs(v["lean"][1]) > 4.0], key=lambda t: -abs(t[1]))
    should_over = sorted([(k, v["shoulder_diff"][1]) for k, v in data.items()
                          if abs(v["shoulder_diff"][1]) > 0.03 and any(
                              t in k.lower() for t in ("idle", "walk", "jog", "run", "strafe"))], key=lambda t: -abs(t[1]))
    worst = sorted(data, key=lambda k: -max(abs(data[k]["lean"][1]), abs(data[k]["pitch"][1]),
                                            (data[k]["lean"][2] - data[k]["lean"][0]) / 2))[:args.top]
    _plot(kept, worst, os.path.join(args.out, "posture_audit_top.png"))
    note = {"clips_with_mean_lean_over_4deg": [[k, round(v, 2)] for k, v in lean_over],
            "idle_or_walk_with_shoulder_diff_over_3cm": [[k, round(v, 3)] for k, v in should_over],
            "clips_audited": len(data),
            "contact_criterion_caveat": ("These are treadmill clips: a foot can be at 0 cm clearance and still move at "
                                         "the locomotion speed in model space, so the brief's 'speed < 0.15 m/s' contact "
                                         "test only fires on stationary clips (all idle clips read 100% contact). "
                                         "`planted` in the JSON is clearance-only and is the usable flag for walk/jog."),
            "clip_sources": sorted({k.split('/')[0] for k in data})}
    with open(os.path.join(args.out, "posture_audit_note.json"), "w") as fh:
        json.dump(note, fh, indent=1)
    print(f"audited {len(data)} clips")
    print("  mean lean over 4 deg:", len(lean_over), "clips", [k for k, _ in lean_over[:6]])
    print("  idle/walk shoulder diff over 3 cm:", len(should_over), "clips", [k for k, _ in should_over[:6]])
    print("  top-lean/pitch clips plotted:", ", ".join(worst[:6]), "...")
    print(f"wrote posture_audit.json/.csv/_note.json and posture_audit_top.png to {args.out}")


def _plot(kept, worst, path, fps=FPS):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    rows, cols = len(worst), 6
    fig, axes = plt.subplots(rows, cols, figsize=(cols * 1.45, rows * 1.35), dpi=100,
                             facecolor="#0b1816")
    frames = (0, None, None)
    for r, key in enumerate(worst):
        clip, ax, lowest = kept[key]
        fs = [int(clip.frames * f) for f in (0.0, 0.5, 0.97)]
        for c in range(cols):
            plane = "front" if c < 3 else "side"
            frame = fs[c % 3]
            a = axes[r][c]
            a.set_facecolor("#0b1816")
            segs, _ = skeleton(None, clip, frame, ax, lowest, plane)
            for (p, q) in segs:
                a.plot([p[0], q[0]], [p[1], q[1]], color="#2EE6C5", lw=1.1)
            a.set_aspect("equal")
            a.set_xticks([])
            a.set_yticks([])
            for sp in a.spines.values():
                sp.set_color("#1d4a44")
            if c == 0:
                a.set_ylabel(key.split("/")[-1][:18], color="#D2FFF3", fontsize=5.5, rotation=0,
                             ha="right", va="center", labelpad=14)
            if r == 0:
                a.set_title(f"{plane} {c % 3 + 1}", color="#7CFFB2", fontsize=6)
    fig.suptitle("posture audit: the 20 clips with the largest natural lean or pitch "
                 "(front and side, 3 frames)", color="#D2FFF3", fontsize=9)
    fig.tight_layout(rect=(0.02, 0, 1, 0.985))
    fig.savefig(path, facecolor="#0b1816")
    plt.close(fig)


if __name__ == "__main__":
    main()
