"""Render and measure the synchronized sword-vs-fist choreography.

    blender -b --factory-startup --python tools/blender/duel_preview.py -- OUT_DIR

This is proof tooling, not a game asset exporter.  It puts both authored tracks in one
scene, attaches a simple proxy sword to the weapon hand, renders the important contact
beats from side/front, and writes their shared frame contract to ``contacts.json``.
"""
import json
import math
import os
import sys

import bpy
from mathutils import Vector

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import karate as K  # noqa: E402
import brawler as B  # noqa: E402


CONTACT_TAGS = {
    "slash_block": "sword attack / forearm guard",
    "fist_counter": "fist counter / sword high cover",
    "sword_parry": "sword parry / checked fist",
    "pommel_hit": "pommel hit / head recoil",
    "low_cut": "low sword cut / leg withdrawal",
    "shin_check": "shin check / low-cut stop",
    "elbow_hit": "elbow hit / sword recoil",
    "reversal": "sword reversal / body evade",
    "perfect_slip": "perfect slip / whiff",
    "body_cross": "body cross / hit reaction",
    "sword_hit": "sword counter / fist recoil",
    "mutual_clash": "simultaneous final clash",
}


def second_rig():
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=K.UAL)
    added = [o for o in bpy.data.objects if o not in before]
    arm = next(o for o in added if o.type == "ARMATURE")
    for b in arm.pose.bones:
        b.rotation_mode = "QUATERNION"
    return K.Rig(arm), added


def tint(objects, colour):
    for o in objects:
        if o.type == "MESH":
            o.color = colour


def proxy_sword(arm):
    """A readable proxy using the katana moveset's exact hand offset/orientation."""
    root = bpy.data.objects.new("Sword_proxy_root", None)
    bpy.context.scene.collection.objects.link(root)
    root.parent = arm
    root.parent_type = "BONE"
    root.parent_bone = "hand_r"
    root.location = (-0.025, 0.075, 0.0)
    root.rotation_euler = tuple(math.radians(v) for v in (0.0, 90.0, 90.0))
    bpy.ops.mesh.primitive_cube_add(size=1)
    blade = bpy.context.object
    blade.name = "Sword_proxy_blade"
    blade.dimensions = (0.045, 0.75, 0.025)
    blade.parent = root
    blade.location = (0.0, 0.50, 0.0)
    blade.color = (0.10, 0.16, 0.24, 1.0)
    bpy.ops.mesh.primitive_cube_add(size=1)
    guard = bpy.context.object
    guard.name = "Sword_proxy_guard"
    guard.dimensions = (0.24, 0.035, 0.045)
    guard.parent = root
    guard.location = (0.0, 0.12, 0.0)
    guard.color = (0.85, 0.48, 0.08, 1.0)
    return blade


def bone_point(rig, bone, tail=False):
    pb = rig.pb[bone]
    p = pb.tail if tail else pb.head
    return rig.arm.matrix_world @ p


def segment_gap(point, a, b):
    ab = b - a
    u = 0.0 if ab.length_squared < 1e-10 else max(0.0, min(1.0, (point - a).dot(ab) / ab.length_squared))
    return (point - (a + ab * u)).length


def look_at(obj, target):
    obj.rotation_euler = (Vector(target) - obj.location).to_track_quat("-Z", "Y").to_euler()


def setup_view(sc):
    cam_data = bpy.data.cameras.new("duel_camera")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = 3.15
    cam = bpy.data.objects.new("duel_camera", cam_data)
    sc.collection.objects.link(cam)
    sc.camera = cam
    sc.render.engine = "BLENDER_WORKBENCH"
    sc.display.shading.light = "STUDIO"
    sc.display.shading.color_type = "OBJECT"
    sc.display.shading.show_shadows = True
    sc.display.shading.show_cavity = True
    sc.display.shading.cavity_type = "BOTH"
    sc.render.resolution_x, sc.render.resolution_y = 960, 620
    sc.render.resolution_percentage = 100
    sc.view_settings.view_transform = "Standard"
    sc.world = bpy.data.worlds.new("duel_world")
    sc.world.color = (0.035, 0.045, 0.065)
    sc.display.shading.background_type = "WORLD"
    return cam


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if not argv:
        raise SystemExit("output directory required")
    out = os.path.abspath(argv[0])
    os.makedirs(out, exist_ok=True)

    fist = K.setup()
    fist_objects = list(bpy.data.objects)
    sword, sword_objects = second_rig()
    tint(fist_objects, (0.12, 0.38, 0.92, 1.0))
    tint(sword_objects, (0.95, 0.20, 0.12, 1.0))
    blade = proxy_sword(sword.arm)

    K.SEQUENCES["Preview_Fist"] = B.ANIMS["Shonen_Fist_Duel"][1]
    K.SEQUENCES["Preview_Sword"] = B.ANIMS["Shonen_Sword_Duel"][1]
    fist_action, fist_marks = K.build(fist, ["Preview_Fist"], "Preview_Fist")
    sword_action, sword_marks = K.build(sword, ["Preview_Sword"], "Preview_Sword")
    fist.arm.animation_data.action = fist_action
    sword.arm.animation_data.action = sword_action

    sc = bpy.context.scene
    sc.render.fps = K.FPS
    sc.frame_start = 1
    sc.frame_end = int(max(fist_action.frame_range[1], sword_action.frame_range[1]))
    cam = setup_view(sc)
    records = []
    sword_by_tag = {tag: frame for _, tag, frame in sword_marks}
    for _, tag, frame in fist_marks:
        if tag not in CONTACT_TAGS:
            continue
        other = sword_by_tag[tag]
        if other != frame:
            raise RuntimeError(f"desynchronized {tag}: fist {frame}, sword {other}")
        sc.frame_set(frame)
        bpy.context.view_layer.update()
        fist_hands = [bone_point(fist, n, True) for n in ("hand_l", "hand_r")]
        sword_targets = [bone_point(sword, n, True) for n in ("Head", "spine_03", "hand_l", "hand_r")]
        blade_a = blade.matrix_world @ Vector((0.0, -0.5, 0.0))
        blade_b = blade.matrix_world @ Vector((0.0, 0.5, 0.0))
        fist_targets = fist_hands + [bone_point(fist, n, True) for n in ("Head", "lowerarm_l", "lowerarm_r", "foot_l", "foot_r")]
        records.append({
            "tag": tag,
            "beat": CONTACT_TAGS[tag],
            "frame": frame,
            "time_s": round((frame - 1) / K.FPS, 4),
            "fist_to_sword_target_m": round(min((a - b).length for a in fist_hands for b in sword_targets), 4),
            "blade_to_fist_target_m": round(min(segment_gap(p, blade_a, blade_b) for p in fist_targets), 4),
        })
        for view, location in (("side", (5.8, 0.0, 1.18)), ("front", (0.0, -5.8, 1.18))):
            cam.location = location
            look_at(cam, (0.0, 0.0, 0.92))
            sc.render.filepath = os.path.join(out, f"{frame:03d}_{tag}_{view}.png")
            bpy.ops.render.render(write_still=True)

    with open(os.path.join(out, "contacts.json"), "w", encoding="utf-8") as f:
        json.dump({"fps": K.FPS, "duration_s": round((sc.frame_end - 1) / K.FPS, 4),
                   "tracks": ["Brawler_Shonen_Fist_Duel", "Brawler_Shonen_Sword_Duel"],
                   "contacts": records}, f, indent=2)
    print("DUEL_PREVIEW", len(records), "beats", out)


if __name__ == "__main__":
    main()
