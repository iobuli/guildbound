"""Karate kicks on the UAL mannequin, posed with a small analytic rig and keyed in Blender.

    blender -b --factory-startup --python tools/blender/karate.py -- [--render DIR] [--export GLB] [--only NAME]

Poses are a few numbers each (see `POSES`): a body yaw, pelvis offset and lean, chest lean and twist, head turn,
an ankle target and knee pole per leg (analytic two-bone IK), hand targets and elbow poles in the chest frame,
foot and toe pitch, and fist curl. `pose()` turns one into bone world rotations and keys them; the support foot
(left) stays planted while the body pivots on it.

Frames (Blender world): +Z up, the mannequin faces -Y, its left is +X. Body frame = world turned by the yaw.
Rotations: pitch + leans forward (about +X), roll + leans to the left (about +Y), yaw/twist + turns left (+Z).

Actions: `Mae_Geri`, `Yoko_Geri`, `Ushiro_Geri` (each stance, chamber, kick, rechamber, stance) and
`Karate_Test` (all three in a row). `--export` writes them with the armature to a GLB the game loads like UAL.
`--render DIR` renders the side view (orthographic, like the reference chart) of `Karate_Test` as PNG frames
plus the nine key poses (`key_<kick>_<chamber|kick|rechamber>.png`).
"""
import json
import math
import os
import sys

import bpy
from mathutils import Matrix, Quaternion, Vector

UAL = os.environ.get("GB_POSE_MODEL", "/home/user/gb_e7/assets/anim/ual2.glb")
FITS = "/home/user/gb_e7/tools/blender/refs/fit.json"
FPS = 60

# Rest-pose landmarks (world): spine_03 head (chest frame origin) and pelvis head, from ual2.glb.
LEGS = {"l": ("thigh_l", "calf_l", "foot_l", "ball_l"), "r": ("thigh_r", "calf_r", "foot_r", "ball_r")}
ARMS = {"l": ("upperarm_l", "lowerarm_l", "hand_l"), "r": ("upperarm_r", "lowerarm_r", "hand_r")}
FINGERS = ("index", "middle", "ring", "pinky")


def rot(axis, deg):
    return Quaternion(Vector(axis), math.radians(deg))


def body_q(yaw=0.0, pitch=0.0, roll=0.0):
    """Yaw about Z, then pitch about the turned X, then roll about the turned Y."""
    return rot((0, 0, 1), yaw) @ rot((1, 0, 0), pitch) @ rot((0, 1, 0), roll)


def frame(d, k):
    """Rotation whose Y axis is `d` and whose Z axis is `k` made perpendicular to it."""
    d = d.normalized()
    k = (k - d * k.dot(d))
    k = k.normalized() if k.length > 1e-6 else d.orthogonal().normalized()
    x = d.cross(k)
    return Matrix((x, d, k)).transposed().to_quaternion()


def two_bone(root, target, l1, l2, pole):
    """Middle joint and clamped end for a two-bone chain from `root` toward `target`, bending toward `pole`."""
    v = target - root
    d = min(max(v.length, 1e-4), (l1 + l2) * 0.999)
    u = v.normalized()
    a = (l1 * l1 - l2 * l2 + d * d) / (2 * d)
    h = math.sqrt(max(l1 * l1 - a * a, 0.0))
    p = pole - u * pole.dot(u)
    p = p.normalized() if p.length > 1e-6 else u.orthogonal().normalized()
    return root + u * a + p * h, root + u * d


class Rig:
    def __init__(self, arm):
        self.arm = arm
        self.pb = arm.pose.bones
        self.rest = {b.name: (arm.matrix_world @ b.matrix_local) for b in arm.data.bones}
        self.rest_rot = {n: m.to_quaternion() for n, m in self.rest.items()}
        self.rest_head = {n: m.translation.copy() for n, m in self.rest.items()}
        self.len = {b.name: b.length for b in arm.data.bones}
        self.last = {}

    def head(self, name):
        return self.arm.matrix_world @ self.pb[name].head

    def set(self, name, q_world_rel, at=None):
        """Bone `name` gets world rotation `q_world_rel @ rest`, its head at `at` (else where its parent puts it)."""
        # The matrix setter solves against the parent's last evaluated pose, so evaluate first.
        bpy.context.view_layer.update()
        if at is None:
            at = self.head(name)
        m = Matrix.Translation(at) @ (q_world_rel @ self.rest_rot[name]).to_matrix().to_4x4()
        self.pb[name].matrix = self.arm.matrix_world.inverted() @ m

    def aim(self, name, d1, k1, d0, k0):
        """World rotation (relative to rest) taking the rest frame (d0, k0) to (d1, k1)."""
        return frame(d1, k1) @ frame(d0, k0).inverted()


