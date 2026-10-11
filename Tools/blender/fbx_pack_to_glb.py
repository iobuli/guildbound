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
    p.add_argument("--height", type=float, default=1.8, help="normalize the bind-pose mesh to this height (m), feet at 0; 0 = off")
    return p.parse_args(argv)


def normalize(model_objs, height):
    """Uniformly scale the model (rig, meshes, and so every clip) so the bind-pose mesh is `height` tall,
    with the lowest vertex on the ground. Applied to the top-level objects about the world origin."""
    from mathutils import Matrix
    zs = [(ob.matrix_world @ v.co).z for ob in model_objs if ob.type == "MESH" for v in ob.data.vertices]
    if not zs or height <= 0:
        return 1.0
    lo, hi = min(zs), max(zs)
    f = height / (hi - lo)
    m = Matrix.Translation((0.0, 0.0, -lo * f)) @ Matrix.Scale(f, 4)
    for ob in model_objs:
        if ob.parent is None:
            ob.matrix_world = m @ ob.matrix_world
    bpy.context.view_layer.update()
    return f


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


def bake_from_empties(rig, objs, name):
    """Clips exported as animated empties (3ds Max dummies): bake them onto same-named bones.
    Returns the baked action, or None if too few bones match."""
    empties = {ob.name: ob for ob in objs if ob.type == "EMPTY"}
    shared = [b for b in rig.pose.bones if b.name in empties]
    if len(shared) < 0.5 * len(rig.pose.bones):
        return None
    ranges = [ob.animation_data.action.frame_range for ob in empties.values() if ob.animation_data and ob.animation_data.action]
    if not ranges:
        return None
    start, end = int(min(r[0] for r in ranges)), int(max(r[1] for r in ranges))
    for pb in shared:
        c = pb.constraints.new("COPY_TRANSFORMS")
        c.target = empties[pb.name]
    bpy.context.view_layer.objects.active = rig
    rig.select_set(True)
    bpy.ops.object.mode_set(mode="POSE")
    bpy.ops.pose.select_all(action="SELECT")
    rig.animation_data.action = None
    bpy.ops.nla.bake(frame_start=start, frame_end=end, only_selected=True, visual_keying=True,
                     clear_constraints=True, use_current_action=False, bake_types={"POSE"})
    bpy.ops.object.mode_set(mode="OBJECT")
    act = rig.animation_data.action
    rig.animation_data.action = None
    if act is not None:
        act.name = name
    return act


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
        name = os.path.splitext(os.path.basename(path))[0].split("@")[-1].replace(".max", "")
        objs = import_fbx(path)
        src = armature_of(objs)
        act = src.animation_data.action if src and src.animation_data else None
        if act is None and src is None:
            act = bake_from_empties(rig, objs, name)
            if act is not None:
                act.use_fake_user = True
                track = rig.animation_data.nla_tracks.new()
                track.name = name
                strip = track.strips.new(name, int(act.frame_range[0]), act)
                if hasattr(strip, "action_slot") and len(getattr(act, "slots", [])) > 0:
                    strip.action_slot = act.slots[0]
                report["clips"].append({"clip": name, "frames": list(act.frame_range), "baked_from": "empties"})
            else:
                report["skipped"].append({"clip": name, "why": "no armature action and empties do not match the rig"})
        elif act is None:
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

    report["normalize_scale"] = normalize(model_objs, a.height)
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
    report["mesh_objects"] = [ob.name for ob in model_objs if ob.type == "MESH"]
    report["out"] = a.out
    with open(a.out + ".report.json", "w") as f:
        json.dump(report, f, indent=2)
    print(f"GLB_DONE clips={len(report['clips'])} skipped={len(report['skipped'])} -> {a.out}")


main()
