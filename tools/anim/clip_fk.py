#!/usr/bin/env python3
"""clip_fk.py - forward kinematics over the UAL clips, plus the audit metrics.

Reads a GLB's skins/animations directly: local TRS per node per frame (linear on translation and
scale, slerp on rotation), FK down the node hierarchy, then the per-frame numbers section 1 asks
for. Nothing here guesses a coordinate convention: the rig's own axes are detected from the clips
(which way the toes point, which clavicle holds +X, that the head sits above the pelvis) and then
mapped onto the engine's model space (+X = the body's LEFT, +Y up, +Z forward), which is what the
numbers in the JSON are expressed in.
"""
from __future__ import annotations

import math

import numpy as np
from pygltflib import GLTF2

BONES = {
    "root": "root", "pelvis": "pelvis", "spine1": "spine_01", "spine2": "spine_02",
    "spine3": "spine_03", "neck": "neck_01", "head": "Head",
    "clavicle_l": "clavicle_l", "clavicle_r": "clavicle_r",
    "upperarm_l": "upperarm_l", "upperarm_r": "upperarm_r",
    "lowerarm_l": "lowerarm_l", "lowerarm_r": "lowerarm_r",
    "hand_l": "hand_l", "hand_r": "hand_r",
    "thigh_l": "thigh_l", "thigh_r": "thigh_r", "calf_l": "calf_l", "calf_r": "calf_r",
    "foot_l": "foot_l", "foot_r": "foot_r", "ball_l": "ball_l", "ball_r": "ball_r",
}


def quat_mat(q):
    x, y, z, w = q
    n = math.sqrt(x * x + y * y + z * z + w * w) or 1.0
    x, y, z, w = x / n, y / n, z / n, w / n
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w), 0.0],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w), 0.0],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y), 0.0],
        [0.0, 0.0, 0.0, 1.0]])


def slerp_batch(a, b, u):
    """Vectorised slerp over whole channels. The per-frame Python version was the real bottleneck:
    ~15k calls of slerp() cost 6.5 s per clip."""
    a = np.asarray(a, np.float64).reshape(-1, 4)
    b = np.asarray(b, np.float64).reshape(-1, 4)
    u = np.asarray(u, np.float64).reshape(-1)
    d = np.sum(a * b, axis=1)
    nb = np.where(d[:, None] < 0, -b, b)
    d = np.abs(d)
    out = np.empty_like(a)
    near = d > 0.9995
    if near.any():
        out[near] = a[near] + u[near, None] * (nb[near] - a[near])
    far = ~near
    if far.any():
        th = np.arccos(np.clip(d[far], -1.0, 1.0))
        sn = np.maximum(np.sin(th), 1e-9)
        out[far] = (np.sin((1 - u[far]) * th) / sn)[:, None] * a[far] + \
                   (np.sin(u[far] * th) / sn)[:, None] * nb[far]
    return out / np.maximum(np.linalg.norm(out, axis=1, keepdims=True), 1e-9)


def quat_mat_batch(q):
    q = np.asarray(q, np.float64)
    q = q / np.maximum(np.linalg.norm(q, axis=1, keepdims=True), 1e-9)
    x, y, z, w = q[:, 0], q[:, 1], q[:, 2], q[:, 3]
    m = np.zeros((len(q), 4, 4), np.float64)
    m[:, 0, 0] = 1 - 2 * (y * y + z * z); m[:, 0, 1] = 2 * (x * y - z * w); m[:, 0, 2] = 2 * (x * z + y * w)
    m[:, 1, 0] = 2 * (x * y + z * w); m[:, 1, 1] = 1 - 2 * (x * x + z * z); m[:, 1, 2] = 2 * (y * z - x * w)
    m[:, 2, 0] = 2 * (x * z - y * w); m[:, 2, 1] = 2 * (y * z + x * w); m[:, 2, 2] = 1 - 2 * (x * x + y * y)
    m[:, 3, 3] = 1.0
    return m


def trs(t, r, s):
    m = quat_mat(r)
    m[:3, 3] = t
    m[0, :3] *= s[0]
    m[1, :3] *= s[1]
    m[2, :3] *= s[2]
    return m


