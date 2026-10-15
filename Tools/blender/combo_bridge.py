"""Combo bridge: a new clip = attack A up to a cut + bridge frames + attack B, keyed along momentum-continuous paths.

Blender -b --factory-startup -P combo_bridge.py -- --glb assets/packs/hero/fighter.glb --pair A:B[:cut] ... --out OUT.glb

Bridge frames use cubic Hermite curves whose end tangents are the clips' own velocities at the join (per bone
rotation in a shared hemisphere, pelvis and root location), so the body's momentum carries through instead of
jumping. Reports the peak centre-of-mass acceleration at the join for a plain cut and for the bridge, and exports
every clip plus the bridges (named combo_A__B) to OUT.glb.
"""
import argparse, os, sys
import bpy
from mathutils import Quaternion, Vector

sys.path.insert(0, os.path.dirname(__file__))
import build_hero as hero  # noqa: E402

SEG = [("B_Pelvis", "B_Spine", .142), ("B_Spine", "B_Spine2", .139), ("B_Spine2", "B_Neck", .216), ("B_Neck", "B_Head", .081),
       ("B_L_UpperArm", "B_L_Forearm", .028), ("B_R_UpperArm", "B_R_Forearm", .028), ("B_L_Forearm", "B_L_Forearm.001", .022),
       ("B_R_Forearm", "B_R_Hand", .022), ("B_L_Thigh", "B_L_Calf", .1), ("B_R_Thigh", "B_R_Calf", .1), ("B_L_Calf", "B_L_Foot", .061), ("B_R_Calf", "B_R_Foot", .061)]


def bind(rig, act):
    rig.animation_data.action = act
    if getattr(act, "slots", None):
        rig.animation_data.action_slot = act.slots[0]


def sample(rig, act, frames):
    bind(rig, act)
    out = []
    for f in frames:
        bpy.context.scene.frame_set(int(f), subframe=f % 1)
        out.append({pb.name: (pb.location.copy(), pb.rotation_quaternion.copy()) for pb in rig.pose.bones})
    return out


def com_track(rig, poses):
    track = []
    for p in poses:
        for n, (loc, rot) in p.items():
            pb = rig.pose.bones[n]
            pb.rotation_mode = "QUATERNION"
            pb.location, pb.rotation_quaternion = loc, rot
        bpy.context.view_layer.update()
        H = {b.name: rig.matrix_world @ b.head for b in rig.pose.bones}
        track.append(sum(((H[a] + H[b]) * .5 * m for a, b, m in SEG), Vector()) / sum(m for *_, m in SEG))
    return track


def peak_accel(track, fps):
    v = [(b - a) * fps for a, b in zip(track, track[1:])]
    return max(((b - a) * fps).length for a, b in zip(v, v[1:]))


def herm(p0, p1, m0, m1, t):
    t2, t3 = t * t, t * t * t
    return p0 * (2 * t3 - 3 * t2 + 1) + m0 * (t3 - 2 * t2 + t) + p1 * (-2 * t3 + 3 * t2) + m1 * (t3 - t2)


def bridge(a_end, a_prev, b_start, b_next, n):
    """n frames between pose a_end and b_start, tangents from each side's last/first frame step (per frame)."""
    out = []
    for k in range(1, n + 1):
        t = k / (n + 1)
        pose = {}
        for name, (la, qa) in a_end.items():
            lb, qb = b_start[name]
            qap, qbn = a_prev[name][1].copy(), b_next[name][1].copy()
            for q in (qb, qap, qbn):
                if q.dot(qa) < 0:
                    q.negate()
            va = Vector(qa) - Vector(qap)
            vb = Vector(qbn) - Vector(qb)
            q = Quaternion(herm(Vector(qa), Vector(qb), va * (n + 1), vb * (n + 1), t)).normalized()
            ml = herm(la, lb, (la - a_prev[name][0]) * (n + 1), (b_next[name][0] - lb) * (n + 1), t)
            pose[name] = (ml, q)
        out.append(pose)
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--glb", required=True)
    p.add_argument("--pair", action="append", required=True)
    p.add_argument("--frames", type=int, default=4)
    p.add_argument("--out", required=True)
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    bpy.context.scene.render.fps = hero.OUT_FPS
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(a.glb))
    rig = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    rig.animation_data_create()
    rig.animation_data.use_nla = False
    for pb in rig.pose.bones:
        pb.rotation_mode = "QUATERNION"
    fps = hero.OUT_FPS
    made = []
    for spec in a.pair:
        parts = spec.split(":")
        A, B = bpy.data.actions[parts[0]], bpy.data.actions[parts[1]]
        a0, a1 = A.frame_range
        cut = a0 + float(parts[2]) * (a1 - a0) if len(parts) > 2 else a0 + 0.7 * (a1 - a0)
        b0, b1 = B.frame_range
        fa = [a0 + k for k in range(int(cut - a0) + 1)]
        fb = [b0 + k for k in range(int(b1 - b0) + 1)]
        pa, pb_ = sample(rig, A, fa), sample(rig, B, fb)
        # B continues from where A's root travel ended
        shift = pa[-1]["Motion"][0] - pb_[0]["Motion"][0] if "Motion" in pa[-1] else Vector()
        for pose in pb_:
            if "Motion" in pose:
                pose["Motion"] = (pose["Motion"][0] + shift, pose["Motion"][1])
        mid = bridge(pa[-1], pa[-2], pb_[0], pb_[1], a.frames)
        rig.animation_data.action = None
        naive = peak_accel(com_track(rig, pa[-3:] + pb_[:3]), fps)
        smooth = peak_accel(com_track(rig, pa[-3:] + mid + pb_[:3]), fps)
        name = f"combo_{A.name}__{B.name}"
        act = bpy.data.actions.new(name)
        rig.animation_data.action = act
        for i, pose in enumerate(pa + mid + pb_):
            for n, (loc, rot) in pose.items():
                bone = rig.pose.bones[n]
                bone.location, bone.rotation_quaternion = loc, rot
                bone.keyframe_insert("location", frame=i)
                bone.keyframe_insert("rotation_quaternion", frame=i)
        rig.animation_data.action = None
        made.append(name)
        print("BRIDGE", name, "frames", len(pa) + len(mid) + len(pb_), f"join COM accel: cut {naive / 9.81:.2f} g -> bridge {smooth / 9.81:.2f} g")
    for t in list(rig.animation_data.nla_tracks):
        rig.animation_data.nla_tracks.remove(t)
    for act in list(bpy.data.actions):
        tr = rig.animation_data.nla_tracks.new()
        tr.name = act.name
        st = tr.strips.new(act.name, int(act.frame_range[0]), act)
        if hasattr(st, "action_slot") and len(getattr(act, "slots", [])) > 0:
            st.action_slot = act.slots[0]
    hero.export_hero(rig, os.path.abspath(a.out), {"clips": [x.name for x in bpy.data.actions], "skipped": [], "bridges": made})


main()
