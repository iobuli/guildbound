"""Build the hero library: one rig (RPG Character) with its own clips plus clips retargeted from other packs.

D:/blender.exe -b --factory-startup -P Tools/blender/build_hero.py -- --archive "<unity archive Assets>" --out assets/packs/hero/fighter.glb

Retarget method: for each mapped bone, the source bone's world rotation relative to its own rest is applied
to the target bone's rest (target_world = src_world * src_rest^-1 * tgt_rest), parents before children, so
different rest poses (T vs A) and bone rolls do not matter. The pelvis also takes the source's translation
from rest, scaled by the leg-length ratio. Normalized to 1.80 m like every library.
"""
import argparse
import glob
import json
import os
import sys

import bpy
from mathutils import Matrix

sys.path.insert(0, os.path.dirname(__file__))

with open(os.path.join(os.path.dirname(__file__), "hero_mapping.json")) as f:
    HERO_MAPPING = json.load(f)
FM_TO_RPG = HERO_MAPPING["bones"]


def args():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    p = argparse.ArgumentParser()
    p.add_argument("--archive", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--height", type=float, default=1.8)
    p.add_argument("--extend-hero", help="Add mage and Mixamo clips to an already built hero GLB without rebaking the rest")
    p.add_argument("--reference", default="D:/New folder (4)/reference", help="root of the cloned reference repos (Mixamo sources)")
    return p.parse_args(argv)


def import_fbx(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=path, use_anim=True)
    return [ob for ob in bpy.data.objects if ob not in before]


def armature_of(objs):
    return next((ob for ob in objs if ob.type == "ARMATURE"), None)


def push(rig, act, name):
    act.name = name
    act.use_fake_user = True
    track = rig.animation_data.nla_tracks.new()
    track.name = name
    strip = track.strips.new(name, int(act.frame_range[0]), act)
    if hasattr(strip, "action_slot") and len(getattr(act, "slots", [])) > 0:
        strip.action_slot = act.slots[0]


def bake_from_empties(rig, objs, name):
    rig.animation_data.use_nla = False
    for pb in rig.pose.bones:
        pb.matrix_basis = Matrix.Identity(4)
    empties = {ob.name: ob for ob in objs if ob.type == "EMPTY"}
    shared = [b for b in rig.pose.bones if b.name in empties]
    ranges = [ob.animation_data.action.frame_range for ob in empties.values() if ob.animation_data and ob.animation_data.action]
    if len(shared) < 0.5 * len(rig.pose.bones) or not ranges:
        return None
    start, end = int(min(r[0] for r in ranges)), int(max(r[1] for r in ranges))
    for pb in shared:
        c = pb.constraints.new("COPY_TRANSFORMS")
        c.target = empties[pb.name]
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="POSE")
    bpy.ops.pose.select_all(action="SELECT")
    rig.animation_data.action = None
    bpy.ops.nla.bake(frame_start=start, frame_end=end, only_selected=True, visual_keying=True,
                     clear_constraints=True, use_current_action=False, bake_types={"POSE"})
    bpy.ops.object.mode_set(mode="OBJECT")
    act = rig.animation_data.action
    rig.animation_data.action = None
    return act


def ordered(rig, names):
    """Mapped target bones, parents before children."""
    out = []
    for b in rig.data.bones:  # Blender lists bones parent-first
        if b.name in names:
            out.append(b.name)
    return out


def leg_length(rig, thigh, foot):
    # world space: the two rigs use different armature units (cm vs m), so local lengths are not comparable
    w = rig.matrix_world
    return ((w @ rig.data.bones[thigh].head_local) - (w @ rig.data.bones[foot].head_local)).length


