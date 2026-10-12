"""Pose audit: every clip, every frame, every model, one ranked digest Claude reads to fix causes.

Blender -b --factory-startup -P pose_audit.py -- --glb A.glb [--glb B.glb ...] [--source SRC.glb] [--out collab/pose_audit]

Checks (same rules the game applies at runtime, so a clean audit means the authority layer has nothing to do):
  range   - segment bend outside joints.rs RULES (deg over the limit)
  knee    - flex over 135 deg, or the knee off the hip-ankle line behind pelvis-forward (m)
  ground  - ball/ankle below the floor (m)
  torso   - hand inside the torso capsule pelvis->neck (m of overlap)
  pop     - a segment turning more than 25 deg in one 30 fps frame and 4x its clip median (deg)
  seam    - cyclic clip whose last frame differs from the first (deg, worst segment)
  source  - segment direction vs the same clip on --source (deg), when the source has the clip
Patterns group findings by (check, bone) across clips; a pattern in many clips is a pipeline cause.
"""
import argparse
import json
import math
import os
import sys

import bpy
from mathutils import Vector

RULES = [
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
SEGS = [("B_Pelvis", "B_Spine"), ("B_Spine", "B_Spine2"), ("B_Spine2", "B_Neck"), ("B_Neck", "B_Head")]
for s in "LR":
    hand = "B_L_Forearm.001" if s == "L" else "B_R_Hand"
    SEGS += [(f"B_{s}_Clavicle", f"B_{s}_UpperArm"), (f"B_{s}_UpperArm", f"B_{s}_Forearm"), (f"B_{s}_Forearm", hand),
             (f"B_{s}_Thigh", f"B_{s}_Calf"), (f"B_{s}_Calf", f"B_{s}_Foot"), (f"B_{s}_Foot", f"B_{s}_Toe0")]
KNEE_MAX = 135.0
TORSO_R = 0.09
CYCLIC = ("walk", "run", "sprint", "idle", "strafe", "jog", "loop", "crouch-walk", "swim")


def load(path):
    before, acts_before = set(bpy.data.objects), set(bpy.data.actions)
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(path))
    new = [o for o in bpy.data.objects if o not in before]
    rig = next(o for o in new if o.type == "ARMATURE")
    for o in new:
        if o.type == "MESH":
            bpy.data.objects.remove(o, do_unlink=True)
    acts = {a.name.split(".")[0] if a.name[-4:-3] == "." else a.name: a for a in bpy.data.actions if a not in acts_before}
    rig.animation_data_create()
    rig.animation_data.use_nla = False
    return rig, acts


def bind(rig, act):
    rig.animation_data.action = act
    if getattr(act, "slots", None):
        rig.animation_data.action_slot = act.slots[0]


def heads(rig):
    mw = rig.matrix_world
    return {pb.name: mw @ pb.head for pb in rig.pose.bones}


def dist_to_segment(p, a, b):
    ab = b - a
    t = max(0.0, min(1.0, (p - a).dot(ab) / max(ab.length_squared, 1e-9)))
    return (p - (a + ab * t)).length


def frame_checks(P, emit):
    for a, j, b, lo, hi in RULES:
        if a in P and j in P and b in P:
            u, v = P[j] - P[a], P[b] - P[j]
            if u.length > 1e-6 and v.length > 1e-6:
                bend = math.degrees(u.angle(v))
                over = max(bend - hi, lo - bend)
                if over > 0.5:
                    emit("range", j, over, f"bend {bend:.0f} range {lo:.0f}-{hi:.0f}")
    if "B_L_Thigh" in P and "B_R_Thigh" in P:
        right = P["B_R_Thigh"] - P["B_L_Thigh"]
        right.z = 0
        fwd = Vector((0, 0, 1)).cross(right).normalized() if right.length > 1e-6 else Vector((0, 1, 0))
        for s in "LR":
            h, k, a = P[f"B_{s}_Thigh"], P[f"B_{s}_Calf"], P[f"B_{s}_Foot"]
            flex = 180.0 - math.degrees((h - k).angle(a - k))
            if flex > KNEE_MAX + 0.5:
                emit("knee_flex", f"B_{s}_Calf", flex - KNEE_MAX, f"flex {flex:.0f}")
            line = (a - h).normalized()
            off = (k - h) - line * (k - h).dot(line)
            if off.dot(fwd) < -0.03 and flex > 8.0:
                emit("knee_back", f"B_{s}_Calf", -off.dot(fwd), "bends backward")
            for bone in (f"B_{s}_Toe0", f"B_{s}_Foot"):
                if bone in P and P[bone].z < -0.02:
                    emit("ground", bone, -P[bone].z, f"z {P[bone].z:.3f}")
    if "B_Pelvis" in P and "B_Neck" in P:
        for hand in ("B_L_Forearm.001", "B_R_Hand"):
            if hand in P:
                d = dist_to_segment(P[hand], P["B_Pelvis"], P["B_Neck"])
                if d < TORSO_R:
                    emit("torso", hand, TORSO_R - d, f"{d:.3f} m from spine")


