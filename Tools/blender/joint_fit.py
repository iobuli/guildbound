"""Joint fit: where the mesh says each joint is, versus where the bone head sits.

Blender -b --factory-startup -P joint_fit.py -- --glb A.glb [--glb B.glb] [--apply OUT.glb]

A joint's centre is where the skin is shared between a bone and its parent: the centroid of vertices weighted to
both (each >= 0.25), taken at the bind pose. The offset from the bone head is the misfit; it is written per bone
into <glb>.rig.json under "joint_fit" (metres, world at bind). Only rings with enough vertices and a symmetric
spread count, so a joint hidden in a single-bone region (fingers under a glove) is skipped, not guessed.
"""
import argparse, json, os, sys
import bpy
from mathutils import Vector

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--glb", action="append", required=True)
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    for g in a.glb:
        for ob in list(bpy.data.objects): bpy.data.objects.remove(ob, do_unlink=True)
        bpy.ops.import_scene.gltf(filepath=os.path.abspath(g))
        rig = next(o for o in bpy.data.objects if o.type == "ARMATURE")
        rig.animation_data_create(); rig.animation_data.action = None
        for pb in rig.pose.bones: pb.matrix_basis.identity()
        bpy.context.view_layer.update()
        mw = rig.matrix_world
        heads = {b.name: mw @ b.head_local for b in rig.data.bones}
        parent = {b.name: b.parent.name for b in rig.data.bones if b.parent}
        rings = {}
        for m in (o for o in bpy.data.objects if o.type == "MESH" and o.parent == rig):
            names = {vg.index: vg.name for vg in m.vertex_groups}
            for v in m.data.vertices:
                w = {names[x.group]: x.weight for x in v.groups if x.group in names}
                for child, par in parent.items():
                    if w.get(child, 0) >= 0.25 and w.get(par, 0) >= 0.25:
                        rings.setdefault(child, []).append(m.matrix_world @ v.co)
        tails = {b.name: (heads[b.children[0].name] if b.children else mw @ b.tail_local) for b in rig.data.bones}
        fit = {}
        for child, pts in rings.items():
            if len(pts) < 12: continue
            c = sum(pts, Vector()) / len(pts)
            off = c - heads[child]
            axis = (tails[child] - heads[child]).normalized() if (tails[child] - heads[child]).length > 1e-4 else Vector((0, 0, 1))
            along = off.dot(axis)
            side = off - axis * along
            fit[child] = {"offset": [round(x, 4) for x in off], "along_m": round(along, 4), "side_m": round(side.length, 4), "dist_m": round(side.length, 4), "verts": len(pts)}
        # limb centre line: vertices a bone drives alone (>= 0.8), their centroid's sideways distance from the bone line
        core = {}
        for m in (o for o in bpy.data.objects if o.type == "MESH" and o.parent == rig):
            names = {vg.index: vg.name for vg in m.vertex_groups}
            for v in m.data.vertices:
                for x in v.groups:
                    if x.group in names and x.weight >= 0.8:
                        core.setdefault(names[x.group], []).append(m.matrix_world @ v.co)
        limbs = {}
        for n in ("B_L_Thigh", "B_R_Thigh", "B_L_Calf", "B_R_Calf", "B_L_UpperArm", "B_R_UpperArm", "B_L_Forearm", "B_R_Forearm", "B_Spine1", "B_Spine2", "B_Neck"):
            pts = core.get(n, [])
            if len(pts) < 20: continue
            h, t = heads[n], tails[n]
            ax = (t - h).normalized()
            sides = [(q - h) - ax * (q - h).dot(ax) for q in pts if 0.0 <= (q - h).dot(ax) <= (t - h).length]
            if len(sides) < 20: continue
            mean = sum(sides, Vector()) / len(sides)
            limbs[n] = {"centre_off_m": round(mean.length, 4), "dir": [round(x, 3) for x in (mean.normalized() if mean.length > 1e-6 else mean)], "verts": len(sides)}
        fit["_limb_centre"] = limbs
        path = os.path.abspath(g) + ".rig.json"
        prof = json.load(open(path)) if os.path.exists(path) else {}
        prof["joint_fit"] = fit
        json.dump(prof, open(path, "w"), indent=1)
        worst = sorted(((k, v) for k, v in fit.items() if k != "_limb_centre"), key=lambda kv: -kv[1]["side_m"])
        print("JOINT_FIT", os.path.basename(g), "sideways", [(k, v["side_m"], v["along_m"]) for k, v in worst[:10]])
        print("LIMB_CENTRE", os.path.basename(g), [(k, v["centre_off_m"], v["dir"]) for k, v in fit["_limb_centre"].items()])

if __name__ == "__main__":
    main()
