"""Build the keys of one move: a wind-up, a snapped strike, a hit-stop, an overshoot, and a follow-through.

Each key is an absolute landmark pose. The sampler (compile.py) hermite-blends them, so the join
from the previous move is the previous pose and there is no pop. Planted feet are the same point
on every key of the strike, with a zero tangent, so they do not slide.
"""
import numpy as np

from .rig import other

HAND = {"fist": 1.0, "guard": 0.70, "relaxed": 0.28, "open": 0.02, "cast": 0.10, "claw": 0.60}
DEFAULT_DUR = {"antic": 0.14, "strike": 0.09, "hold": 0.05, "overshoot": 0.05, "follow": 0.16}


def hand_value(v, fallback):
    if isinstance(v, (int, float)):
        return float(v)
    return HAND.get(v, fallback)


def hands_of(spec, default):
    spec = spec or {}
    return {"l": hand_value(spec.get("l", default[0]), 0.7), "r": hand_value(spec.get("r", default[1]), 0.7)}


def durations(move):
    """Ramp shortens the strike, but nothing is allowed to collapse into a one-frame pop."""
    d = dict(DEFAULT_DUR)
    d.update(move.get("dur") or {})
    ramp = max(0.45, float(move.get("ramp", 1.0)))
    d["antic"] = max(0.08, d["antic"] / ramp ** 0.5)
    d["strike"] = max(0.09, d["strike"] / ramp)
    d["hold"] = max(0.045, d["hold"] / ramp)
    d["overshoot"] = max(0.04, d["overshoot"] / ramp)
    d["follow"] = max(0.07, d["follow"] / ramp)
    return d


def _key(t, pose, hands, zero, plant, tag):
    return {"t": round(t, 4), "pose": pose, "hands": hands, "zero": zero, "plant": set(plant), "tag": tag}


def _feet(side):
    return {f"ankle_{side}", f"ball_{side}"}


def _both():
    return _feet("l") | _feet("r")


def _locks(rig, entry, move, catch=0.45):
    """Plant foot steps fully and pivots; the other foot stays back until the follow-through."""
    plant = move.get("plant", "l")
    free = other(plant)
    step = float(move.get("step", 0.12))
    pivot = float(move.get("pivot", 0.0))
    pitch = min(26.0, abs(pivot) * 0.45) if pivot else float(move.get("pitch", 0.0))
    ball_p = rig.step_ball(entry, plant, step)
    ankle_p = rig.ankle_from_ball(ball_p, plant, yaw=pivot, pitch=pitch)
    ball_b = rig.step_ball(entry, free, step * catch)
    ankle_b = rig.ankle_from_ball(ball_b, free)
    ball_c = rig.step_ball(entry, free, step * float(move.get("catch", 1.0)))
    ankle_c = rig.ankle_from_ball(ball_c, free)
    return plant, free, (ankle_p, ball_p), (ankle_b, ball_b), (ankle_c, ball_c)


def _stamp(pose, side, ankle, ball):
    pose[f"ankle_{side}"] = np.array(ankle, float).copy()
    pose[f"ball_{side}"] = np.array(ball, float).copy()


def _dirs_from(src, rig):
    elbows, knees = {}, {}
    for s in "lr":
        elbows[s] = src[f"elbow_{s}"] - src[f"shoulder_{s}"]
        hint = src[f"knee_{s}"] - 0.5 * (src[f"hip_{s}"] + src[f"ankle_{s}"])
        knees[s] = hint if np.linalg.norm(hint) > 0.02 else rig.fwd
    return elbows, knees


def _assemble(rig, src, pelvis, plant, free, lock, free_foot, elbows, knees, caps, support):
    pose = rig.shape_on(src, pelvis)
    _stamp(pose, plant, *lock)
    _stamp(pose, free, *free_foot)
    rig.drop_to_fit(pose, support)
    for s, frac in caps.items():
        rig.clamp_wrist(pose, s, frac)
    for s, direction in elbows.items():
        rig.seat_elbow(pose, s, direction)
    for s, direction in knees.items():
        rig.seat_knee(pose, s, direction)
    for s in "lr":
        if s not in support:
            rig.clamp_ankle(pose, s, 0.93)
    # the drop and the clamp must not move a locked plant foot
    _stamp(pose, plant, *lock)
    if free in support:
        _stamp(pose, free, *free_foot)
    return pose


def _pelvis_from_clip(src, plant, lock_ball, rig):
    return lock_ball + (src["pelvis"] - src[f"ball_{plant}"])


def _bulge(rig, pose, a, b, limb, amount, elbow_dir):
    """Bow the striking limb off the straight line. The elbow pole stays on the same side as the rest of the strike."""
    if amount <= 0:
        return pose
    d = b[limb] - a[limb]
    side = np.cross(rig.up, d)
    if np.linalg.norm(side) < 1e-4:
        side = rig.left
    pose = rig.copy(pose)
    pose[limb] = pose[limb] + _unit(side) * amount * 0.35 + rig.up * amount * 0.65
    rig.clamp_wrist(pose, limb[-1], 0.96)
    rig.seat_elbow(pose, limb[-1], elbow_dir)
    return pose


def _unit(v):
    n = float(np.linalg.norm(v))
    return v / n if n > 1e-9 else v


def _push(rig, pose, limb, a, b, frac_cap):
    pose = rig.copy(pose)
    d = b[limb] - a[limb]
    dist = float(np.linalg.norm(d))
    if dist < 1e-4:
        return pose
    pose[limb] = b[limb] + _unit(d) * min(0.07, max(0.03, 0.14 * dist))
    rig.clamp_wrist(pose, limb[-1], frac_cap)
    return pose