def rest_dir(rig, name):
    m = rig.rest[name]
    return (m.to_quaternion() @ Vector((0, 1, 0))).normalized()


def pose(rig, p, origin):
    """Apply pose `p` (see POSES) with the body origin at `origin` (world)."""
    for b in rig.pb:
        b.matrix_basis = Matrix.Identity(4)
    bpy.context.view_layer.update()
    B = rot((0, 0, 1), p.get("yaw", 0.0))
    body = lambda v: origin + B @ Vector(v)
    # Pelvis: position and lean.
    q_pel = body_q(p.get("yaw", 0.0), *p.get("tilt", (0.0, 0.0)))
    pel = rig.rest_head["pelvis"] + Vector(p.get("pel", (0, 0, 0)))
    rig.set("pelvis", q_pel, body(pel))
    # Spine: chest lean and twist relative to the pelvis, spread over the three spine bones; head on top.
    cp, cr, ct = p.get("chest", (0.0, 0.0, 0.0))
    q_chest = q_pel @ rot((0, 0, 1), ct) @ rot((1, 0, 0), cp) @ rot((0, 1, 0), cr)
    for i, n in enumerate(("spine_01", "spine_02", "spine_03")):
        rig.set(n, q_pel.slerp(q_chest, (i + 1) / 3))
    hp, hy, *hr = p.get("head", (0.0, 0.0))
    q_head = q_chest @ rot((0, 0, 1), hy) @ rot((1, 0, 0), hp) @ rot((0, 1, 0), hr[0] if hr else 0.0)
    rig.set("neck_01", q_chest.slerp(q_head, 0.5))
    rig.set("Head", q_head)
    # Legs. A planted (or `ankle`) leg: two-bone IK onto an ankle target in the body frame. A lifted leg with
    # `thigh` / `knee`: joint angles relative to the pelvis, so a blend between two poses swings the shin around the
    # knee (a snap kick's arc) instead of dragging the foot along a line.
    for side, (thigh, calf, foot, ball) in LEGS.items():
        leg = p[side]
        s = 1 if side == "l" else -1
        bpy.context.view_layer.update()
        hip = rig.head(thigh)
        h0 = hinge(Vector((0, -1, 0)), rig.rest_head[foot] - rig.rest_head[thigh])
        d0t, d0c = rest_dir(rig, thigh), rest_dir(rig, calf)

        def solve_ik(target, pole):
            knee, ankle = two_bone(hip, target, rig.len[thigh], rig.len[calf], pole)
            # The knee is a hinge: its axis comes from the pole and the reach, and each bone's front is
            # (bone direction x hinge), built the same way at rest, so the shin never twists round.
            h = hinge(pole, ankle - hip)
            return (rig.aim(thigh, knee - hip, (knee - hip).cross(h), d0t, d0t.cross(h0)),
                    rig.aim(calf, ankle - knee, (ankle - knee).cross(h), d0c, d0c.cross(h0)))

        pole = B @ Vector(leg.get("pole", (0, -1, 0)))
        if leg.get("ik"):
            # A lifted leg placed by its ankle (older poses): IK plus a foot pitch.
            q_thigh, q_calf = solve_ik(body(leg["ankle"]), pole)
            q_foot = q_calf @ rot((1, 0, 0), leg.get("foot", 0.0))
        else:
            # Joint angles relative to the pelvis (hip flexion forward, abduction out, twist knee-out; knee; ankle)
            # blended toward a planted foot by `plant` (1 = on its ground spot, flat, turned with the body), so a
            # kicking foot leaves the floor and lands again without a pop.
            w = max(0.0, min(1.0, leg.get("plant", 0.0)))
            fl, ab, tw = leg.get("thigh", (0.0, 0.0, 0.0))
            f_thigh = q_pel @ rot((1, 0, 0), -fl) @ rot((0, 1, 0), -s * ab) @ rot((0, 0, 1), s * tw)
            f_calf = f_thigh @ rot((1, 0, 0), leg.get("knee", 0.0))
            f_foot = f_calf @ rot((1, 0, 0), leg.get("foot", 0.0))
            if w > 0.0:
                i_thigh, i_calf = solve_ik(body(leg["ankle"]), pole)
                i_foot = rot((0, 0, 1), p.get("yaw", 0.0) * leg.get("turn", 1.0))
                q_thigh, q_calf, q_foot = f_thigh.slerp(i_thigh, w), f_calf.slerp(i_calf, w), f_foot.slerp(i_foot, w)
            else:
                q_thigh, q_calf, q_foot = f_thigh, f_calf, f_foot
        rig.set(thigh, q_thigh)
        rig.set(calf, q_calf)
        rig.set(foot, q_foot)
        rig.set(ball, q_foot @ rot((1, 0, 0), leg.get("toe", 0.0)))
    # Arms: joint angles relative to the chest, from the arm hanging at the side (the rest T-pose turned down 90 deg,
    # palm in). Shoulder flexion swings the arm forward, abduction out, twist about the arm; the elbow bends the
    # forearm forward; the wrist flexes the hand. Fingers curl into a fist.
    for side, (upper, lower, hand) in ARMS.items():
        a = p["h" + side]
        s = 1 if side == "l" else -1
        fl, ab, tw = a.get("shoulder", (0.0, 0.0, 0.0))
        hang = rot((0, 1, 0), s * 90)
        q_up = q_chest @ rot((1, 0, 0), -fl) @ rot((0, 1, 0), -s * ab) @ hang @ rot((1, 0, 0), s * tw)
        rig.set(upper, q_up)
        q_lo = q_up @ rot((0, 0, 1), -s * a.get("elbow", 0.0))
        rig.set(lower, q_lo)
        q_hand = q_lo @ rot((0, 0, 1), -s * a.get("wrist", 0.0))
        rig.set(hand, q_hand)
        curl = a.get("fist", 80.0) * s
        for f in FINGERS:
            for k in range(1, 4):
                n = f"{f}_0{k}_{side}"
                if n in rig.pb:
                    rig.set(n, q_hand @ rot((0, 1, 0), curl * k))
        # Thumb: wraps across the closed fingers, aimed at the middle bones of the index and middle fingers.
        if a.get("fist", 80.0) > 30 and f"thumb_01_{side}" in rig.pb:
            q_prev = q_hand
            for k, aim_at in ((1, "index_02"), (2, "middle_02"), (3, "ring_02")):
                n = f"thumb_0{k}_{side}"
                bpy.context.view_layer.update()
                th = rig.head(n)
                now = q_prev @ rest_dir(rig, n)
                q_prev = now.rotation_difference(rig.head(f"{aim_at}_{side}") - th) @ q_prev
                rig.set(n, q_prev)
        else:
            for k in range(2, 4):
                n = f"thumb_0{k}_{side}"
                if n in rig.pb:
                    rig.set(n, q_hand @ rot((0, 1, 0), curl * 0.4 * (k - 1)))
    bpy.context.view_layer.update()


