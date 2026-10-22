"""Causal physics pass over Kev landmark frames.

The clip stays the author. Each frame, in order:
  1. Ground contact. The stance ankle drives the root, so a treadmill slide becomes travel and a
     foot roll does not yank the pelvis. The heel, ball or toe is pinned to where it planted.
     A toe-off stops driving but stays pinned until it lifts. Heel/toe roll keeps the sole up.
  2. Balance. If the centre of mass falls outside the shoes, the pelvis and trunk shift back over
     the support, limited by leg reach. A swinging foot that is about to land steps toward the
     capture point.
  3. Momentum. Landing with downward speed compresses the body and releases. An impulse slides the
     root (knockback) and a hop leaves the ground on a ballistic vertical. Airborne, horizontal
     travel from the last push coasts.
  4. Secondary motion. Underdamped springs on the wrists, fingers and head, then the wrist is put
     back on the forearm so the arm never stretches.

Hit-stop freezes the last pose. Impulse adds root velocity the combo system can fire on a hit.
Everything is deterministic and numpy-only. Turn a piece off with config flags.
"""
import math

import numpy as np

from . import body

DEFAULTS = {
    "gravity": body.G,
    "plant_on": 0.02,
    "plant_off": 0.035,
    "half_width": 0.045,
    "mu": 1.0,
    "balance": True,
    "contact": True,
    "momentum": True,
    "secondary": True,
    "max_shift": 0.08,
    "max_step": 0.12,
    "max_compression": 0.07,
    "air_follow": 0.0,
    "comp_hz": 4.0,
    "comp_zeta": 0.7,
    "comp_gain": 0.45,
    "wrist_hz": 7.0,
    "wrist_zeta": 0.62,
    "wrist_lag": 0.035,
    "head_hz": 6.0,
    "head_zeta": 0.8,
    "head_lag": 0.02,
    "finger_hz": 11.0,
    "finger_zeta": 0.55,
    "finger_lag": 0.015,
}

FINGER_WORDS = ("knuckle", "fingertip", "thumb", "index", "middle", "ring", "pinky")
FOOT = ("ankle", "ball", "heel", "toe_tip", "toe", "instep")


def _cfg(config):
    out = dict(DEFAULTS)
    if config:
        out.update(config)
    return out


def _smooth(a, b, x):
    if a == b:
        return 1.0
    t = min(1.0, max(0.0, (x - a) / (b - a)))
    return t * t * (3 - 2 * t)


def _cap(v, limit):
    n = float(np.linalg.norm(v))
    if n <= limit or n < 1e-12:
        return np.asarray(v, float)
    return np.asarray(v, float) * (limit / n)


def _translate(lm, delta):
    d = np.asarray(delta, float)
    for k in lm:
        lm[k] = lm[k] + d


def _foot_keys(lm, side):
    keys = []
    for name in FOOT:
        k = f"{name}_{side}"
        if k in lm:
            keys.append(k)
    return keys


def _fingers(lm, side):
    suf = f"_{side}"
    out = []
    for k in lm:
        if not k.endswith(suf):
            continue
        base = k[: -len(suf)]
        if base.startswith(FINGER_WORDS) or base in ("knuckle", "fingertip"):
            out.append(k)
    return out


def _lift_angle(origin, point, axis, floor):
    v = np.asarray(point, float) - np.asarray(origin, float)
    target = float(floor) - float(origin[1])

    def y_at(th):
        return float((body.rodrigues(axis, th) @ v)[1])

    th = 0.0
    for _ in range(6):
        y0 = y_at(th)
        dy = (y_at(th + 1e-3) - y0) / 1e-3
        if abs(dy) < 1e-7:
            break
        th -= (y0 - target) / dy
        th = max(-0.8, min(0.8, th))
    return th


