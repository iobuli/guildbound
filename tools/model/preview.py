"""Offline previews of exported hero GLBs (CPU rasteriser, not the game renderer).

    python -I tools/model/preview.py <out.png> <sex> <part,part,...> [clip lib/name@t ...]

Loads assets/characters/hero/<sex>/body.glb plus the named parts, hides the body groups the
worn garments cover (as the game does), poses them with ual2 rest + clip rotations (by joint
name, pelvis translation copied: the game's convention for UAL clips) and renders front, three-
quarter and back views per pose.
"""
from __future__ import annotations

import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import gbmesh as gm  # noqa: E402
import render  # noqa: E402

ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
ASSETS = os.path.join(ROOT, "assets")


class Part:
    """A loaded GLB: per mesh node, its primitives with material colour / texture."""

    def __init__(self, path, colours=None):
        self.doc = gm.Doc(path)
        self.skin = gm.Skin(self.doc)
        self.items = []
        colours = colours or {}
        for node in self.doc.g.nodes:
            if node.mesh is None:
                continue
            for p in self.doc.g.meshes[node.mesh].primitives:
                a = gm.prim_arrays(self.doc, p)
                mat = self.doc.g.materials[p.material]
                col = list(mat.pbrMetallicRoughness.baseColorFactor or [1, 1, 1, 1])
                for key, c in colours.items():
                    if mat.name == "hero_" + key or mat.name.startswith("hero_" + key):
                        col = list(c) + [1.0]
                tex = None
                ti = mat.pbrMetallicRoughness.baseColorTexture
                if ti is not None:
                    img = self.doc.g.images[self.doc.g.textures[ti.index].source]
                    tex = render.load_tex(self.doc.image_bytes(img)[0])
                vcol = self.doc.acc(p.attributes.COLOR_0) if p.attributes.COLOR_0 is not None else None
                targets = [self.doc.acc(t["POSITION"] if isinstance(t, dict) else t.POSITION).astype(float) for t in (p.targets or [])]
                self.items.append(dict(node=node.name, arr=a, colour=col, tex=tex, vcol=vcol, targets=targets, mat=mat.name))


def srgb_to_lin(c):
    c = np.asarray(c, float)
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def pose_mats(skin, clip=None, t=0.0, donors=None):
    loc = skin.rest_local.copy()
    if clip:
        lib, name = clip.split("/", 1)
        doc, clips, by_name = donors[lib]
        dur, ch = clips[name]
        loc = gm.pose_locals(skin, doc, ch, min(t, dur), by_name)
    world = skin.fk(loc)
    return np.einsum("nij,njk->nik", world, skin.ibm)


def load_donors(libs=("ual1", "ual2")):
    out = {}
    for lib in libs:
        doc = gm.Doc(os.path.join(ASSETS, "anim", f"{lib}.glb"))
        clips = gm.clips(doc)
        by_name = {n.name: i for i, n in enumerate(doc.g.nodes)}
        out[lib] = (doc, clips, by_name)
    return out


def surfaces(parts, mats, morph=None, hidden=()):
    out = []
    for part in parts:
        for it in part.items:
            if it["node"] in hidden:
                continue
            a = it["arr"]
            pos = a["pos"].copy()
            if morph and it["targets"]:
                for k, v in morph.items():
                    if k < len(it["targets"]):
                        pos += v * it["targets"][k]
            p = gm.skin_points(pos, a["joints"], a["weights"], mats)
            n = gm.skin_vectors(a["nrm"], a["joints"], a["weights"], mats)
            col = np.asarray(it["colour"][:3])
            out.append(render.Surface(p, n, a["idx"], color=col ** (1 / 2.2), uv=a["uv"], tex=it["tex"], vcol=it["vcol"]))
    return out


def hidden_nodes(body: Part, worn):
    hid = set()
    for it in body.items:
        n = it["node"]
        if n.startswith("body_hide_") and set(n[len("body_hide_"):].split("+")) & set(worn):
            hid.add(n)
    return hid


def main():
    out, sex, names = sys.argv[1], sys.argv[2], [n for n in sys.argv[3].split(",") if n]
    poses = sys.argv[4:] or [None]
    colours = {"skin": (0.80, 0.60, 0.46)}
    body = Part(os.path.join(ASSETS, "characters", "hero", sex, "body.glb"), colours)
    parts = [body] + [Part(os.path.join(ASSETS, "characters", "hero", sex, f"{n}.glb"), colours) for n in names]
    hid = hidden_nodes(body, names)
    donors = load_donors() if poses != [None] else None
    ims = []
    for pose in poses:
        clip, t = (pose.split("@") + ["0"])[:2] if pose else (None, "0")
        mats = pose_mats(body.skin, clip, float(t), donors)
        s = surfaces(parts, mats, hidden=hid)
        if os.environ.get("VIEW") == "face":
            head = (mats[body.skin.names.index("Head")] @ np.linalg.inv(body.skin.ibm[body.skin.names.index("Head")]))[:3, 3]
            c = head + np.array([0, 0.09, 0.02])
            ims.append(render.render(s, c + [0, 0.02, 0.8], c, size=(300, 300), fov_deg=25))
            ims.append(render.render(s, c + [0.6, 0.05, 0.55], c, size=(300, 300), fov_deg=25))
            ims.append(render.render(s, c + [0.3, 0.1, -0.75], c, size=(300, 300), fov_deg=25))
            continue
        ims.append(render.render(s, (0, 1.0, 4.4), (0, 0.92, 0), size=(300, 520)))
        ims.append(render.render(s, (3.0, 1.2, 3.0), (0, 0.92, 0), size=(300, 520)))
        ims.append(render.render(s, (0, 1.1, -4.4), (0, 0.92, 0), size=(300, 520)))
    render.sheet(ims, cols=3).save(out)


if __name__ == "__main__":
    main()