def hinge(pole, reach):
    """A hinge joint's axis: pole x reach (the rest leg, pole forward and reaching down, gives +X)."""
    h = pole.cross(reach)
    return h.normalized() if h.length > 1e-6 else Vector((1, 0, 0))


def key_all(rig, f):
    for b in rig.pb:
        q = b.rotation_quaternion.copy()
        prev = rig.last.get(b.name)
        if prev is not None and prev.dot(q) < 0:
            q.negate()
            b.rotation_quaternion = q
        rig.last[b.name] = q
        b.keyframe_insert("rotation_quaternion", frame=f)
        if b.name == "pelvis":
            b.keyframe_insert("location", frame=f)


# --- Poses ------------------------------------------------------------------------------------------------
# Body frame: origin on the ground under the rest pelvis, facing -Y. Support (left) ankle stays at SUPPORT.
SUPPORT = (0.10, 0.02, 0.104)
GUARD = {
    "hl": {"shoulder": (-6, 8, 0), "elbow": 35},
    "hr": {"shoulder": (-6, 8, 0), "elbow": 35},
}


def P(**kw):
    p = {"l": {"ankle": SUPPORT, "planted": True, "pole": (0.05, -1, 0)}}
    p.update(GUARD)
    p.update(kw)
    return p


RIGHT_SPOT = (-0.13, 0.12, 0.104)
STANDING_R = {"ankle": RIGHT_SPOT, "pole": (-0.05, -1, 0), "planted": True, "thigh": (-8, 3, 0), "knee": 6, "foot": 2}
STANCE = P(pel=(0.02, 0.0, -0.015), r=STANDING_R)
POSES = {
    "Mae_Geri": {
        # Knee up just above the hip, shin hanging back under it, foot relaxed with the toes curled up; the
        # shoulders sit back to keep the weight over the support foot.
        "chamber": P(pel=(0.085, -0.045, -0.01), tilt=(-5, 0), chest=(-12, 0, 0), head=(26, 0),
                     r={"thigh": (100, 0, 0), "knee": 138, "foot": -10, "toe": -30},
                     hl={"shoulder": (-12, 8, 0), "elbow": 22}, hr={"shoulder": (-12, 8, 0), "elbow": 22}),
        # Leg straight and rising a little, the foot extended in line with it and the toes pulled back (ball of
        # the foot); hips pushed into the kick, shoulders back over the support foot.
        "kick": P(pel=(0.07, -0.07, -0.02), tilt=(-10, 0), chest=(-18, 0, 0), head=(41, 0),
                  r={"thigh": (97, 0, 0), "knee": 5, "foot": 60, "toe": -45},
                  hl={"shoulder": (-19, 8, 0), "elbow": 31}, hr={"shoulder": (-19, 8, 0), "elbow": 31}),
    },
    "Yoko_Geri": {
        # Body turned to the camera, head to the target; knee lifted high in front and a little toward the
        # target, foot by the support knee; support foot already turning; lead fist at the target, near fist at
        # the hip.
        "chamber": P(yaw=75, pel=(0.06, -0.02, -0.03), tilt=(0, 6), chest=(0, 4, 0), head=(5, -70, -8),
                     l={"ankle": SUPPORT, "planted": True, "pole": (0.05, -1, 0), "turn": 1.8},
                     r={"thigh": (100, 12, -20), "knee": 125, "foot": -15, "toe": -20},
                     hl={"shoulder": (-30, 10, 0), "elbow": 100}, hr={"shoulder": (10, 80, 0), "elbow": 15}),
        # Leg straight out at the target with the heel and blade leading, torso leaning ~35 deg away, head level
        # on the target; support heel turned to the target.
        "kick": P(yaw=90, pel=(0.09, 0.0, -0.03), tilt=(0, 24), chest=(0, 12, 0), head=(5, -75, -30),
                  l={"ankle": SUPPORT, "planted": True, "pole": (0.05, -1, 0), "turn": 2.0},
                  r={"thigh": (12, 80, -20), "knee": 3, "foot": -10, "toe": -20},
                  hl={"shoulder": (-30, 10, 0), "elbow": 100}, hr={"shoulder": (40, 30, 0), "elbow": 100}),
    },
    "Ushiro_Geri": {
        # Turned away from the target, hunched, knee to the chest, foot tucked under it heel-first, hands on the knee.
        "chamber": P(yaw=180, pel=(0.04, -0.03, -0.03), tilt=(15, 0), chest=(20, 0, 0), head=(-15, 0, 0),
                     r={"thigh": (110, 0, 0), "knee": 140, "foot": -20},
                     hl={"shoulder": (40, 5, 0), "elbow": 60}, hr={"shoulder": (40, 5, 0), "elbow": 60}),
        # Leg straight back at hip height, heel leading, torso near horizontal over a vertical support leg.
        "kick": P(yaw=180, pel=(0.05, 0.05, -0.03), tilt=(60, 0), chest=(20, 0, 0), head=(-40, 0, 0),
                  r={"thigh": (-30, 0, 0), "knee": 3, "foot": -20},
                  hl={"shoulder": (30, 10, 0), "elbow": 110}, hr={"shoulder": (30, 10, 0), "elbow": 110}),
    },
}