def dirs(P):
    return {f"{a}->{b}": (P[b] - P[a]).normalized() for a, b in SEGS if a in P and b in P and (P[b] - P[a]).length > 1e-6}


def sample(rig, act, ts=None, remap=None, fix=None):
    """[(t, {bone: world head})] at every frame (or at the normalized times ts)."""
    bind(rig, act)
    s0, s1 = act.frame_range
    if ts is None:
        frames = [s0 + k for k in range(int(round(s1 - s0)) + 1)]
        if frames and frames[-1] < s1 - 1e-3:
            frames.append(s1)
    else:
        frames = [s0 + t * (s1 - s0) for t in ts]
    out = []
    for f in frames:
        bpy.context.scene.frame_set(int(f), subframe=f % 1)
        P = heads(rig)
        if remap:
            corr = fix(rig) if fix else None
            P = {remap[n]: (corr @ v if corr else v) for n, v in P.items() if n in remap}
        out.append(((f - s0) / max(s1 - s0, 1), P))
    return out


def audit_clip(model, name, poses, src_poses=None):
    worst, errs = {}, []

    def emit(check, bone, value, info, t=0.0):
        if (check, bone) not in worst or value > worst[(check, bone)][0]:
            worst[(check, bone)] = (value, round(t, 3), info)

    seq = []
    for n, (t, P) in enumerate(poses):
        frame_checks(P, lambda c, b, v, i: emit(c, b, v, i, t))
        D = dirs(P)
        seq.append(D)
        if src_poses:
            SD = dirs(src_poses[n][1])
            for k, d in D.items():
                if k in SD:
                    e = math.degrees(d.angle(SD[k]))
                    errs.append(e)
                    if e > 10.0:
                        emit("source", k, e, f"{e:.0f} deg off source", t)
    for k in seq[0] if seq else []:
        steps = [math.degrees(seq[i][k].angle(seq[i - 1][k])) for i in range(1, len(seq)) if k in seq[i] and k in seq[i - 1]]
        if len(steps) < 3:
            continue
        med = sorted(steps)[len(steps) // 2]
        i = max(range(len(steps)), key=lambda n: steps[n])
        if steps[i] > 25.0 and steps[i] > 4.0 * max(med, 1.0):
            emit("pop", k, steps[i], f"{steps[i]:.0f} deg in one frame (median {med:.1f})", poses[i + 1][0])
    low = name.lower()
    if seq and low.split("-")[-1] not in ("start", "end") and any(c in low for c in CYCLIC) and "to" not in low.split("-"):
        seam = max((math.degrees(seq[0][k].angle(seq[-1][k])) for k in seq[0] if k in seq[-1]), default=0.0)
        if seam > 6.0:
            emit("seam", "loop", seam, f"first vs last frame {seam:.0f} deg")
    summary = {"frames": len(poses), "issues": len(worst), "source_mean_deg": round(sum(errs) / len(errs), 1) if errs else None}
    return [{"model": model, "clip": name, "check": c, "bone": b, "value": round(v, 3), "t": t, "info": i}
            for (c, b), (v, t, i) in worst.items()], summary


def audit_model(path, src):
    rig, acts = load(path)
    model = os.path.basename(path)
    findings, clips = [], {}
    for name in sorted(acts):
        poses = sample(rig, acts[name])
        src_poses = sample(src[0], src[1][name], [t for t, _ in poses]) if src and name in src[1] else None
        f, clips[name] = audit_clip(model, name, poses, src_poses)
        findings += f
    bpy.data.objects.remove(rig, do_unlink=True)
    return findings, clips


HBM = os.path.join(os.path.dirname(__file__), "..", "..", "assets", "packs", "_sources", "kevin_human_basic", "Animations", "Male")


def audit_kevin():
    """Baseline for the nc_* clips: the raw Kevin Iglesias FBX, bones renamed by the same map build_master uses."""
    import glob
    with open(os.path.join(os.path.dirname(__file__), "hero_mapping.json")) as fh:
        remap = json.load(fh)["mage"]["bones"]
    nc = {"nc_jump_begin": "HumanM@Jump01 - Begin", "nc_fall": "HumanM@Fall01", "nc_jump_land": "HumanM@Jump01 - Land"}
    for kind in ("Walk", "Run"):
        for d in ("Forward", "ForwardRight", "Right", "BackwardRight", "Backward", "BackwardLeft", "Left", "ForwardLeft"):
            nc[f"nc_{kind.lower()}_{d.lower()}"] = f"HumanM@{kind}01_{d}"
    for d in ("Forward", "ForwardRight", "ForwardLeft"):
        nc[f"nc_sprint_{d.lower()}"] = f"HumanM@Sprint01_{d}"
    files = {os.path.splitext(os.path.basename(f))[0]: f for f in glob.glob(os.path.join(HBM, "**", "*.fbx"), recursive=True) if "[RM]" not in f}

    def fix(rig):
        # the animated B-root carries a unit conversion and quarter turn absent from the bind pose (as in retarget)
        mw = rig.matrix_world
        return (mw @ rig.data.bones["B-root"].matrix_local) @ (mw @ rig.pose.bones["B-root"].matrix).inverted()

    findings, clips = [], {}
    fps = bpy.context.scene.render.fps
    for name, stem in sorted(nc.items()):
        if stem not in files:
            continue
        before, acts_before = set(bpy.data.objects), set(bpy.data.actions)
        bpy.ops.import_scene.fbx(filepath=files[stem], use_anim=True)
        rig = next(o for o in bpy.data.objects if o not in before and o.type == "ARMATURE")
        f, clips[name] = audit_clip("kevin", name, sample(rig, rig.animation_data.action, remap=remap, fix=fix))
        findings += f
        for o in [o for o in bpy.data.objects if o not in before]:
            bpy.data.objects.remove(o, do_unlink=True)
        for a in [a for a in bpy.data.actions if a not in acts_before]:
            bpy.data.actions.remove(a)
    bpy.context.scene.render.fps = fps
    return findings, clips


METRES = ("ground", "torso", "knee_back")


def mark_introduced(findings, baseline):
    """A finding is ours (pipeline-introduced) unless the source clip already has it about as badly."""
    base = {(f["clip"], f["check"], f["bone"]): f["value"] for f in baseline}
    for f in findings:
        b = base.get((f["clip"], f["check"], f["bone"]), 0.0)
        margin = 0.02 if f["check"] in METRES else 4.0
        f["src"] = b
        f["introduced"] = f["check"] == "source" or f["value"] > b * 1.2 + margin


def patterns(findings):
    groups = {}
    for f in findings:
        if not f.get("introduced", True):
            continue
        g = groups.setdefault((f["model"], f["check"], f["bone"]), {"model": f["model"], "check": f["check"], "bone": f["bone"], "clips": 0, "worst": 0.0, "at": ""})
        g["clips"] += 1
        if f["value"] > g["worst"]:
            g["worst"], g["at"] = f["value"], f"{f['clip']} @{f['t']}"
    return sorted(groups.values(), key=lambda g: (-g["clips"], -g["worst"]))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--glb", action="append", required=True)
    p.add_argument("--source")
    p.add_argument("--kevin", action="store_true", help="baseline the nc_* clips on the raw Kevin FBX")
    p.add_argument("--out", default=os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "collab", "pose_audit"))
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    # the glTF importer converts seconds to frames at the scene rate; clips are exported at 30 fps
    bpy.context.scene.render.fps, bpy.context.scene.render.fps_base = 30, 1.0
    baseline, models = [], {}
    if a.source:
        baseline, models["source:" + os.path.basename(a.source)] = audit_model(a.source, None)
    if a.kevin:
        kb, models["source:kevin"] = audit_kevin()
        baseline += kb
    src = load(a.source) if a.source else None
    findings = []
    for g in a.glb:
        f, c = audit_model(g, src)
        findings += f
        models[os.path.basename(g)] = c
    mark_introduced(findings, baseline)
    pats = patterns(findings)
    out = {"models": models, "patterns": pats, "findings": sorted(findings, key=lambda f: -f["value"]),
           "source_findings": baseline}
    with open(a.out + ".json", "w") as fh:
        json.dump(out, fh, indent=1)
    lines = []
    for m, c in models.items():
        src_means = [v["source_mean_deg"] for v in c.values() if v["source_mean_deg"] is not None]
        lines.append(f"MODEL {m}: {len(c)} clips, {sum(1 for v in c.values() if v['issues'])} with issues"
                     + (f", source mean {sum(src_means) / len(src_means):.1f} deg" if src_means else ""))
    inherited = sum(1 for f in findings if not f["introduced"])
    lines.append(f"PATTERNS introduced by the pipeline (model check bone: clips, worst @ clip t); {inherited} findings inherited from the source omitted")
    lines += [f"  {g['model']} {g['check']} {g['bone']}: {g['clips']}, {g['worst']:.3g} @ {g['at']}" for g in pats[:60]]
    with open(a.out + ".txt", "w") as fh:
        fh.write("\n".join(lines) + "\n")
    print("POSE_AUDIT_BEGIN\n" + "\n".join(lines) + "\nPOSE_AUDIT_END")


main()