def _phases(t, dur, built, mid):
    """built: name -> (pose, hands, zero, plant). Order is the strike."""
    order = ("set", "via", "hit", "hold", "overshoot", "follow")
    dts = (dur["antic"], dur["strike"] * mid, dur["strike"] * (1 - mid), dur["hold"], dur["overshoot"], dur["follow"])
    keys = []
    for name, dt in zip(order, dts):
        t += max(dt, 1 / 60)
        pose, hands, zero, plant = built[name]
        keys.append(_key(t, pose, hands, zero, plant, name))
    return keys


def _tag(keys, ident):
    for k in keys:
        k["tag"] = f"{ident}:{k['tag']}"
    return keys


def build_clip_strike(rig, move, entry_key):
    """A catalogue clip supplies the arm shape. Feet step, plant, and (optionally) pivot on the ball."""
    do = move["do"]
    clip = move.get("clip") or ("Punch_Jab" if do == "jab" else "Punch_Cross")
    limb = move.get("limb") or ("wrist_l" if do != "cross" else "wrist_r")
    if clip not in rig.info["clips"]:
        return build_straight(rig, move, entry_key, limb[-1])
    entry = entry_key["pose"]
    plant, free, lock, back, catch = _locks(rig, entry, move)
    guard_c, load_c, peak_c, fol_c = rig.clip_strike(clip, limb)
    elbows_l, knees_l = _dirs_from(load_c, rig)
    elbows_p, knees_p = _dirs_from(peak_c, rig)
    elbows_f, knees_f = _dirs_from(fol_c, rig)
    caps = {"l": 0.96, "r": 0.96}
    support = (plant, free)
    ident = move.get("id", do)
    hs = hands_of(move.get("hands"), ("fist" if limb.endswith("l") else "guard", "fist" if limb.endswith("r") else "guard"))

    def make(src, free_foot, elbows, knees):
        pel = _pelvis_from_clip(src, plant, lock[1], rig)
        return _assemble(rig, src, pel, plant, free, lock, free_foot, elbows, knees, caps, support)

    set_p = make(load_c, back, elbows_l, knees_l)
    hit_p = make(peak_c, back, elbows_p, knees_p)
    via_src = rig.mix(load_c, peak_c, 0.42)
    via_p = make(via_src, back, elbows_p, knees_p)
    via_p = _bulge(rig, via_p, set_p, hit_p, limb, float(move.get("arc", 0.04)), elbows_p[limb[-1]])
    _stamp(via_p, plant, *lock)
    _stamp(via_p, free, *back)
    over_p = _push(rig, hit_p, limb, set_p, hit_p, 0.98)
    rig.seat_elbow(over_p, limb[-1], elbows_p[limb[-1]])
    _stamp(over_p, plant, *lock)
    _stamp(over_p, free, *back)
    fol_p = make(fol_c, catch, elbows_f, knees_f)
    _stamp(fol_p, plant, *lock)
    hold_p = rig.copy(hit_p)
    feet = _both()
    lead = _feet(plant)
    z_feet = feet
    built = {
        "set": (set_p, hs, z_feet, feet),
        "via": (via_p, hs, z_feet, feet),
        "hit": (hit_p, hs, None, feet),
        "hold": (hold_p, hs, None, feet),
        "overshoot": (over_p, hs, z_feet | {limb}, feet),
        "follow": (fol_p, hs, lead, lead),
    }
    return _tag(_phases(entry_key["t"], durations(move), built, 0.42), ident)


def build_straight(rig, move, entry_key, side):
    """Procedural straight punch used when the source clip is not on this model."""
    move = dict(move)
    move["hand"] = side
    return build_hook(rig, move, entry_key, straight=True)


def _yaw_upper(rig, pose, yaw):
    if abs(yaw) < 0.5:
        return pose
    spun = rig.yaw_about(pose, pose["pelvis"], yaw,
                         ("neck", "shoulder_l", "shoulder_r", "elbow_l", "elbow_r", "wrist_l", "wrist_r", "crown", "nose", "head_joint"))
    hips = rig.yaw_about(pose, pose["pelvis"], yaw * 0.65, ("hip_l", "hip_r"))
    spun["hip_l"], spun["hip_r"] = hips["hip_l"], hips["hip_r"]
    return spun


def _procedural_body(rig, entry, plant, free, lock, free_foot, src, yaw, pelvis_y, support, caps, elbows, knees):
    pel = lock[1] + (src["pelvis"] - src[f"ball_{plant}"])
    pel = rig.set_up(pel, pelvis_y)
    pose = _assemble(rig, src, pel, plant, free, lock, free_foot, elbows, knees, caps, support)
    pose = _yaw_upper(rig, pose, yaw)
    _stamp(pose, plant, *lock)
    if free in support:
        _stamp(pose, free, *free_foot)
    rig.drop_to_fit(pose, support)
    _stamp(pose, plant, *lock)
    if free in support:
        _stamp(pose, free, *free_foot)
    for s, frac in caps.items():
        rig.clamp_wrist(pose, s, frac)
    return pose