def anatomical_frame(rig, hip, head, foot, toe):
    """Rest axes from anatomy, independent of bone roll or authoring units."""
    point = lambda name: (rig.matrix_world @ rig.data.bones[name].matrix_local).translation
    up = (point(head) - point(hip)).normalized()
    forward = point(toe) - point(foot)
    forward = (forward - up * forward.dot(up)).normalized()
    right = forward.cross(up).normalized()
    forward = up.cross(right).normalized()
    return Matrix((right, forward, up)).transposed().to_quaternion()


def retarget(src, tgt, name, mapping, motion_root=None, time_scale=1.0):
    """time_scale maps source frames to scene frames (scene fps / source fps) for actions taken raw from
    a .blend authored at another frame rate."""
    act_src = src.animation_data.action if src.animation_data else None
    if act_src is None:
        return None
    missing = [f"{s} -> {t}" for s, t in mapping.items() if s not in src.data.bones or t not in tgt.data.bones]
    if missing:
        raise ValueError(f"{name}: UNKNOWN mapped bones: {missing}")
    start, end = int(act_src.frame_range[0]), int(act_src.frame_range[1])
    inv = {v: k for k, v in mapping.items()}
    order = ordered(tgt, set(mapping.values()))
    scale = leg_length(tgt, "B_L_Thigh", "B_L_Foot") / max(leg_length(src, inv["B_L_Thigh"], inv["B_L_Foot"]), 1e-6)
    sw, tw = src.matrix_world, tgt.matrix_world
    alignment = None
    if motion_root:
        s_frame = anatomical_frame(src, inv["B_Pelvis"], inv["B_Head"], inv["B_L_Foot"], inv["B_L_Toe0"])
        t_frame = anatomical_frame(tgt, "B_Pelvis", "B_Head", "B_L_Foot", "B_L_Toe0")
        alignment = t_frame @ s_frame.inverted()
    for b in order:
        tgt.pose.bones[b].rotation_mode = "QUATERNION"
    tgt.animation_data_create()
    # Existing library tracks are for export, never inputs to the next bake.
    tgt.animation_data.use_nla = False
    for pb in tgt.pose.bones:
        pb.matrix_basis = Matrix.Identity(4)
    act = bpy.data.actions.new(name)
    tgt.animation_data.action = act
    pelvis = "B_Pelvis"
    for frame in range(start, end + 1):
        bpy.context.scene.frame_set(frame)
        correction = None
        if motion_root:
            root_rest = sw @ src.data.bones[motion_root].matrix_local
            root_pose = sw @ src.pose.bones[motion_root].matrix
            # Kevin's animated B-root carries a 100x unit conversion and a
            # quarter-turn absent from the bind bones. Remove that conversion
            # before solving local motion, then map the rest coordinate axes.
            correction = root_rest @ root_pose.inverted()
        for b in order:
            s_name = inv[b]
            s_pb, s_bone = src.pose.bones[s_name], src.data.bones[s_name]
            t_pb, t_bone = tgt.pose.bones[b], tgt.data.bones[b]
            s_world = sw @ s_pb.matrix
            if correction is not None:
                s_world = correction @ s_world
            s_rest = sw @ s_bone.matrix_local
            t_rest = tw @ t_bone.matrix_local
            delta = s_world.to_quaternion() @ s_rest.to_quaternion().inverted()
            if alignment is not None:
                delta = alignment @ delta @ alignment.inverted()
            rot = delta @ t_rest.to_quaternion()
            if b == pelvis:
                travel = s_world.translation - s_rest.translation
                if alignment is not None:
                    travel = alignment @ travel
                loc = t_rest.translation + travel * scale
            else:
                bpy.context.view_layer.update()
                loc = (tw @ t_pb.matrix).translation
            # Keep the target's world scale. Unit scale here turns a centimetre
            # rig into metre-sized bones, contaminating every descendant's pose.
            world = Matrix.LocRotScale(loc, rot, t_rest.to_scale())
            t_pb.matrix = tw.inverted() @ world
            key = start + (frame - start) * time_scale
            t_pb.keyframe_insert("rotation_quaternion", frame=key)
            if b == pelvis:
                t_pb.keyframe_insert("location", frame=key)
        bpy.context.view_layer.update()
    tgt.animation_data.action = None
    for b in order:
        pb = tgt.pose.bones[b]
        pb.matrix_basis = Matrix.Identity(4)
    return act


