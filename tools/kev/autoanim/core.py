"""Tracker-agnostic motion capture cleanup for Kev Studio.

Input is a timestamped mapping of named body keypoints.  Keypoints may already be
3D, or may be 2D image coordinates with a registered depth map.  Output uses the
landmark names and +Y-up/+Z-forward world convention expected by Kev Studio's
``/api/control`` endpoint and reference files.

Only NumPy is required.  Camera/tracker SDKs belong in small provider adapters and
call :func:`attach_depth`; this keeps the motion pipeline testable without hardware.
"""
from dataclasses import dataclass, field
import math
from typing import Dict, Iterable, Mapping, MutableMapping, Optional, Sequence, Tuple

import numpy as np


Vec3 = np.ndarray


@dataclass(frozen=True)
class CameraIntrinsics:
    """Pinhole camera intrinsics in pixels."""

    fx: float
    fy: float
    cx: float
    cy: float
    width: int
    height: int

    @classmethod
    def from_mapping(cls, value: Mapping[str, float], width: int = 0, height: int = 0):
        w = int(value.get("width", width))
        h = int(value.get("height", height))
        if not w or not h:
            raise ValueError("camera width and height are required")
        # A useful fallback for prerecorded footage without calibration metadata.
        fx = float(value.get("fx", 0.9 * w))
        fy = float(value.get("fy", fx))
        return cls(fx, fy, float(value.get("cx", w / 2)), float(value.get("cy", h / 2)), w, h)


def robust_depth(depth: np.ndarray, u: float, v: float, radius: int = 2, scale: float = 1.0) -> Tuple[Optional[float], float]:
    """Median metric depth near a keypoint and a 0..1 sample-confidence score.

    Zero, negative, NaN and infinite sensor values are ignored.  A MAD pass rejects
    foreground/background edge contamination.  ``scale`` converts source units to m.
    """

    a = np.asarray(depth)
    if a.ndim != 2 or not np.isfinite([u, v]).all():
        return None, 0.0
    x, y = int(round(u)), int(round(v))
    x0, x1 = max(0, x - radius), min(a.shape[1], x + radius + 1)
    y0, y1 = max(0, y - radius), min(a.shape[0], y + radius + 1)
    values = a[y0:y1, x0:x1].astype(float).ravel() * float(scale)
    values = values[np.isfinite(values) & (values > 0.05) & (values < 20.0)]
    if not len(values):
        return None, 0.0
    med = float(np.median(values))
    mad = float(np.median(np.abs(values - med)))
    if mad > 1e-6:
        values = values[np.abs(values - med) <= max(3.5 * mad, 0.015)]
        if len(values):
            med = float(np.median(values))
    area = max((x1 - x0) * (y1 - y0), 1)
    coverage = min(1.0, len(values) / area)
    spread = float(np.median(np.abs(values - med))) if len(values) else 1.0
    confidence = coverage * math.exp(-spread / max(med * 0.02, 0.01))
    return med, float(np.clip(confidence, 0.0, 1.0))


def unproject(u: float, v: float, depth_m: float, intrinsics: CameraIntrinsics) -> Vec3:
    """Image pixel + depth -> camera metres (X right, Y down, Z away)."""

    z = float(depth_m)
    return np.array([(u - intrinsics.cx) * z / intrinsics.fx,
                     (v - intrinsics.cy) * z / intrinsics.fy, z], dtype=float)


def attach_depth(keypoints: Mapping[str, Mapping[str, float]], depth_map: np.ndarray,
                 intrinsics: CameraIntrinsics, depth_scale: float = 1.0,
                 normalized_xy: bool = True, radius: int = 2) -> Dict[str, dict]:
    """Fuse 2D tracker keypoints with an aligned depth frame.

    Result is accepted directly as an ``image_depth`` frame by :class:`MotionPipeline`.
    A keypoint without valid sensor depth is retained with its original relative ``z``
    when present, allowing the pipeline to bridge short depth holes.
    """

    out = {}
    for name, point in keypoints.items():
        x, y = float(point["x"]), float(point["y"])
        u = x * (intrinsics.width - 1) if normalized_xy else x
        v = y * (intrinsics.height - 1) if normalized_xy else y
        z, depth_conf = robust_depth(depth_map, u, v, radius, depth_scale)
        tracker_conf = float(point.get("confidence", point.get("visibility", 1.0)))
        row = {"x": u, "y": v, "confidence": tracker_conf * depth_conf}
        if z is not None:
            row["z"] = z
        elif "z" in point:
            row["relative_z"] = float(point["z"])
            row["confidence"] = tracker_conf * 0.25
        out[name] = row
    return out


