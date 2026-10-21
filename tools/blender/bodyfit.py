"""Test mannequin lofted from the proportion chart's silhouettes, with deformation-friendly topology.

    blender -b --factory-startup --python tools/blender/bodyfit.py -- [--out assets/anim/blocky.glb] [--render PREFIX]

Profiles were measured off the chart's male figure (front silhouette = width, side silhouette = depth, 734 px =
1.8315 m; see the table below). Each chain is a loft of superellipse rings (12 verts, exponent 3: squarish with soft
corners, the planar-anatomy look), a ring every `STEP` m along the bones plus extra loops either side of each joint,
all quads, capped at the ends. Torso rings are placed by height; limb profiles by the fraction along the chain
(hip -> knee -> ankle, shoulder -> elbow -> wrist), because the chart's joints sit at slightly different heights than
the UAL skeleton's. Skin weights blend linearly across each joint (`BLEND` m either side), so bends fold instead of
the blocks separating. Every UAL / karate animation plays on it unchanged.
"""
import math
import sys

import bpy
from mathutils import Vector

UAL = "/home/user/gb_e7/assets/anim/ual2.glb"
STEP = 0.03
BLEND = 0.05
SEG = 12
EXP = 3.0
# Torso by height (z, width, depth) and head, measured from the chart.
TORSO = [(0.86, 0.33, 0.18), (0.92, 0.36, 0.20), (1.00, 0.29, 0.19), (1.10, 0.20, 0.13), (1.20, 0.22, 0.15), (1.28, 0.33, 0.21),
         (1.36, 0.56, 0.25), (1.44, 0.60, 0.25), (1.49, 0.64, 0.20), (1.53, 0.40, 0.15)]
HEAD = [(1.53, 0.24, 0.13), (1.58, 0.19, 0.14), (1.64, 0.15, 0.18), (1.72, 0.155, 0.19), (1.79, 0.14, 0.17), (1.83, 0.08, 0.09)]
# Limbs by fraction along the chain (t, width, depth); the joint sits at t = 0.5.
LEG = [(0.0, 0.17, 0.14), (0.15, 0.14, 0.12), (0.35, 0.11, 0.11), (0.5, 0.06, 0.075), (0.6, 0.14, 0.11), (0.72, 0.13, 0.10),
       (0.86, 0.08, 0.07), (1.0, 0.06, 0.06)]
ARM = [(0.0, 0.24, 0.19), (0.2, 0.15, 0.13), (0.45, 0.11, 0.10), (0.55, 0.16, 0.13), (0.75, 0.13, 0.10), (1.0, 0.07, 0.055)]
COLOURS = {"torso": (0.85, 0.55, 0.50), "head": (0.80, 0.70, 0.72), "arm": (0.92, 0.62, 0.58), "leg": (0.88, 0.52, 0.48),
           "end": (0.95, 0.80, 0.75)}


def lerp_profile(table, x):
    for (a, wa, da), (b, wb, db) in zip(table, table[1:]):
        if a <= x <= b:
            u = (x - a) / (b - a) if b > a else 0.0
            return wa + (wb - wa) * u, da + (db - da) * u
    return (table[0][1], table[0][2]) if x < table[0][0] else (table[-1][1], table[-1][2])


def ring(c, axis, front, w, d):
    side = axis.cross(front).normalized()
    pts = []
    for k in range(SEG):
        a = 2 * math.pi * k / SEG
        ca, sa = math.cos(a), math.sin(a)
        x = math.copysign(abs(ca) ** (2 / EXP), ca) * w / 2
        y = math.copysign(abs(sa) ** (2 / EXP), sa) * d / 2
        pts.append(c + side * x + front * y)
    return pts