def normalize(objs, height):
    zs = [(ob.matrix_world @ v.co).z for ob in objs if ob.type == "MESH" for v in ob.data.vertices]
    lo, hi = min(zs), max(zs)
    f = height / (hi - lo)
    m = Matrix.Translation((0.0, 0.0, -lo * f)) @ Matrix.Scale(f, 4)
    for ob in objs:
        if ob.parent is None:
            ob.matrix_world = m @ ob.matrix_world
    bpy.context.view_layer.update()
    return f


def main():
    a = args()
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    rpg = os.path.join(a.archive, "ExplosiveLLC", "RPG Character Mecanim Animation Pack FREE")
    fm = os.path.join(a.archive, "FightingMotionsVolume1", "FBX")
    report = {"clips": [], "skipped": [], "mapping": HERO_MAPPING}

    if a.extend_hero:
        bpy.ops.import_scene.gltf(filepath=os.path.abspath(a.extend_hero))
        model = list(bpy.data.objects)
        rig = armature_of(model)
        existing = list(bpy.data.actions)
        rig.animation_data_create()
        for track in list(rig.animation_data.nla_tracks):
            rig.animation_data.nla_tracks.remove(track)
        rig.animation_data.action = None
        appended = set(HERO_MAPPING["mage"]["clips"]) | {n for sec in EXTRA_SECTIONS for clips in HERO_MAPPING[sec]["sources"].values() for n in clips}
        for act in existing:
            if act.name not in appended:
                push(rig, act, act.name)
                report["clips"].append(act.name)
            else:
                bpy.data.actions.remove(act)
        append_mage(a.archive, rig, report)
        for sec in EXTRA_SECTIONS:
            append_sources(sec, a.reference, rig, report)
        export_hero(rig, a.out, report)
        return

    model = import_fbx(os.path.join(rpg, "Models", "Characters", "RPG-Character-Bones.FBX"))
    rig = armature_of(model)
    rig.animation_data_create()
    if rig.animation_data.action:
        rig.animation_data.action = None

    # the RPG character's own clips (3ds Max empties baked onto its bones)
    for path in sorted(glob.glob(os.path.join(rpg, "Animations", "Unarmed", "*.FBX"))):
        name = os.path.splitext(os.path.basename(path))[0].split("@")[-1].replace(".max", "")
        objs = import_fbx(path)
        act = bake_from_empties(rig, objs, name)
        (report["clips"] if act else report["skipped"]).append(name)
        if act:
            push(rig, act, name)
        for ob in objs:
            bpy.data.objects.remove(ob, do_unlink=True)

    # FightingMotions clips retargeted onto the RPG rig
    for path in sorted(glob.glob(os.path.join(fm, "**", "*.fbx"), recursive=True)):
        name = os.path.splitext(os.path.basename(path))[0]
        if name == "BlueGuy":
            continue
        objs = import_fbx(path)
        src = armature_of(objs)
        act = retarget(src, rig, name, FM_TO_RPG) if src else None
        (report["clips"] if act else report["skipped"]).append(name)
        if act:
            push(rig, act, name)
        for ob in objs:
            bpy.data.objects.remove(ob, do_unlink=True)
        for act_left in [x for x in bpy.data.actions if x.users == 0 and not x.use_fake_user]:
            bpy.data.actions.remove(act_left)

    append_mage(a.archive, rig, report)
    for sec in EXTRA_SECTIONS:
        append_sources(sec, a.reference, rig, report)
    report["normalize_scale"] = normalize(model, a.height)
    export_hero(rig, a.out, report)


