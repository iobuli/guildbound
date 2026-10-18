"""Shared glTF I/O, skeleton and skinning helpers for the model tools (`tools/model/`).

Reads .glb and .gltf (external .bin / images), gives numpy arrays per accessor, the node
hierarchy with rest TRS, forward kinematics, linear blend skinning and animation sampling.
Writes GLBs with embedded buffers and images.  numpy + pygltflib only.
"""
from __future__ import annotations

import base64
import json
import os
import struct

import numpy as np
import pygltflib

COMP = {5126: np.float32, 5123: np.uint16, 5125: np.uint32, 5121: np.uint8, 5122: np.int16, 5120: np.int8}
NCOMP = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


class Doc:
    """A loaded glTF document plus raw buffer bytes."""

    def __init__(self, path: str):
        self.path = path
        self.dir = os.path.dirname(os.path.abspath(path))
        self.g = pygltflib.GLTF2().load(path)
        self.buffers = []
        for b in self.g.buffers:
            if b.uri is None:
                self.buffers.append(bytes(self.g.binary_blob()))
            elif b.uri.startswith("data:"):
                self.buffers.append(base64.b64decode(b.uri.split(",", 1)[1]))
            else:
                with open(os.path.join(self.dir, b.uri), "rb") as f:
                    self.buffers.append(f.read())

    # -- accessors ------------------------------------------------------------------------------
    def acc(self, i: int) -> np.ndarray:
        a = self.g.accessors[i]
        bv = self.g.bufferViews[a.bufferView]
        data = self.buffers[bv.buffer]
        dt = np.dtype(COMP[a.componentType])
        n = NCOMP[a.type]
        item = dt.itemsize * n
        stride = bv.byteStride or item
        off = (bv.byteOffset or 0) + (a.byteOffset or 0)
        if stride == item:
            arr = np.frombuffer(data, dtype=dt, count=a.count * n, offset=off).reshape(a.count, n)
        else:
            raw = np.frombuffer(data, dtype=np.uint8, count=stride * (a.count - 1) + item, offset=off)
            arr = np.stack([np.frombuffer(raw[k * stride:k * stride + item].tobytes(), dt) for k in range(a.count)])
        arr = np.array(arr)
        if a.normalized and dt.kind in "ui":
            arr = arr.astype(np.float64) / np.iinfo(dt).max
        return arr

    def image_bytes(self, img) -> tuple[bytes, str]:
        if img.bufferView is not None:
            bv = self.g.bufferViews[img.bufferView]
            data = self.buffers[bv.buffer][(bv.byteOffset or 0):(bv.byteOffset or 0) + bv.byteLength]
            return bytes(data), img.mimeType or "image/png"
        uri = img.uri
        if uri.startswith("data:"):
            head, b = uri.split(",", 1)
            return base64.b64decode(b), head[5:].split(";")[0]
        p = os.path.join(self.dir, uri)
        if not os.path.exists(p):
            # Some Quaternius exports reference "<name>_png.png" next to a "<name>.png".
            alt = p.replace("_png.png", ".png")
            if os.path.exists(alt):
                p = alt
            else:
                raise FileNotFoundError(p)
        with open(p, "rb") as f:
            return f.read(), "image/png"

    # -- hierarchy ------------------------------------------------------------------------------
    def parents(self) -> list[int | None]:
        par = [None] * len(self.g.nodes)
        for i, n in enumerate(self.g.nodes):
            for c in n.children or []:
                par[c] = i
        return par

    def local(self, i: int) -> np.ndarray:
        n = self.g.nodes[i]
        if n.matrix:
            return np.array(n.matrix, dtype=np.float64).reshape(4, 4).T
        return trs(n.translation or [0, 0, 0], n.rotation or [0, 0, 0, 1], n.scale or [1, 1, 1])

    def world_rest(self) -> np.ndarray:
        par = self.parents()
        out = [None] * len(self.g.nodes)

        def w(i):
            if out[i] is None:
                out[i] = self.local(i) if par[i] is None else w(par[i]) @ self.local(i)
            return out[i]

        for i in range(len(self.g.nodes)):
            w(i)
        return np.stack(out)

    def node_index(self, name: str) -> int:
        for i, n in enumerate(self.g.nodes):
            if n.name == name:
                return i
        raise KeyError(name)