@dataclass
class MotionConfig:
    input_space: str = "normalized_image"
    target_height_m: float = 1.75
    preserve_metric_scale: bool = False
    mirror_x: bool = False
    min_confidence: float = 0.25
    hold_seconds: float = 0.18
    min_cutoff_hz: float = 1.4
    velocity_cutoff_hz: float = 1.0
    beta: float = 0.08
    max_speed_m_s: float = 12.0
    plant_speed_m_s: float = 0.18
    release_speed_m_s: float = 1.2
    plant_height_m: float = 0.08
    release_height_m: float = 0.14


def _confidence(raw) -> float:
    if not isinstance(raw, Mapping):
        return 1.0
    vals = [float(raw[k]) for k in ("confidence", "visibility", "presence") if k in raw]
    return min(vals) if vals else 1.0


def _xyz(raw) -> Tuple[float, float, float]:
    if isinstance(raw, Mapping):
        return float(raw["x"]), float(raw["y"]), float(raw.get("z", raw.get("relative_z", 0.0)))
    if len(raw) < 2:
        raise ValueError("keypoint needs x and y")
    return float(raw[0]), float(raw[1]), float(raw[2] if len(raw) > 2 else 0.0)


def _alpha(dt: float, cutoff: float) -> float:
    tau = 1.0 / (2.0 * math.pi * max(cutoff, 1e-4))
    return 1.0 / (1.0 + tau / max(dt, 1e-6))


@dataclass
class _FilterState:
    value: Vec3
    raw: Vec3
    velocity: Vec3
    t: float
    seen_t: float


class AdaptiveFilter:
    """Confidence gate + speed clamp + vector One-Euro filter."""

    def __init__(self, config: MotionConfig):
        self.c = config
        self.state: Dict[str, _FilterState] = {}
        self.clamped = 0
        self.held = 0
        self.dropped = 0

    def update(self, name: str, value: Vec3, confidence: float, t: float) -> Optional[Vec3]:
        value = np.asarray(value, float)
        if value.shape != (3,) or not np.isfinite(value).all():
            confidence = 0.0
        old = self.state.get(name)
        if old is None:
            if confidence < self.c.min_confidence:
                self.dropped += 1
                return None
            self.state[name] = _FilterState(value.copy(), value.copy(), np.zeros(3), t, t)
            return value.copy()
        dt = t - old.t
        if dt <= 0.0 or dt > 1.0:
            if confidence >= self.c.min_confidence:
                self.state[name] = _FilterState(value.copy(), value.copy(), np.zeros(3), t, t)
                return value.copy()
            return None
        if confidence < self.c.min_confidence:
            if t - old.seen_t <= self.c.hold_seconds:
                self.held += 1
                old.value = old.value + old.velocity * min(dt, 1.0 / 30.0)
                old.t = t
                return old.value.copy()
            self.dropped += 1
            old.t = t
            return None
        delta = value - old.raw
        speed = float(np.linalg.norm(delta)) / dt
        if speed > self.c.max_speed_m_s:
            value = old.raw + delta * (self.c.max_speed_m_s * dt / speed)
            self.clamped += 1
        raw_velocity = (value - old.raw) / dt
        av = _alpha(dt, self.c.velocity_cutoff_hz)
        velocity = old.velocity + av * (raw_velocity - old.velocity)
        cutoff = self.c.min_cutoff_hz + self.c.beta * float(np.linalg.norm(velocity))
        ap = _alpha(dt, cutoff)
        filtered = old.value + ap * (value - old.value)
        self.state[name] = _FilterState(filtered, value, velocity, t, t)
        return filtered.copy()