class Builder:
    def __init__(self):
        self.verts, self.faces, self.weights, self.mats = [], [], [], []

    def loft(self, rings, wts, mat):
        """rings: list of vertex lists; wts: per ring {bone: weight}. Quads between rings, fans at the ends."""
        base = len(self.verts)
        for r, w in zip(rings, wts):
            for p in r:
                self.verts.append(p)
                self.weights.append(w)
        n = len(rings)
        for i in range(n - 1):
            for k in range(SEG):
                a, b = base + i * SEG + k, base + i * SEG + (k + 1) % SEG
                self.faces.append((a, b, b + SEG, a + SEG))
                self.mats.append(mat)
        for i, flip in ((0, True), (n - 1, False)):
            c = sum(rings[i], Vector()) / SEG
            ci = len(self.verts)
            self.verts.append(c)
            self.weights.append(wts[i])
            for k in range(SEG):
                a, b = base + i * SEG + k, base + i * SEG + (k + 1) % SEG
                self.faces.append((ci, b, a) if flip else (ci, a, b))
                self.mats.append(mat)

    def box(self, c0, c1, front, w0, d0, w1, d1, wts, mat):
        axis = (c1 - c0).normalized()
        f = (front - axis * front.dot(axis)).normalized()
        self.loft([ring(c0, axis, f, w0, d0), ring(c1, axis, f, w1, d1)], [wts, wts], mat)


def blend(a, b, s, joint, width=BLEND):
    """Weights across a joint at arclength `joint`: all `a` before, all `b` after, linear over +-width."""
    u = min(1.0, max(0.0, (s - (joint - width)) / (2 * width)))
    return {a: 1 - u, b: u} if 0 < u < 1 else ({a: 1.0} if u == 0 else {b: 1.0})