# -- math ---------------------------------------------------------------------------------------
def quat_mat(q) -> np.ndarray:
    x, y, z, w = q
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])


def trs(t, r, s) -> np.ndarray:
    m = np.eye(4)
    m[:3, :3] = quat_mat(r) * np.asarray(s, dtype=np.float64)[None, :]
    m[:3, 3] = t
    return m


def slerp(a, b, t):
    a = np.asarray(a, float); b = np.asarray(b, float)
    d = float(np.dot(a, b))
    if d < 0:
        b = -b; d = -d
    if d > 0.9995:
        r = a + t * (b - a)
        return r / np.linalg.norm(r)
    th = np.arccos(d)
    return (np.sin((1 - t) * th) * a + np.sin(t * th) * b) / np.sin(th)


# -- skin ---------------------------------------------------------------------------------------
class Skin:
    """Skin 0 of a document: joint node indices, names, inverse bind matrices."""

    def __init__(self, doc: Doc, k: int = 0):
        s = doc.g.skins[k]
        self.doc = doc
        self.joints = list(s.joints)
        self.names = [doc.g.nodes[j].name for j in self.joints]
        self.ibm = doc.acc(s.inverseBindMatrices).reshape(-1, 4, 4).transpose(0, 2, 1).astype(np.float64)
        par = doc.parents()
        jset = {j: n for n, j in enumerate(self.joints)}
        self.parent = [jset.get(par[j]) for j in self.joints]
        # Chain above the skeleton root (e.g. Armature, root's own rotation is a joint).
        root = self.joints[0]
        chain = []
        p = par[root]
        while p is not None:
            chain.append(p)
            p = par[p]
        base = np.eye(4)
        for p in reversed(chain):
            base = base @ doc.local(p)
        self.base = base
        self.rest_local = np.stack([doc.local(j) for j in self.joints])
        self.rest_trs = []
        for j in self.joints:
            n = doc.g.nodes[j]
            self.rest_trs.append((np.array(n.translation or [0, 0, 0], float), np.array(n.rotation or [0, 0, 0, 1], float),
                                  np.array(n.scale or [1, 1, 1], float)))

    def fk(self, local: np.ndarray) -> np.ndarray:
        out = np.zeros_like(local)
        for i, p in enumerate(self.parent):
            out[i] = (self.base if p is None else out[p]) @ local[i]
        return out

    def rest_world(self) -> np.ndarray:
        return self.fk(self.rest_local)


def skin_points(pos: np.ndarray, joints: np.ndarray, weights: np.ndarray, mats: np.ndarray) -> np.ndarray:
    """Linear blend skinning. mats: (J,4,4) = world_now @ ibm."""
    p = np.concatenate([pos, np.ones((len(pos), 1))], 1)
    out = np.zeros((len(pos), 3))
    for k in range(joints.shape[1]):
        m = mats[joints[:, k]]
        out += weights[:, k:k + 1] * np.einsum("nij,nj->ni", m, p)[:, :3]
    return out


def skin_vectors(vec: np.ndarray, joints: np.ndarray, weights: np.ndarray, mats: np.ndarray) -> np.ndarray:
    out = np.zeros((len(vec), 3))
    for k in range(joints.shape[1]):
        m = mats[joints[:, k]][:, :3, :3]
        out += weights[:, k:k + 1] * np.einsum("nij,nj->ni", m, vec)
    return out


# -- animation ----------------------------------------------------------------------------------
def clips(doc: Doc):
    """{name: (duration, {node_index: {path: (times, values)}})}"""
    out = {}
    for a in doc.g.animations:
        ch = {}
        dur = 0.0
        for c in a.channels:
            s = a.samplers[c.sampler]
            t = doc.acc(s.input)[:, 0].astype(np.float64)
            v = doc.acc(s.output).astype(np.float64)
            if s.interpolation == "CUBICSPLINE":
                v = v.reshape(len(t), 3, -1)[:, 1]
            ch.setdefault(c.target.node, {})[c.target.path] = (t, v)
            dur = max(dur, float(t[-1]))
        out[a.name] = (dur, ch)
    return out


def sample(track, t):
    times, vals = track
    if t <= times[0]:
        return vals[0]
    if t >= times[-1]:
        return vals[-1]
    k = int(np.searchsorted(times, t)) - 1
    u = (t - times[k]) / max(times[k + 1] - times[k], 1e-9)
    if vals.shape[1] == 4:
        return slerp(vals[k], vals[k + 1], u)
    return vals[k] * (1 - u) + vals[k + 1] * u


