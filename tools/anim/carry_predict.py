#!/usr/bin/env python3
"""carry_predict.py - section 3: the predicted E12 carry pose, drawn from the code.

A port of `crates/game/src/motion/carry.rs` `angles()` and `apply()` (branch ev/E12-carry). The
model-space `turn` helper is reproduced exactly as written: a bone's world rotation is rotated in
model space, then converted back to local through its parent, and every affected bone is computed
against the ORIGINAL forward kinematics before the pose is re-FK'd.

Weapon masses come from the branch's `assets/anim/movesets/*.json` (`weapon.mass`, `weapon.hand`);
the sheathed greatsword is the back load at 30 %.

    ../venv/bin/python tools/anim/carry_predict.py --anim assets/anim --out docs/anim
"""
from __future__ import annotations

import argparse
import json
import math
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from clip_fk import BONES, Clip, detect_axes, engine  # noqa: E402

# --- carried over from carry.rs verbatim -------------------------------------------------------
DROP_PER_KG = 0.03
DROP_MAX = 0.14
LEAN_PER_KG = 0.018
LEAN_MAX = 0.08
PITCH_BACK_PER_KG = 0.012
HEAVY = 2.0
PITCH_HELD_PER_KG = 0.025
PITCH_MAX = 0.12


def angles(hand, back=0.0):
    """carry::angles - drop per shoulder, torso lean (+ = toward the body's right), chest pitch."""
    h = [max(0.0, x) if math.isfinite(x) else 0.0 for x in hand]
    held = h[0] + h[1]
    drop = [min(m * DROP_PER_KG, DROP_MAX) for m in h]
    lean = max(-LEAN_MAX, min(LEAN_MAX, (h[0] - h[1]) * LEAN_PER_KG))
    back = back if math.isfinite(back) else 0.0
    pitch = min(PITCH_MAX, max(0.0, back) * PITCH_BACK_PER_KG + max(0.0, held - HEAVY) * PITCH_HELD_PER_KG)
    return {"drop": drop, "lean": lean, "pitch": pitch}


def rot_z(a):
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, -s, 0, 0], [s, c, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1.0]])


def rot_x(a):
    c, s = math.cos(a), math.sin(a)
    return np.array([[1, 0, 0, 0], [0, c, -s, 0], [0, s, c, 0], [0, 0, 0, 1.0]])


def apply_carry(clip, frame, a):
    """carry::apply on a copy of the clip's locals at `frame`; returns new local matrices."""
    if a["drop"] == [0.0, 0.0] and a["lean"] == 0.0 and a["pitch"] == 0.0:
        return None
    order = clip.order
    W = clip.W[frame]
    L = clip.L[frame].copy()
    target = {}

    def turn(bone, delta):
        i = clip.index[BONES[bone]]
        par = clip.parent.get(i)
        parent_rot = np.eye(4) if par is None else W[par].copy()
        parent_rot[:3, 3] = 0
        world_rot = W[i].copy()
        world_rot[:3, 3] = 0
        m = np.linalg.inv(parent_rot) @ delta @ world_rot
        if np.all(np.isfinite(m)):
            target[i] = m

    for side in (0, 1):
        if a["drop"][side] > 0.0:
            s = -1.0 if side == 0 else 1.0
            turn("clavicle_" + ("l" if side == 0 else "r"), rot_z(s * a["drop"][side]))
    delta = rot_z(a["lean"] * 0.5) @ rot_x(a["pitch"] * 0.5)
    for bone in ("spine2", "spine3"):                     # spine[1] and spine[2] in the Rust
        turn(bone, delta)
    for i, m in target.items():
        new = m.copy()
        new[:3, 3] = L[i][:3, 3]
        # The Rust pose stores a pure Quat, so take the rotation only: `parent^-1 * delta *
        # world_rot` already carries the inherited scale, and multiplying by the column norms
        # again damped the delta about 5x (the lean barely moved, which the checks caught).
        R = m[:3, :3]
        u, _, vt = np.linalg.svd(R)
        R = u @ vt
        if np.linalg.det(R) < 0:
            u[:, -1] *= -1
            R = u @ vt
        new[:3, :3] = R
        L[i] = new
    return L


def fk_from(clip, L):
    W = np.zeros_like(L)
    for i in clip.order:
        par = clip.parent.get(i)
        W[i] = L[i] if par is None else W[par] @ L[i]
    return W


