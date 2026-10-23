"""Paired-animation contact alignment and onion-skin scene manifests."""
import copy
import json
import math

import numpy as np


def sample(reference, t):
    frames = reference["samples"]
    if not frames:
        raise ValueError("empty reference")
    return min(frames, key=lambda s: abs(float(s["t"]) - float(t)))


def _forward(landmarks):
    p = {k: np.asarray(v, float) for k, v in landmarks.items()}
    left = p["shoulder_l"] - p["shoulder_r"]
    up = p["neck"] - p["pelvis"]
    f = np.cross(left, up)
    f[1] = 0.0
    n = float(np.linalg.norm(f))
    return f / n if n > 1e-8 else np.array([0.0, 0.0, 1.0])


def yaw_matrix(angle):
    c, s = math.cos(angle), math.sin(angle)
    return np.array([[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]])


def fit(attacker_landmarks, defender_landmarks, contacts, face_to_face=True):
    """Fit attacker yaw + translation so all named contacts coincide."""

    a = {k: np.asarray(v, float) for k, v in attacker_landmarks.items()}
    d = {k: np.asarray(v, float) for k, v in defender_landmarks.items()}
    src, dst = [], []
    for contact in contacts:
        source, target = contact["source"], contact["target"]
        if source not in a or target not in d:
            raise ValueError(f"contact needs {source!r} and {target!r}")
        src.append(a[source])
        dst.append(d[target] + np.asarray(contact.get("offset", [0, 0, 0]), float))
    src, dst = np.asarray(src), np.asarray(dst)
    if len(src) >= 2 and np.linalg.norm(src[:, [0, 2]].ptp(axis=0)) > 1e-5:
        x = src[:, [0, 2]] - src[:, [0, 2]].mean(axis=0)
        y = dst[:, [0, 2]] - dst[:, [0, 2]].mean(axis=0)
        c = float(np.sum(x[:, 0] * y[:, 0] + x[:, 1] * y[:, 1]))
        s = float(np.sum(x[:, 0] * y[:, 1] - x[:, 1] * y[:, 0]))
        angle = math.atan2(s, c)
    elif face_to_face:
        fa, fd = _forward(a), _forward(d)
        want = -fd
        angle = math.atan2(want[0], want[2]) - math.atan2(fa[0], fa[2])
    else:
        angle = 0.0
    r = yaw_matrix(angle)
    translation = np.mean(dst - (r @ src.T).T, axis=0)
    fitted = (r @ src.T).T + translation
    errors = np.linalg.norm(fitted - dst, axis=1)
    return r, translation, {"mean_m": float(errors.mean()), "max_m": float(errors.max()),
                            "per_contact_m": [float(x) for x in errors]}


def transform_reference(reference, rotation, translation, name=None):
    out = copy.deepcopy(reference)
    for frame in out["samples"]:
        frame["landmarks"] = {k: [round(float(x), 6) for x in rotation @ np.asarray(v, float) + translation]
                              for k, v in frame["landmarks"].items()}
    out["name"] = name or f"{reference.get('name', 'motion')}__aligned"
    out["clip"] = out["name"]
    return out


def align(attacker, defender, contacts, attacker_time, defender_time, attacker_actor=None, defender_actor=None):
    af = sample(attacker, attacker_time)
    df = sample(defender, defender_time)
    rotation, translation, error = fit(af["landmarks"], df["landmarks"], contacts)
    aligned = transform_reference(attacker, rotation, translation)
    yaw = math.atan2(rotation[0, 2], rotation[0, 0])
    inverse_r = rotation.T
    viewer_t = inverse_r @ -translation
    manifest = {
        "kind": "kev_interaction",
        "attacker": attacker_actor or {"reference": attacker.get("name")},
        "defender": defender_actor or {"reference": defender.get("name")},
        "contact_time": {"attacker": attacker_time, "defender": defender_time},
        "contacts": contacts,
        "attacker_transform": {"position": translation.tolist(), "yaw_deg": math.degrees(yaw)},
        "viewer_enemy_transform": {"position": viewer_t.tolist(), "yaw_deg": -math.degrees(yaw)},
        "alignment_error": error,
    }
    return aligned, manifest


def align_files(attacker_path, defender_path, contacts_path, aligned_path, manifest_path,
                attacker_time, defender_time, attacker_actor=None, defender_actor=None):
    with open(attacker_path, encoding="utf-8") as f:
        attacker = json.load(f)
    with open(defender_path, encoding="utf-8") as f:
        defender = json.load(f)
    with open(contacts_path, encoding="utf-8") as f:
        contacts = json.load(f)["contacts"]
    aligned, manifest = align(attacker, defender, contacts, attacker_time, defender_time,
                              attacker_actor, defender_actor)
    for path, value in ((aligned_path, aligned), (manifest_path, manifest)):
        with open(path, "w", encoding="utf-8") as f:
            json.dump(value, f, indent=1)
            f.write("\n")
    return aligned, manifest