def slerp(a, b, u):
    a, b = np.asarray(a, float), np.asarray(b, float)
    d = float(np.dot(a, b))
    if d < 0:
        b, d = -b, -d
    if d > 0.9995:
        return a + u * (b - a)
    th = math.acos(max(-1.0, min(1.0, d)))
    return (math.sin((1 - u) * th) * a + math.sin(u * th) * b) / math.sin(th)


class Clip:
    """One animation, sampled at `fps` onto the node hierarchy."""

    def __init__(self, path, name, fps=30.0, gltf=None):
        # Parsing the GLB costs 6.5 s in pygltflib, so pass a preloaded `gltf` when walking many
        # clips out of the same file; otherwise every clip re-parses the whole file.
        self.gltf = gltf if gltf is not None else GLTF2().load(path)
        self._blob = self.gltf.binary_blob()   # cached: rebuilding it per channel cost ~6 s per clip
        self.fps = fps
        self.name = name
        nodes = self.gltf.nodes
        self.node_names = [n.name for n in nodes]
        self.index = {n: i for i, n in enumerate(self.node_names)}
        self.parent = {}
        for i, n in enumerate(nodes):
            for c in (n.children or []):
                self.parent[c] = i
        anim = next(a for a in self.gltf.animations if a.name == name)
        self.channels = []
        self.duration = 0.0
        for ch in anim.channels:
            s = anim.samplers[ch.sampler]
            times = self._read(s.input, 1)[:, 0]   # flatten: scalars, not (N,1)
            vals = self._read(s.output, 4 if ch.target.path == "rotation" else 3)
            self.duration = max(self.duration, float(times[-1]))
            self.channels.append((ch.target.node, ch.target.path, times, vals))
        self.frames = max(1, int(self.duration * fps) + 1)
        self.order = sorted(range(len(self.node_names)), key=lambda i: self._depth(i))

    def _read(self, acc_i, comps):
        acc = self.gltf.accessors[acc_i]
        view = self.gltf.bufferViews[acc.bufferView]
        blob = self._blob
        off = (view.byteOffset or 0) + (acc.byteOffset or 0)   # pygltflib keeps glTF camelCase
        n = acc.count * comps
        arr = np.frombuffer(blob, dtype=np.float32, count=n, offset=off)
        return arr.reshape(acc.count, comps)

    def _depth(self, i):
        d = 0
        while i in self.parent:
            i = self.parent[i]
            d += 1
        return d

    def precompute(self):
        """Sample every channel onto every frame at once, then FK once.

        The first version did searchsorted per channel per frame and rebuilt the hierarchy each
        time: 6.6 s for one 76-frame clip, i.e. ~10 minutes for the library. This samples all
        frames in one pass into T/R/S arrays, composes the local matrices once, then only walks
        the hierarchy per frame.
        """
        tf = np.arange(self.frames, dtype=np.float64) / self.fps
        n = len(self.gltf.nodes)
        T = np.zeros((self.frames, n, 3), np.float64)
        Q = np.zeros((self.frames, n, 4), np.float64)
        S = np.ones((self.frames, n, 3), np.float64)
        for i, node in enumerate(self.gltf.nodes):
            T[:, i] = np.asarray(node.translation if node.translation else [0, 0, 0], np.float64)
            Q[:, i] = np.asarray(node.rotation if node.rotation else [0, 0, 0, 1], np.float64)
            S[:, i] = np.asarray(node.scale if node.scale else [1, 1, 1], np.float64)
        for node, path, times, vals in self.channels:
            i = np.clip(np.searchsorted(times, tf, side="right") - 1, 0, len(times) - 1)
            j = np.minimum(i + 1, len(times) - 1)
            span = np.maximum(times[j] - times[i], 1e-6)
            u = np.clip((tf - times[i]) / span, 0.0, 1.0)
            if path == "rotation":
                Q[:, node] = slerp_batch(vals[i], vals[j], u)
            else:
                v = vals[i] + u[:, None] * (vals[j] - vals[i])
                if path == "translation":
                    T[:, node] = v
                else:
                    S[:, node] = v
        self.L = np.zeros((self.frames, n, 4, 4), np.float64)
        for i in range(n):
            m = quat_mat_batch(Q[:, i])                      # (frames, 4, 4)
            m[:, 0, :3] *= S[:, i, 0:1]
            m[:, 1, :3] *= S[:, i, 1:2]
            m[:, 2, :3] *= S[:, i, 2:3]
            m[:, :3, 3] = T[:, i]
            self.L[:, i] = m
        self._fk_all()

    def _fk_all(self):
        self.W = np.zeros_like(self.L)
        for i in self.order:
            par = self.parent.get(i)
            self.W[:, i] = self.L[:, i] if par is None else np.einsum("nij,njk->nik", self.W[:, par], self.L[:, i])

    def local(self, frame):
        return self.L[frame]

    def world(self, frame):
        return self.W[frame]

    def pos(self, mats, bone):
        i = self.index[BONES[bone]]
        return mats[i][:3, 3].copy()

    def rot(self, mats, bone):
        return mats[self.index[BONES[bone]]][:3, :3].copy()