def build_hook(rig, move, entry_key, straight=False):
    side = move.get("hand", "l")
    entry = entry_key["pose"]
    plant, free, lock, back, catch = _locks(rig, entry, move, catch=0.1)
    g = rig.guard
    sign = 1.0 if side == "l" else -1.0
    cap = 0.97 if straight else 0.94
    caps = {"l": cap, "r": cap}
    hs = hands_of(move.get("hands"), ("fist" if side == "l" else "guard", "fist" if side == "r" else "guard"))
    elbows, knees = _dirs_from(g, rig)
    out_dir = rig.left * sign
    elbows[side] = rig.up * (0.15 if straight else 1.0) + out_dir * (0.35 if straight else 0.85) - rig.fwd * 0.1
    support = (plant, free)
    base_y = rig.up_of(g["pelvis"])
    arc = 0.0 if straight else float(move.get("arc", 0.07))

    def wrist_at(pose, forward, across, up, back_m=0.0):
        return pose["nose"] + rig.fwd * forward - out_dir * across + rig.up * up - rig.fwd * back_m

    def shaped(yaw, y, free_foot, forward, across, up, back_m=0.0):
        pose = _procedural_body(rig, entry, plant, free, lock, free_foot, g, yaw * sign, base_y + y, support, caps, elbows, knees)
        pose[f"wrist_{side}"] = wrist_at(pose, forward, across, up, back_m)
        rig.clamp_wrist(pose, side, cap)
        rig.seat_elbow(pose, side, elbows[side])
        _stamp(pose, plant, *lock)
        _stamp(pose, free, *free_foot)
        return pose

    if straight:
        set_p = shaped(-8, -0.02, back, 0.02, -0.02, -0.08, 0.10)
        hit_p = shaped(8, 0.0, back, 0.36, 0.02, 0.02)
    else:
        # hand starts outside the lead shoulder and finishes past the centre line, elbow kept high
        set_p = shaped(-28, -0.03, back, -0.02, -0.22, 0.0)
        hit_p = shaped(42, 0.01, back, 0.05, 0.34, 0.05)
    via_p = rig.mix(set_p, hit_p, 0.5)
    _stamp(via_p, plant, *lock)
    _stamp(via_p, free, *back)
    via_p = _bulge(rig, via_p, set_p, hit_p, f"wrist_{side}", arc if arc else (0.035 if straight else 0.06), elbows[side])
    _stamp(via_p, plant, *lock)
    _stamp(via_p, free, *back)
    over_p = _push(rig, hit_p, f"wrist_{side}", set_p, hit_p, cap)
    rig.seat_elbow(over_p, side, elbows[side])
    _stamp(over_p, plant, *lock)
    _stamp(over_p, free, *back)
    fol_p = shaped(8, 0.0, catch, 0.08, 0.0, 0.0)
    hold_p = rig.copy(hit_p)
    feet = _both()
    limb = f"wrist_{side}"
    built = {
        "set": (set_p, hs, feet, feet),
        "via": (via_p, hs, feet, feet),
        "hit": (hit_p, hs, None, feet),
        "hold": (hold_p, hs, None, feet),
        "overshoot": (over_p, hs, feet | {limb}, feet),
        "follow": (fol_p, hs, _feet(plant), _feet(plant)),
    }
    return _tag(_phases(entry_key["t"], durations(move), built, 0.4), move.get("id", "hook"))


def build_uppercut(rig, move, entry_key):
    """Rising fist. Heels lift onto the ball of the foot and the pelvis rises with the punch (the launcher)."""
    side = move.get("hand", "r")
    entry = entry_key["pose"]
    move = dict(move)
    move.setdefault("pivot", 18.0)
    move.setdefault("pitch", 22.0)
    plant, free, lock, back, catch = _locks(rig, entry, move, catch=0.05)
    # rear heel lifts too, a little less, so the launch reads in both legs
    pitch_f = float(move.get("pitch", 22.0)) * 0.65
    ball_b, ankle_b = back[1], rig.ankle_from_ball(back[1], free, yaw=0, pitch=pitch_f)
    back = (ankle_b, ball_b)
    g = rig.guard
    sign = 1.0 if side == "l" else -1.0
    caps = {"l": 0.9, "r": 0.9}
    hs = hands_of(move.get("hands"), ("fist" if side == "l" else "guard", "fist" if side == "r" else "guard"))
    elbows, knees = _dirs_from(g, rig)
    elbows[side] = -rig.up + rig.fwd * 0.35 + rig.left * sign * 0.2
    support = (plant, free)
    base_y = rig.up_of(g["pelvis"])

    def shaped(yaw, y, free_foot, wrist):
        pose = _procedural_body(rig, entry, plant, free, lock, free_foot, g, yaw * sign, base_y + y, support, caps, elbows, knees)
        pose[f"wrist_{side}"] = wrist(pose)
        rig.clamp_wrist(pose, side, 0.9)
        rig.seat_elbow(pose, side, elbows[side])
        _stamp(pose, plant, *lock)
        _stamp(pose, free, *free_foot)
        return pose

    set_p = shaped(-16, -0.06, back, lambda p: p["pelvis"] + rig.fwd * 0.14 + rig.up * 0.12 + rig.left * sign * 0.1)
    hit_p = shaped(10, 0.09, back, lambda p: p["nose"] + rig.fwd * 0.16 + rig.up * 0.08 - rig.left * sign * 0.02)
    via_p = rig.mix(set_p, hit_p, 0.45)
    _stamp(via_p, plant, *lock)
    _stamp(via_p, free, *back)
    via_p[f"wrist_{side}"] = via_p[f"wrist_{side}"] + rig.fwd * 0.04
    rig.clamp_wrist(via_p, side, 0.9)
    rig.seat_elbow(via_p, side, elbows[side])
    over_p = rig.copy(hit_p)
    over_p[f"wrist_{side}"] = hit_p[f"wrist_{side}"] + rig.up * 0.05 + rig.fwd * 0.02
    rig.clamp_wrist(over_p, side, 0.92)
    rig.seat_elbow(over_p, side, elbows[side])
    # land the heels, catch the back foot, fist stays up
    fol_ball = catch[1]
    fol_ankle = rig.ankle_from_ball(fol_ball, free)
    fol_lock_ankle = rig.ankle_from_ball(lock[1], plant)
    fol_p = shaped(4, 0.0, (fol_ankle, fol_ball), lambda p: p["nose"] + rig.fwd * 0.10 + rig.up * 0.02)
    _stamp(fol_p, plant, fol_lock_ankle, lock[1])
    hold_p = rig.copy(hit_p)
    balls = {f"ball_{plant}", f"ball_{free}"}
    ankles = {f"ankle_{plant}", f"ankle_{free}"}
    planted = balls | ankles
    built = {
        "set": (set_p, hs, planted, planted),
        "via": (via_p, hs, planted, planted),
        "hit": (hit_p, hs, None, planted),
        "hold": (hold_p, hs, None, planted),
        "overshoot": (over_p, hs, planted | {f"wrist_{side}"}, planted),
        "follow": (fol_p, hs, balls | _feet(plant), balls | _feet(plant)),
    }
    return _tag(_phases(entry_key["t"], durations(move), built, 0.38), move.get("id", "uppercut"))


