"""Quality numbers for a solved combo. Gates catch pops, stretched limbs, and feet that slide while planted.

Jerk uses the same height-normalised third difference as tools/kev/catalog/spatial.py so a combo can be
compared with a catalogue clip. Anime strikes stay sharp, and jerk is gated so a chain cannot be
rougher than a hard catalogue hit.
The gates are the glitches: a one-frame joint pop, a bone that changes length, a planted foot that moves.
"""
import math

import numpy as np

PAIRS = (("upperarm_l", "lowerarm_l"), ("lowerarm_l", "hand_l"), ("upperarm_r", "lowerarm_r"), ("lowerarm_r", "hand_r"),
         ("thigh_l", "calf_l"), ("calf_l", "foot_l"), ("thigh_r", "calf_r"), ("calf_r", "foot_r"),
         ("foot_l", "ball_l"), ("foot_r", "ball_r"))

# Jerk is the height-normalised third difference. A raw jab measures about 640 and a raw sword
# swing about 1650. Chains of anime strikes sit between those, so the gate is the sword swing:
# a combo may be sharper than one punch and must not be rougher than that hit.
GATES = {"max_joint_step_deg": 52.0, "segment_drift_cm": 1.5, "plant_slip_cm": 1.5, "reach_mean_cm": 3.0, "reach_max_cm": 8.0,
         "jerk": 1700.0}


def _quat_angle(a, b):
    a = np.asarray(a, float)
    b = np.asarray(b, float)
    a = a / max(np.linalg.norm(a), 1e-9)
    b = b / max(np.linalg.norm(b), 1e-9)
    d = min(1.0, abs(float(np.dot(a, b))))
    return math.degrees(2 * math.acos(d))


def _horiz(p, up):
    p = np.asarray(p, float)
    return p - up * float(np.dot(p, up))


def measure(info, samples, solved, fps):
    """solved[i] = {landmarks, joints, quats, reach, balance}."""
    up = np.array(info["bm"]["body"]["up"], float)
    up = up / np.linalg.norm(up)
    H = float(info["bm"]["body"]["height"])
    names = sorted(samples[0]["landmarks"])
    L = np.array([[solved[i]["landmarks"][n] for n in names] for i in range(len(solved))], float)
    jerk = 0.0
    if len(L) > 3:
        jerk = float(np.mean([np.linalg.norm(np.diff(L[:, k], 3, axis=0), axis=1).mean() for k in range(L.shape[1])])) * fps ** 3 / H

    # joint angle step
    bones = sorted(solved[0]["quats"])
    worst = (0.0, bones[0], 0)
    steps = []
    for i in range(1, len(solved)):
        for b in bones:
            if b not in solved[i]["quats"] or b not in solved[i - 1]["quats"]:
                continue
            ang = _quat_angle(solved[i - 1]["quats"][b], solved[i]["quats"][b])
            steps.append(ang)
            if ang > worst[0]:
                worst = (ang, b, i)
    # segment lengths against the rig rest
    rest_w = info["rest_world"]
    index = info["index"]
    drifts = []
    worst_seg = (0.0, "")
    for a, b in PAIRS:
        if a not in index or b not in index:
            continue
        rest = float(np.linalg.norm(rest_w[index[a]][:3, 3] - rest_w[index[b]][:3, 3]))
        for fr in solved:
            if a not in fr["joints"] or b not in fr["joints"]:
                continue
            length = float(np.linalg.norm(np.array(fr["joints"][a]) - np.array(fr["joints"][b])))
            d = abs(length - rest) * 100
            drifts.append(d)
            if d > worst_seg[0]:
                worst_seg = (d, f"{a}-{b}")

    # planted foot: deviation of the measured ankle/ball from the median of each planted run
    slip = []
    target_slip = []
    for side in "lr":
        for joint in (f"ankle_{side}", f"ball_{side}"):
            run = []
            def close(run):
                if len(run) < 4:
                    return
                meas = np.array([_horiz(solved[i]["landmarks"][joint], up) for i in run])
                targ = np.array([_horiz(samples[i]["landmarks"][joint], up) for i in run])
                med_m, med_t = np.median(meas, axis=0), np.median(targ, axis=0)
                slip.append(float(np.linalg.norm(meas - med_m, axis=1).max()) * 100)
                target_slip.append(float(np.linalg.norm(targ - med_t, axis=1).max()) * 100)
            for i, s in enumerate(samples):
                if joint in s["plant"]:
                    run.append(i)
                else:
                    close(run)
                    run = []
            close(run)

    reaches = []
    for fr in solved:
        for k, v in fr["reach"].items():
            if k.startswith("fingers"):
                continue
            reaches.append(v * 100)
    over = sum(1 for fr in solved if fr["balance"].get("over_foot")) / max(1, len(solved))
    pel = np.array([solved[i]["landmarks"]["pelvis"] for i in range(len(solved))])
    travel = float(np.linalg.norm(_horiz(pel[-1], up) - _horiz(pel[0], up)))
    out = {
        "frames": len(solved),
        "duration_s": round(samples[-1]["t"], 3),
        "jerk": round(jerk, 1),
        "max_joint_step_deg": round(worst[0], 2),
        "max_joint_step_bone": worst[1],
        "max_joint_step_frame": worst[2],
        "mean_joint_step_deg": round(float(np.mean(steps)) if steps else 0.0, 3),
        "segment_drift_cm": round(worst_seg[0], 3),
        "segment_drift_pair": worst_seg[1],
        "plant_slip_cm": round(max(slip) if slip else 0.0, 3),
        "target_plant_slip_cm": round(max(target_slip) if target_slip else 0.0, 3),
        "reach_mean_cm": round(float(np.mean(reaches)) if reaches else 0.0, 3),
        "reach_max_cm": round(max(reaches) if reaches else 0.0, 3),
        "over_foot": round(over, 3),
        "root_travel_m": round(travel, 3),
    }
    fails = []
    if out["max_joint_step_deg"] > GATES["max_joint_step_deg"]:
        fails.append(f"joint pop {out['max_joint_step_deg']} deg on {out['max_joint_step_bone']} @ {out['max_joint_step_frame']}")
    if out["segment_drift_cm"] > GATES["segment_drift_cm"]:
        fails.append(f"segment drift {out['segment_drift_cm']} cm ({out['segment_drift_pair']})")
    if out["plant_slip_cm"] > GATES["plant_slip_cm"]:
        fails.append(f"planted foot slip {out['plant_slip_cm']} cm")
    if out["reach_mean_cm"] > GATES["reach_mean_cm"]:
        fails.append(f"mean reach {out['reach_mean_cm']} cm")
    if out["reach_max_cm"] > GATES["reach_max_cm"]:
        fails.append(f"max reach {out['reach_max_cm']} cm")
    if out["jerk"] > GATES["jerk"]:
        fails.append(f"jerk {out['jerk']} over {GATES['jerk']}")
    out["ok"] = not fails
    out["fails"] = fails
    return out