def append_mage(archive, rig, report):
    mage = HERO_MAPPING["mage"]
    for name, relative in mage["clips"].items():
        objs = import_fbx(os.path.join(archive, relative))
        src = armature_of(objs)
        act = retarget(src, rig, name, mage["bones"], motion_root="B-root") if src else None
        if not act:
            raise ValueError(f"{name}: mage source has no armature/action")
        push(rig, act, name)
        report["clips"].append(name)
        for ob in objs:
            bpy.data.objects.remove(ob, do_unlink=True)


EXTRA_SECTIONS = ("mixamo", "ual")


def load_source(path, wanted):
    """Armature objects, the wanted actions by name, and scene fps / source fps."""
    ext = os.path.splitext(path)[1].lower()
    if ext == ".blend":
        with bpy.data.libraries.load(path, link=False) as (src_data, dst):
            dst.objects = list(src_data.objects)
            dst.actions = [n for n in src_data.actions if n in wanted]
            dst.scenes = src_data.scenes[:1]
        src_fps = dst.scenes[0].render.fps / dst.scenes[0].render.fps_base if dst.scenes else bpy.context.scene.render.fps
        for sc in dst.scenes:
            bpy.data.scenes.remove(sc)
        objs = [ob for ob in dst.objects if ob is not None]
        for ob in objs:
            bpy.context.scene.collection.objects.link(ob)
        acts = {a.name: a for a in dst.actions if a is not None}
        return objs, acts, (bpy.context.scene.render.fps / bpy.context.scene.render.fps_base) / src_fps
    # glTF and FBX importers already convert source time into scene frames
    before_objs, before_acts = set(bpy.data.objects), set(bpy.data.actions)
    if ext in (".glb", ".gltf"):
        bpy.ops.import_scene.gltf(filepath=path)
    else:
        bpy.ops.import_scene.fbx(filepath=path, use_anim=True)
    objs = [ob for ob in bpy.data.objects if ob not in before_objs]
    acts = {a.name: a for a in bpy.data.actions if a not in before_acts}
    return objs, acts, 1.0


def append_sources(section, reference, rig, report):
    """Clips from another rig family, retargeted onto the hero. Paths are relative to `reference`
    (absolute paths are used as they are)."""
    mx = HERO_MAPPING[section]
    for relative, clips in mx["sources"].items():
        path = os.path.join(reference, relative)
        objs, acts, time_scale = load_source(path, set(clips.values()))
        src = max((ob for ob in objs if ob.type == "ARMATURE"), key=lambda r: len(r.data.bones))
        src.animation_data_create()
        # imported NLA strips would stack on top of the action being sampled
        for t in list(src.animation_data.nla_tracks):
            src.animation_data.nla_tracks.remove(t)
        for name, action in clips.items():
            act_src = acts.get(action)
            if act_src is None:
                report["skipped"].append(f"{name}: no action {action!r} in {relative}")
                continue
            src.animation_data.action = act_src
            if hasattr(src.animation_data, "action_slot") and len(getattr(act_src, "slots", [])) > 0:
                src.animation_data.action_slot = act_src.slots[0]
            act = retarget(src, rig, name, mx["bones"], time_scale=time_scale)
            push(rig, act, name)
            report["clips"].append(name)
        for ob in objs:
            bpy.data.objects.remove(ob, do_unlink=True)
        for act in acts.values():
            if not act.use_fake_user:
                bpy.data.actions.remove(act)


def export_hero(rig, output, report):
    rig.animation_data.use_nla = True
    os.makedirs(os.path.dirname(os.path.abspath(output)), exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=output, export_format="GLB", export_animations=True,
                              export_animation_mode="NLA_TRACKS", export_force_sampling=True, export_skins=True, export_yup=True)
    with open(output + ".report.json", "w") as f:
        json.dump(report, f, indent=2)
    print(f"HERO_DONE clips={len(report['clips'])} skipped={len(report['skipped'])} -> {output}")


if __name__ == "__main__":
    main()
