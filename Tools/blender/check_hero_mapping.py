"""Headless hero mapping audit. Run with Blender -b -P this_file -- --archive ASSETS.

Reports mapped rest/pose axes and sampled bone positions in Blender world space.
This is an offline check; no archive assets are changed.
"""
import argparse
import json
import math
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
import build_hero as hero


def positions(rig, names):
    return {name: [round(v, 5) for v in (rig.matrix_world @ rig.pose.bones[name].matrix).translation]
            for name in names}


def strike_landmarks(rig, act, start, end):
    """Measured forward extension, relative to pelvis (root travel excluded).

    These are presentation landmarks, not collision rules. Select the striking
    limb by its motion and report it so candidate contacts can be inspected.
    """
    name = act.name.lower()
    kick = name.startswith(("hk_", "bk_", "lk_"))
    bones = ["B_L_Foot", "B_R_Foot"] if kick else ["B_L_Forearm.001", "B_R_Hand"]
    if "left" in name or "attack-l" in name:
        bones = bones[:1]
    elif "right" in name or "attack-r" in name:
        bones = bones[1:]
    samples = {bone: [] for bone in bones}
    for k in range(65):
        frame = start + (end - start) * k / 64
        bpy.context.scene.frame_set(int(frame), subframe=frame % 1)
        bpy.context.view_layer.update()
        pelvis = (rig.matrix_world @ rig.pose.bones["B_Pelvis"].matrix).translation
        for bone in bones:
            point = (rig.matrix_world @ rig.pose.bones[bone].matrix).translation
            # RPG model faces Blender -Y (glTF +Z); runtime removes root travel.
            samples[bone].append(-(point - pelvis).y)
    bone, values = max(samples.items(), key=lambda row: max(row[1]) - min(row[1]))
    peak = max(range(1, 64), key=lambda i: values[i])
    threshold = max(values) - (max(values) - min(values)) * 0.08
    first, last = peak, peak
    while first > 1 and values[first - 1] >= threshold:
        first -= 1
    while last < 63 and values[last + 1] >= threshold:
        last += 1
    # A single sampled peak still gets a finite contact interval.
    first, last = min(first, peak - 1), max(last, peak + 1)
    valid = 0 < first < peak < last < 64 and max(values) - min(values) > 0.05
    return {"contact_start": round(first / 64, 6), "contact_peak": round(peak / 64, 6),
            "contact_end": round(last / 64, 6), "bone": bone, "samples": 65,
            "extension_m": round(values[peak], 5), "excursion_m": round(max(values) - min(values), 5),
            "valid": valid}


