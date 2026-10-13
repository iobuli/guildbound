"""Append retargeted clips to an existing hero GLB, without rebuilding the whole library (minutes, not 45).

Blender -b --factory-startup -P add_clips.py -- --glb assets/packs/hero/fighter.glb --kevin nc_idle=HumanM@Idle01 [...] --out OUT.glb
Blender -b --factory-startup -P add_clips.py -- --glb IN.glb --ual --drop Unarmed- --out OUT.glb

--ual adds every Quaternius Universal Animation Library 1 and 2 clip as u1_<name> / u2_<name> (lowercase; the
T-pose and the root-motion duplicates skipped): the same skeleton as our rig, so the map is a rename, fingers
included. --drop removes every clip whose name starts with the prefix.

Kevin Iglesias Human Basic Motions sources (the relaxed run's family, so a stop blends run -> idle in one style)
are retargeted onto the GLB's own rig with the same rest alignment, 30 fps bake, loop closing and despike as the
full build; every clip already in the GLB is kept as it is.
"""
import argparse
import glob
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
import build_hero as hero  # noqa: E402
import build_master as master  # noqa: E402

HBM = os.path.join(os.path.dirname(__file__), "..", "..", "assets", "packs", "_sources", "kevin_human_basic", "Animations", "Male")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--glb", required=True)
    p.add_argument("--kevin", action="append", default=[], help="NAME=HumanM@Stem")
    p.add_argument("--ual", action="store_true")
    p.add_argument("--drop", action="append", default=[], help="remove clips with this name prefix")
    p.add_argument("--out", required=True)
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    bpy.context.scene.render.fps, bpy.context.scene.render.fps_base = hero.OUT_FPS, 1.0
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(a.glb))
    rig = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    rig.animation_data_create()
    rig.animation_data.use_nla = False
    for prefix in a.drop:
        for x in [x for x in bpy.data.actions if x.name.startswith(prefix)]:
            bpy.data.actions.remove(x)
    kept = [x for x in bpy.data.actions]
    files = {os.path.splitext(os.path.basename(f))[0]: f for f in glob.glob(os.path.join(HBM, "**", "*.fbx"), recursive=True) if "[RM]" not in f}
    report = {"clips": [x.name for x in kept], "skipped": [], "added": []}
    for spec in a.kevin:
        name, stem = spec.split("=", 1)
        if name in bpy.data.actions:
            bpy.data.actions.remove(bpy.data.actions[name])
        if stem not in files:
            report["skipped"].append(f"{name}: no {stem}")
            continue
        objs = hero.import_fbx(files[stem])
        src = hero.armature_of(objs)
        act = hero.retarget(src, rig, name, hero.HERO_MAPPING["mage"]["bones"], motion_root="B-root") if src else None
        if act:
            hero.push(rig, act, name, report)
            report["added"].append(name)
            report["clips"].append(name)
        for o in objs:
            bpy.data.objects.remove(o, do_unlink=True)
        for x in [x for x in bpy.data.actions if x.users == 0 and not x.use_fake_user and x not in kept and x.name not in report["added"]]:
            bpy.data.actions.remove(x)
    if a.ual:
        for tag, path in (("u1", master.UAL1), ("u2", master.UAL2)):
            objs, acts, time_scale = hero.load_source(path, None)
            src = max((o for o in objs if o.type == "ARMATURE"), key=lambda r: len(r.data.bones))
            src.animation_data_create()
            for t in list(src.animation_data.nla_tracks):
                src.animation_data.nla_tracks.remove(t)
            for clip, act in sorted(acts.items()):
                if clip == "A_TPose" or clip.endswith("_RM"):
                    continue
                name = f"{tag}_{clip.lower()}"
                if name in bpy.data.actions:
                    bpy.data.actions.remove(bpy.data.actions[name])
                src.animation_data.action = act
                if hasattr(src.animation_data, "action_slot") and len(getattr(act, "slots", [])) > 0:
                    src.animation_data.action_slot = act.slots[0]
                out = hero.retarget(src, rig, name, master.UAL_MAP, time_scale=time_scale)
                if out:
                    hero.push(rig, out, name, report)
                    report["added"].append(name)
                    report["clips"].append(name)
            for o in objs:
                bpy.data.objects.remove(o, do_unlink=True)
            for x in [x for x in bpy.data.actions if x not in kept and x.name not in report["added"]]:
                bpy.data.actions.remove(x)
    # one NLA track per clip for export (existing clips and the new ones)
    for t in list(rig.animation_data.nla_tracks):
        rig.animation_data.nla_tracks.remove(t)
    rig.animation_data.action = None
    for act in [x for x in bpy.data.actions if x in kept or x.name in report["added"]]:
        tr = rig.animation_data.nla_tracks.new()
        tr.name = act.name
        st = tr.strips.new(act.name, int(act.frame_range[0]), act)
        if hasattr(st, "action_slot") and len(getattr(act, "slots", [])) > 0:
            st.action_slot = act.slots[0]
    print("ADDED", report["added"], "skipped", report["skipped"])
    hero.export_hero(rig, os.path.abspath(a.out), report)


main()