# Mae geri, as a real one moves: weight onto the support foot, knee driven up, the shin snapped out around the knee
# (ease-out: most of the travel in the first frames) with the hips thrusting and the shoulders going back, a short
# lock, the same fast pullback, the foot lowered and planted, a small absorbing dip, settle.
MAE_SHIFT = P(pel=(0.075, -0.015, -0.025), tilt=(-2, 0), chest=(-3, 0, 0), head=(6, 0), r=STANDING_R)
MAE_LAND = P(pel=(0.07, -0.02, -0.045), tilt=(2, 0), chest=(2, 0, 0), head=(2, 0), r=dict(STANDING_R, knee=12))
# (pose, seconds to reach it, easing, tag)
SEQUENCES = {
    "Mae_Geri": [(STANCE, 0.20, "smooth", "stance"), (MAE_SHIFT, 0.10, "smooth", "shift"),
                 (POSES["Mae_Geri"]["chamber"], 0.18, "smooth", "chamber"), (POSES["Mae_Geri"]["kick"], 0.13, "snap", "kick"),
                 (POSES["Mae_Geri"]["kick"], 0.05, "linear", "lock"), (POSES["Mae_Geri"]["chamber"], 0.11, "smooth", "rechamber"),
                 (MAE_LAND, 0.18, "smooth", "land"), (STANCE, 0.22, "smooth", "stance")],
}
# Yoko and ushiro geri are thrust kicks (kekomi): a slightly longer drive (0.15 s) with the hips behind it. Yoko
# pivots on the support foot as the hips turn side-on; ushiro turns its back to the target in the chamber and turns
# back to face it while it settles.
YOKO_SHIFT = P(yaw=30, pel=(0.075, -0.01, -0.03), tilt=(0, 3), chest=(0, 2, 0), head=(5, -30, 0),
               l={"ankle": SUPPORT, "planted": True, "pole": (0.05, -1, 0), "turn": 1.4}, r=STANDING_R)