DIRECT = {
    "left_shoulder": "shoulder_l", "right_shoulder": "shoulder_r",
    "left_elbow": "elbow_l", "right_elbow": "elbow_r",
    "left_wrist": "wrist_l", "right_wrist": "wrist_r",
    "left_index": "knuckle_l", "right_index": "knuckle_r",
    "left_hip": "hip_l", "right_hip": "hip_r",
    "left_knee": "knee_l", "right_knee": "knee_r",
    "left_ankle": "ankle_l", "right_ankle": "ankle_r",
    "left_heel": "heel_l", "right_heel": "heel_r",
    "left_foot_index": "ball_l", "right_foot_index": "ball_r",
    "left_toe": "ball_l", "right_toe": "ball_r",
}
KEV_NAMES = set(DIRECT.values()) | {"pelvis", "neck", "head_joint", "crown", "nose", "toe_tip_l", "toe_tip_r"}


class LandmarkMapper:
    def __init__(self, config: MotionConfig):
        self.c = config

    def _point(self, raw, space: str, intr: Optional[CameraIntrinsics], width: int, height: int) -> Vec3:
        x, y, z = _xyz(raw)
        if space == "kev":
            p = np.array([x, y, z], float)
        elif space in ("camera_m", "image_depth"):
            if space == "image_depth":
                if intr is None:
                    raise ValueError("image_depth input needs intrinsics")
                p0 = unproject(x, y, z, intr)
            else:
                p0 = np.array([x, y, z], float)
            p = np.array([p0[0], -p0[1], -p0[2]], float)
        elif space == "normalized_image":
            aspect = width / max(height, 1) if width and height else 16 / 9
            # MediaPipe relative z is negative toward camera.  Kev forward is +Z.
            p = np.array([(x - 0.5) * self.c.target_height_m * aspect,
                          (1.0 - y) * self.c.target_height_m,
                          -z * self.c.target_height_m], float)
        else:
            raise ValueError(f"unknown input space: {space}")
        if self.c.mirror_x:
            p[0] *= -1.0
        return p

    def map(self, frame: Mapping) -> Tuple[Dict[str, Vec3], Dict[str, float]]:
        raw = frame.get("landmarks", {})
        space = frame.get("space", self.c.input_space)
        image = frame.get("image", {})
        width, height = int(image.get("width", 0)), int(image.get("height", 0))
        intr = None
        if space == "image_depth":
            intr = CameraIntrinsics.from_mapping(frame.get("intrinsics", {}), width, height)
        points, conf = {}, {}
        for source, value in raw.items():
            dest = source if source in KEV_NAMES else DIRECT.get(source.lower())
            if not dest:
                continue
            try:
                p = self._point(value, space, intr, width, height)
            except (KeyError, TypeError, ValueError):
                continue
            if np.isfinite(p).all():
                # Duplicate toe aliases keep the higher-confidence observation.
                c = _confidence(value)
                if dest not in points or c > conf[dest]:
                    points[dest], conf[dest] = p, c

        def derive(name: str, names: Sequence[str], weights: Optional[Sequence[float]] = None):
            if not all(n in points for n in names):
                return
            w = np.asarray(weights if weights is not None else np.ones(len(names)), float)
            w /= w.sum()
            points[name] = sum((points[n] * w[i] for i, n in enumerate(names)), np.zeros(3))
            conf[name] = min(conf[n] for n in names)

        derive("pelvis", ("hip_l", "hip_r"))
        derive("neck", ("shoulder_l", "shoulder_r"))
        # Prefer ears for head centre.  Otherwise place it along neck -> nose.
        ear_values = []
        for source in ("left_ear", "right_ear"):
            if source in raw:
                try:
                    ear_values.append((self._point(raw[source], space, intr, width, height), _confidence(raw[source])))
                except (KeyError, TypeError, ValueError):
                    pass
        if len(ear_values) == 2:
            points["head_joint"] = (ear_values[0][0] + ear_values[1][0]) * 0.5
            conf["head_joint"] = min(ear_values[0][1], ear_values[1][1])
        elif "neck" in points and "nose" in points:
            points["head_joint"] = points["neck"] + 0.72 * (points["nose"] - points["neck"])
            conf["head_joint"] = min(conf["neck"], conf["nose"])
        if "head_joint" in points and "neck" in points and "crown" not in points:
            points["crown"] = points["head_joint"] + 0.48 * (points["head_joint"] - points["neck"])
            conf["crown"] = min(conf["head_joint"], conf["neck"])
        for side in "lr":
            ball = f"ball_{side}"
            if ball in points and f"toe_tip_{side}" not in points:
                ankle = points.get(f"ankle_{side}")
                direction = points[ball] - ankle if ankle is not None else np.array([0.0, 0.0, 0.08])
                n = float(np.linalg.norm(direction))
                points[f"toe_tip_{side}"] = points[ball] + direction / max(n, 1e-6) * 0.08
                conf[f"toe_tip_{side}"] = conf[ball] * 0.8
        return points, conf