def pose_locals(skin: Skin, donor_doc: Doc, channels, t, names_to_donor_node) -> np.ndarray:
    """Local matrices of `skin`'s joints at time t, rotations copied by name from the donor,
    translations kept at our rest except the pelvis (copied, like the game's `Pose.pelvis`)."""
    loc = skin.rest_local.copy()
    for i, name in enumerate(skin.names):
        node = names_to_donor_node.get(name)
        if node is None or node not in channels:
            continue
        tr, r, s = skin.rest_trs[i]
        ch = channels[node]
        if "rotation" in ch:
            r = sample(ch["rotation"], t)
        if name == "pelvis" and "translation" in ch:
            tr = sample(ch["translation"], t)
        loc[i] = trs(tr, r, s)
    return loc


# -- writing ------------------------------------------------------------------------------------
class Writer:
    """Accumulates one GLB: buffer views, accessors, meshes, materials, images, nodes."""

    def __init__(self):
        self.blob = bytearray()
        self.g = {"asset": {"version": "2.0", "generator": "guildbound tools/model"},
                  "buffers": [], "bufferViews": [], "accessors": [], "meshes": [], "materials": [],
                  "nodes": [], "skins": [], "scenes": [{"nodes": []}], "scene": 0,
                  "images": [], "textures": [], "samplers": []}

    def _view(self, data: bytes, target=None) -> int:
        while len(self.blob) % 4:
            self.blob.append(0)
        v = {"buffer": 0, "byteOffset": len(self.blob), "byteLength": len(data)}
        if target:
            v["target"] = target
        self.blob += data
        self.g["bufferViews"].append(v)
        return len(self.g["bufferViews"]) - 1

    def accessor(self, arr: np.ndarray, kind: str, comp=5126, target=None, minmax=False, normalized=False) -> int:
        dt = COMP[comp]
        a = np.ascontiguousarray(arr, dtype=dt)
        n = NCOMP[kind]
        count = a.size // n
        view = self._view(a.tobytes(), target)
        acc = {"bufferView": view, "componentType": comp, "count": int(count), "type": kind}
        if normalized:
            acc["normalized"] = True
        if minmax:
            r = a.reshape(count, n)
            acc["min"] = [float(x) for x in r.min(0)]
            acc["max"] = [float(x) for x in r.max(0)]
        self.g["accessors"].append(acc)
        return len(self.g["accessors"]) - 1

    def image(self, data: bytes, mime="image/png", name=None) -> int:
        v = self._view(data)
        img = {"bufferView": v, "mimeType": mime}
        if name:
            img["name"] = name
        self.g["images"].append(img)
        if not self.g["samplers"]:
            self.g["samplers"].append({"magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497})
        self.g["textures"].append({"sampler": 0, "source": len(self.g["images"]) - 1})
        return len(self.g["textures"]) - 1

    def material(self, name, color, rough=0.85, metal=0.0, base_tex=None, normal_tex=None, double=False) -> int:
        pbr = {"baseColorFactor": [float(c) for c in color], "metallicFactor": float(metal), "roughnessFactor": float(rough)}
        if base_tex is not None:
            pbr["baseColorTexture"] = {"index": base_tex}
        m = {"name": name, "pbrMetallicRoughness": pbr}
        if normal_tex is not None:
            m["normalTexture"] = {"index": normal_tex}
        if double:
            m["doubleSided"] = True
        self.g["materials"].append(m)
        return len(self.g["materials"]) - 1

    def mesh(self, name, prims) -> int:
        """prims: list of dicts with pos, nrm, uv(optional), joints (N,4), weights (N,4), idx, material."""
        out = []
        for p in prims:
            attrs = {
                "POSITION": self.accessor(p["pos"], "VEC3", target=34962, minmax=True),
                "NORMAL": self.accessor(p["nrm"], "VEC3", target=34962),
                "JOINTS_0": self.accessor(p["joints"].astype(np.uint16), "VEC4", comp=5123, target=34962),
                "WEIGHTS_0": self.accessor(p["weights"], "VEC4", target=34962),
            }
            if p.get("uv") is not None:
                attrs["TEXCOORD_0"] = self.accessor(p["uv"], "VEC2", target=34962)
            if p.get("color") is not None:
                attrs["COLOR_0"] = self.accessor(p["color"], "VEC4", target=34962)
            idx = np.asarray(p["idx"], dtype=np.uint32).reshape(-1)
            out.append({"attributes": attrs, "indices": self.accessor(idx, "SCALAR", comp=5125, target=34963),
                        "material": p["material"], "mode": 4})
        self.g["meshes"].append({"name": name, "primitives": out})
        return len(self.g["meshes"]) - 1

    def node(self, **kw) -> int:
        self.g["nodes"].append({k: v for k, v in kw.items() if v is not None})
        return len(self.g["nodes"]) - 1

    def save(self, path: str):
        self.g["buffers"] = [{"byteLength": len(self.blob)}]
        for k in ["images", "textures", "samplers", "skins", "materials"]:
            if not self.g[k]:
                del self.g[k]
        js = json.dumps(self.g, separators=(",", ":")).encode()
        while len(js) % 4:
            js += b" "
        while len(self.blob) % 4:
            self.blob.append(0)
        total = 12 + 8 + len(js) + 8 + len(self.blob)
        with open(path, "wb") as f:
            f.write(struct.pack("<III", 0x46546C67, 2, total))
            f.write(struct.pack("<II", len(js), 0x4E4F534A))
            f.write(js)
            f.write(struct.pack("<II", len(self.blob), 0x004E4942))
            f.write(self.blob)


def copy_skeleton(w: Writer, src: Doc, skin: Skin, ibm=None, extra_children=None):
    """Copy the skeleton nodes (Armature, root chain, joints) of `src` into `w`, same names,
    same order of joints, same TRS. Returns (armature node, {src node: new node}, skin index).
    `extra_children`: {joint name: [new node indices]} appended as children of that joint."""
    par = src.parents()
    keep = set(skin.joints)
    # ancestors of the root joint
    p = par[skin.joints[0]]
    chain = []
    while p is not None:
        chain.append(p)
        p = par[p]
    keep |= set(chain)
    mapping = {}
    order = list(reversed(chain)) + skin.joints
    for i in order:
        n = src.g.nodes[i]
        kw = {"name": n.name}
        if n.matrix:
            kw["matrix"] = list(n.matrix)
        else:
            if n.translation:
                kw["translation"] = list(n.translation)
            if n.rotation:
                kw["rotation"] = list(n.rotation)
            if n.scale:
                kw["scale"] = list(n.scale)
        mapping[i] = w.node(**kw)
    for i in order:
        kids = [mapping[c] for c in (src.g.nodes[i].children or []) if c in mapping]
        if extra_children and src.g.nodes[i].name in extra_children:
            kids += extra_children[src.g.nodes[i].name]
        if kids:
            w.g["nodes"][mapping[i]]["children"] = kids
    mats = skin.ibm if ibm is None else ibm
    acc = w.accessor(mats.transpose(0, 2, 1).reshape(-1, 16), "MAT4")
    w.g["skins"].append({"inverseBindMatrices": acc, "joints": [mapping[j] for j in skin.joints],
                         "skeleton": mapping[order[0]] if chain else mapping[skin.joints[0]]})
    top = mapping[order[0]]
    return top, mapping, len(w.g["skins"]) - 1


def prim_arrays(doc: Doc, prim) -> dict:
    a = prim.attributes
    out = {"pos": doc.acc(a.POSITION).astype(np.float64), "nrm": doc.acc(a.NORMAL).astype(np.float64),
           "idx": doc.acc(prim.indices).reshape(-1, 3).astype(np.int64),
           "joints": doc.acc(a.JOINTS_0).astype(np.int64), "weights": doc.acc(a.WEIGHTS_0).astype(np.float64),
           "material": prim.material}
    out["uv"] = doc.acc(a.TEXCOORD_0).astype(np.float64) if a.TEXCOORD_0 is not None else None
    return out


def vertex_normals(pos, tri):
    n = np.zeros_like(pos)
    fn = np.cross(pos[tri[:, 1]] - pos[tri[:, 0]], pos[tri[:, 2]] - pos[tri[:, 0]])
    for k in range(3):
        np.add.at(n, tri[:, k], fn)
    l = np.linalg.norm(n, axis=1, keepdims=True)
    return n / np.maximum(l, 1e-12)