def _adjust_foot(lm, side, lock, pivot, added, rest, floor):
    """Pitch about the locked contact, then lift, so the sole stays out of the floor.

    Horizontal stick is the root solve's job. Moving one foot in x/z here fights the other plant.
    """
    del lock, added, rest
    keys = _foot_keys(lm, side)
    if not keys or pivot not in lm:
        return
    low_key, low = body.lowest_sole(lm, side)
    if low_key is None or float(low[1]) >= floor - 1e-5:
        return
    origin = lm[pivot]
    ak, bk = f"ankle_{side}", f"ball_{side}"
    if low_key != pivot and float(origin[1]) <= floor + 1e-3 and ak in lm and bk in lm:
        _, axis = body._basis(lm[ak], lm[bk])
        theta = _lift_angle(origin, lm[low_key], axis, floor)
        R = body.rodrigues(axis, theta)
        o = origin.copy()
        for k in keys:
            lm[k] = o + R @ (lm[k] - o)
    low_key, low = body.lowest_sole(lm, side)
    if low is not None and float(low[1]) < floor - 1e-5:
        dy = floor - float(low[1])
        up = np.array([0.0, dy, 0.0])
        for k in keys:
            lm[k] = lm[k] + up


class PhysicsPass:
    def __init__(self, floor=None, rest=None, config=None):
        self.floor_given = floor
        self.rest = {k: np.array(v, float) for k, v in rest.items()} if rest else None
        self.cfg = _cfg(config)
        self._springs()
        self.reset()

    def _springs(self):
        c = self.cfg
        self.wrist = {s: body.Spring(c["wrist_hz"], c["wrist_zeta"], c["wrist_lag"]) for s in "lr"}
        self.head = body.Spring(c["head_hz"], c["head_zeta"], c["head_lag"])
        self.finger = {s: {} for s in "lr"}

    def reset(self):
        self.feet = {}
        self.offset = np.zeros(2)
        self.offset_step = np.zeros(2)
        self.prev_offset = np.zeros(2)
        self.root_v = np.zeros(2)
        self.coast_v = np.zeros(2)
        self.coast_o = np.zeros(2)
        self.air_t = 0.0
        self.was_grounded = False
        self.knock_v = np.zeros(3)
        self.y_off = 0.0
        self.hold = 0.0
        self.c = 0.0
        self.comp_v = 0.0
        self.prev_sole_y = {}
        self.sole_vy = {}
        self.prev_ankle_xz = {}
        self.ankle_xz_v = {}
        self.prev_locked = {}
        self.was_planted = set()
        self.lifted = set()
        self.knock_off = np.zeros(2)
        self.prev_com = None
        self.prev_com_y = None
        self.takeoff = None
        self.last = None
        self.last_diag = {}
        self.shift_peak = 0.0
        self.step_peak = 0.0
        self.comp_peak = 0.0
        self.floor = 0.0 if self.floor_given is None else float(self.floor_given)
        self._ready = self.floor_given is not None
        for s in "lr":
            self.wrist[s].reset()
            self.finger[s] = {}
        self.head.reset()

    def hit_stop(self, seconds):
        self.hold = max(self.hold, float(seconds))

    def impulse(self, vec):
        self.knock_v = self.knock_v + np.asarray(vec, float)
        self.knock_v = _cap(self.knock_v, 8.0)
        for s in "lr":
            self.wrist[s].kick(-np.asarray(vec, float) * 0.35)
        self.head.kick(-np.asarray(vec, float) * 0.2)

    def summary(self):
        return {
            "compression_peak_m": round(self.comp_peak, 4),
            "pelvis_shift_peak_m": round(self.shift_peak, 4),
            "step_peak_m": round(self.step_peak, 4),
            "root_offset_m": [round(float(x), 4) for x in self.offset],
        }

    def status(self):
        return {
            "plants": "+".join(s for s in "lr" if s in self.feet) or "air",
            "compression_m": round(self.c, 4),
            "offset_m": [round(float(x), 4) for x in self.offset],
            "knock_mps": round(float(np.linalg.norm(self.knock_v)), 3),
            "hit_stop_s": round(self.hold, 3),
            **self.summary(),
        }

    def step(self, landmarks, dt):
        dt = min(0.05, float(dt) if dt and dt > 0 else 1.0 / 30.0)
        keys = [k for k in landmarks if not k.startswith("_")]
        if self.hold > 0 and self.last is not None:
            self.hold = max(0.0, self.hold - dt)
            diag = dict(self.last_diag)
            diag["hit_stop"] = True
            return {"landmarks": body.lists(self.last, keys), "diag": diag}

        if not self._ready:
            self.floor = float(body.infer_floor([landmarks], self.rest))
            self._ready = True

        lm, added = body.enrich(landmarks, self.rest)
        auth_y = {}
        for s in "lr":
            _, p = body.lowest_sole(lm, s)
            if p is not None:
                prev = self.prev_sole_y.get(s)
                auth_y[s] = 0.0 if prev is None else (float(p[1]) - prev) / dt
                self.prev_sole_y[s] = float(p[1])
        self.sole_vy = auth_y
        self.ankle_xz_v = {}
        for s in "lr":
            ank = lm.get(f"ankle_{s}")
            if ank is None:
                continue
            xz = body.xz(ank)
            prev = self.prev_ankle_xz.get(s)
            self.ankle_xz_v[s] = 0.0 if prev is None else float(np.linalg.norm(xz - prev)) / dt
            self.prev_ankle_xz[s] = xz
        self._hop(dt)
        if abs(self.y_off) > 1e-8:
            _translate(lm, np.array([0.0, self.y_off, 0.0]))

        down = {} if not self.cfg["contact"] else self._down(lm)
        if self.cfg["contact"]:
            self._roll_locks(lm, down)
        held = [s for s in down if s in self.feet]
        if held:
            self._release_toe_off(lm, held)
            held = [s for s in down if s in self.feet]
        if held:
            # The ankle of the oldest driving plant owns the root. A toe-off stops driving but
            # stays pinned, so the pelvis does not reverse and the sole does not skate away.
            driving = [s for s in held if self.feet[s].get("drive", True)]
            owners = driving or held
            desires = {}
            for s in owners:
                ankle = lm.get(f"ankle_{s}")
                if ankle is not None and "alock" in self.feet[s]:
                    desires[s] = self.feet[s]["alock"] - body.xz(ankle) - self.offset
                elif self.feet[s]["pivot"] in lm:
                    desires[s] = self.feet[s]["lock"] - body.xz(lm[self.feet[s]["pivot"]]) - self.offset
            primary = max(owners, key=lambda s: self.feet[s].get("age", 0))
            raw = desires.get(primary, np.zeros(2))
            # Two planted feet that want opposite root motion (a sword swing, both soles
            # skating apart) must not hand either skate to the pelvis. Re-pin them where
            # they stand. A walk keeps a single driving foot, so it still travels.
            disagree = False
            if len(desires) >= 2 and float(np.linalg.norm(raw)) > 0.05:
                for s, other in desires.items():
                    if s != primary and float(np.dot(raw, other)) < 0 and float(np.linalg.norm(other)) > 0.05:
                        disagree = True
            if disagree:
                for s in held:
                    low_key, low = body.lowest_sole(lm, s)
                    ankle = lm.get(f"ankle_{s}")
                    if low_key is None or low is None or ankle is None:
                        continue
                    self.feet[s]["pivot"] = low_key
                    self.feet[s]["lock"] = body.xz(low) + self.offset
                    self.feet[s]["alock"] = body.xz(ankle) + self.offset
                self.offset_step[:] = 0.0
                desired = np.zeros(2)
            else:
                # Slower than a stroll. A landing settle should not become travel.
                desired = np.zeros(2) if float(np.linalg.norm(raw)) < 0.008 else np.asarray(raw, float)
                if float(np.dot(desired, self.offset_step)) < 0 and float(np.linalg.norm(self.offset_step)) > 0.004:
                    desired = self.offset_step.copy()
                else:
                    desired = self.offset_step + _cap(desired - self.offset_step, 0.004)
            self.offset_step = _cap(np.array(desired, float), 0.06)
            self.offset = self.offset + self.offset_step
            _translate(lm, np.array([float(self.offset[0]), 0.0, float(self.offset[1])]))
            self.knock_off[:] = 0.0
            self._decay_knock(dt, grounded=True)
            slide = body.xz(self.knock_v) * dt
            if float(np.linalg.norm(slide)) > 1e-8:
                _translate(lm, np.array([slide[0], 0.0, slide[1]]))
                self.offset = self.offset + slide
                for s in held:
                    self.feet[s]["lock"] = self.feet[s]["lock"] + slide
                    if "alock" in self.feet[s]:
                        self.feet[s]["alock"] = self.feet[s]["alock"] + slide
            for s in held:
                self._pin_foot(lm, s)
        else:
            self._coast(dt)
            self._decay_knock(dt, grounded=False)
            self.knock_off = self.knock_off + body.xz(self.knock_v) * dt
            world = self.offset + self.knock_off
            _translate(lm, np.array([world[0], 0.0, world[1]]))

        if self.cfg["contact"] and self.cfg["air_follow"] > 0:
            self._air(lm, dt, grounded=bool(held))

        # A foot that is not held yet can still be steered onto a better landing. The lock below
        # captures that biased point; later frames stick to it.
        if self.cfg["balance"]:
            self._step_swing(lm, [s for s in "lr" if s not in self.feet and s not in self.lifted], dt)

        for s in list(down):
            if s in self.feet or s in self.lifted:
                continue
            low_key, low = body.lowest_sole(lm, s)
            ankle = lm.get(f"ankle_{s}")
            if low_key is not None and ankle is not None:
                self.feet[s] = {"pivot": low_key, "lock": body.xz(low), "alock": body.xz(ankle), "age": 0}
        if self.cfg["contact"]:
            for s in list(self.feet):
                if s not in down:
                    continue
                before = lm[f"ankle_{s}"].copy() if f"ankle_{s}" in lm else None
                hip = lm[f"hip_{s}"].copy() if f"hip_{s}" in lm else None
                knee = lm[f"knee_{s}"].copy() if f"knee_{s}" in lm else None
                _adjust_foot(lm, s, self.feet[s]["lock"], self.feet[s]["pivot"], added, self.rest, self.floor)
                if before is not None and hip is not None and knee is not None and f"ankle_{s}" in lm:
                    lm[f"knee_{s}"] = body.transfer_pole(hip, hip, before, lm[f"ankle_{s}"], knee)
                self._settle_sole(lm, s)
            for s in "lr":
                if s in self.feet:
                    continue
                _, low = body.lowest_sole(lm, s)
                if low is not None and float(low[1]) < self.floor - 1e-4:
                    self._lift_sole(lm, s, float(self.floor - low[1]))
        for s in list(self.feet):
            if s not in down:
                del self.feet[s]
            else:
                self.feet[s]["age"] = self.feet[s].get("age", 0) + 1

        if self.cfg["balance"]:
            self._balance(lm)
        if self.cfg["momentum"]:
            self._compress(lm, down, auth_y, dt)
        if self.cfg["secondary"]:
            self._secondary(lm, dt)

        grounded_now = bool(self.feet)
        if grounded_now and self.was_grounded:
            instant = (self.offset - self.prev_offset) / dt
            self.root_v = 0.5 * self.root_v + 0.5 * instant
        self.was_grounded = grounded_now
        self.prev_offset = self.offset.copy()
        com = body.centre_of_mass(lm)
        self.prev_com = body.xz(com)
        self.prev_com_y = float(com[1])
        self.was_planted = set(self.feet)
        diag = self._diag(lm, dt)
        self.prev_locked = {s: body.xz(lm[st["pivot"]]) for s, st in self.feet.items() if st["pivot"] in lm}
        self.last = {k: lm[k].copy() for k in keys if k in lm}
        self.last_diag = diag
        return {"landmarks": body.lists(lm, keys), "diag": diag}

    def _hop(self, dt):
        """Vertical impulse. Grounded downward hits become compression; an upward hit hops, then gravity."""
        g = self.cfg["gravity"]
        if self.was_grounded and self.knock_v[1] < 0 and self.y_off <= 1e-4:
            self.comp_v += -float(self.knock_v[1]) * self.cfg["comp_gain"]
            self.knock_v[1] = 0.0
            return
        if self.y_off > 0 or self.knock_v[1] > 0:
            self.y_off += float(self.knock_v[1]) * dt
            self.knock_v[1] -= g * dt
            if self.y_off <= 0:
                self.comp_v += max(0.0, -float(self.knock_v[1])) * self.cfg["comp_gain"]
                self.y_off = 0.0
                self.knock_v[1] = 0.0

    def _decay_knock(self, dt, grounded):
        speed = math.hypot(float(self.knock_v[0]), float(self.knock_v[2]))
        if speed < 1e-6:
            self.knock_v[0] = 0.0
            self.knock_v[2] = 0.0
            return
        if grounded:
            drop = self.cfg["mu"] * self.cfg["gravity"] * dt
            keep = 0.0 if drop >= speed else (speed - drop) / speed
            self.knock_v[0] *= keep
            self.knock_v[2] *= keep

    def _air(self, lm, dt, grounded):
        """Optional blend toward a ballistic root. Off unless config air_follow > 0."""
        if grounded or self.cfg["air_follow"] <= 0:
            self.takeoff = None
            return
        com_y = float(body.centre_of_mass(lm)[1])
        if self.takeoff is None:
            vy = 0.0 if self.prev_com_y is None else (com_y - self.prev_com_y) / dt
            self.takeoff = (com_y, vy, 0.0)
        y0, vy, t = self.takeoff
        t += dt
        self.takeoff = (y0, vy, t)
        target = y0 + vy * t - 0.5 * self.cfg["gravity"] * t * t
        _translate(lm, np.array([0.0, (target - com_y) * self.cfg["air_follow"], 0.0]))

    def _release_toe_off(self, lm, held):
        """A stance ankle that reverses is leaving. Hand the root to the other foot without a pop."""
        if float(np.linalg.norm(self.offset_step)) <= 0.004 or not held:
            return
        primary = max(held, key=lambda s: self.feet[s].get("age", 0))
        ankle = lm.get(f"ankle_{primary}")
        if ankle is None or "alock" not in self.feet[primary]:
            return
        desired = self.feet[primary]["alock"] - body.xz(ankle) - self.offset
        if float(np.dot(desired, self.offset_step)) >= 0:
            return
        # A foot still on the floor has finished its skate. Handing that step to the
        # other planted foot walks a landing across the room (Jump_Land, 0.20 m).
        # Only a sole that is rising is leaving, and only that one hands the travel on.
        # Both feet down, neither leaving, and the stance ankle has stopped
        # moving in the clip: the skate is over. Passing the step to the other
        # foot is what walked Jump_Land from 0.03 m to 0.20 m. A walk's stance
        # ankle is still sliding, so it keeps the hand-off.
        if (len(held) >= 2 and self.sole_vy.get(primary, 0.0) < 0.15
                and self.ankle_xz_v.get(primary, 1.0) < 0.12):
            self.offset_step[:] = 0.0
            return
        # Keep the plant so the sole stays pinned, but stop it steering the root into reverse.
        self.feet[primary]["drive"] = False
        for s in held:
            if s == primary or not self.feet[s].get("drive", True):
                continue
            ank = lm.get(f"ankle_{s}")
            if ank is None or "alock" not in self.feet[s]:
                continue
            desired = self.feet[s]["alock"] - body.xz(ank) - self.offset
            # Keep a foot that is already on the same travel. Otherwise carry the current step
            # so the hand-off does not stop the body or throw it.
            if float(np.dot(desired, self.offset_step)) < 0 or float(np.linalg.norm(desired - self.offset_step)) > 0.02:
                self.feet[s]["alock"] = body.xz(ank) + self.offset + self.offset_step

    def _lift_sole(self, lm, side, dy):
        ak = f"ankle_{side}"
        before = lm[ak].copy() if ak in lm else None
        hip = lm[f"hip_{side}"].copy() if f"hip_{side}" in lm else None
        knee = lm[f"knee_{side}"].copy() if f"knee_{side}" in lm else None
        up = np.array([0.0, dy, 0.0])
        for k in _foot_keys(lm, side):
            lm[k] = lm[k] + up
        if before is not None and hip is not None and knee is not None and ak in lm:
            lm[f"knee_{side}"] = body.transfer_pole(hip, hip, before, lm[ak], knee)

    def _roll_locks(self, lm, down):
        """Follow the sole point that is on the floor. Keep the foot's current correction so the root does not jump."""
        margin = 0.004
        for s in list(self.feet):
            if s not in down:
                continue
            piv = self.feet[s]["pivot"]
            if piv not in lm:
                continue
            low_key, low = body.lowest_sole(lm, s)
            if low_key is None or low_key == piv:
                continue
            if float(low[1]) > float(lm[piv][1]) - margin and float(lm[piv][1]) <= self.floor + self.cfg["plant_on"]:
                continue
            old = self.feet[s]["lock"] - (body.xz(lm[piv]) + self.offset)
            self.feet[s]["pivot"] = low_key
            self.feet[s]["lock"] = body.xz(low) + self.offset + old

    def _settle_sole(self, lm, side):
        """Rebuild heel and toe from the ankle and ball, then put the contact back on its lock.

        The pass returns ankle and ball only. A later enrich() must reproduce the same sole,
        or the contact we just pinned is not the one the metrics and the viewer see.
        """
        body.resole(lm, self.rest, [f"heel_{side}", f"toe_tip_{side}"])
        st = self.feet.get(side)
        if not st or st["pivot"] not in lm:
            return
        err = _cap(st["lock"] - body.xz(lm[st["pivot"]]), 0.05)
        if float(np.linalg.norm(err)) < 1e-4:
            return
        ak = f"ankle_{side}"
        before = lm[ak].copy() if ak in lm else None
        hip = lm[f"hip_{side}"].copy() if f"hip_{side}" in lm else None
        knee = lm[f"knee_{side}"].copy() if f"knee_{side}" in lm else None
        d = np.array([float(err[0]), 0.0, float(err[1])])
        for k in _foot_keys(lm, side):
            lm[k] = lm[k] + d
        body.resole(lm, self.rest, [f"heel_{side}", f"toe_tip_{side}"])
        if before is not None and hip is not None and knee is not None and ak in lm:
            lm[f"knee_{side}"] = body.transfer_pole(hip, hip, before, lm[ak], knee)

    def _pin_foot(self, lm, side):
        """Move one foot onto its contact lock, capped so a wild clip cannot pull the leg apart."""
        st = self.feet.get(side)
        if not st or st["pivot"] not in lm:
            return
        err = _cap(st["lock"] - body.xz(lm[st["pivot"]]), 0.05)
        if float(np.linalg.norm(err)) < 1e-5:
            return
        d = np.array([float(err[0]), 0.0, float(err[1])])
        ak = f"ankle_{side}"
        before = lm[ak].copy() if ak in lm else None
        hip = lm[f"hip_{side}"].copy() if f"hip_{side}" in lm else None
        knee = lm[f"knee_{side}"].copy() if f"knee_{side}" in lm else None
        for k in _foot_keys(lm, side):
            lm[k] = lm[k] + d
        if before is not None and hip is not None and knee is not None and ak in lm:
            lm[f"knee_{side}"] = body.transfer_pole(hip, hip, before, lm[ak], knee)

    def _coast(self, dt):
        if self.was_grounded:
            self.coast_v = self.root_v.copy()
            self.coast_o = self.offset.copy()
            self.air_t = 0.0
        self.air_t += dt
        self.offset = self.coast_o + self.coast_v * self.air_t

    def _down(self, lm):
        down = {}
        on, off = self.cfg["plant_on"], self.cfg["plant_off"]
        for s in "lr":
            _, p = body.lowest_sole(lm, s)
            if p is None:
                continue
            y = float(p[1])
            if s in self.lifted:
                if y > self.floor + off:
                    self.lifted.discard(s)
                else:
                    continue
            if s in self.feet:
                if y <= self.floor + off:
                    down[s] = True
            elif y <= self.floor + on:
                down[s] = True
        return down

    def _step_swing(self, lm, free, dt):
        if not self.feet or not free:
            return
        com = body.centre_of_mass(lm)
        poly = body.support_polygon(lm, list(self.feet), self.cfg["half_width"])
        _, dist, inside = body.closest_on_poly(poly, body.xz(com))
        if inside or dist < 0.02:
            return
        h = max(float(com[1] - self.floor), 0.3)
        cp = body.xz(com) + self.root_v * math.sqrt(h / self.cfg["gravity"])
        for s in free:
            _, low = body.lowest_sole(lm, s)
            if low is None:
                continue
            height = float(low[1]) - self.floor
            if height > 0.2:
                continue
            w = _smooth(0.2, 0.02, height)
            err = cp - body.xz(lm[f"ankle_{s}"])
            bias = _cap(np.array([err[0], 0.0, err[1]]), min(self.cfg["max_step"], 0.02)) * w
            if float(np.linalg.norm(bias)) < 1e-4:
                continue
            self.step_peak = max(self.step_peak, float(np.linalg.norm(bias)))
            for k in _foot_keys(lm, s) + ([f"knee_{s}"] if f"knee_{s}" in lm else []):
                lm[k] = lm[k] + bias

    def _balance(self, lm):
        if not self.feet:
            return
        com = body.centre_of_mass(lm)
        poly = body.support_polygon(lm, list(self.feet), self.cfg["half_width"])
        target, dist, inside = body.closest_on_poly(poly, body.xz(com))
        if inside or dist < 0.008:
            return
        trial = {k: v.copy() for k, v in lm.items()}
        body.shift_upper(trial, np.array([1.0, 0.0, 0.0]))
        frac = float(body.centre_of_mass(trial)[0] - com[0])
        delta = (target - body.xz(com)) / max(frac, 0.25)
        shift = _cap(np.array([delta[0], 0.0, delta[1]]), min(self.cfg["max_shift"], 0.012))
        shift = self._clamp_reach(lm, shift)
        if float(np.linalg.norm(shift)) < 1e-4:
            return
        self.shift_peak = max(self.shift_peak, float(np.linalg.norm(shift)))
        old_hip = {s: lm[f"hip_{s}"].copy() for s in "lr" if f"hip_{s}" in lm}
        old_ankle = {s: lm[f"ankle_{s}"].copy() for s in "lr" if f"ankle_{s}" in lm}
        old_knee = {s: lm[f"knee_{s}"].copy() for s in "lr" if f"knee_{s}" in lm}
        body.shift_upper(lm, shift)
        for s in old_knee:
            if s in old_hip and s in old_ankle and f"hip_{s}" in lm and f"ankle_{s}" in lm:
                lm[f"knee_{s}"] = body.transfer_pole(old_hip[s], lm[f"hip_{s}"], old_ankle[s], lm[f"ankle_{s}"], old_knee[s])

    def _clamp_reach(self, lm, shift):
        best = 0.0
        lo, hi = 0.0, 1.0
        for _ in range(10):
            mid = 0.5 * (lo + hi)
            if self._reaches(lm, shift * mid):
                best = mid
                lo = mid
            else:
                hi = mid
        return shift * best

    def _reaches(self, lm, delta):
        for s in self.feet:
            reach = body.leg_reach(self.rest, s)
            ak, hk = f"ankle_{s}", f"hip_{s}"
            if reach is None or ak not in lm or hk not in lm:
                continue
            if float(np.linalg.norm(lm[ak] - (lm[hk] + delta))) > reach * 0.98:
                return False
        return True

    def _compress(self, lm, down, vys, dt):
        for s, vy in vys.items():
            if s in self.feet and s not in self.was_planted and vy < -0.55:
                self.comp_v += (-vy) * self.cfg["comp_gain"]
        # Substep. One 30 Hz Euler step damps a landing impulse away before it becomes a drop.
        n = max(1, int(math.ceil(dt / (1.0 / 120.0))))
        h = dt / n
        w = 2 * math.pi * self.cfg["comp_hz"]
        z = self.cfg["comp_zeta"]
        for _ in range(n):
            acc = -w * w * self.c - 2 * z * w * self.comp_v
            self.comp_v += acc * h
            self.c += self.comp_v * h
            self.c = min(self.cfg["max_compression"], max(0.0, self.c))
        self.comp_peak = max(self.comp_peak, self.c)
        if self.c < 1e-4:
            return
        # Each frame starts from the authored pose, so the spring's c is the whole drop, not a delta.
        drop = self._clamp_reach(lm, np.array([0.0, -self.c, 0.0]))
        if float(-drop[1]) < 1e-5:
            return
        old_hip = {s: lm[f"hip_{s}"].copy() for s in "lr" if f"hip_{s}" in lm}
        old_ankle = {s: lm[f"ankle_{s}"].copy() for s in "lr" if f"ankle_{s}" in lm}
        old_knee = {s: lm[f"knee_{s}"].copy() for s in "lr" if f"knee_{s}" in lm}
        body.shift_upper(lm, drop)
        for s in old_knee:
            if s in old_hip and s in old_ankle and f"hip_{s}" in lm and f"ankle_{s}" in lm:
                lm[f"knee_{s}"] = body.transfer_pole(old_hip[s], lm[f"hip_{s}"], old_ankle[s], lm[f"ankle_{s}"], old_knee[s])

    def _secondary(self, lm, dt):
        for s in "lr":
            wk, ek = f"wrist_{s}", f"elbow_{s}"
            if wk not in lm:
                continue
            target = lm[wk].copy()
            sprung = self.wrist[s].step(target, dt)
            if ek in lm:
                length = float(np.linalg.norm(target - lm[ek]))
                d = sprung - lm[ek]
                n = float(np.linalg.norm(d))
                if n > 1e-6 and length > 1e-6:
                    sprung = lm[ek] + d / n * length
            for fk in _fingers(lm, s):
                spr = self.finger[s].get(fk)
                if spr is None:
                    spr = body.Spring(self.cfg["finger_hz"], self.cfg["finger_zeta"], self.cfg["finger_lag"])
                    self.finger[s][fk] = spr
                local = self.finger[s][fk].step(lm[fk] - target, dt)
                lm[fk] = sprung + local
            lm[wk] = sprung
        if "head_joint" in lm:
            target = lm["head_joint"].copy()
            sprung = self.head.step(target, dt)
            if "neck" in lm:
                length = float(np.linalg.norm(target - lm["neck"]))
                d = sprung - lm["neck"]
                n = float(np.linalg.norm(d))
                if n > 1e-6 and length > 1e-6:
                    sprung = lm["neck"] + d / n * length
            delta = sprung - target
            lm["head_joint"] = sprung
            for k in ("crown", "nose"):
                if k in lm:
                    lm[k] = lm[k] + delta

    def _diag(self, lm, dt):
        slip = 0.0
        for s, st in self.feet.items():
            piv = st["pivot"]
            if piv in lm and s in self.prev_locked:
                slip = max(slip, float(np.linalg.norm(body.xz(lm[piv]) - self.prev_locked[s])) / dt)
        pen = 0.0
        for s in "lr":
            _, p = body.lowest_sole(lm, s)
            if p is not None:
                pen = max(pen, self.floor - float(p[1]))
        com = body.centre_of_mass(lm)
        poly = body.support_polygon(lm, list(self.feet), self.cfg["half_width"])
        _, dist, inside = body.closest_on_poly(poly, body.xz(com)) if self.feet else (None, 0.0, True)
        return {
            "plants": "+".join(s for s in "lr" if s in self.feet) or "air",
            "slip_mps": round(slip, 4),
            "penetration_mm": round(max(0.0, pen) * 1000, 3),
            "com_off_m": round(0.0 if inside else dist, 4),
            "compression_m": round(self.c, 4),
            "offset_m": [round(float(x), 4) for x in self.offset],
            "hit_stop": False,
        }