def _kick_pelvis(rig, plant_ball, height):
    """Stack the plant-side hip over the ball of the stance foot (single-foot support is only ~5 cm wide)."""
    # hip offset is applied later by shape_on; shift the pelvis so that hip lands on the ball.
    return rig.set_up(plant_ball, height)


def _kick_pose(rig, src, yaw, pelvis, plant, lock, kick, ankle, ball, knee_dir, elbow_dir, caps, wrist=None):
    elbows, knees = _dirs_from(src, rig)
    elbows.update(elbow_dir or {})
    knees[kick] = knee_dir
    knees[plant] = rig.fwd
    pose = rig.shape_on(src, pelvis)
    pose = _yaw_upper(rig, pose, yaw)
    _stamp(pose, plant, *lock)
    pose[f"ankle_{kick}"] = np.array(ankle, float)
    pose[f"ball_{kick}"] = np.array(ball, float)
    rig.drop_to_fit(pose, (plant,))
    _stamp(pose, plant, *lock)
    rig.clamp_ankle(pose, kick, 0.92)
    for s, frac in caps.items():
        rig.clamp_wrist(pose, s, frac)
    for s in "lr":
        rig.seat_elbow(pose, s, elbows[s])
        rig.seat_knee(pose, s, knees[s])
    if wrist:
        for s, w in wrist.items():
            pose[f"wrist_{s}"] = w(pose)
            rig.clamp_wrist(pose, s, caps.get(s, 0.96))
            rig.seat_elbow(pose, s, elbows[s])
    _stamp(pose, plant, *lock)
    return pose