YOKO_LAND = P(yaw=40, pel=(0.07, -0.01, -0.045), tilt=(0, 4), chest=(0, 2, 0), head=(5, -40, 0),
              l={"ankle": SUPPORT, "planted": True, "pole": (0.05, -1, 0), "turn": 1.4}, r=dict(STANDING_R, knee=12))
USHIRO_SHIFT = P(yaw=90, pel=(0.075, -0.01, -0.03), tilt=(5, 0), chest=(5, 0, 0), head=(0, 60, 0), r=STANDING_R)
USHIRO_LAND = P(yaw=150, pel=(0.07, -0.01, -0.045), tilt=(8, 0), chest=(5, 0, 0), head=(-5, 60, 0), r=dict(STANDING_R, knee=12))


def kick_sequence(name, shift, land, drive):
    k = POSES[name]
    return [(STANCE, 0.20, "smooth", "stance"), (shift, 0.12, "smooth", "shift"), (k["chamber"], 0.20, "smooth", "chamber"),
            (k["kick"], drive, "snap", "kick"), (k["kick"], 0.06, "linear", "lock"), (k["chamber"], 0.16, "smooth", "rechamber"),
            (land, 0.20, "smooth", "land"), (STANCE, 0.30, "smooth", "stance")]


SEQUENCES["Yoko_Geri"] = kick_sequence("Yoko_Geri", YOKO_SHIFT, YOKO_LAND, 0.20)
SEQUENCES["Ushiro_Geri"] = kick_sequence("Ushiro_Geri", USHIRO_SHIFT, USHIRO_LAND, 0.20)
TIMES = (0.25, 0.30, 0.13, 0.12, 0.17, 0.30)
EASES = ("smooth", "smooth", "out", "smooth", "smooth", "smooth")


def sequence(name):
    if name in SEQUENCES:
        return SEQUENCES[name]
    k = POSES[name]
    poses = [STANCE, k["chamber"], k["kick"], k["kick"], k["chamber"], STANCE]
    tags = ["stance", "chamber", "kick", "lock", "rechamber", "stance"]
    return list(zip(poses, TIMES, EASES, tags))


