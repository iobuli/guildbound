"""Kimodo (SOMA) BVH motions onto the UAL skeleton, as Blender actions.

    blender -b --factory-startup --python tools/kimodo/bvh_to_ual.py -- --bvh a.bvh[,b.bvh...] [--names A,B] [--export GLB] [--render DIR]

Generate with `kimodo_gen ... --bvh --bvh_standard_tpose` (the standard T-pose rest matches our T-pose rest, so a joint's
world rotation is the same delta on both skeletons). Reading is generic BVH (hierarchy, channel order, units): nothing in
the file is executed. Joint rotations: SOMA global rotation (FK from the BVH) -> our bone's world rotation relative to
its own rest, so bone roll differences between the skeletons cancel. Root: the hips' height scaled by our/their hip
height, horizontal travel removed (clips are in place; the controller moves the body); heading and turning are kept.
SOMA -> our bones: Hips pelvis, Spine1-2 spine_01-02, Chest spine_03, Neck1 neck_01, Head Head, <Side>Shoulder clavicle,
Arm upperarm, ForeArm lowerarm, Hand hand, Thumb/Index/Middle/Ring/Pinky 1-3 -> <finger>_01-03, Leg thigh, Shin calf,
Foot foot, ToeBase ball.
"""
import os
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), "blender"))
import bpy  # noqa: E402
from mathutils import Matrix, Quaternion  # noqa: E402
import karate as K  # noqa: E402

FINGERS = {"Thumb": "thumb", "Index": "index", "Middle": "middle", "Ring": "ring", "Pinky": "pinky"}


def soma_map():
    m = {"Hips": "pelvis", "Spine1": "spine_01", "Spine2": "spine_02", "Chest": "spine_03", "Neck1": "neck_01", "Head": "Head"}
    for S, s in (("Left", "l"), ("Right", "r")):
        m.update({f"{S}Shoulder": f"clavicle_{s}", f"{S}Arm": f"upperarm_{s}", f"{S}ForeArm": f"lowerarm_{s}", f"{S}Hand": f"hand_{s}",
                  f"{S}Leg": f"thigh_{s}", f"{S}Shin": f"calf_{s}", f"{S}Foot": f"foot_{s}", f"{S}ToeBase": f"ball_{s}"})
        for F, f in FINGERS.items():
            for k in (1, 2, 3):
                m[f"{S}Hand{F}{k}"] = f"{f}_0{k}_{s}"
    return m


def parse_bvh(path):
    names, parent, offset, chans = [], [], [], []
    stack = []
    with open(path) as f:
        lines = iter(f)
        for line in lines:
            t = line.split()
            if not t:
                continue
            if t[0] in ("ROOT", "JOINT"):
                names.append(t[1])
                parent.append(stack[-1] if stack else -1)
                offset.append([0.0, 0.0, 0.0])
                chans.append([])
                stack.append(len(names) - 1)
            elif t[0] == "End":
                stack.append(-2)
            elif t[0] == "OFFSET" and stack and stack[-1] >= 0:
                offset[stack[-1]] = [float(x) for x in t[1:4]]
            elif t[0] == "CHANNELS":
                chans[stack[-1]] = t[2:2 + int(t[1])]
            elif t[0] == "}":
                stack.pop()
            elif t[0] == "MOTION":
                break
        frames = int(next(lines).split()[1])
        dt = float(next(lines).split()[2])
        data = np.array([[float(x) for x in next(lines).split()] for _ in range(frames)])
    return names, np.array(parent), np.array(offset), chans, data, dt


def rot_axis(axis, a):
    c, s = np.cos(a), np.sin(a)
    return {"X": np.array([[1, 0, 0], [0, c, -s], [0, s, c]]), "Y": np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]]),
            "Z": np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])}[axis]


def fk(names, parent, offset, chans, data):
    """Global rotation (F, J, 3, 3) and position (F, J, 3), centimetres, from channel data."""
    F, J = len(data), len(names)
    G = np.zeros((F, J, 3, 3))
    P = np.zeros((F, J, 3))
    col = 0
    for j in range(J):
        pos = np.zeros((F, 3))
        R = np.repeat(np.eye(3)[None], F, 0)
        for c in chans[j]:
            v = data[:, col]
            col += 1
            if c.endswith("position"):
                pos[:, "XYZ".index(c[0])] = v
            else:
                R = R @ np.stack([rot_axis(c[0], x) for x in np.radians(v)])
        local = np.array(offset[j]) + pos
        p = parent[j]
        if p < 0:
            G[:, j], P[:, j] = R, local
        else:
            G[:, j] = G[:, p] @ R
            P[:, j] = P[:, p] + np.einsum("fij,fj->fi", G[:, p], local)
    return G, P