def build_roundhouse(rig, move, entry_key):
    kick = move.get("foot", "r")
    plant = move.get("plant", other(kick))
    entry = entry_key["pose"]
    move = dict(move, plant=plant)
    plant_s, free, lock, back, catch = _locks(rig, entry, move, catch=0.0)
    assert free == kick
    g = rig.guard
    sign = 1.0 if kick == "r" else -1.0  # outward
    outward = -rig.left * sign
    caps = {"l": 0.95, "r": 0.95}
    hs = hands_of(move.get("hands"), ("guard", "guard"))
    base = rig.up_of(g["pelvis"])
    hip_off = rig.horiz(g[f"hip_{plant}"] - g["pelvis"])

    def pel(y):
        return rig.set_up(rig.horiz(lock[1]) - hip_off + rig.fwd * 0.02, base + y)

    def ankle_ball(pose, reach, lift, lateral):
        hip = pose[f"hip_{kick}"]
        aim = _unit(rig.fwd * reach + outward * lateral + rig.up * lift)
        ank = hip + aim * rig.leg * 0.9
        ball = ank + _unit(aim + rig.up * 0.15) * rig.foot
        return ank, ball

    # coil: knee already up on the kicking hip, so the strike extends instead of swinging the foot off the floor
    set_pel = pel(-0.04)
    coil_tmp = _kick_pose(rig, g, -12 * sign, set_pel, plant, lock, kick, lock[0], lock[1], rig.up, None, caps)
    coil_ank = coil_tmp[f"hip_{kick}"] + rig.up * 0.30 + rig.fwd * 0.08 + outward * 0.08
    coil_ball = coil_ank + rig.fwd * rig.foot * 0.7 + rig.up * 0.02
    set_p = _kick_pose(rig, g, -12 * sign, set_pel, plant, lock, kick, coil_ank, coil_ball,
                       rig.up * 0.8 + rig.fwd * 0.35 + outward * 0.2, None, caps)
    # chamber
    via_pel = pel(-0.02)
    via_tmp = _kick_pose(rig, g, 36 * sign, via_pel, plant, lock, kick,
                         lock[0], lock[1], rig.up + outward * 0.3, None, caps)
    ank = via_tmp[f"hip_{kick}"] + rig.fwd * 0.22 + rig.up * 0.16 + outward * 0.16
    ball = ank + rig.fwd * rig.foot
    via_p = _kick_pose(rig, g, 36 * sign, via_pel, plant, lock, kick, ank, ball, rig.up * 0.55 + outward * 0.55 + rig.fwd * 0.35, None, caps)
    # extend
    hit_tmp = _kick_pose(rig, g, 58 * sign, pel(0.02), plant, lock, kick, lock[0], lock[1], outward + rig.up * 0.2, None, caps)
    ank, ball = ankle_ball(hit_tmp, 0.82, 0.22, 0.38)
    hit_p = _kick_pose(rig, g, 58 * sign, pel(0.02), plant, lock, kick, ank, ball, outward * 0.8 + rig.up * 0.35 + rig.fwd * 0.3, None, caps)
    over_p = rig.copy(hit_p)
    over_p[f"ankle_{kick}"] = hit_p[f"ankle_{kick}"] + _unit(hit_p[f"ankle_{kick}"] - hit_p[f"hip_{kick}"]) * 0.04
    rig.clamp_ankle(over_p, kick, 0.95)
    rig.seat_knee(over_p, kick, outward + rig.up * 0.2)
    # land the kicking foot forward of the plant, flat, and square the hips back toward the target
    land_ball = rig.step_ball({"ball_" + kick: lock[1] + rig.fwd * 0.34 + outward * 0.12}, kick, 0.0)
    # step_ball reads entry[ball_side]; pass a fake entry
    land_ball = rig.set_up(lock[1] + rig.fwd * 0.36 + outward * 0.1, rig.up_of(g[f"ball_{kick}"]))
    land_ankle = rig.ankle_from_ball(land_ball, kick)
    fol_p = _kick_pose(rig, g, 12 * sign, pel(-0.03), plant, lock, kick, land_ankle, land_ball, rig.fwd, None, caps)
    hold_p = rig.copy(hit_p)
    lead = _feet(plant)
    built = {
        "set": (set_p, hs, lead, lead),
        "via": (via_p, hs, lead, lead),
        "hit": (hit_p, hs, None, lead),
        "hold": (hold_p, hs, None, lead),
        "overshoot": (over_p, hs, lead, lead),
        "follow": (fol_p, hs, lead, lead),
    }
    return _tag(_phases(entry_key["t"], durations(move), built, 0.46), move.get("id", "roundhouse"))


def build_spin_back(rig, move, entry_key):
    """Turn on the ball of the stance foot and throw the other heel forward through a full spin."""
    kick = move.get("foot", "r")
    plant = move.get("plant", other(kick))
    entry = entry_key["pose"]
    move = dict(move, plant=plant)
    move.setdefault("pivot", 90.0)
    move.setdefault("step", move.get("step", 0.06))
    _, _, lock, _, _ = _locks(rig, entry, move, catch=0.0)
    g = rig.guard
    sign = 1.0 if kick == "r" else -1.0
    outward = -rig.left * sign
    caps = {"l": 0.95, "r": 0.95}
    hs = hands_of(move.get("hands"), ("guard", "guard"))
    base = rig.up_of(g["pelvis"])
    hip_off = rig.horiz(g[f"hip_{plant}"] - g["pelvis"])

    def pel(y, along=0.0):
        return rig.set_up(rig.horiz(lock[1]) - hip_off + rig.fwd * along, base + y)

    def ext(pose, reach, lift, lateral):
        hip = pose[f"hip_{kick}"]
        aim = _unit(rig.fwd * reach + outward * lateral + rig.up * lift)
        ank = hip + aim * rig.leg * 0.9
        ball = ank + _unit(rig.fwd + rig.up * 0.25) * rig.foot
        return ank, ball

    # chamber stays on the kicking hip the whole spin, so the thigh turns instead of swinging through the floor
    coil_tmp = _kick_pose(rig, g, -20 * sign, pel(-0.04), plant, lock, kick, lock[0], lock[1], rig.up, None, caps)
    coil_ank = coil_tmp[f"hip_{kick}"] + rig.up * 0.22 + outward * 0.14 - rig.fwd * 0.06
    coil_ball = coil_ank + rig.fwd * rig.foot * 0.55 + rig.up * 0.02
    coil = _kick_pose(rig, g, -20 * sign, pel(-0.04), plant, lock, kick, coil_ank, coil_ball,
                      rig.up * 0.7 + outward * 0.45, None, caps)
    mid_tmp = _kick_pose(rig, g, 50 * sign, pel(0.0, 0.04), plant, lock, kick, lock[0], lock[1], rig.up, None, caps)
    ank = mid_tmp[f"hip_{kick}"] + rig.up * 0.28 + rig.fwd * 0.16 + outward * 0.1
    ball = ank + rig.fwd * rig.foot * 0.7
    via_p = _kick_pose(rig, g, 50 * sign, pel(0.0, 0.04), plant, lock, kick, ank, ball, rig.up * 0.75 + outward * 0.4 + rig.fwd * 0.2, None, caps)
    hit_tmp = _kick_pose(rig, g, 145 * sign, pel(0.03, 0.08), plant, lock, kick, lock[0], lock[1], outward, None, caps)
    ank, ball = ext(hit_tmp, 0.9, 0.18, 0.12)
    hit_p = _kick_pose(rig, g, 165 * sign, pel(0.03, 0.08), plant, lock, kick, ank, ball, outward * 0.5 + rig.up * 0.45 + rig.fwd * 0.4, None, caps)
    over_p = rig.copy(hit_p)
    over_p[f"ankle_{kick}"] = hit_p[f"ankle_{kick}"] + rig.fwd * 0.04
    rig.clamp_ankle(over_p, kick, 0.95)
    rig.seat_knee(over_p, kick, outward * 0.4 + rig.up * 0.4)
    land_ball = rig.set_up(lock[1] + rig.fwd * 0.42, rig.up_of(g[f"ball_{kick}"]))
    land_ankle = rig.ankle_from_ball(land_ball, kick)
    fol_p = _kick_pose(rig, g, 18 * sign, pel(-0.04, 0.1), plant, lock, kick, land_ankle, land_ball, rig.fwd, None, caps)
    hold_p = rig.copy(hit_p)
    lead = _feet(plant)
    built = {
        "set": (coil, hs, lead, lead),
        "via": (via_p, hs, lead, lead),
        "hit": (hit_p, hs, None, lead),
        "hold": (hold_p, hs, None, lead),
        "overshoot": (over_p, hs, lead, lead),
        "follow": (fol_p, hs, lead, lead),
    }
    return _tag(_phases(entry_key["t"], durations(move), built, 0.48), move.get("id", "spin"))