def origin_for(p):
    """Body origin that keeps the planted left foot fixed while allowing authored stage travel.

    ``stage`` is a world-space XYZ offset.  It is deliberately separate from ``pel``: stage
    travel moves the whole fighter (dash, retreat, choreographed spacing), while pelvis offsets
    shape the pose over the planted feet.
    """
    B = rot((0, 0, 1), p.get("yaw", 0.0))
    left = p.get("l", {})
    spot = Vector(left["ankle"]) if left.get("planted") and "ankle" in left else Vector(SUPPORT)
    return spot - B @ spot + Vector(p.get("stage", (0.0, 0.0, 0.0)))


DEFAULTS = {"yaw": 0.0, "pel": (0.0, 0.0, 0.0), "stage": (0.0, 0.0, 0.0),
            "tilt": (0.0, 0.0), "chest": (0.0, 0.0, 0.0), "head": (0.0, 0.0, 0.0)}
LIMB = {"pole": (0.0, -1.0, 0.0), "foot": 0.0, "toe": 0.0, "twist": 0.0, "turn": 1.0, "planted": False, "fist": 80.0, "roll": 0.0,
        "shoulder": (0.0, 0.0, 0.0), "elbow": 0.0, "wrist": 0.0,
        "thigh": (0.0, 0.0, 0.0), "knee": 0.0, "plant": 0.0, "ik": False, "ankle": (0.0, 0.0, 0.104)}


def full(p):
    q = dict(DEFAULTS)
    q.update({k: v for k, v in p.items() if k not in ("l", "r", "hl", "hr")})
    for k in ("l", "r", "hl", "hr"):
        limb = dict(LIMB)
        if k in ("hl", "hr"):
            limb["pole"] = (0.0, 0.3, -1.0)
        limb.update(p[k])
        if k in ("l", "r"):
            if limb["planted"]:
                limb["plant"] = 1.0
            elif "ankle" in p[k] and "thigh" not in p[k]:
                limb["ik"] = True
        q[k] = limb
    return q


def mix(a, b, t):
    """Pose parameters blended: numbers and vectors linearly, flags from the nearer end."""
    if isinstance(a, dict):
        return {k: mix(a[k], b[k], t) for k in a}
    if isinstance(a, bool):
        return a if t < 0.5 else b
    if isinstance(a, (int, float)):
        return a + (b - a) * t
    return tuple(x + (y - x) * t for x, y in zip(a, b))


def ease(kind, t):
    if kind == "snap":
        # A strike: speeds up most of the way and peaks ~61 % through (1.7x the mean), then the joint locks.
        u = t ** 1.4
        return u * u * (3 - 2 * u)
    if kind == "out":
        return 1 - (1 - t) ** 3
    if kind == "linear":
        return t
    return t * t * (3 - 2 * t)


def build(rig, names, action_name):
    """Every frame: the rig solved on blended pose parameters between the keys, so feet travel sensible paths
    (straight or arcing, never through the body) and the support foot stays planted."""
    # Blender 4.x exposed Action.fcurves directly; 5.x stores them in layered channel bags.
    # Asking for linear insertion up front works in both and is exactly what dense 60 fps keys need.
    bpy.context.preferences.edit.keyframe_new_interpolation_type = "LINEAR"
    act = bpy.data.actions.new(action_name)
    rig.arm.animation_data_create()
    rig.arm.animation_data.action = act
    rig.last = {}
    keys = []
    f = 1
    for name in names:
        for pz, dt, ek, tag in sequence(name):
            f += max(1, round(dt * FPS))
            keys.append((f, full(pz), name, tag, ek))
    prev_f, prev_p = 1, keys[0][1]
    for (kf, kp, name, tag, ek) in keys:
        for fr in range(prev_f, kf + 1):
            t = 1.0 if kf == prev_f else (fr - prev_f) / (kf - prev_f)
            pz = mix(prev_p, kp, ease(ek, t))
            pose(rig, pz, origin_for(pz))
            key_all(rig, fr)
        prev_f, prev_p = kf + 1, kp
    for fr in range(prev_f, prev_f + 8):
        key_all(rig, fr)
    for fc in getattr(act, "fcurves", ()):
        for kp in fc.keyframe_points:
            kp.interpolation = "LINEAR"
    act.frame_range = (1, prev_f + 7)
    return act, [(n, t, kf) for (kf, _, n, t, _) in keys]