def detect_axes(clip, frames=(0,)):
    """Which way the rig's own axes point, measured from the clips rather than assumed."""
    w = clip.world(frames[0])
    pel, head = clip.pos(w, "pelvis"), clip.pos(w, "head")
    cl, cr = clip.pos(w, "clavicle_l"), clip.pos(w, "clavicle_r")
    bl, br = clip.pos(w, "ball_l"), clip.pos(w, "ball_r")
    toes = (bl + br) / 2 - pel
    left_sign = float(np.sign(cl[0] - cr[0])) or 1.0
    fwd = toes.copy()
    fwd[1] = 0.0
    fwd /= max(np.linalg.norm(fwd), 1e-9)
    return {"left_sign": left_sign, "forward": fwd, "head_up": float(head[1] - pel[1])}


def engine(v, ax):
    """Rig-space vector -> engine model space: +X left, +Y up, +Z forward."""
    out = np.array([v[0] * ax["left_sign"], v[1], float(np.dot(v[[0, 2]], ax["forward"][[0, 2]]))], float)
    return out


def metrics(clip, frame, ax, lowest=None):
    w = clip.world(frame)
    pel, sp3, head = clip.pos(w, "pelvis"), clip.pos(w, "spine3"), clip.pos(w, "head")
    line = engine(sp3 - pel, ax)
    lat = math.degrees(math.atan2(line[0], max(line[1], 1e-6)))     # + = chest toward the body's right
    pitch = math.degrees(math.atan2(line[2], max(line[1], 1e-6)))   # + = forward
    sh = {"l": clip.pos(w, "upperarm_l")[1], "r": clip.pos(w, "upperarm_r")[1]}
    com = (pel + clip.pos(w, "spine2") + head) / 3.0
    fl, fr = clip.pos(w, "foot_l"), clip.pos(w, "foot_r")
    mid = (fl + fr) / 2.0
    off = engine(com - mid, ax)
    feet = {}
    for s, fb in (("l", "foot_l"), ("r", "foot_r")):
        f = clip.pos(w, fb)
        b = clip.pos(w, "ball_" + s)
        base = lowest if lowest is not None else 0.0
        feet[s] = {"foot_h": float(f[1] - base), "ball_h": float(b[1] - base)}
    hands = {}
    for s in ("l", "r"):
        h = clip.pos(w, "hand_" + s)
        hands[s] = {"h": float(h[1]), "chest_d": float(np.linalg.norm(h - sp3))}
    root = clip.pos(w, "root")
    root_m = clip.rot(w, "root")
    yaw = math.atan2(root_m[0, 2], root_m[2, 2])
    return {"lean": lat, "pitch": pitch, "shoulder_diff": float(sh["l"] - sh["r"]),
            "com_off": [float(off[0]), float(off[2])], "feet": feet, "hands": hands,
            "root": {"pos": root.tolist(), "yaw": float(yaw)}}
