"""Convert a .blend character (rig + meshes) plus actions from other .blend files into one GLB.

D:/blender.exe -b --factory-startup "PATH/ybot.blend" -P Tools/blender/blend_pack_to_glb.py -- \
    --anims "PATH/animations/*/*.blend" --out assets/packs/sekiro_proto/ybot.glb

The opened file is the model. Its own actions are kept unless --drop-own. Actions from --anims are
appended; an action whose bone channels are not all in the model rig is skipped and listed in the report.
Mesh modifiers (mirror, decimate, solidify) are applied on export. --meshes keeps only the named meshes.
"""
import argparse
import glob
import json
import os
import sys

import bpy
from mathutils import Matrix


def args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    p = argparse.ArgumentParser()
    p.add_argument("--anims", default="", help="glob of .blend files whose actions are appended, quoted")
    p.add_argument("--out", required=True)
    p.add_argument("--drop-own", action="store_true")
    p.add_argument("--meshes", default="", help="comma-separated mesh names to keep (default all)")
    p.add_argument("--height", type=float, default=1.8, help="bind-pose mesh height (m), feet at 0; 0 = off")
    return p.parse_args(argv)


def channel_bones(act):
    out = set()
    curves = list(getattr(act, "fcurves", []) or [])
    for layer in getattr(act, "layers", []):
        for strip in layer.strips:
            for bag in getattr(strip, "channelbags", []):
                curves.extend(bag.fcurves)
    for fc in curves:
        if fc.data_path.startswith('pose.bones["'):
            out.add(fc.data_path.split('"')[1])
    return out


def add_track(rig, act):
    act.use_fake_user = True
    track = rig.animation_data.nla_tracks.new()
    track.name = act.name
    strip = track.strips.new(act.name, int(act.frame_range[0]), act)
    if hasattr(strip, "action_slot") and len(getattr(act, "slots", [])) > 0:
        strip.action_slot = act.slots[0]


def main():
    a = args()
    rigs = [ob for ob in bpy.data.objects if ob.type == "ARMATURE"]
    if not rigs:
        sys.exit("no armature in the opened file")
    rig = max(rigs, key=lambda r: len(r.data.bones))
    keep = {s for s in a.meshes.split(",") if s}
    for ob in list(bpy.data.objects):
        if ob.type == "MESH" and keep and ob.name not in keep:
            bpy.data.objects.remove(ob, do_unlink=True)
        elif ob.type not in ("MESH", "ARMATURE") or (ob.type == "ARMATURE" and ob is not rig):
            bpy.data.objects.remove(ob, do_unlink=True)
    bones = {b.name for b in rig.data.bones}
    rig.animation_data_create()
    rig.animation_data.action = None
    for t in list(rig.animation_data.nla_tracks):
        rig.animation_data.nla_tracks.remove(t)

    report = {"model": bpy.data.filepath, "bones": len(bones), "clips": [], "skipped": []}
    names = set()

    def take(act, origin):
        cb = channel_bones(act)
        missing = sorted(cb - bones)
        if not cb:
            report["skipped"].append({"clip": act.name, "from": origin, "why": "no bone channels"})
        elif missing:
            report["skipped"].append({"clip": act.name, "from": origin, "why": "bones not in model rig", "missing": missing[:10]})
        elif act.name in names:
            report["skipped"].append({"clip": act.name, "from": origin, "why": "duplicate name"})
        else:
            names.add(act.name)
            add_track(rig, act)
            report["clips"].append({"clip": act.name, "from": origin, "frames": list(act.frame_range), "channels_bones": len(cb)})

    own = list(bpy.data.actions)
    if a.drop_own:
        for act in own:
            bpy.data.actions.remove(act)
    else:
        for act in own:
            take(act, "model")

    for path in sorted(glob.glob(a.anims)) if a.anims else []:
        if os.path.abspath(path) == os.path.abspath(bpy.data.filepath):
            continue
        with bpy.data.libraries.load(path, link=False) as (src, dst):
            dst.actions = list(src.actions)
        for act in dst.actions:
            if act is not None:
                take(act, os.path.basename(path))

    meshes = [ob for ob in bpy.data.objects if ob.type == "MESH"]
    scale = 1.0
    zs = [(ob.matrix_world @ v.co).z for ob in meshes for v in ob.data.vertices]
    if zs and a.height > 0:
        lo, hi = min(zs), max(zs)
        scale = a.height / (hi - lo)
        m = Matrix.Translation((0.0, 0.0, -lo * scale)) @ Matrix.Scale(scale, 4)
        for ob in bpy.data.objects:
            if ob.parent is None:
                ob.matrix_world = m @ ob.matrix_world
        bpy.context.view_layer.update()
    report["normalize_scale"] = scale
    report["mesh_objects"] = [ob.name for ob in meshes]

    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=a.out,
        export_format="GLB",
        export_animations=True,
        export_animation_mode="NLA_TRACKS",
        export_force_sampling=True,
        export_skins=True,
        export_apply=True,
        export_yup=True,
    )
    report["out"] = a.out
    with open(a.out + ".report.json", "w") as f:
        json.dump(report, f, indent=2)
    print(f"GLB_DONE clips={len(report['clips'])} skipped={len(report['skipped'])} -> {a.out}")


main()