def joint(W, clip, bone):
    """World position of a named bone from a world matrix stack."""
    return W[clip.index[BONES[bone]]][:3, 3]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--anim", default="assets/anim")
    ap.add_argument("--movesets", default="assets/anim/movesets")
    ap.add_argument("--out", default="docs/anim")
    ap.add_argument("--frames", type=int, default=3, help="walk frames sampled per row")
    args = ap.parse_args()

    # masses straight from the branch's movesets
    cases = []
    for names, label in ((("unarmed",), "unarmed"), (("dagger",), "dagger"), (("katana",), "katana"),
                         (("axe",), "axe"), (("greatsword",), "greatsword"), (("crossbow",), "crossbow")):
        for n in names:
            p = os.path.join(args.movesets, n + ".json")
            if not os.path.isfile(p):
                continue
            w = json.load(open(p)).get("weapon", {})
            mass = w.get("mass")
            hand = w.get("hand") or "r"
            if mass is None:                          # a moveset with no mass cannot carry
                print(f"  {n}: no weapon.mass, skipped")
                continue
            if label == "unarmed":
                cases.append((label, [0.0, 0.0], 0.0, "none", None))
            else:
                cases.append((label, [mass if hand == "l" else 0.0, mass if hand == "r" else 0.0], 0.0,
                              hand, mass))
    gs = os.path.join(args.movesets, "greatsword.json")
    gmass = json.load(open(gs)).get("weapon", {}).get("mass", 3.0) if os.path.isfile(gs) else 3.0
    cases.append(("greatsword sheathed", [0.0, 0.0], 0.30 * gmass, "back", 0.30 * gmass))

    clips = {}
    from pygltflib import GLTF2
    shared = GLTF2().load(os.path.join(args.anim, "ual1.glb"))
    for nm in ("Idle_Loop", "Walk_Loop"):
        c = Clip(os.path.join(args.anim, "ual1.glb"), nm, gltf=shared)
        c.precompute()
        clips[nm] = (c, detect_axes(c))

    report = {"_port": "crates/game/src/motion/carry.rs angles()/apply(), ev/E12-carry",
              "_constants": {"DROP_PER_KG": DROP_PER_KG, "DROP_MAX": DROP_MAX, "LEAN_PER_KG": LEAN_PER_KG,
                             "LEAN_MAX": LEAN_MAX, "PITCH_BACK_PER_KG": PITCH_BACK_PER_KG,
                             "PITCH_HELD_PER_KG": PITCH_HELD_PER_KG, "PITCH_MAX": PITCH_MAX},
              "_convention": "reported lean is the engine's: + = toward the body's right",
              "cases": {}}
    problems = []
    for label, hand, back, hname, mass in cases:
        a = angles(hand, back)
        entry = {"mass_kg": mass, "hand": hname, "hand_kg": hand, "back_kg": back,
                 "drop_deg": [round(math.degrees(x), 2) for x in a["drop"]],
                 "lean_deg": round(math.degrees(a["lean"]), 2),
                 "pitch_deg": round(math.degrees(a["pitch"]), 2)}
        for nm, (clip, ax) in clips.items():
            res = {}
            isolated = {}
            for f in (0, clip.frames // 2):
                W0 = clip.W[f]
                L2 = apply_carry(clip, f, a)
                W1 = fk_from(clip, L2) if L2 is not None else W0
                pel, sp3 = joint(W1, clip, "pelvis"), joint(W1, clip, "spine3")
                line = engine(sp3 - pel, ax)
                lean_engine = -math.degrees(math.atan2(line[0], max(line[1], 1e-6)))
                pitch = math.degrees(math.atan2(line[2], max(line[1], 1e-6)))
                base = engine(joint(W0, clip, "spine3") - joint(W0, clip, "pelvis"), ax)
                # The spec's metric is the chest LINE. A rotation at a bone's own origin does not
                # displace that bone, so the line moves far less than the rotation applied; the
                # upper-body effects are reported beside it, since that is what reads on screen.
                head0 = engine(joint(W0, clip, "head") - joint(W0, clip, "pelvis"), ax)
                head1 = engine(joint(W1, clip, "head") - joint(W1, clip, "pelvis"), ax)
                # upper chest segment (spine_03 -> head): the pitch is applied at spine_03, so this
                # segment tilts directly, unlike the head's absolute offset which the lean's
                # cross-term confounds at small angles
                up0 = engine(joint(W0, clip, "head") - joint(W0, clip, "spine3"), ax)
                up1 = engine(joint(W1, clip, "head") - joint(W1, clip, "spine3"), ax)
                up_pitch0 = math.degrees(math.atan2(up0[2], max(up0[1], 1e-6)))
                up_pitch1 = math.degrees(math.atan2(up1[2], max(up1[1], 1e-6)))
                sl0 = joint(W0, clip, "clavicle_l") - joint(W0, clip, "clavicle_r")
                sl1 = joint(W1, clip, "clavicle_l") - joint(W1, clip, "clavicle_r")
                s0, s1 = engine(sl0, ax), engine(sl1, ax)
                res[f] = {"lean_deg": round(lean_engine, 3),
                          "lean_base_deg": round(-math.degrees(math.atan2(base[0], max(base[1], 1e-6))), 3),
                          "head_offset_mm": round(float(head1[0] - head0[0]) * 1000, 2),
                          "head_fwd_mm": round(float(head1[2] - head0[2]) * 1000, 2),
                          "upper_pitch_deg_base": round(up_pitch0, 3),
                          "upper_pitch_deg": round(up_pitch1, 3),
                          "shoulder_tilt_deg_base": round(math.degrees(math.atan2(s0[1], max(abs(s0[0]), 1e-6))), 3),
                          "shoulder_tilt_deg": round(math.degrees(math.atan2(s1[1], max(abs(s1[0]), 1e-6))), 3),
                          "pitch_deg": round(pitch, 3),
                          "pitch_base_deg": round(math.degrees(math.atan2(base[2], max(base[1], 1e-6))), 3),
                          "foot_moved_m": float(max(np.linalg.norm(joint(W0, clip, "ball_l") - joint(W1, clip, "ball_l")),
                                                    np.linalg.norm(joint(W0, clip, "ball_r") - joint(W1, clip, "ball_r"))))}
            # isolate the two terms for the direction checks: together they cross-talk (a lateral
            # roll changes the segment's measured sagittal angle), so each is checked alone
            for tag, aa in (("pitch_only", {"drop": [0.0, 0.0], "lean": 0.0, "pitch": a["pitch"]}),
                            ("lean_only", {"drop": [0.0, 0.0], "lean": a["lean"], "pitch": 0.0})):
                L2 = apply_carry(clip, 0, aa)
                W0 = clip.W[0]
                W1 = fk_from(clip, L2) if L2 is not None else W0
                head0 = engine(joint(W0, clip, "head") - joint(W0, clip, "pelvis"), ax)
                head1 = engine(joint(W1, clip, "head") - joint(W1, clip, "pelvis"), ax)
                up0 = engine(joint(W0, clip, "head") - joint(W0, clip, "spine3"), ax)
                up1 = engine(joint(W1, clip, "head") - joint(W1, clip, "spine3"), ax)
                isolated[tag] = {
                    "head_offset_mm": round(float(head1[0] - head0[0]) * 1000, 2),
                    "upper_pitch_deg_base": round(math.degrees(math.atan2(up0[2], max(up0[1], 1e-6))), 3),
                    "upper_pitch_deg": round(math.degrees(math.atan2(up1[2], max(up1[1], 1e-6))), 3)}
            entry[nm + "_isolated"] = isolated
            entry[nm] = res
            # checks: lean away from the heavy side, feet unchanged, no flips
            iso = entry[nm + "_isolated"]
            if hand[1] > 0 and a["lean"] < 0 and not iso["lean_only"]["head_offset_mm"] > 0:
                problems.append(f"{label}/{nm}: heavy right hand but the upper body did not shift away from it "
                                f"(lean-only head offset {iso['lean_only']['head_offset_mm']} mm)")
            if hand[0] > 0 and a["lean"] > 0 and not iso["lean_only"]["head_offset_mm"] < 0:
                problems.append(f"{label}/{nm}: heavy left hand but the upper body did not shift away from it")
            if a["pitch"] > 0 and not iso["pitch_only"]["upper_pitch_deg"] > iso["pitch_only"]["upper_pitch_deg_base"]:
                problems.append(f"{label}/{nm}: pitch should tilt the upper chest forward "
                                f"({iso['pitch_only']['upper_pitch_deg_base']} -> {iso['pitch_only']['upper_pitch_deg']} deg)")
            if res[0]["foot_moved_m"] > 1e-9:
                problems.append(f"{label}/{nm}: a foot moved {res[0]['foot_moved_m']:.2e} m (it must not)")
            if L2 is not None:
                for i in clip.order:
                    m = L2[i][:3, :3]
                    if not np.all(np.isfinite(m)):
                        problems.append(f"{label}/{nm}: non-finite rotation on node {clip.node_names[i]}")
                        break
                    d = np.linalg.det(m)
                    if abs(d) < 1e-6 or not np.isfinite(d):
                        problems.append(f"{label}/{nm}: degenerate rotation on node {clip.node_names[i]}")
                        break
        report["cases"][label] = entry
    report["_checks"] = {"problems": problems or "none: lean goes away from the heavy hand in every case, "
                                                  "feet are untouched, no bone flips",
                         "feet_expected": "foot IK runs after carry and the offsets only touch spine and clavicles"}
    os.makedirs(args.out, exist_ok=True)
    with open(os.path.join(args.out, "carry_targets_from_code.json"), "w") as fh:
        json.dump(report, fh, indent=1)
    _plot(clips, cases, os.path.join(args.out, "carry_predicted.png"))

    print(f"{'case':22s} {'mass':>5s} {'hand':>5s} {'drop L/R (deg)':>16s} {'lean':>7s} {'pitch':>7s}   measured lean base -> carried (engine sign)")
    for label, hand, back, hname, mass in cases:
        e = report["cases"][label]
        meas = e["Idle_Loop"][0]
        print(f"{label:22s} {str(mass):>5s} {hname:>5s} {str(e['drop_deg']):>16s} {e['lean_deg']:>7.2f} "
              f"{e['pitch_deg']:>7.2f}   {meas['lean_base_deg']:>7.2f} -> {meas['lean_deg']:>7.2f}")
    print("checks:", report["_checks"]["problems"] if isinstance(report["_checks"]["problems"], str)
          else f"{len(problems)} problems: {problems[:4]}")
    print(f"wrote carry_targets_from_code.json and carry_predicted.png to {args.out}")


def _plot(clips, cases, path):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    rows, cols = len(cases), 4
    fig, axes = plt.subplots(rows, cols, figsize=(cols * 2.3, rows * 2.0), dpi=110, facecolor="#0b1816")
    segs = [("root", "pelvis"), ("pelvis", "spine1"), ("spine1", "spine2"), ("spine2", "spine3"),
            ("spine3", "neck"), ("neck", "head"),
            ("spine3", "clavicle_l"), ("clavicle_l", "upperarm_l"), ("upperarm_l", "lowerarm_l"),
            ("lowerarm_l", "hand_l"),
            ("spine3", "clavicle_r"), ("clavicle_r", "upperarm_r"), ("upperarm_r", "lowerarm_r"),
            ("lowerarm_r", "hand_r"),
            ("pelvis", "thigh_l"), ("thigh_l", "calf_l"), ("calf_l", "foot_l"), ("foot_l", "ball_l"),
            ("pelvis", "thigh_r"), ("thigh_r", "calf_r"), ("calf_r", "foot_r"), ("foot_r", "ball_r")]
    from clip_fk import metrics
    for r, (label, hand, back, hname, mass) in enumerate(cases):
        a = angles(hand, back)
        for c, (nm, plane) in enumerate((("Idle_Loop", "front"), ("Idle_Loop", "side"),
                                         ("Walk_Loop", "front"), ("Walk_Loop", "side"))):
            clip, ax = clips[nm]
            frame = 0 if nm == "Idle_Loop" else clip.frames // 3
            W0 = clip.W[frame]
            L2 = apply_carry(clip, frame, a)
            W1 = fk_from(clip, L2) if L2 is not None else W0
            ax_ = axes[r][c]
            ax_.set_facecolor("#0b1816")
            for W, col, lw in ((W0, "#5a6b68", 1.0), (W1, "#FFC857", 1.8)):
                for (b1, b2) in segs:
                    p = joint(W, clip, b1)
                    q = joint(W, clip, b2)
                    v1, v2 = engine(p, ax), engine(q, ax)
                    x1, y1 = (v1[0], v1[1]) if plane == "front" else (v1[2], v1[1])
                    x2, y2 = (v2[0], v2[1]) if plane == "front" else (v2[2], v2[1])
                    ax_.plot([x1, x2], [y1, y2], color=col, lw=lw)
            ax_.set_aspect("equal")
            ax_.set_xticks([]); ax_.set_yticks([])
            for sp in ax_.spines.values():
                sp.set_color("#1d4a44")
            if r == 0:
                ax_.set_title(f"{nm.split('_')[0]} {plane}", color="#7CFFB2", fontsize=7)
            if c == 0:
                ax_.set_ylabel(f"{label}\n{mass if mass else 0} kg", color="#D2FFF3", fontsize=6,
                               rotation=0, ha="right", va="center", labelpad=34)
    fig.suptitle("E12 carry predicted from motion/carry.rs: clip pose in grey, carried pose in gold",
                 color="#D2FFF3", fontsize=10)
    fig.tight_layout(rect=(0.03, 0, 1, 0.98))
    fig.savefig(path, facecolor="#0b1816")
    plt.close(fig)


if __name__ == "__main__":
    main()