def build_jump_kick(rig, move, entry_key):
    """Crouch, leave the ground, kick at the top, land forward with a squash."""
    kick = move.get("foot", "l")
    other_f = other(kick)
    entry = entry_key["pose"]
    g = rig.guard
    travel = float(move.get("step", 0.4))
    caps = {"l": 0.95, "r": 0.95}
    hs = hands_of(move.get("hands"), ("guard", "guard"))
    base = rig.up_of(g["pelvis"])
    # takeoff feet stay where the combo already is
    feet0 = {s: (entry[f"ankle_{s}"].copy(), entry[f"ball_{s}"].copy()) for s in "lr"}

    def on_feet(src_y, ankles, yaw, wrists):
        pel = rig.pelvis_between(ankles["l"][0], ankles["r"][0], base + src_y, 0.5)
        elbows, knees = _dirs_from(g, rig)
        pose = rig.shape_on(g, pel)
        pose = _yaw_upper(rig, pose, yaw)
        for s in "lr":
            _stamp(pose, s, *ankles[s])
        support = tuple(s for s in "lr" if rig.up_of(ankles[s][0]) < rig.up_of(g[f"ankle_{s}"]) + 0.05)
        if support:
            rig.drop_to_fit(pose, support)
            for s in support:
                _stamp(pose, s, *ankles[s])
        for s in "lr":
            if s not in support:
                rig.clamp_ankle(pose, s, 0.92)
        for s in "lr":
            rig.seat_elbow(pose, s, elbows[s])
            rig.seat_knee(pose, s, knees[s] if s in support else rig.fwd + rig.up)
        if wrists:
            for s, w in wrists.items():
                pose[f"wrist_{s}"] = w(pose)
                rig.clamp_wrist(pose, s, caps[s])
                rig.seat_elbow(pose, s, -rig.fwd if s == kick else elbows[s])
        return pose

    crouch_ank = feet0
    set_p = on_feet(-0.11, crouch_ank, 0.0, {
        "l": lambda p: p["pelvis"] - rig.fwd * 0.12 + rig.up * 0.35 + rig.left * 0.16,
        "r": lambda p: p["pelvis"] - rig.fwd * 0.12 + rig.up * 0.35 - rig.left * 0.16,
    })
    # rise: both feet tuck up under a rising pelvis that has started forward
    def tucked(s, pel_guess_fwd):
        hip_h = base + 0.18
        ank = rig.add(entry["pelvis"], 0.1 if s == "l" else -0.1, 0.22, pel_guess_fwd)
        ank = rig.set_up(ank, hip_h)
        return ank, ank + rig.fwd * rig.foot * 0.5

    rise_ank = {"l": tucked("l", 0.1), "r": tucked("r", 0.1)}
    via_p = on_feet(0.16, rise_ank, 0.0, None)
    apex_ank = {"l": tucked("l", 0.22), "r": tucked("r", 0.22)}
    hit_tmp = on_feet(0.28, apex_ank, -8.0 if kick == "l" else 8.0, None)
    aim = _unit(rig.fwd * 0.9 + rig.up * 0.12 + (rig.left if kick == "l" else -rig.left) * 0.06)
    k_ank = hit_tmp[f"hip_{kick}"] + aim * rig.leg * 0.88
    k_ball = k_ank + _unit(rig.fwd + rig.up * 0.2) * rig.foot
    apex_ank = dict(apex_ank)
    apex_ank[kick] = (k_ank, k_ball)
    hit_p = on_feet(0.28, apex_ank, -10.0 if kick == "l" else 10.0, {
        kick: lambda p: p["shoulder_" + other(kick)] + rig.fwd * 0.05,
        other_f: lambda p: p[f"shoulder_{other_f}"] - rig.fwd * 0.05 + rig.up * 0.05,
    })
    over_p = rig.copy(hit_p)
    over_p[f"ankle_{kick}"] = hit_p[f"ankle_{kick}"] + rig.fwd * 0.035
    rig.clamp_ankle(over_p, kick, 0.95)
    # land forward, both feet flat, pelvis low (squash) then the combo settle stands it up
    def landed(s):
        ball = rig.set_up(feet0[s][1] + rig.fwd * travel + (rig.left if s == "l" else -rig.left) * 0.01, rig.up_of(g[f"ball_{s}"]))
        return rig.ankle_from_ball(ball, s), ball

    land = {"l": landed("l"), "r": landed("r")}
    fol_p = on_feet(-0.09, land, 0.0, None)
    hold_p = rig.copy(hit_p)
    none = set()
    both = _both()
    built = {
        "set": (set_p, hs, both, both),
        "via": (via_p, hs, none, none),
        "hit": (hit_p, hs, None, none),
        "hold": (hold_p, hs, None, none),
        "overshoot": (over_p, hs, {f"ankle_{kick}"}, none),
        "follow": (fol_p, hs, both, both),
    }
    return _tag(_phases(entry_key["t"], durations(move), built, 0.4), move.get("id", "jump"))