def foot_contact_curve(rig, start, end, sole_heights, root_reference_z):
    curve = []
    for k in range(65):
        frame = start + (end - start) * k / 64
        bpy.context.scene.frame_set(int(frame), subframe=frame % 1)
        bpy.context.view_layer.update()
        root_z = (rig.matrix_world @ rig.pose.bones["Motion"].matrix).translation.z
        row = []
        for side, bone in enumerate(["B_L_Foot", "B_R_Foot"]):
            height = (rig.matrix_world @ rig.pose.bones[bone].matrix).translation.z - (root_z - root_reference_z)
            t = max(0.0, min(1.0, (height - sole_heights[side] - 0.02) / 0.035))
            row.append(round(1.0 - t * t * (3.0 - 2.0 * t), 6))
        curve.append(row)
    return curve


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--archive")
    p.add_argument("--glb")
    p.add_argument("--out")
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    if a.glb:
        bpy.ops.import_scene.gltf(filepath=os.path.abspath(a.glb))
        rig = next(ob for ob in bpy.data.objects if ob.type == "ARMATURE")
        rig.animation_data_create()
        rig.animation_data.use_nla = False
        # glTF inverse-bind meshes are not undeformed world-space geometry.
        # Calibrate ankle offsets from the native neutral standing pose instead
        # of subtracting an unposed mesh bound (which was over a metre off).
        neutral = bpy.data.actions["Unarmed-Idle-Static"]
        rig.animation_data.action = neutral
        if getattr(neutral, "slots", None):
            rig.animation_data.action_slot = neutral.slots[0]
        bpy.context.scene.frame_set(int(neutral.frame_range[0]))
        bpy.context.view_layer.update()
        root_reference_z = (rig.matrix_world @ rig.pose.bones["Motion"].matrix).translation.z
        sole_heights = [(rig.matrix_world @ rig.pose.bones[bone].matrix).translation.z for bone in ["B_L_Foot", "B_R_Foot"]]
        if not all(0.03 < height < 0.2 for height in sole_heights):
            raise ValueError(f"Neutral stance ankle calibration is invalid: {sole_heights}")
        rows = []
        combat_clips = {c for names in hero.HERO_MAPPING["combat"].values() for c in names}
        combat_clips.update(c for names in hero.HERO_MAPPING.get("spells", {}).values() for c in names)
        landmarks = {}
        foot_contacts = {}
        for act in sorted(bpy.data.actions, key=lambda x: x.name):
            rig.animation_data.action = None
            for pb in rig.pose.bones:
                pb.matrix_basis.identity()
            rig.animation_data.action = act
            if getattr(act, "slots", None):
                rig.animation_data.action_slot = act.slots[0]
            start, end = act.frame_range
            max_scale_error = 0.0
            finite = True
            min_upright = float("inf")
            travel_m = 0.0
            previous_root = None
            pelvis_heights = []
            foot_heights = []
            for k in range(9):
                frame = start + (end - start) * k / 8
                bpy.context.scene.frame_set(int(frame), subframe=frame % 1)
                bpy.context.view_layer.update()
                root = (rig.matrix_world @ rig.pose.bones["Motion"].matrix).translation
                if previous_root is not None:
                    travel_m += math.hypot(root.x - previous_root.x, root.y - previous_root.y)
                previous_root = root.copy()
                for pb in rig.pose.bones:
                    finite &= all(math.isfinite(v) for row in pb.matrix for v in row)
                    max_scale_error = max(max_scale_error, *(abs(v - 1.0) for v in pb.scale))
                pos = positions(rig, ["B_Head", "B_Pelvis"])
                pelvis_heights.append(pos["B_Pelvis"][2] - root.z)
                feet = positions(rig, ["B_L_Foot", "B_R_Foot"])
                foot_heights.append(min(feet["B_L_Foot"][2], feet["B_R_Foot"][2]) - root.z)
                min_upright = min(min_upright, pos["B_Head"][2] - pos["B_Pelvis"][2])
            upright = "idle" not in act.name.lower() or min_upright > 0.4
            rows.append({"clip": act.name, "samples": 9, "finite": finite,
                         "travel_m": round(travel_m, 6),
                         "max_bone_scale_error": round(max_scale_error, 6),
                         "head_above_pelvis_m": round(min_upright, 5),
                         "pelvis_relative_root_z": [round(min(pelvis_heights), 5), round(max(pelvis_heights), 5)],
                         "feet_relative_root_z": [round(min(foot_heights), 5), round(max(foot_heights), 5)],
                         "pass": finite and max_scale_error < 0.001 and upright})
            if act.name in combat_clips:
                landmarks[act.name] = strike_landmarks(rig, act, start, end)
            foot_contacts[act.name] = foot_contact_curve(rig, start, end, sole_heights, root_reference_z)
        found = {row["clip"] for row in rows}
        missing_roles = {r: c for r, c in hero.HERO_MAPPING["roles"].items() if c not in found}
        for kind, names in hero.HERO_MAPPING["locomotion"].items():
            missing_roles.update({f"{kind}_{i}": c for i, c in enumerate(names) if c not in found})
        for cluster, names in hero.HERO_MAPPING["combat"].items():
            missing_roles.update({f"combat_{cluster}_{i}": c for i, c in enumerate(names) if c not in found})
        for spell, names in hero.HERO_MAPPING.get("spells", {}).items():
            missing_roles.update({f"spell_{spell}_{i}": c for i, c in enumerate(names) if c not in found})
        result = {"glb": a.glb, "clip_count": len(rows), "samples": len(rows) * 9,
                  "missing_roles": missing_roles, "clips": rows, "strike_landmarks": landmarks,
                  "foot_contact_samples": len(rows) * 65, "sole_height_m": sole_heights,
                  "pass": bool(rows) and not missing_roles and all(r["pass"] for r in rows) and all(m["valid"] for m in landmarks.values())}
        if a.out:
            os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
            with open(a.out, "w") as f:
                json.dump(result, f, indent=2)
        print("HERO_MAPPING_AUDIT " + json.dumps({k: v for k, v in result.items() if k not in ["clips", "strike_landmarks"]}))
        if not result["pass"]:
            raise RuntimeError("Hero mapping audit failed; see report")
        # Derived pack data stays beside the git-ignored GLB. The controller uses
        # measured root travel per cycle rather than a guessed playback speed.
        with open(a.glb + ".motion.json", "w") as f:
            json.dump({"schema": 3, "verified": True, "strides_m": {r["clip"]: r["travel_m"] for r in rows if r["travel_m"] > 0.05}, "strikes": landmarks, "foot_contacts": foot_contacts, "sole_height_m": sole_heights}, f, indent=2)
        return
    if not a.archive:
        p.error("provide --glb or --archive")
    model = hero.import_fbx(os.path.join(a.archive, "ExplosiveLLC", "RPG Character Mecanim Animation Pack FREE", "Models", "Characters", "RPG-Character-Bones.FBX"))
    tgt = hero.armature_of(model)
    tgt.animation_data_clear()
    src = hero.armature_of(hero.import_fbx(os.path.join(a.archive, "FightingMotionsVolume1", "FBX", "idle_Tiramis.fbx")))
    bpy.context.scene.frame_set(1)
    result = {"target_bones": [b.name for b in tgt.data.bones], "source": positions(src, ["Hip", "Head", "Foot_L", "Foot_R"]),
              "target_rest": positions(tgt, ["B_Pelvis", "B_Head", "B_L_Foot", "B_R_Foot"]),
              "source_root": [[b.name, list(b.matrix_local.to_quaternion()), list(src.pose.bones[b.name].rotation_quaternion)] for b in src.data.bones if b.parent is None]}
    act = hero.retarget(src, tgt, "audit_idle", hero.FM_TO_RPG)
    tgt.animation_data.action = act
    bpy.context.scene.frame_set(1)
    bpy.context.view_layer.update()
    result["mapped"] = positions(tgt, ["B_Pelvis", "B_Head", "B_L_Foot", "B_R_Foot"])
    print("HERO_MAPPING_AUDIT " + json.dumps(result))


if __name__ == "__main__":
    main()
