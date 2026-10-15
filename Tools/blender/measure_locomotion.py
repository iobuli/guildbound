"""Measure each locomotion clip's real travel on the hero: direction relative to the model's forward,
speed and stride. Forward comes from the skeleton (left/right thighs and up), never from a naming guess.

D:/blender.exe -b --factory-startup -P Tools/blender/measure_locomotion.py -- --glb assets/packs/hero/fighter.glb --out OUT.json
"""
import argparse
import json
import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
with open(os.path.join(os.path.dirname(__file__), "hero_mapping.json")) as f:
    MAPPING = json.load(f)


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--glb", required=True)
    a.add_argument("--out", required=True)
    a = a.parse_args(sys.argv[sys.argv.index("--") + 1:])
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(a.glb))
    rig = next(ob for ob in bpy.data.objects if ob.type == "ARMATURE")
    rig.animation_data_create()
    rig.animation_data.use_nla = False
    fps = bpy.context.scene.render.fps / bpy.context.scene.render.fps_base

    def pos(name):
        return rig.matrix_world @ rig.pose.bones[name].matrix.translation

    def set_action(act):
        rig.animation_data.action = act
        if getattr(act, "slots", None):
            rig.animation_data.action_slot = act.slots[0]

    # model frame from the neutral stance: left = right thigh -> left thigh, up = world Z, forward = left x up
    set_action(bpy.data.actions["Unarmed-Idle-Static"])
    bpy.context.scene.frame_set(0)
    bpy.context.view_layer.update()
    left = pos("B_L_Thigh") - pos("B_R_Thigh")
    left.z = 0
    left.normalize()
    up = left.copy()
    up.x, up.y, up.z = 0.0, 0.0, 1.0
    forward = left.cross(up)
    forward.normalize()
    right = -left
    rows = {}
    for kind, names in MAPPING["locomotion"].items():
        for slot, name in enumerate(names):
            act = bpy.data.actions.get(name)
            if act is None:
                rows[name] = {"missing": True}
                continue
            set_action(act)
            start, end = act.frame_range
            def local(frame):
                bpy.context.scene.frame_set(int(frame), subframe=frame % 1)
                bpy.context.view_layer.update()
                m = pos("Motion").copy()
                return m, {b: pos(b) - m for b in ("B_Pelvis", "B_L_Foot", "B_R_Foot")}
            p0, l0 = local(start)
            p1, l1 = local(end)
            # loop closure in the in-place (Motion-relative) space: 0 = seamless loop
            seam = {b: round((l1[b] - l0[b]).length, 4) for b in l0}
            drift = l1["B_Pelvis"] - l0["B_Pelvis"]
            d = p1 - p0
            d.z = 0
            dur = (end - start) / fps
            ang = math.degrees(math.atan2(d.dot(right), d.dot(forward)))
            rows[name] = {"kind": kind, "slot": slot, "slot_angle_deg": slot * 45 if slot <= 4 else slot * 45 - 360,
                          "travel_angle_deg": round(ang, 1), "stride_m": round(d.length, 3), "duration_s": round(dur, 3),
                          "speed_mps": round(d.length / max(dur, 1e-3), 3),
                          "seam_m": seam, "pelvis_drift": [round(v, 4) for v in drift], "frames": [start, end]}
    with open(a.out, "w") as f:
        json.dump({"forward": list(forward), "right": list(right), "clips": rows}, f, indent=2)
    for name, r in rows.items():
        print("LOCO", name, json.dumps(r))


main()
