"""Skin fixes on a built model, keeping every clip: Blender -b --factory-startup -P fix_skin.py -- --glb IN --out OUT

Pauldrons: the Quaternius outfit pieces come weighted 100% to the upper arm, so the rigid plate on top of the
shoulder swings round the shoulder joint with the arm and cuts into the chest and neck when the arm drops or
reaches (boss, 2026-10-13: "the shoulder clipping is annoying"). A pauldron is strapped to the shoulder girdle:
its top now follows the clavicle and hands over to the upper arm toward its lower edge (smoothstep along the arm).
"""
import argparse
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
import build_hero as hero  # noqa: E402


def smooth(a, b, x):
    t = max(0.0, min(1.0, (x - a) / (b - a)))
    return t * t * (3 - 2 * t)


def reweight_pauldrons(rig):
    changed = 0
    mw = rig.matrix_world
    for m in [o for o in bpy.data.objects if o.type == "MESH" and o.parent == rig and "Pauldron" in o.name]:
        names = {g.index: g.name for g in m.vertex_groups}
        for side in ("L", "R"):
            arm, clav, fore = f"B_{side}_UpperArm", f"B_{side}_Clavicle", f"B_{side}_Forearm"
            if arm not in m.vertex_groups:
                continue
            if clav not in m.vertex_groups:
                m.vertex_groups.new(name=clav)
            ga, gc = m.vertex_groups[arm], m.vertex_groups[clav]
            head = mw @ rig.data.bones[arm].head_local
            axis = (mw @ rig.data.bones[fore].head_local) - head
            length = axis.length
            axis.normalize()
            for v in m.data.vertices:
                w_arm = next((g.weight for g in v.groups if names.get(g.group) == arm), 0.0)
                if w_arm <= 0.0:
                    continue
                along = (m.matrix_world @ v.co - head).dot(axis) / length
                # on the shoulder top (along <= 0.05) the clavicle carries it; by a third down the arm, the arm
                share = smooth(0.05, 0.35, along)
                ga.add([v.index], w_arm * share, "REPLACE")
                gc.add([v.index], w_arm * (1.0 - share), "REPLACE")
                changed += 1
    return changed


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--glb", required=True)
    p.add_argument("--out", required=True)
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    bpy.context.scene.render.fps, bpy.context.scene.render.fps_base = hero.OUT_FPS, 1.0
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(a.glb))
    rig = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    rig.animation_data_create()
    rig.animation_data.use_nla = False
    n = reweight_pauldrons(rig)
    for t in list(rig.animation_data.nla_tracks):
        rig.animation_data.nla_tracks.remove(t)
    rig.animation_data.action = None
    for act in list(bpy.data.actions):
        tr = rig.animation_data.nla_tracks.new()
        tr.name = act.name
        st = tr.strips.new(act.name, int(act.frame_range[0]), act)
        if hasattr(st, "action_slot") and len(getattr(act, "slots", [])) > 0:
            st.action_slot = act.slots[0]
    print("REWEIGHTED", n, "pauldron vertices")
    hero.export_hero(rig, os.path.abspath(a.out), {"clips": [x.name for x in bpy.data.actions], "skipped": [], "skin": f"pauldron reweighted ({n} vertices)"})


if __name__ == "__main__":
    main()
