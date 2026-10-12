"""Inspect a supplied FBX before adding its explicit retarget mapping."""
import argparse
import json
import sys
import bpy

parser = argparse.ArgumentParser()
parser.add_argument("--fbx", required=True)
args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])
bpy.ops.import_scene.fbx(filepath=args.fbx, use_anim=True)
for rig in [o for o in bpy.data.objects if o.type == "ARMATURE"]:
    bpy.context.scene.frame_set(1)
    bpy.context.view_layer.update()
    points = {}
    for name in ["B-hips", "B-head", "B-foot.L"]:
        if name in rig.data.bones:
            rest = rig.matrix_world @ rig.data.bones[name].matrix_local
            pose = rig.matrix_world @ rig.pose.bones[name].matrix
            points[name] = {"rest": list(rest.translation), "pose": list(pose.translation), "rest_rot": list(rest.to_quaternion()), "pose_rot": list(pose.to_quaternion())}
    print("MAGE_RIG " + json.dumps({"name": rig.name, "bones": [b.name for b in rig.data.bones],
          "action": rig.animation_data.action.name if rig.animation_data and rig.animation_data.action else None,
          "actions": [{"name": a.name, "range": list(a.frame_range)} for a in bpy.data.actions], "points": points}))