def chain(bld, pts, bones, table, front, mat, end_pad=0.0):
    """A limb loft through joint points pts[0..2] (bones[0] for the first segment, bones[1] for the second)."""
    l1, l2 = (pts[1] - pts[0]).length, (pts[2] - pts[1]).length
    total = l1 + l2 + end_pad
    samples = sorted(set([i * STEP for i in range(int(total / STEP) + 1)] + [total, l1 - BLEND, l1 - 0.015, l1, l1 + 0.015, l1 + BLEND]))
    rings, wts = [], []
    for s in samples:
        if s <= l1:
            c, axis = pts[0].lerp(pts[1], s / l1), (pts[1] - pts[0]).normalized()
            t = 0.5 * s / l1
        else:
            u = (s - l1) / l2
            axis = (pts[2] - pts[1]).normalized()
            c = pts[1] + axis * (s - l1)
            t = 0.5 + 0.5 * min(u, 1.0)
        w, d = lerp_profile(table, t)
        f = (front - axis * front.dot(axis)).normalized()
        rings.append(ring(c, axis, f, w, d))
        wts.append(blend(bones[0], bones[1], s, l1))
    bld.loft(rings, wts, mat)


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opt = {argv[i]: argv[i + 1] for i in range(0, len(argv) - 1, 2)}
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=UAL)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    for o in list(bpy.data.objects):
        if o.type == "MESH":
            bpy.data.objects.remove(o)
    for a in list(bpy.data.actions):
        bpy.data.actions.remove(a)
    H = {b.name: arm.matrix_world @ b.head_local for b in arm.data.bones}
    T = {b.name: arm.matrix_world @ b.tail_local for b in arm.data.bones}
    mats = list(COLOURS)
    bld = Builder()
    up, fwd = Vector((0, 0, 1)), Vector((0, -1, 0))
    # Torso: rings by height, weights blended between the spine bones by height.
    spine = [("pelvis", H["pelvis"].z), ("spine_01", H["spine_01"].z), ("spine_02", H["spine_02"].z), ("spine_03", H["spine_03"].z), ("neck_01", H["neck_01"].z)]

    def spine_w(z):
        for (a, za), (b, zb) in zip(spine, spine[1:]):
            if z <= zb:
                return blend(a, b, z, zb, 0.04) if z > za else {a: 1.0}
        return {"neck_01": 1.0}

    zs = [TORSO[0][0] + i * STEP for i in range(int((TORSO[-1][0] - TORSO[0][0]) / STEP) + 1)] + [TORSO[-1][0]]
    bld.loft([ring(Vector((0, 0.01, z)), up, fwd, *lerp_profile(TORSO, z)) for z in zs], [spine_w(z) for z in zs], mats.index("torso"))
    zs = [HEAD[0][0] + i * STEP for i in range(int((HEAD[-1][0] - HEAD[0][0]) / STEP) + 1)] + [HEAD[-1][0]]
    bld.loft([ring(Vector((0, 0.0, z)), up, fwd, *lerp_profile(HEAD, z)) for z in zs],
             [blend("neck_01", "Head", z, H["Head"].z, 0.03) for z in zs], mats.index("head"))
    for s in "lr":
        chain(bld, [H[f"thigh_{s}"], H[f"calf_{s}"], H[f"foot_{s}"]], [f"thigh_{s}", f"calf_{s}"], LEG, fwd, mats.index("leg"))
        chain(bld, [H[f"upperarm_{s}"], H[f"lowerarm_{s}"], H[f"hand_{s}"]], [f"upperarm_{s}", f"lowerarm_{s}"], ARM, fwd, mats.index("arm"))
        # Hand: a flat mitten box; foot: heel-to-ball and toe boxes.
        hand_dir = (T[f"hand_{s}"] - H[f"hand_{s}"]).normalized()
        bld.box(H[f"hand_{s}"], H[f"hand_{s}"] + hand_dir * 0.17, up, 0.09, 0.04, 0.10, 0.03, {f"hand_{s}": 1.0}, mats.index("end"))
        heel = H[f"foot_{s}"] + Vector((0, 0.06, -0.06))
        bld.box(heel, H[f"ball_{s}"], up, 0.08, 0.08, 0.10, 0.05, {f"foot_{s}": 1.0}, mats.index("end"))
        bld.box(H[f"ball_{s}"], H[f"ball_{s}"] + Vector((0, -0.07, 0)), up, 0.10, 0.04, 0.09, 0.03, {f"ball_{s}": 1.0}, mats.index("end"))
    mesh = bpy.data.meshes.new("Body")
    mesh.from_pydata([tuple(v) for v in bld.verts], [], bld.faces)
    for k in mats:
        m = bpy.data.materials.new(k)
        m.diffuse_color = (*COLOURS[k], 1)
        m.use_nodes = True
        m.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (*COLOURS[k], 1)
        mesh.materials.append(m)
    for poly, mi in zip(mesh.polygons, bld.mats):
        poly.material_index = mi
    mesh.validate()
    obj = bpy.data.objects.new("Body", mesh)
    bpy.context.scene.collection.objects.link(obj)
    groups = {}
    for i, w in enumerate(bld.weights):
        for bone, v in w.items():
            if bone not in groups:
                groups[bone] = obj.vertex_groups.new(name=bone)
            groups[bone].add([i], v, "REPLACE")
    obj.parent = arm
    obj.modifiers.new("Armature", "ARMATURE").object = arm
    # Flat shading with an edge-split look: planar anatomy.
    for p in mesh.polygons:
        p.use_smooth = False
    if "--render" in opt:
        sc = bpy.context.scene
        cam = bpy.data.cameras.new("c")
        cam.type = "ORTHO"
        cam.ortho_scale = 2.1
        co = bpy.data.objects.new("c", cam)
        sc.collection.objects.link(co)
        sc.camera = co
        sc.render.engine = "BLENDER_WORKBENCH"
        sc.display.shading.color_type = "MATERIAL"
        sc.view_settings.view_transform = "Standard"
        sc.render.film_transparent = True
        sc.render.resolution_x, sc.render.resolution_y = 800, 1000
        for tag, loc in (("front", (0, -6, 0.95)), ("side", (6, 0, 0.95)), ("34", (4.2, -4.2, 1.3))):
            co.location = loc
            co.rotation_euler = (Vector((0, 0, 0.95)) - co.location).to_track_quat("-Z", "Y").to_euler()
            sc.render.filepath = f"{opt['--render']}_{tag}.png"
            bpy.ops.render.render(write_still=True)
    out = opt.get("--out", "/home/user/gb_e7/assets/anim/blocky.glb")
    for o in bpy.data.objects:
        o.select_set(o in (arm, obj))
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", use_selection=True, export_animations=False)
    print("BODYFIT DONE", len(bld.verts), "verts", len(bld.faces), "faces", out)


main()