GLTF_TO_BLENDER = np.array([[1, 0, 0], [0, 0, -1], [0, 1, 0]], dtype=float)  # (x, y, z)_gltf -> (x, -z, y)_blender


def quat_of(m3):
    q = Matrix(m3.tolist()).to_quaternion()
    return Quaternion((q.w, q.x, q.y, q.z))


def import_bvh(rig, path, name):
    names, parent, offset, chans, data, dt = parse_bvh(path)
    G, P = fk(names, parent, offset, chans, data)
    mapping = soma_map()
    idx = {n: i for i, n in enumerate(names)}
    hips = idx["Hips"]
    # Hips height in the BVH rest pose (cm) -> our pelvis rest height (m); horizontal travel is dropped below.
    rest_hips_cm = offset[idx["Hips"]][1] + (offset[parent[hips]][1] if parent[hips] >= 0 else 0.0)
    scale = rig.rest_head["pelvis"].z / (rest_hips_cm if rest_hips_cm > 1 else 100.0)
    act = bpy.data.actions.new(f"Kimodo_{name}")
    rig.arm.animation_data_create()
    rig.arm.animation_data.action = act
    rig.last = {}
    fps = 1.0 / dt
    sc = bpy.context.scene
    sc.render.fps = int(round(fps))
    # Ordered so parents come first (our armature's own bone order is parent-first).
    ours = [b.name for b in rig.arm.data.bones]
    inv = {v: k for k, v in mapping.items()}
    for b in rig.pb:
        b.matrix_basis = Matrix.Identity(4)
    for f in range(len(data)):
        bpy.context.view_layer.update()
        h = P[f, hips] * scale  # metres in gltf axes
        pel = np.array([0.0, h[1], 0.0])
        pel_b = GLTF_TO_BLENDER @ pel
        for bone in ours:
            if bone not in inv:
                continue
            Gj = G[f, idx[inv[bone]]]
            Rb = GLTF_TO_BLENDER @ Gj @ GLTF_TO_BLENDER.T
            if bone == "pelvis":
                rig.set(bone, quat_of(Rb), __import__("mathutils").Vector(pel_b.tolist()))
            else:
                rig.set(bone, quat_of(Rb))
        bpy.context.view_layer.update()
        K.key_all(rig, f + 1)
    for fc in act.fcurves:
        for kp in fc.keyframe_points:
            kp.interpolation = "LINEAR"
    act.frame_range = (1, len(data))
    act.use_fake_user = True
    return act


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    opt = {argv[i]: argv[i + 1] for i in range(0, len(argv) - 1, 2)}
    paths = opt["--bvh"].split(",")
    names = opt["--names"].split(",") if "--names" in opt else [os.path.splitext(os.path.basename(p))[0] for p in paths]
    rig = K.setup()
    acts = [import_bvh(rig, p, n) for p, n in zip(paths, names)]
    sc = bpy.context.scene
    if "--render" in opt:
        out = opt["--render"].rstrip("/")
        cam = bpy.data.cameras.new("c")
        cam.lens = 45
        co = bpy.data.objects.new("c", cam)
        sc.collection.objects.link(co)
        sc.camera = co
        co.location = (3.0, -3.4, 1.4)
        co.rotation_euler = (__import__("mathutils").Vector((0, 0, 0.85)) - co.location).to_track_quat("-Z", "Y").to_euler()
        sc.render.engine = "BLENDER_WORKBENCH"
        sc.display.shading.color_type = "OBJECT"
        sc.view_settings.view_transform = "Standard"
        sc.world = bpy.data.worlds.new("w")
        sc.world.color = (0.95, 0.85, 0.88)
        sc.display.shading.background_type = "WORLD"
        sc.render.resolution_x, sc.render.resolution_y = 480, 600
        for o in bpy.data.objects:
            if o.type == "MESH":
                o.color = (0.72, 0.70, 0.66, 1)
        for n, a in zip(names, acts):
            os.makedirs(f"{out}/{n}", exist_ok=True)
            rig.arm.animation_data.action = a
            sc.frame_start, sc.frame_end = 1, int(a.frame_range[1])
            sc.render.filepath = f"{out}/{n}/f_"
            bpy.ops.render.render(animation=True)
    if "--export" in opt:
        rig.arm.animation_data.action = None
        for o in bpy.data.objects:
            o.select_set(o.type in ("ARMATURE", "MESH"))
        bpy.ops.export_scene.gltf(filepath=opt["--export"], export_format="GLB", use_selection=True,
                                  export_animation_mode="ACTIONS", export_force_sampling=True)
    print("KIMODO IMPORT DONE", names)


main()
