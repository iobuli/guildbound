"""Fit a model's torso volume and limb radii from its skinned mesh, for self-collision (arms through the body).

Blender -b --factory-startup -P body_fit.py -- --glb assets/packs/hero/fighter.glb [--check CLIP@T ...]

The torso is three elliptic sections along the spine chain, each in a frame built from joint positions only
(axis = next joint - joint, lateral = right minus left clavicle/thigh), so the same numbers work in Blender and
Bevy: lateral half-width a, depth half-width b, centre offset (cx lateral, cy forward). Written to <glb>.body.json.
--check reports, for poses, how deep arm vertices sit inside those sections.
"""
import argparse
import json
import math
import os
import sys

import bpy
from mathutils import Vector

SECTIONS = [
    ("B_Pelvis", "B_Spine", ("B_R_Thigh", "B_L_Thigh")),
    ("B_Spine", "B_Spine2", ("B_R_Thigh", "B_L_Thigh")),
    ("B_Spine2", "B_Neck", ("B_R_Clavicle", "B_L_Clavicle")),
]
TORSO_BONES = {"B_Pelvis", "B_Spine", "B_Spine1", "B_Spine2"}
ARM_BONES = {f"B_{s}_{b}" for s in "LR" for b in ("UpperArm", "Forearm")} | {"B_L_Forearm.001", "B_R_Hand"}
LIMBS = [("B_L_UpperArm", "B_L_Forearm"), ("B_L_Forearm", "B_L_Forearm.001"), ("B_L_Forearm.001", "B_L_Finger2"),
         ("B_R_UpperArm", "B_R_Forearm"), ("B_R_Forearm", "B_R_Hand"), ("B_R_Hand", "B_R_Finger2")]


def frame(P, a, b, lr):
    up = (P[b] - P[a])
    length = up.length
    up = up / length
    right = P[lr[0]] - P[lr[1]]
    right = (right - up * right.dot(up)).normalized()
    fwd = up.cross(right)
    return P[a], up, right, fwd, length


def local(p, fr):
    o, up, right, fwd, length = fr
    d = p - o
    return d.dot(up) / length, d.dot(right), d.dot(fwd)


def heads(rig):
    return {b.name: rig.matrix_world @ b.head for b in rig.pose.bones}


def skinned(meshes, rig, pick):
    """World positions of vertices whose strongest group is in `pick`, with that group name."""
    dg = bpy.context.evaluated_depsgraph_get()
    out = []
    for m in meshes:
        ev = m.evaluated_get(dg)
        me = ev.to_mesh()
        names = {g.index: g.name for g in m.vertex_groups}
        for v in m.data.vertices:
            if not v.groups:
                continue
            g = max(v.groups, key=lambda x: x.weight)
            n = names.get(g.group)
            if n in pick and g.weight > 0.5:
                out.append((ev.matrix_world @ me.vertices[v.index].co, n))
        ev.to_mesh_clear()
    return out


def pct(vals, q):
    vals = sorted(vals)
    return vals[min(len(vals) - 1, int(q * len(vals)))] if vals else 0.0


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--glb", required=True)
    p.add_argument("--check", nargs="*", default=[])
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    bpy.context.scene.render.fps = 30
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(a.glb))
    rig = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    meshes = [o for o in bpy.data.objects if o.type == "MESH" and o.parent == rig]
    rig.animation_data_create()
    rig.animation_data.use_nla = False
    rig.animation_data.action = None
    for pb in rig.pose.bones:
        pb.matrix_basis.identity()
    bpy.context.view_layer.update()
    P = heads(rig)
    torso = [v for v, _ in skinned(meshes, rig, TORSO_BONES)]
    sections = []
    for a_, b_, lr in SECTIONS:
        fr = frame(P, a_, b_, lr)
        pts = [local(v, fr) for v in torso]
        band = [(x, y) for t, x, y in pts if -0.15 <= t <= 1.15] if a_ == "B_Pelvis" else [(x, y) for t, x, y in pts if 0.0 <= t <= 1.0]
        cx = (pct([x for x, _ in band], 0.97) + pct([x for x, _ in band], 0.03)) / 2
        cy = (pct([y for _, y in band], 0.97) + pct([y for _, y in band], 0.03)) / 2
        sections.append({"from": a_, "to": b_, "lateral": list(lr), "a": round(pct([abs(x - cx) for x, _ in band], 0.95), 4),
                         "b": round(pct([abs(y - cy) for _, y in band], 0.95), 4), "cx": round(cx, 4), "cy": round(cy, 4), "verts": len(band)})
    limbs = []
    arm = skinned(meshes, rig, ARM_BONES)
    for a_, b_ in LIMBS:
        if a_ not in P or b_ not in P:
            continue
        seg = P[b_] - P[a_]
        ds = []
        for v, n in arm:
            if n != a_:
                continue
            t = max(0.0, min(1.0, (v - P[a_]).dot(seg) / seg.length_squared))
            ds.append((v - (P[a_] + seg * t)).length)
        limbs.append({"from": a_, "to": b_, "radius": round(pct(ds, 0.8), 4)})
    out = {"sections": sections, "limbs": limbs}
    path = os.path.abspath(a.glb) + ".body.json"
    with open(path, "w") as fh:
        json.dump(out, fh, indent=1)
    print("BODY_FIT", json.dumps(out))
    for spec in a.check:
        clip, t = spec.rsplit("@", 1)
        act = bpy.data.actions[clip]
        rig.animation_data.action = act
        if getattr(act, "slots", None):
            rig.animation_data.action_slot = act.slots[0]
        s0, s1 = act.frame_range
        f = s0 + float(t) * (s1 - s0)
        bpy.context.scene.frame_set(int(f), subframe=f % 1)
        P = heads(rig)
        worst = {}
        fingers = {pb.name for pb in rig.pose.bones if pb.name.startswith(("index", "middle", "ring", "pinky", "thumb")) or "Finger" in pb.name}
        for v, n in skinned(meshes, rig, ARM_BONES | fingers):
            for s in sections:
                fr = frame(P, s["from"], s["to"], s["lateral"])
                tt, x, y = local(v, fr)
                if not 0.0 <= tt <= 1.0:
                    continue
                rho = math.hypot((x - s["cx"]) / s["a"], (y - s["cy"]) / s["b"])
                if rho < 1.0:
                    depth = (1.0 - rho) * min(s["a"], s["b"])
                    if depth > worst.get(n, (0, ""))[0]:
                        worst[n] = (round(depth, 3), s["from"])
        print("CLIP_CHECK", spec, worst)


main()