def build_sheet(rig, move, entry_key):
    """A grid pose sheet (tools/kev/grid/sheets). Lifted to this rig, planted, and retimed with the combo envelope."""
    import json
    import os
    import sys
    sheet_name = move.get("sheet", "jump_kick")
    path = move.get("path")
    if not path:
        path = os.path.join(os.path.dirname(__file__), "..", "grid", "sheets", sheet_name + ".json")
    with open(path) as f:
        sheet = json.load(f)
    argv = sys.argv[:]
    sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "grid"))
    import posesheet
    sys.argv = argv
    lifted = {}
    for name, pose in sheet["poses"].items():
        raw = posesheet.lift(sheet, pose, rig.srv, rig.info)
        lifted[name] = _fit_sheet_pose(rig, rig.take(raw))
    seq = [a[0] for a in sheet["sequence"]]
    # opener, the airborne strike (highest foot in the middle of the sheet), and the closer
    opener, closer = seq[0], seq[-1]
    middle = [n for n in seq[1:-1]] or [closer]

    def ankle_height(n):
        pose = lifted[n]
        return max(rig.up_of(pose[f"ankle_{s}"]) for s in "lr")

    strike = max(middle, key=ankle_height)
    before = []
    for n in seq:
        if n == strike:
            break
        if n != opener:
            before.append(n)
    rise = before[-1] if before else strike
    entry = entry_key["pose"]
    plant = move.get("plant", "r")
    # keep the stance foot under the combo: slide the whole sheet so its opener plant matches entry
    delta = rig.horiz(entry[f"ankle_{plant}"] - lifted[opener][f"ankle_{plant}"])

    def placed(name):
        pose = rig.copy(lifted[name])
        for k in pose:
            pose[k] = pose[k] + delta
        return pose

    hs = hands_of(move.get("hands"), ("fist", "fist"))
    set_p, via_p, hit_p, fol_p = placed(opener), placed(rise), placed(strike), placed(closer)
    over_p = rig.copy(hit_p)
    # which foot leaves the ground at the strike: the one higher than the opener
    air = max("lr", key=lambda s: rig.up_of(hit_p[f"ankle_{s}"]) - rig.up_of(set_p[f"ankle_{s}"]))
    over_p[f"ankle_{air}"] = hit_p[f"ankle_{air}"] + rig.fwd * 0.03
    rig.clamp_ankle(over_p, air, 0.95)
    hold_p = rig.copy(hit_p)
    low = set()
    for s in "lr":
        if rig.up_of(set_p[f"ankle_{s}"]) < rig.up_of(g_ankle(rig, s)) + 0.06:
            low |= _feet(s)
    land = set()
    for s in "lr":
        if rig.up_of(fol_p[f"ankle_{s}"]) < rig.up_of(g_ankle(rig, s)) + 0.06:
            land |= _feet(s)
    built = {
        "set": (set_p, hs, low, low),
        "via": (via_p, hs, set(), set()),
        "hit": (hit_p, hs, None, set()),
        "hold": (hold_p, hs, None, set()),
        "overshoot": (over_p, hs, set(), set()),
        "follow": (fol_p, hs, land, land),
    }
    return _tag(_phases(entry_key["t"], durations(move), built, 0.45), move.get("id", sheet_name))


def g_ankle(rig, side):
    return rig.guard[f"ankle_{side}"]


def _fit_sheet_pose(rig, pose):
    """A side-view sheet has a shorter torso than the rig. Stretch the spine back to the rig's
    neck length and carry the arms with it, then keep every reach inside the limb."""
    neck = pose["neck"] - pose["pelvis"]
    dist = float(np.linalg.norm(neck))
    want = float(np.linalg.norm(rig.guard["neck"] - rig.guard["pelvis"]))
    if dist > 1e-4 and abs(dist - want) / want > 0.05:
        extra = neck * (want / dist - 1.0)
        for k in ("neck", "crown", "nose", "head_joint", "shoulder_l", "shoulder_r", "elbow_l", "elbow_r", "wrist_l", "wrist_r"):
            pose[k] = pose[k] + extra
    # shoulders sit on the rig, not on the drawing
    for s in "lr":
        off = rig.guard[f"shoulder_{s}"] - rig.guard["neck"]
        old = pose[f"shoulder_{s}"].copy()
        pose[f"shoulder_{s}"] = pose["neck"] + off
        pose[f"wrist_{s}"] = pose[f"shoulder_{s}"] + (pose[f"wrist_{s}"] - old)
        pose[f"elbow_{s}"] = pose[f"shoulder_{s}"] + (pose[f"elbow_{s}"] - old)
        rig.clamp_wrist(pose, s, 0.93)
        rig.seat_elbow(pose, s, pose[f"elbow_{s}"] - pose[f"shoulder_{s}"])
    for s in "lr":
        if rig.up_of(pose[f"ankle_{s}"]) > rig.up_of(rig.guard[f"ankle_{s}"]) + 0.08:
            rig.clamp_ankle(pose, s, 0.93)
        rig.seat_knee(pose, s, rig.fwd)
    return pose