def _unpack(frames):
    if frames and isinstance(frames[0], dict) and isinstance(frames[0].get("landmarks"), dict):
        return [f["landmarks"] for f in frames], True
    return frames, False


def correct(frames, dt=1.0 / 30.0, floor=None, rest=None, config=None, events=None):
    """Landmark frames in, corrected frames out, with before/after diagnostics.

    `frames` is a list of `{landmark: [x, y, z]}` or a list of `{t, landmarks}` samples.
    `events` is a list of `{frame, hit_stop}` and `{frame, impulse: [x, y, z]}` a combo can
    schedule. `dt` is seconds per frame (the live controller runs at 30 Hz).
    """
    from .metrics import measure

    raw, wrapped = _unpack(frames)
    if floor is None:
        floor = body.infer_floor(raw, rest)
    sim = PhysicsPass(floor=floor, rest=rest, config=config)
    by = {}
    for ev in events or []:
        by.setdefault(int(ev["frame"]), []).append(ev)
    out, diags = [], []
    for i, fr in enumerate(raw):
        for ev in by.get(i, []):
            if "hit_stop" in ev:
                sim.hit_stop(ev["hit_stop"])
            if "impulse" in ev:
                sim.impulse(ev["impulse"])
        step = sim.step(fr, dt)
        out.append(step["landmarks"])
        diags.append(step["diag"])
    if wrapped:
        packed = []
        for src, lm in zip(frames, out):
            packed.append({**src, "landmarks": lm})
        out_frames = packed
    else:
        out_frames = out
    return {
        "frames": out_frames,
        "floor": floor,
        "diagnostics": {
            "before": measure(raw, dt, floor, rest),
            "after": measure(out, dt, floor, rest),
            "frames": diags,
            "pass": sim.summary(),
        },
    }
