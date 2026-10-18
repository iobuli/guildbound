"""Rebind a mesh skinned to a UAL-named skeleton with a different rest pose onto ual2's skeleton.

ual2's skeleton is never changed: the game writes ual2 rest translations and clip rotations into
the bones, so a mesh must sit on ual2's joints in ual2's rest pose and use ual2's inverse bind
matrices. The Quaternius base characters share the joint names, order and axis conventions
(each bone's frame differs from ual2's only by the small turn between their bone directions) but
not the proportions (joint positions differ by up to 15 cm at the fingertips).

Each vertex moves by the weighted sum over its joints j of a displacement that is interpolated
along bone j: the joint's own move at the bone's start, its end joint's move at the end, by the
vertex's projection on the source bone. Fingers and hand also scale across the bone (by the
square root of the length ratio) so a lengthened finger does not look thin. The clavicle's
start uses the neck's move: UAL rigs place the clavicle joint at different depths inside the
chest, which says nothing about where the skin is.
Normals turn with the change of the surface (face normals before / after, per welded vertex).
"""
from __future__ import annotations

import numpy as np

import gbmesh as gm

FINGERS = ["thumb", "index", "middle", "ring", "pinky"]


def end_joint(names: list[str], i: int) -> int | None:
    n = names[i]
    pick = {"root": "pelvis", "pelvis": "spine_01", "spine_01": "spine_02", "spine_02": "spine_03",
            "spine_03": "neck_01", "neck_01": "Head"}
    if n in pick:
        return names.index(pick[n])
    for s in "lr":
        table = {f"clavicle_{s}": f"upperarm_{s}", f"upperarm_{s}": f"lowerarm_{s}", f"lowerarm_{s}": f"hand_{s}",
                 f"hand_{s}": f"middle_01_{s}", f"thigh_{s}": f"calf_{s}", f"calf_{s}": f"foot_{s}",
                 f"foot_{s}": f"ball_{s}", f"ball_{s}": f"ball_leaf_{s}"}
        if n in table:
            return names.index(table[n])
        for f in FINGERS:
            for a, b in [("01", "02"), ("02", "03"), ("03", "04_leaf")]:
                if n == f"{f}_{a}_{s}":
                    return names.index(f"{f}_{b}_{s}")
    return None


def is_hand(name: str) -> bool:
    return name.startswith("hand_") or any(name.startswith(f + "_") for f in FINGERS)


def weld_ids(pos: np.ndarray, eps=1e-5) -> np.ndarray:
    key = np.round(pos / eps).astype(np.int64)
    _, inv = np.unique(key, axis=0, return_inverse=True)
    return inv.reshape(-1)


def welded_normals(pos, tri, ids):
    n_ids = ids.max() + 1
    fn = np.cross(pos[tri[:, 1]] - pos[tri[:, 0]], pos[tri[:, 2]] - pos[tri[:, 0]])
    acc = np.zeros((n_ids, 3))
    for k in range(3):
        np.add.at(acc, ids[tri[:, k]], fn)
    acc /= np.maximum(np.linalg.norm(acc, axis=1, keepdims=True), 1e-12)
    return acc[ids]


def rotate_normals(nrm, before, after):
    """Turn each normal by the rotation that carries `before` onto `after`."""
    out = np.empty_like(nrm)
    for i in range(len(nrm)):
        a, b = before[i], after[i]
        v = np.cross(a, b)
        c = float(a @ b)
        if np.linalg.norm(v) < 1e-9:
            out[i] = nrm[i]
            continue
        vx = np.array([[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]])
        r = np.eye(3) + vx + vx @ vx * (1 / (1 + c))
        out[i] = r @ nrm[i]
    return out / np.maximum(np.linalg.norm(out, axis=1, keepdims=True), 1e-12)


class Rebinder:
    def __init__(self, src: gm.Skin, dst: gm.Skin):
        assert src.names == dst.names, "joint names/order must match ual2"
        self.names = dst.names
        self.ps = np.linalg.inv(src.ibm)[:, :3, 3]  # source bind joint positions
        self.pd = dst.rest_world()[:, :3, 3]
        self.delta = self.pd - self.ps
        self.end = [end_joint(self.names, i) for i in range(len(self.names))]
        neck = self.names.index("neck_01")
        self.start_delta = self.delta.copy()
        for s in "lr":
            self.start_delta[self.names.index(f"clavicle_{s}")] = self.delta[neck]

    def displace(self, pos: np.ndarray, joints: np.ndarray, weights: np.ndarray) -> np.ndarray:
        out = pos.copy()
        for k in range(joints.shape[1]):
            w = weights[:, k]
            live = w > 0
            for j in np.unique(joints[live, k]):
                sel = live & (joints[:, k] == j)
                p = pos[sel]
                e = self.end[j]
                d0 = self.start_delta[j]
                if e is None:
                    d = np.broadcast_to(d0, p.shape)
                    radial_term = 0
                else:
                    a, b = self.ps[j], self.ps[e]
                    ab = b - a
                    L = ab @ ab
                    t = np.clip(((p - a) @ ab) / max(L, 1e-12), 0.0, 1.0)
                    d = d0[None, :] * (1 - t[:, None]) + self.delta[e][None, :] * t[:, None]
                    radial_term = 0
                    if is_hand(self.names[j]):
                        ratio = np.linalg.norm(self.pd[e] - self.pd[j]) / max(np.sqrt(L), 1e-9)
                        sp = np.sqrt(ratio)
                        foot = a + t[:, None] * ab
                        radial_term = (sp - 1.0) * (p - foot)
                out[sel] += w[sel, None] * (d + radial_term)
        return out


def rebind_prim(rb: Rebinder, arr: dict) -> dict:
    """Return a copy of a primitive dict (gbmesh.prim_arrays) moved onto the ual2 skeleton."""
    out = dict(arr)
    ids = weld_ids(arr["pos"])
    before = welded_normals(arr["pos"], arr["idx"], ids)
    out["pos"] = rb.displace(arr["pos"], arr["joints"], arr["weights"])
    after = welded_normals(out["pos"], arr["idx"], ids)
    out["nrm"] = rotate_normals(arr["nrm"], before, after)
    w = arr["weights"] / arr["weights"].sum(1, keepdims=True)
    out["weights"] = w
    return out
