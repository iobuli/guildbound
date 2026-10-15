"""Rig profile: every control variable of a model's bones, fitted to that model, so one set of rules drives all.

Blender -b --factory-startup -P rig_profile.py -- --glb A.glb [--glb B.glb ...] [--base assets/packs/hero/fighter.glb]

The base model (the hero) defines the anatomy once: joint ranges as offsets from its own rest bends, segment mass
fractions, look limits, finger curl. Every other model is a clone of that profile edited by its own measurements:
rest bends (so a rig whose bind pose already bends the neck 30 deg gets the same freedom as one with a straight
neck), bind bone lengths (the no-stretch rule), height and mass, torso sections and limb radii (self-collision),
which hands have fingers and which side the palm faces. Written to <glb>.rig.json; the game reads it per rig.
"""
import argparse
import json
import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
import body_fit  # noqa: E402

# (segment start, joint, segment end, min bend, max bend) on the base model, degrees (joints.rs RULES)
BASE_RULES = [
    ("B_Pelvis", "B_Spine", "B_Spine1", 0.0, 40.0),
    ("B_Spine", "B_Spine1", "B_Spine2", 0.0, 35.0),
    ("B_Spine1", "B_Spine2", "B_Neck", 0.0, 50.0),
    ("B_Spine2", "B_Neck", "B_Head", 0.0, 70.0),
    ("B_L_Clavicle", "B_L_UpperArm", "B_L_Forearm", 0.0, 150.0),
    ("B_R_Clavicle", "B_R_UpperArm", "B_R_Forearm", 0.0, 150.0),
    ("B_L_UpperArm", "B_L_Forearm", "B_L_Forearm.001", 0.0, 155.0),
    ("B_R_UpperArm", "B_R_Forearm", "B_R_Hand", 0.0, 155.0),
    ("B_L_Forearm", "B_L_Forearm.001", "B_L_Finger2", 0.0, 80.0),
    ("B_R_Forearm", "B_R_Hand", "B_R_Finger2", 0.0, 80.0),
    ("B_L_Calf", "B_L_Foot", "B_L_Toe0", 15.0, 145.0),
    ("B_R_Calf", "B_R_Foot", "B_R_Toe0", 15.0, 145.0),
]
KNEE_MAX_FLEX = 135.0
MASS_KG_AT_1_8 = 75.0


def load(path):
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(path))
    rig = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    rig.animation_data_create()
    rig.animation_data.use_nla = False
    rig.animation_data.action = None
    for pb in rig.pose.bones:
        pb.matrix_basis.identity()
    bpy.context.view_layer.update()
    return rig


def rest_bends(P):
    out = {}
    for a, j, b, *_ in BASE_RULES:
        if a in P and j in P and b in P:
            u, v = P[j] - P[a], P[b] - P[j]
            if u.length > 1e-6 and v.length > 1e-6:
                out[j] = math.degrees(u.angle(v))
    return out


def profile(path, base_bends):
    rig = load(path)
    P = body_fit.heads(rig)
    bends = rest_bends(P)
    rules = []
    for a, j, b, lo, hi in BASE_RULES:
        if j not in bends:
            continue
        # clone the base range, edited by how far this model's rest already bends the joint
        shift = bends[j] - base_bends.get(j, bends[j])
        rules.append({"from": a, "joint": j, "to": b, "rest_deg": round(bends[j], 1),
                      "min_deg": round(max(0.0, lo + shift), 1), "max_deg": round(min(179.0, hi + shift), 1)})
    lengths = {pb.name: round((P[pb.name] - P[pb.parent.name]).length, 4) for pb in rig.pose.bones if pb.parent}
    zs = [v.z for v in P.values()]
    height = max(zs) - min(zs) + 0.12  # head joint to crown
    fingers = {s: [n for n in ("index", "middle", "ring", "pinky", "thumb") if any(b.startswith(f"{n}_0") and b.endswith(f"_{s}") for b in P) or (n == "middle" and f"B_{s.upper()}_Finger2" in P)] for s in "lr"}
    palm = {}
    for s, hand in (("l", "B_L_Forearm.001"), ("r", "B_R_Hand")):
        need = (f"index_01_{s}", f"index_02_{s}", f"pinky_01_{s}", f"thumb_02_{s}", hand)
        if all(n in P for n in need):
            along = (P[f"index_02_{s}"] - P[f"index_01_{s}"]).normalized()
            across = (P[f"index_01_{s}"] - P[f"pinky_01_{s}"]).normalized()
            n = along.cross(across).normalized()
            # the palm faces the side the thumb sits on; how far the thumb sits off the finger plane says how sure
            palm[s] = {"faces": "thumb side", "thumb_off_plane_m": round(abs(n.dot(P[f"thumb_02_{s}"] - P[hand])), 4)}
    meshes = [o for o in bpy.data.objects if o.type == "MESH" and o.parent == rig]
    body = None
    if meshes:
        torso = [v for v, _ in body_fit.skinned(meshes, rig, body_fit.TORSO_BONES)]
        if torso:
            body = []
            for a_, b_, lr in body_fit.SECTIONS:
                if not all(n in P for n in (a_, b_, *lr)):
                    continue
                fr = body_fit.frame(P, a_, b_, lr)
                band = [(x, y) for t, x, y in (body_fit.local(v, fr) for v in torso) if 0.0 <= t <= 1.0]
                if len(band) > 20:
                    cx = (body_fit.pct([x for x, _ in band], 0.97) + body_fit.pct([x for x, _ in band], 0.03)) / 2
                    cy = (body_fit.pct([y for _, y in band], 0.97) + body_fit.pct([y for _, y in band], 0.03)) / 2
                    body.append({"from": a_, "to": b_, "lateral": list(lr), "a": round(body_fit.pct([abs(x - cx) for x, _ in band], 0.95), 4),
                                 "b": round(body_fit.pct([abs(y - cy) for _, y in band], 0.95), 4), "cx": round(cx, 4), "cy": round(cy, 4)})
    return {
        "model": os.path.basename(path), "height_m": round(height, 3), "mass_kg": round(MASS_KG_AT_1_8 * (height / 1.8) ** 3, 1),
        "rules": rules, "knee_max_flex_deg": KNEE_MAX_FLEX, "look": {"yaw_deg": 52.0, "pitch_deg": 46.0},
        "fingers": fingers, "palm": palm, "lengths": lengths, "torso": body,
        "bones": len(rig.pose.bones),
    }


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--glb", action="append", required=True)
    p.add_argument("--base", default=os.path.join(os.path.dirname(__file__), "..", "..", "assets", "packs", "hero", "fighter.glb"))
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    base_bends = rest_bends(body_fit.heads(load(a.base)))
    for g in a.glb:
        prof = profile(g, base_bends)
        with open(os.path.abspath(g) + ".rig.json", "w") as fh:
            json.dump(prof, fh, indent=1)
        print("RIG_PROFILE", prof["model"], "height", prof["height_m"], "mass", prof["mass_kg"], "fingers", {k: len(v) for k, v in prof["fingers"].items()}, "palm", prof["palm"],
              "rules", [(r["joint"], r["rest_deg"], r["min_deg"], r["max_deg"]) for r in prof["rules"]][:12])


main()