@dataclass
class _FootState:
    last: Optional[Vec3] = None
    last_t: float = 0.0
    stable: int = 0
    moving: int = 0
    anchor: Optional[Vec3] = None


class FootLocker:
    def __init__(self, config: MotionConfig):
        self.c = config
        self.feet = {"l": _FootState(), "r": _FootState()}
        self.floor = None
        self.locked_frames = 0

    def apply(self, points: MutableMapping[str, Vec3], t: float):
        soles = [points[n][1] for s in "lr" for n in (f"heel_{s}", f"ball_{s}") if n in points]
        if soles:
            low = float(min(soles))
            self.floor = low if self.floor is None else min(low, self.floor + 0.002)
        floor = self.floor if self.floor is not None else 0.0
        for side, state in self.feet.items():
            names = [n for n in (f"heel_{side}", f"ball_{side}", f"ankle_{side}") if n in points]
            if len(names) < 2:
                state.stable = 0
                continue
            center = np.mean([points[n] for n in names], axis=0)
            dt = t - state.last_t if state.last is not None else 0.0
            speed = float(np.linalg.norm(center - state.last)) / dt if dt > 1e-6 else 999.0
            height = min(float(points[n][1] - floor) for n in names)
            planted = height <= self.c.plant_height_m and speed <= self.c.plant_speed_m_s
            state.stable = state.stable + 1 if planted else 0
            if state.anchor is None and state.stable >= 3:
                state.anchor = center.copy()
                state.moving = 0
            if state.anchor is not None:
                release = height > self.c.release_height_m or speed > self.c.release_speed_m_s
                state.moving = state.moving + 1 if release else 0
                if state.moving >= 2:
                    state.anchor = None
                    state.stable = 0
                else:
                    delta = state.anchor - center
                    delta[1] *= 0.65
                    for n in names:
                        points[n] = points[n] + delta
                    knee = f"knee_{side}"
                    hip = f"hip_{side}"
                    if knee in points:
                        points[knee] = points[knee] + delta * 0.3
                    if hip in points:
                        points[hip] = points[hip] + delta * 0.08
                    self.locked_frames += 1
            state.last, state.last_t = center, t


