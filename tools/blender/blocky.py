"""Blocky test mannequin: planar anatomy blocks on the UAL skeleton, for reading animation clearly.

    blender -b --factory-startup --python tools/blender/blocky.py -- [--out assets/anim/blocky.glb] [--render PNG]

Each bone gets a tapered box from its head to its tail (cross-sections in metres: width across the body x depth),
rigidly bound to that bone (one vertex group, weight 1), so every UAL / Kev animation plays on it unchanged. Shapes
follow the planar-anatomy sheets: box head, wedge ribcage wider at the shoulders, a pelvis block, a deltoid cap,
tapered thighs, calves and forearms, flat feet and mitten hands. Muscle groups are coloured flat; the joints get
pink bands (a small sphere at each joint) so bending reads at a glance.
"""
import math
import sys

import bpy
from mathutils import Matrix, Vector

UAL = "/home/user/gb_e7/assets/anim/ual2.glb"
# bone: (width_head, depth_head, width_tail, depth_tail, colour group, extend tail m[, extend head m])
# Male heroic proportions from the proportion chart: head / chest / hips 1/6 of height each, thigh and shin 1/4;
# shoulders 3 heads (0.69 m deltoid to deltoid), chest 0.51 m, waist 0.26 m, big forearms and calves.
BLOCKS = {
    "pelvis": (0.3, 0.22, 0.35, 0.24, 'glute', 0.0, 0.14),
    "spine_01": (0.26, 0.2, 0.28, 0.21, 'abs', 0.0, 0.03),
    "spine_02": (0.27, 0.21, 0.34, 0.24, 'lat', 0.0, 0.03),
    "spine_03": (0.34, 0.25, 0.51, 0.25, 'pec', 0.04, 0.03),
    "neck_01": (0.14, 0.13, 0.12, 0.12, 'neck', 0.0),
    "Head": (0.17, 0.21, 0.16, 0.2, 'head', 0.1, 0.02),
    "clavicle_l": (0.13, 0.13, 0.15, 0.15, 'trap', 0.0),
    "clavicle_r": (0.13, 0.13, 0.15, 0.15, 'trap', 0.0),
    "upperarm_l": (0.17, 0.16, 0.12, 0.11, 'delt', 0.0),
    "upperarm_r": (0.17, 0.16, 0.12, 0.11, 'delt', 0.0),
    "lowerarm_l": (0.19, 0.14, 0.1, 0.07, 'forearm', 0.0),
    "lowerarm_r": (0.19, 0.14, 0.1, 0.07, 'forearm', 0.0),
    "hand_l": (0.11, 0.045, 0.12, 0.04, 'hand', 0.08),
    "hand_r": (0.11, 0.045, 0.12, 0.04, 'hand', 0.08),
    "thigh_l": (0.18, 0.19, 0.12, 0.12, 'quad', 0.0, 0.03),
    "thigh_r": (0.18, 0.19, 0.12, 0.12, 'quad', 0.0, 0.03),
    "calf_l": (0.16, 0.17, 0.1, 0.09, 'calf', 0.0),
    "calf_r": (0.16, 0.17, 0.1, 0.09, 'calf', 0.0),
    "foot_l": (0.1, 0.08, 0.1, 0.045, 'foot', 0.0),
    "foot_r": (0.1, 0.08, 0.1, 0.045, 'foot', 0.0),
    "ball_l": (0.1, 0.04, 0.09, 0.03, 'foot', 0.03),
    "ball_r": (0.1, 0.04, 0.09, 0.03, 'foot', 0.03),
}
COLOURS = {"glute": (0.80, 0.45, 0.55), "abs": (0.40, 0.65, 0.60), "lat": (0.55, 0.45, 0.75), "pec": (0.85, 0.55, 0.42),
           "neck": (0.70, 0.60, 0.75), "head": (0.78, 0.74, 0.85), "trap": (0.65, 0.68, 0.45), "delt": (0.55, 0.70, 0.80),
           "forearm": (0.85, 0.60, 0.65), "hand": (0.90, 0.88, 0.85), "quad": (0.55, 0.55, 0.80), "calf": (0.50, 0.72, 0.68),
           "foot": (0.82, 0.82, 0.80), "joint": (1.0, 0.55, 0.55)}
UPRIGHT = {"pelvis", "spine_01", "spine_02", "spine_03", "Head"}
JOINTS = ["neck_01", "upperarm_l", "upperarm_r", "lowerarm_l", "lowerarm_r", "hand_l", "hand_r", "thigh_l", "thigh_r",
          "calf_l", "calf_r", "foot_l", "foot_r", "spine_02"]


