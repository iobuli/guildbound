"""Convert one character rig + its clip FBXs into a single GLB with every clip as a named animation.

D:/blender.exe -b --factory-startup -P Tools/blender/fbx_pack_to_glb.py -- \
    --model PATH/BlueGuy.fbx --clips "PATH/FBX/*.fbx" --out assets/packs/fighting_motions/fighter.glb

Clips whose bones do not match the model's armature are skipped and listed in the report
(<out>.report.json), never silently retargeted. Clip names are the FBX file stems.
"""
import argparse
import glob
import json
import os
import sys

import bpy


def args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    p = argparse.ArgumentParser()
    p.add_argument("--model", required=True)
    p.add_argument("--clips", required=True, help="glob, quoted")
    p.add_argument("--out", required=True)
    p.add_argument("--exclude", default="", help="comma-separated substrings of clip paths to skip")
    return p.parse_args(argv)


def clear_scene():
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    for act in list(bpy.data.actions):
        bpy.data.actions.remove(act)


def import_fbx(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=path, use_anim=True)
    return [ob for ob in bpy.data.objects if ob not in before]


def armature_of(objs):
    arms = [ob for ob in objs if ob.type == "ARMATURE"]
    return arms[0] if arms else None


def main():
    a = args()
    clear_scene()
    model_objs = import_fbx(a.model)
    rig = armature_of(model_objs)
    if rig is None:
        sys.exit(f"no armature in {a.model}")
    bones = {b.name for b in rig.data.bones}
    # the model file's own take, if any, is not a clip
    if rig.animation_data:
        rig.animation_data.action = None
    for act in list(bpy.data.actions):
        bpy.data.actions.remove(act)

    excludes = [s for s in a.exclude.split(",") if s]
    report = {"model": a.model, "bones": len(bones), "clips": [], "skipped": []}
    rig.animation_data_create()
    for path in sorted(glob.glob(a.clips, recursive=True)):
        if os.path.abspath(path) == os.path.abspath(a.model) or any(s in path for s in excludes):
            continue
        name = os.path.splitext(os.path.basename(path))[0]
        objs = import_fbx(path)
        src = armature_of(objs)
        act = src.animation_data.action if src and src.animation_data else None
        if act is None:
            report["skipped"].append({"clip": name, "why": "no armature action"})
        else:
            channel_bones = set()
            for fc in act.fcurves if hasattr(act, "fcurves") else []:
                if fc.data_path.startswith('pose.bones["'):
                    channel_bones.add(fc.data_path.split('"')[1])
            if not channel_bones:
                # Blender 4.4+ layered actions keep curves in channelbags
                for layer in getattr(act, "layers", []):
                    for strip in layer.strips:
                        for bag in getattr(strip, "channelbags", []):
                            for fc in bag.fcurves:
                                if fc.data_path.startswith('pose.bones["'):
                                    channel_bones.add(fc.data_path.split('"')[1])
            missing = sorted(channel_bones - bones)
            if missing:
                report["skipped"].append({"clip": name, "why": "bones not in model rig", "missing": missing[:10]})
            else:
                act.name = name
                act.use_fake_user = True
                track = rig.animation_data.nla_tracks.new()
                track.name = name
                start, end = act.frame_range
                strip = track.strips.new(name, int(start), act)
                if hasattr(strip, "action_slot") and len(getattr(act, "slots", [])) > 0:
                    strip.action_slot = act.slots[0]
                report["clips"].append({"clip": name, "frames": [start, end], "channels_bones": len(channel_bones)})
        for ob in objs:
            bpy.data.objects.remove(ob, do_unlink=True)

    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=a.out,
        export_format="GLB",
        export_animations=True,
        export_animation_mode="NLA_TRACKS",
        export_force_sampling=True,
        export_skins=True,
        export_yup=True,
    )
    xs = [v.co for ob in model_objs if ob.type == "MESH" for v in ob.data.vertices]
    report["mesh_objects"] = [ob.name for ob in model_objs if ob.type == "MESH"]
    report["out"] = a.out
    with open(a.out + ".report.json", "w") as f:
        json.dump(report, f, indent=2)
    print(f"GLB_DONE clips={len(report['clips'])} skipped={len(report['skipped'])} -> {a.out}")


main()