class MotionPipeline:
    """Map, calibrate, smooth and contact-stabilize tracker observations."""

    def __init__(self, config: Optional[MotionConfig] = None):
        self.config = config or MotionConfig()
        self.mapper = LandmarkMapper(self.config)
        self.filter = AdaptiveFilter(self.config)
        self.contacts = FootLocker(self.config)
        self.origin = None
        self.scale = None
        self.frames_in = 0
        self.frames_out = 0

    def _calibrate(self, points: Mapping[str, Vec3]):
        if "pelvis" not in points:
            return
        feet = [points[n][1] for n in ("heel_l", "heel_r", "ball_l", "ball_r", "ankle_l", "ankle_r") if n in points]
        floor = min(feet) if feet else points["pelvis"][1] - self.config.target_height_m * 0.53
        crown = points.get("crown")
        measured = float(crown[1] - floor) if crown is not None else self.config.target_height_m
        if self.config.preserve_metric_scale or measured < 0.2:
            scale = 1.0
        else:
            scale = self.config.target_height_m / measured
        self.origin = np.array([points["pelvis"][0], floor, points["pelvis"][2]], float)
        self.scale = float(np.clip(scale, 0.1, 10.0))

    def process(self, frame: Mapping) -> Optional[dict]:
        self.frames_in += 1
        t = float(frame.get("t", (self.frames_in - 1) / 60.0))
        points, confidence = self.mapper.map(frame)
        if self.origin is None:
            self._calibrate(points)
        if self.origin is None:
            return None
        calibrated = {n: (p - self.origin) * self.scale for n, p in points.items()}
        filtered, kept_conf = {}, {}
        names = set(calibrated) | set(self.filter.state)
        for name in names:
            p = calibrated.get(name, np.zeros(3))
            c = confidence.get(name, 0.0)
            q = self.filter.update(name, p, c, t)
            if q is not None:
                filtered[name] = q
                kept_conf[name] = c
        if "pelvis" not in filtered:
            return None
        self.contacts.apply(filtered, t)
        self.frames_out += 1
        return {"t": t,
                "landmarks": {n: [round(float(x), 6) for x in p] for n, p in sorted(filtered.items())},
                "confidence": {n: round(float(kept_conf.get(n, 0.0)), 4) for n in sorted(filtered)}}

    def stats(self) -> dict:
        return {"frames_in": self.frames_in, "frames_out": self.frames_out,
                "held_points": self.filter.held, "dropped_points": self.filter.dropped,
                "speed_clamps": self.filter.clamped, "foot_locked_frames": self.contacts.locked_frames,
                "scale": self.scale}


def synthetic_frames(count: int = 180, fps: float = 60.0, noise: float = 0.003,
                     dropout: float = 0.0, spikes: float = 0.0, seed: int = 7) -> Iterable[dict]:
    """Deterministic normalized-image walk used by examples and stress tests."""

    rng = np.random.default_rng(seed)
    base = {
        "nose": (0.50, 0.13, -0.03), "left_ear": (0.53, 0.14, 0.0), "right_ear": (0.47, 0.14, 0.0),
        "left_shoulder": (0.57, 0.27, 0.0), "right_shoulder": (0.43, 0.27, 0.0),
        "left_elbow": (0.62, 0.43, -0.01), "right_elbow": (0.38, 0.43, 0.01),
        "left_wrist": (0.64, 0.58, -0.02), "right_wrist": (0.36, 0.58, 0.02),
        "left_index": (0.65, 0.60, -0.03), "right_index": (0.35, 0.60, 0.03),
        "left_hip": (0.54, 0.54, 0.0), "right_hip": (0.46, 0.54, 0.0),
        "left_knee": (0.55, 0.73, 0.0), "right_knee": (0.45, 0.73, 0.0),
        "left_ankle": (0.55, 0.92, 0.0), "right_ankle": (0.45, 0.92, 0.0),
        "left_heel": (0.55, 0.95, 0.02), "right_heel": (0.45, 0.95, 0.02),
        "left_foot_index": (0.56, 0.96, -0.04), "right_foot_index": (0.44, 0.96, -0.04),
    }
    for i in range(count):
        phase = i / fps * 2 * math.pi * 1.2
        row = {}
        for name, xyz in base.items():
            p = np.array(xyz, float)
            if name.startswith("left_") and name not in ("left_heel", "left_foot_index", "left_ankle"):
                p[1] += 0.012 * math.sin(phase)
            if name.startswith("right_") and name not in ("right_heel", "right_foot_index", "right_ankle"):
                p[1] -= 0.012 * math.sin(phase)
            p += rng.normal(0.0, noise, 3)
            conf = 0.98
            if rng.random() < dropout:
                conf = 0.05
            if rng.random() < spikes:
                p += rng.normal(0.0, 3.0, 3)
            row[name] = {"x": float(p[0]), "y": float(p[1]), "z": float(p[2]), "confidence": conf}
        yield {"t": i / fps, "space": "normalized_image", "image": {"width": 1280, "height": 720}, "landmarks": row}