def setup():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=UAL)
    for o in list(bpy.data.objects):
        if o.type not in ("ARMATURE", "MESH") or o.name == "Icosphere":
            bpy.data.objects.remove(o)
    for a in list(bpy.data.actions):
        bpy.data.actions.remove(a)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    arm.animation_data_create()
    arm.animation_data.action = None
    for b in arm.pose.bones:
        b.rotation_mode = "QUATERNION"
    return Rig(arm)


def camera(sc):
    cam = bpy.data.cameras.new("side")
    cam.type = "ORTHO"
    cam.ortho_scale = 2.6
    co = bpy.data.objects.new("side", cam)
    sc.collection.objects.link(co)
    sc.camera = co
    co.location = (6, 0, 0.95)
    co.rotation_euler = (math.radians(90), 0, math.radians(90))
    sc.render.engine = "BLENDER_WORKBENCH"
    sc.display.shading.light = "STUDIO"
    sc.display.shading.color_type = "MATERIAL" if "blocky" in UAL else "OBJECT"
    sc.render.resolution_x, sc.render.resolution_y = 520, 620
    sc.view_settings.view_transform = "Standard"
    sc.world = bpy.data.worlds.new("w")
    sc.world.color = (0.95, 0.85, 0.88)
    sc.display.shading.background_type = "WORLD"
    for o in bpy.data.objects:
        if o.type == "MESH":
            o.color = (0.72, 0.70, 0.66, 1)


# Isolation views: an orthographic side view (from the figure's left, so it faces screen-left like the chart) with
# a transparent background for onion-skinning, plus front and three-quarter views.
VIEW_RES = (700, 820)
VIEW_SCALE = 2.4
VIEW_Z = 0.95
GLTF = Matrix(((1, 0, 0, 0), (0, 0, 1, 0), (0, -1, 0, 0), (0, 0, 0, 1)))


def look_at(obj, target):
    d = Vector(target) - obj.location
    obj.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()


def views(sc, out, name):
    camera(sc)
    sc.render.resolution_x, sc.render.resolution_y = VIEW_RES
    co = sc.camera
    co.data.ortho_scale = VIEW_SCALE
    shots = [("side", (6, 0, VIEW_Z), True, "ORTHO"), ("front", (0, -6, VIEW_Z), False, "ORTHO"),
             ("34", (4.0, -4.6, VIEW_Z + 0.9), False, "PERSP")]
    for tag, loc, transparent, kind in shots:
        co.location = loc
        look_at(co, (0, 0, VIEW_Z if kind == "ORTHO" else VIEW_Z - 0.05))
        co.data.type = kind
        co.data.lens = 85
        sc.render.film_transparent = transparent
        sc.render.filepath = f"{out}/{name}_{tag}.png"
        bpy.ops.render.render(write_still=True)
    # Close-up of the near (left) hand, from in front and a little to the side.
    bpy.context.view_layer.update()
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    hand = arm.matrix_world @ arm.pose.bones["hand_l"].tail
    co.data.type = "PERSP"
    co.data.lens = 85
    co.location = hand + Vector((0.55, -0.85, 0.15))
    look_at(co, hand)
    sc.render.film_transparent = False
    sc.render.filepath = f"{out}/{name}_hand.png"
    bpy.ops.render.render(write_still=True)
    with open(f"{out}/{name}_view.json", "w") as f:
        ppm = VIEW_RES[1] / VIEW_SCALE
        json.dump({"ppm": ppm, "centre": [VIEW_RES[0] / 2, VIEW_RES[1] / 2], "view_z": VIEW_Z,
                   "map": "side px = (cx + y_blender * ppm, cy - (z_blender - view_z) * ppm); y_blender = -z_gltf, z_blender = y_gltf"}, f)