def material(name, rgb):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*rgb, 1)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = (*rgb, 1)
    b.inputs["Roughness"].default_value = 0.8
    return m


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
    mats = {k: material(k, v) for k, v in COLOURS.items()}
    verts, faces, groups, mat_idx = [], [], [], []
    order = list(COLOURS)
    up = Vector((0, 0, 1))
    for bone in arm.data.bones:
        name = bone.name
        if name in BLOCKS:
            w0, d0, w1, d1, grp, ext, *pre = BLOCKS[name]
            m = arm.matrix_world @ bone.matrix_local
            h, t = m.translation, (arm.matrix_world @ bone.tail_local)
            axis = (t - h).normalized()
            if name in UPRIGHT:
                # Torso and head blocks stand upright (the pelvis bone itself tilts).
                axis = Vector((0, 0, 1)) * (1 if (t - h).z >= 0 else -1)
                t = h + axis * (t - h).length
            t = t + axis * ext
            h = h - axis * (pre[0] if pre else 0.0)
            # Cross-section frame: depth toward the body's front (-Y) made perpendicular to the bone.
            front = Vector((0, -1, 0))
            if abs(axis.dot(front)) > 0.8:
                front = up.copy()
            front = (front - axis * front.dot(axis)).normalized()
            side = axis.cross(front).normalized()
            base = len(verts)
            for p, w, d in ((h, w0, d0), (t, w1, d1)):
                for sx, sz in ((-1, -1), (1, -1), (1, 1), (-1, 1)):
                    verts.append(p + side * (sx * w / 2) + front * (sz * d / 2))
                    groups.append(name)
            for f in ((0, 1, 2, 3), (4, 7, 6, 5), (0, 4, 5, 1), (1, 5, 6, 2), (2, 6, 7, 3), (3, 7, 4, 0)):
                faces.append(tuple(base + i for i in f))
                mat_idx.append(order.index(grp))
        if name in JOINTS:
            # Joint band: a low-poly ball at the bone's head.
            m = arm.matrix_world @ bone.matrix_local
            c = m.translation
            r = {"neck_01": 0.06, "spine_02": 0.13}.get(name, 0.055 if "arm" in name or "hand" in name else 0.07)
            base = len(verts)
            rings, segs = 4, 8
            for i in range(rings + 1):
                th = math.pi * i / rings
                for j in range(segs):
                    ph = 2 * math.pi * j / segs
                    verts.append(c + Vector((math.sin(th) * math.cos(ph), math.sin(th) * math.sin(ph), math.cos(th))) * r)
                    groups.append(name)
            for i in range(rings):
                for j in range(segs):
                    a, b = base + i * segs + j, base + i * segs + (j + 1) % segs
                    faces.append((a, b, b + segs, a + segs))
                    mat_idx.append(order.index("joint"))
    mesh = bpy.data.meshes.new("Blocky")
    mesh.from_pydata([tuple(v) for v in verts], [], faces)
    for k in order:
        mesh.materials.append(mats[k])
    for poly, mi in zip(mesh.polygons, mat_idx):
        poly.material_index = mi
        poly.use_smooth = False
    obj = bpy.data.objects.new("Blocky", mesh)
    bpy.context.scene.collection.objects.link(obj)
    for name in set(groups):
        vg = obj.vertex_groups.new(name=name)
        vg.add([i for i, g in enumerate(groups) if g == name], 1.0, "REPLACE")
    obj.parent = arm
    mod = obj.modifiers.new("Armature", "ARMATURE")
    mod.object = arm
    if "--render" in opt:
        sc = bpy.context.scene
        cam = bpy.data.cameras.new("c")
        co = bpy.data.objects.new("c", cam)
        sc.collection.objects.link(co)
        sc.camera = co
        cam.lens = 50
        cam.type = "ORTHO"
        cam.ortho_scale = 2.1
        sc.render.engine = "BLENDER_WORKBENCH"
        sc.display.shading.color_type = "MATERIAL"
        sc.view_settings.view_transform = "Standard"
        sc.render.resolution_x, sc.render.resolution_y = 700, 900
        for tag, loc in (("front", (0, -6, 0.95)), ("side", (6, 0, 0.95))):
            co.location = loc
            co.rotation_euler = (Vector((0, 0, 0.95)) - co.location).to_track_quat("-Z", "Y").to_euler()
            sc.render.filepath = opt["--render"].replace(".png", f"_{tag}.png")
            bpy.ops.render.render(write_still=True)
    out = opt.get("--out", "/home/user/gb_e7/assets/anim/blocky.glb")
    for o in bpy.data.objects:
        o.select_set(o in (arm, obj))
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", use_selection=True, export_animations=False)
    print("BLOCKY DONE", len(verts), "verts", out)


main()
