"""Rig measurements and pose placement. World axes match Kev Studio: +Y up, +Z forward, +X character-left, metres."""
import numpy as np

from .geom import rot_axis, unit

DRIVEN = ("pelvis", "hip_l", "hip_r", "knee_l", "knee_r", "ankle_l", "ankle_r", "ball_l", "ball_r",
          "neck", "shoulder_l", "shoulder_r", "elbow_l", "elbow_r", "wrist_l", "wrist_r",
          "crown", "nose", "head_joint")
UPPER = ("neck", "shoulder_l", "shoulder_r", "elbow_l", "elbow_r", "wrist_l", "wrist_r", "crown", "nose", "head_joint", "hip_l", "hip_r")


def other(side):
    return "r" if side == "l" else "l"


class Rig:
    def __init__(self, srv, model):
        self.srv = srv
        self.model = model
        self.info = srv.model_info(model)
        if not self.info.get("bm"):
            raise RuntimeError(f"{model} has no body map")
        body = self.info["bm"]["body"]
        self.up = unit(body["up"])
        self.fwd = unit(body["forward"])
        self.left = unit(np.cross(self.up, self.fwd))
        self.height = float(body["height"])
        self.floor = float(body["floor"])
        self.rest = {k: np.array(v["rest"], float) for k, v in self.info["bm"]["landmarks"].items()}
        self.arm = float(np.linalg.norm(self.rest["wrist_l"] - self.rest["shoulder_l"]))
        self.leg = float(np.linalg.norm(self.rest["ankle_l"] - self.rest["hip_l"]))
        self.foot = float(np.linalg.norm(self.rest["ball_l"] - self.rest["ankle_l"]))
        self._tracks = {}
        self.guard = self._guard()

    def take(self, landmarks):
        return {k: np.array(landmarks[k], float) for k in DRIVEN}

    def copy(self, pose):
        return {k: np.array(v, float).copy() for k, v in pose.items()}

    def up_of(self, p):
        return float(np.dot(p, self.up))

    def set_up(self, p, c):
        return p - self.up * np.dot(p, self.up) + self.up * c

    def horiz(self, p):
        return p - self.up * np.dot(p, self.up)

    def add(self, p, x=0.0, y=0.0, z=0.0):
        """Offset in character space: x left, y up, z forward."""
        return np.asarray(p, float) + self.left * x + self.up * y + self.fwd * z

    def _pose_at(self, clip, t):
        info = self.info
        if clip not in self._tracks:
            self._tracks[clip] = self.srv.clip_tracks(info, clip)
        Q, P = {}, {}
        for (bone, path), (tt, v, kind) in self._tracks[clip].items():
            (Q if path == "rotation" else P)[bone] = self.srv.eval_track(tt, v, kind, float(t))
        return self.take(self.srv.kev_read(info, Q, P)["landmarks"])

    def _guard(self):
        clips = self.info["clips"]
        if "Punch_Jab" in clips:
            return self._pose_at("Punch_Jab", 0.0)
        return self.take(self.rest)

    def clip_strike(self, clip, limb):
        """(guard, coil, peak, follow) poses of a clip, coil exaggerated away from the peak."""
        dur = max(t[-1] for t, _, _ in self.srv.clip_tracks(self.info, clip).values())
        ts = np.linspace(0.0, float(dur), 96)
        poses = [self._pose_at(clip, t) for t in ts]

        def fwd(p):
            return float(np.dot(p[limb] - p["pelvis"], self.fwd))

        peak_i = int(np.argmax([fwd(p) for p in poses]))
        load_i = int(np.argmin([fwd(poses[i]) for i in range(peak_i + 1)]))
        g, pk = fwd(poses[0]), fwd(poses[peak_i])
        thr = pk - 0.42 * (pk - g)
        fol_i = min(len(poses) - 1, peak_i + 6)
        for i in range(peak_i + 1, len(poses)):
            if fwd(poses[i]) <= thr:
                fol_i = i
                break
        guard, load, peak, follow = poses[0], self.copy(poses[load_i]), poses[peak_i], poses[fol_i]
        away = load[limb] - peak[limb]
        if np.linalg.norm(away) < 0.05:
            away = guard[limb] - peak[limb]
        if np.linalg.norm(away) < 0.04:
            away = -self.fwd
        away = unit(away)
        load[limb] = peak[limb] + away * max(float(np.linalg.norm(load[limb] - peak[limb])) * 1.3, 0.11)
        load[limb] = load[limb] - self.up * 0.025
        sh = f"shoulder_{limb[-1]}"
        load[sh] = load[sh] - self.fwd * 0.025
        self.clamp_wrist(load, limb[-1], 0.97)
        return guard, load, peak, follow

    def clamp_wrist(self, pose, side, frac=0.96):
        root, end = pose[f"shoulder_{side}"], pose[f"wrist_{side}"]
        v = end - root
        d = float(np.linalg.norm(v))
        cap = self.arm * frac
        if d > cap:
            pose[f"wrist_{side}"] = root + v / d * cap

    def clamp_ankle(self, pose, side, frac=0.94):
        hip, ank = pose[f"hip_{side}"], pose[f"ankle_{side}"]
        v = ank - hip
        d = float(np.linalg.norm(v))
        cap = self.leg * frac
        if d > cap and d > 1e-6:
            nudged = hip + v / d * cap
            pose[f"ball_{side}"] = pose[f"ball_{side}"] + (nudged - ank)
            pose[f"ankle_{side}"] = nudged

    def drop_to_fit(self, pose, sides):
        """Crouch the body until each support hip can reach its ankle. Feet stay put."""
        for _ in range(10):
            over = 0.0
            for s in sides:
                d = float(np.linalg.norm(pose[f"ankle_{s}"] - pose[f"hip_{s}"]))
                over = max(over, d - self.leg * 0.94)
            if over <= 1e-4:
                return
            drop = min(0.025, over)
            for k in list(pose):
                if k.startswith("ankle") or k.startswith("ball"):
                    continue
                pose[k] = pose[k] - self.up * drop

    def seat_knee(self, pose, side, direction=None, amount=0.18):
        hip, ank = pose[f"hip_{side}"], pose[f"ankle_{side}"]
        mid = (hip + ank) * 0.5
        hint = self.fwd if direction is None else direction
        u = ank - hip
        n = float(np.linalg.norm(u))
        if n < 1e-6:
            pose[f"knee_{side}"] = mid + hint
            return
        u = u / n
        h = hint - u * np.dot(hint, u)
        if np.linalg.norm(h) < 1e-6:
            h = np.cross(u, self.up)
        pose[f"knee_{side}"] = mid + unit(h) * amount

    def seat_elbow(self, pose, side, direction, amount=0.14):
        a, b = pose[f"shoulder_{side}"], pose[f"wrist_{side}"]
        u = b - a
        n = float(np.linalg.norm(u))
        if n < 1e-6:
            pose[f"elbow_{side}"] = a + direction
            return
        u = u / n
        h = np.asarray(direction, float) - u * np.dot(direction, u)
        if np.linalg.norm(h) < 1e-6:
            h = np.cross(u, self.up)
        pose[f"elbow_{side}"] = a + u * n * 0.48 + unit(h) * amount

    def ball_from_ankle(self, ankle, side):
        return ankle + (self.guard[f"ball_{side}"] - self.guard[f"ankle_{side}"])

    def ankle_from_ball(self, ball, side, yaw=0.0, pitch=0.0):
        off = self.guard[f"ankle_{side}"] - self.guard[f"ball_{side}"]
        # Pitch lifts the heel. Yaw spins the foot on the ball. Outward yaw is + for the left foot, - for the right.
        sign = 1.0 if side == "l" else -1.0
        off = rot_axis(off, self.left * sign, pitch)
        off = rot_axis(off, self.up, yaw * sign)
        return ball + off

    def step_ball(self, entry, side, step):
        ball = entry[f"ball_{side}"] + self.fwd * step
        return self.set_up(ball, self.up_of(self.guard[f"ball_{side}"]))

    def shape_on(self, src, pelvis):
        """Upper body and hips of src, re-seated on a new pelvis (offsets from src's pelvis)."""
        origin = src["pelvis"]
        out = {"pelvis": np.array(pelvis, float).copy()}
        for k in UPPER:
            out[k] = pelvis + (src[k] - origin)
        return out

    def yaw_about(self, pose, pivot, deg, keys):
        out = self.copy(pose)
        for k in keys:
            out[k] = pivot + rot_axis(pose[k] - pivot, self.up, deg)
        return out

    def pelvis_between(self, plant, free, up_c, bias=0.6):
        h = self.horiz(plant) * bias + self.horiz(free) * (1.0 - bias)
        return h + self.up * up_c

    def mix(self, a, b, u):
        return {k: a[k] * (1 - u) + b[k] * u for k in a}