def dump(rig, path):
    """Each bone's world transform now times its rest world transform inverted, in glTF space (Kev's pose file)."""
    bpy.context.view_layer.update()
    out = {}
    for pb in rig.pb:
        d = (rig.arm.matrix_world @ pb.matrix) @ rig.rest[pb.name].inverted()
        g = GLTF @ d @ GLTF.inverted()
        out[pb.name] = [g[r][c] for r in range(4) for c in range(4)]
    with open(path, "w") as f:
        json.dump({"deltas": out}, f)


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opt = {argv[i]: argv[i + 1] for i in range(0, len(argv) - 1, 2)}
    rig = setup()
    sc = bpy.context.scene
    sc.render.fps = FPS
    if "--pose" in opt:
        # One pose in isolation: `Mae_Geri.chamber` or `stance`.
        set_name, _, phase = opt["--pose"].partition(".")
        pz = full(STANCE if set_name == "stance" else POSES[set_name][phase])
        pose(rig, pz, origin_for(pz))
        out = opt.get("--views", "/tmp")
        name = opt["--pose"].replace(".", "_")
        dump(rig, f"{out}/{name}_pose.json")
        views(sc, out, name)
        print("KARATE POSE DONE", name)
        return
    kicks = [opt["--only"]] if "--only" in opt else list(POSES)
    acts = [build(rig, [k], k)[0] for k in kicks]
    test, marks = build(rig, kicks, "Karate_Test")
    for a in acts + [test]:
        a.use_fake_user = True
    if "--render" in opt:
        out = opt["--render"].rstrip("/")
        camera(sc)
        rig.arm.animation_data.action = test
        for (name, tag, f) in marks:
            if tag in ("chamber", "kick", "rechamber"):
                sc.frame_set(f)
                sc.render.filepath = f"{out}/key_{name}_{tag}.png"
                bpy.ops.render.render(write_still=True)
        if opt.get("--anim", "1") == "0":
            print("KARATE DONE (keys only)")
            return
        sc.frame_start, sc.frame_end = 1, int(test.frame_range[1])
        sc.render.film_transparent = opt.get("--transparent", "0") == "1"
        sc.render.image_settings.color_mode = "RGBA"
        sc.render.filepath = f"{out}/f_"
        sc.render.image_settings.file_format = "PNG"
        bpy.ops.render.render(animation=True)
    if "--dump-frames" in opt:
        # Kev's pose file for every frame of the test action (for `bodymap.py track`).
        d = opt["--dump-frames"]
        rig.arm.animation_data.action = test
        for fr in range(1, int(test.frame_range[1]) + 1):
            sc.frame_set(fr)
            dump(rig, f"{d}/p_{fr:04d}.json")
    if "--export" in opt:
        rig.arm.animation_data.action = None
        for o in bpy.data.objects:
            o.select_set(o.type in ("ARMATURE", "MESH"))
        bpy.ops.export_scene.gltf(filepath=opt["--export"], export_format="GLB", use_selection=True,
                                  export_animation_mode="ACTIONS", export_force_sampling=True, export_frame_step=1)
    print("KARATE DONE", [(n, t, f) for n, t, f in marks])



def apply_fits(path=FITS):
    """Kev's fitted values (`posefit.py`) over the hand-written poses: {"Set.phase": {"r.thigh.0": 101.5, ...}}."""
    try:
        with open(path) as f:
            fits = json.load(f)
    except (OSError, ValueError):
        return
    for key, vals in fits.items():
        set_name, _, phase = key.partition(".")
        if set_name in POSES and phase in POSES[set_name]:
            for k, v in vals.items():
                set_param(POSES[set_name][phase], k, v)


def get_param(p, path):
    keys = path.split(".")
    node = full(p)
    for k in keys[:-1]:
        node = node[k]
    return node[int(keys[-1])] if keys[-1].isdigit() else node[keys[-1]]


def set_param(p, path, v):
    """Set `yaw`, `pel.1`, `r.knee` or `r.thigh.0` in a pose dict (missing parts start from the defaults)."""
    keys = path.split(".")
    full_p = full(p)
    if keys[0] in ("l", "r", "hl", "hr"):
        limb = p.setdefault(keys[0], {})
        if keys[-1].isdigit():
            cur = list(limb.get(keys[1], full_p[keys[0]][keys[1]]))
            cur[int(keys[-1])] = v
            limb[keys[1]] = tuple(cur)
        else:
            limb[keys[1]] = v
    elif keys[-1].isdigit():
        cur = list(p.get(keys[0], full_p[keys[0]]))
        cur[int(keys[-1])] = v
        p[keys[0]] = tuple(cur)
    else:
        p[keys[0]] = v


apply_fits()
if __name__ == "__main__":
    main()