def build_offsets(rig, move, entry_key):
    """Authoring path: per-phase offsets in character space (x left, y up, z forward) from the entry pose.

    phases.load / via / contact / follow. contact is the hit. Wrists and pelvis are offsets, not absolutes.
    yaw is degrees, positive turning the left shoulder forward.
    """
    entry = entry_key["pose"]
    phases = move["phases"]
    plant, free, lock, back, catch = _locks(rig, entry, move, catch=float(move.get("catch", 0.25)))
    caps = {"l": float(move.get("reach", 0.96)), "r": float(move.get("reach", 0.96))}
    default_hands = hands_of(move.get("hands"), ("cast", "cast"))
    g = rig.guard
    elbows0, knees0 = _dirs_from(g, rig)

    def one(phase, free_foot):
        spec = phases[phase]
        hs = hands_of(spec.get("hands"), (default_hands["l"], default_hands["r"])) if spec.get("hands") else default_hands
        # numeric hands_of default expects names; pass through floats by using the values directly
        if spec.get("hands"):
            hs = {"l": hand_value(spec["hands"].get("l", default_hands["l"]), default_hands["l"]),
                  "r": hand_value(spec["hands"].get("r", default_hands["r"]), default_hands["r"])}
        pel_off = spec.get("pelvis", [0, 0, 0])
        pel = rig.add(entry["pelvis"], *pel_off)
        src = rig.copy(entry)
        pose = _assemble(rig, src, pel, plant, free, lock, free_foot, elbows0, knees0, caps, (plant, free))
        pose = _yaw_upper(rig, pose, float(spec.get("yaw", 0.0)))
        _stamp(pose, plant, *lock)
        _stamp(pose, free, *free_foot)
        for s, off in (spec.get("wrists") or {}).items():
            pose[f"wrist_{s}"] = rig.add(entry[f"wrist_{s}"], *off)
            # keep the wrist near the yawed shoulder: the offset is from the entry wrist, which is what authors type
            rig.clamp_wrist(pose, s, caps[s])
            direction = elbows0[s]
            if spec.get("elbows", {}).get(s) == "down":
                direction = -rig.up + rig.fwd * 0.2
            elif spec.get("elbows", {}).get(s) == "out":
                direction = rig.left * (1 if s == "l" else -1) + rig.up * 0.4
            rig.seat_elbow(pose, s, direction)
        rig.drop_to_fit(pose, (plant, free))
        _stamp(pose, plant, *lock)
        _stamp(pose, free, *free_foot)
        for s, off in (spec.get("wrists") or {}).items():
            rig.clamp_wrist(pose, s, caps[s])
        return pose, hs

    set_p, hs_s = one("load", back)
    hit_p, hs_h = one("contact", back)
    if "via" in phases:
        via_p, hs_v = one("via", back)
    else:
        via_p = rig.mix(set_p, hit_p, 0.5)
        _stamp(via_p, plant, *lock)
        _stamp(via_p, free, *back)
        limb = "wrist_r" if "r" in (phases["contact"].get("wrists") or {}) else "wrist_l"
        via_p = _bulge(rig, via_p, set_p, hit_p, limb, float(move.get("arc", 0.03)), elbows0[limb[-1]])
        _stamp(via_p, plant, *lock)
        _stamp(via_p, free, *back)
        hs_v = hs_h
    over_p = rig.copy(hit_p)
    for s in (phases["contact"].get("wrists") or {}):
        d = hit_p[f"wrist_{s}"] - set_p[f"wrist_{s}"]
        if np.linalg.norm(d) > 1e-3:
            over_p[f"wrist_{s}"] = hit_p[f"wrist_{s}"] + _unit(d) * 0.035
            rig.clamp_wrist(over_p, s, caps[s])
    fol_p, hs_f = one("follow", catch)
    _stamp(fol_p, plant, *lock)
    hold_p = rig.copy(hit_p)
    feet = _both()
    lead = _feet(plant)
    built = {
        "set": (set_p, hs_s, feet, feet),
        "via": (via_p, hs_v, feet, feet),
        "hit": (hit_p, hs_h, None, feet),
        "hold": (hold_p, hs_h, None, feet),
        "overshoot": (over_p, hs_h, feet, feet),
        "follow": (fol_p, hs_f, lead, lead),
    }
    return _tag(_phases(entry_key["t"], durations(move), built, 0.42), move.get("id", "offsets"))


BUILDERS = {
    "jab": build_clip_strike,
    "cross": build_clip_strike,
    "clip": build_clip_strike,
    "hook": build_hook,
    "straight": build_straight,
    "uppercut": build_uppercut,
    "roundhouse": build_roundhouse,
    "spin_back": build_spin_back,
    "jump_kick": build_jump_kick,
    "sheet": build_sheet,
    "offsets": build_offsets,
}


def build_move(rig, move, entry_key):
    kind = move["do"]
    if kind not in BUILDERS:
        raise KeyError(f"unknown move '{kind}' (have {', '.join(sorted(BUILDERS))})")
    if kind == "straight":
        return build_straight(rig, move, entry_key, move.get("hand", "l"))
    return BUILDERS[kind](rig, move, entry_key)
