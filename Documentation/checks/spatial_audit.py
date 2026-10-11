"""Spatial audit of the Codex motion-lab retarget (read-only: never modifies the lab or any asset).
blender -b --factory-startup -P spatial_audit.py -- <project_root> <lab_body_blend> <out_dir>
Replays the lab's world-rotation retarget on every source frame and compares SOURCE rig vs RETARGETED body in
spine-length units (rig scale cancels). Measures: floor clearance/penetration, stance-foot skate, hip height,
hand reach (forward, up, distance from shoulder), hand-to-torso proximity, knee direction/angle, root drift/yaw."""
import bpy, sys, json, math
from pathlib import Path
from mathutils import Vector, Matrix

root, lab, out = map(Path, [a for a in sys.argv[sys.argv.index('--') + 1:] if not a.startswith('--')])
out.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.open_mainfile(filepath=str(lab))
scene = bpy.context.scene
target = next(o for o in scene.objects if o.type == 'ARMATURE')
target.animation_data_clear()
target_base = target.matrix_world.copy()
target_rest = {b.name: (target_base @ b.matrix_local).to_quaternion() for b in target.data.bones}
fight = root / 'Assets/FightingMotionsVolume1/FBX'
spell = root / 'Assets/Kevin Iglesias/Human Animations/Animations/Male/Combat/Spellcasting'
direction = spell / 'MagicAttacks/Directional'
clips = {'stance': fight / 'idle_A.fbx', 'light_left': fight / 'hp_straight_left_A.fbx',
         'light_right': fight / 'nomove/hp_straight_right_A2.fbx', 'heavy': fight / 'nomove/bk_rh_right_A2.fbx',
         'guard': fight / 'hb_front_A.fbx', 'hit': fight / 'hd_front_A.fbx', 'get_up': fight / 'getup_A.fbx',
         'plain_fire': direction / 'HumanM@MagicAttackDirect1H01_R.fbx',
         'charge': direction / 'HumanM@MagicAttackDirect2H01 - Load.fbx',
         'release': direction / 'HumanM@MagicAttackDirect2H01 - Cast.fbx',
         'cast_interrupt': spell / 'HumanM@CastingDamage01.fbx'}
mapping = {'B-spine': 'Body', 'B-chest': 'Chest', 'B-neck': 'Neck', 'B-head': 'Head'}
for side, suffix in [('L', '.L'), ('R', '.R')]:
    for dest, src in [('shoulder', 'Collar'), ('upperArm', 'Shoulder'), ('forearm', 'ForeArm'), ('hand', 'Hand'),
                      ('thigh', 'Thigh'), ('shin', 'Shin'), ('foot', 'Foot'), ('toe', 'Toe')]:
        mapping['B-' + dest + suffix] = src + '_' + side

# logical joint -> (target bone, FM source bone, same-rig source bone)
J = {'hips': ('B-hips', 'Hip', 'B-hips'), 'spine': ('B-spine', 'Body', 'B-spine'), 'chest': ('B-chest', 'Chest', 'B-chest'),
     'head': ('B-head', 'Head', 'B-head')}
for s, sfx in (('L', '.L'), ('R', '.R')):
    J['up' + s] = ('B-upperArm' + sfx, 'Shoulder_' + s, 'B-upperArm' + sfx)
    J['fore' + s] = ('B-forearm' + sfx, 'ForeArm_' + s, 'B-forearm' + sfx)
    J['hand' + s] = ('B-hand' + sfx, 'Hand_' + s, 'B-hand' + sfx)
    J['thigh' + s] = ('B-thigh' + sfx, 'Thigh_' + s, 'B-thigh' + sfx)
    J['shin' + s] = ('B-shin' + sfx, 'Shin_' + s, 'B-shin' + sfx)
    J['foot' + s] = ('B-foot' + sfx, 'Foot_' + s, 'B-foot' + sfx)
    J['toe' + s] = ('B-toe' + sfx, 'Toe_' + s, 'B-toe' + sfx)
UP = Vector((0, 0, 1))
AUTO_ALIGN = '--no-align' not in sys.argv
RENDER = '--render' in sys.argv
if RENDER:
    scene.render.engine = 'BLENDER_WORKBENCH'
    scene.render.resolution_x, scene.render.resolution_y = 400, 500
    scene.display.shading.light = 'STUDIO'
    cam_data = bpy.data.cameras.new('audit_cam'); cam_data.type = 'ORTHO'; cam_data.ortho_scale = 2.4
    cam = bpy.data.objects.new('audit_cam', cam_data); scene.collection.objects.link(cam)
    cam.rotation_euler = (math.radians(90), 0, math.radians(90)); scene.camera = cam


def analyse(pts, units):
    """pts: list of {joint: Vector(world)}; units: list of spine length per frame. Returns metrics in spine units."""
    n = len(pts)
    P = [{k: v / units[i] for k, v in pts[i].items()} for i in range(n)]
    floor0 = min(P[0][k].z for k in ('footL', 'footR', 'toeL', 'toeR'))
    clear = [min(p[k].z for k in ('footL', 'footR', 'toeL', 'toeR')) - floor0 for p in P]
    # stance foot skate: lowest foot point per frame; planted if within 0.08 spine units of floor
    skate, max_speed = 0.0, 0.0
    for i in range(1, n):
        for k in ('toeL', 'toeR', 'footL', 'footR'):
            a, b = P[i - 1][k], P[i][k]
            if a.z - floor0 < 0.08 and b.z - floor0 < 0.08:
                d = math.hypot(b.x - a.x, b.y - a.y)
                skate += d / 4.0
                max_speed = max(max_speed, d)
    hip_h = [p['hips'].z - floor0 for p in P]
    res = {'frames': n, 'floor_clearance_min': min(clear), 'floor_clearance_max': max(clear),
           'hip_height_min': min(hip_h), 'hip_height_max': max(hip_h), 'stance_skate_total': skate,
           'stance_skate_max_step': max_speed}
    for s in ('L', 'R'):
        reach, fwd, up, tors, knee_back, knee_ang = [], [], [], [], 0, []
        for p in P:
            right = Vector((p['thighR'].x - p['thighL'].x, p['thighR'].y - p['thighL'].y, 0))
            right = right.normalized() if right.length > 1e-6 else Vector((1, 0, 0))
            forward = UP.cross(right).normalized()
            h = p['hand' + s]
            reach.append((h - p['up' + s]).length)
            fwd.append((h - p['hips']).dot(forward))
            up.append(h.z - p['hips'].z)
            # distance of the hand to the torso axis (spine -> chest -> head polyline)
            best = 1e9
            for a, b in (('spine', 'chest'), ('chest', 'head')):
                A, B = p[a], p[b]
                ab = B - A
                t = max(0.0, min(1.0, (h - A).dot(ab) / max(ab.length_squared, 1e-9)))
                best = min(best, (h - (A + ab * t)).length)
            tors.append(best)
            th, sh, ft, to = p['thigh' + s], p['shin' + s], p['foot' + s], p['toe' + s]
            v1, v2 = (th - sh), (ft - sh)
            knee_ang.append(math.degrees(v1.angle(v2)) if v1.length > 1e-6 and v2.length > 1e-6 else 180.0)
            line = ft - th
            t = max(0.0, min(1.0, (sh - th).dot(line) / max(line.length_squared, 1e-9)))
            off = sh - (th + line * t)
            fwd_foot = Vector((to.x - ft.x, to.y - ft.y, 0))
            if fwd_foot.length > 1e-6 and Vector((off.x, off.y, 0)).dot(fwd_foot.normalized()) < -0.05:
                knee_back += 1
        res['hand' + s] = {'reach_from_shoulder_max': max(reach), 'forward_max': max(fwd), 'forward_min': min(fwd),
                           'up_max': max(up), 'up_min': min(up), 'torso_distance_min': min(tors)}
        res['knee' + s] = {'angle_min_deg': min(knee_ang), 'angle_max_deg': max(knee_ang), 'frames_bending_backwards': knee_back}
    ang = {}
    for s_ in ('L', 'R'):
        el, up_el, th_el = [], [], []
        for p in P:
            u, f_, h_ = p['up' + s_], p['fore' + s_], p['hand' + s_]
            v1, v2 = (u - f_), (h_ - f_)
            el.append(math.degrees(v1.angle(v2)) if v1.length > 1e-6 and v2.length > 1e-6 else 180.0)
            d = (f_ - u)
            up_el.append(math.degrees(d.angle(Vector((0, 0, -1)))) if d.length > 1e-6 else 0.0)  # 0 = arm hangs down
            t = (p['shin' + s_] - p['thigh' + s_])
            th_el.append(math.degrees(t.angle(Vector((0, 0, -1)))) if t.length > 1e-6 else 0.0)
        ang['elbow' + s_], ang['upper_arm_from_down' + s_], ang['thigh_from_down' + s_] = el, up_el, th_el
    res['angles'] = ang
    h0, h1 = P[0]['hips'], P[-1]['hips']
    res['root_drift_xy'] = math.hypot(h1.x - h0.x, h1.y - h0.y)
    return res


report = {}
for label, path in clips.items():
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=str(path), automatic_bone_orientation=True)
    added = set(bpy.data.objects) - before
    source = next(o for o in added if o.type == 'ARMATURE')
    action = source.animation_data.action
    lo, hi = action.frame_range
    same = 'B-spine' in source.pose.bones
    bone_map = {n: n for n in target.pose.bones.keys() if n in source.pose.bones} if same else mapping
    hip = source.pose.bones.get('B-hips') if same else source.pose.bones['Hip']
    scene.frame_set(math.floor(lo), subframe=lo - math.floor(lo))
    bpy.context.view_layer.update()
    # FBX axis conversion: some rigs (Kevin Iglesias) come in lying down after the B-root correction. Detect it from
    # hips->head at the first frame and stand the whole source frame up with a rigid alignment (identity if upright).
    align = Matrix.Identity(4)
    if same and 'B-root' in source.pose.bones and AUTO_ALIGN:
        db = source.data.bones  # rest pose, not the first animated frame: a leaning first frame must not skew the alignment
        up0 = source.matrix_world @ db['B-head'].head_local - source.matrix_world @ db['B-hips'].head_local
        if up0.length > 1e-6 and up0.normalized().z < 0.7:
            align = up0.normalized().rotation_difference(UP).to_matrix().to_4x4()
    align_deg = math.degrees(align.to_quaternion().angle)
    source_rest = {b.name: (align @ source.matrix_world @ b.matrix_local).to_quaternion() for b in source.data.bones}
    hip_rest = (align @ source.matrix_world @ hip.bone.matrix_local).to_quaternion()
    hip_origin = align @ source.matrix_world @ hip.bone.head_local
    col = 2 if same else 1
    spts, tpts, su, tu = [], [], [], []
    N = int(round(hi - lo)) + 1
    for i in range(N):
        frame = lo + (hi - lo) * i / max(N - 1, 1)
        scene.frame_set(math.floor(frame), subframe=frame - math.floor(frame))
        bpy.context.view_layer.update()
        corr = Matrix.Identity(4)
        if same and 'B-root' in source.pose.bones:
            c = source.pose.bones['B-root']
            delta = (source.matrix_world @ c.matrix) @ (source.matrix_world @ c.bone.matrix_local).inverted()
            corr = delta.inverted()
        corr = align @ corr
        hip_cur = (corr @ source.matrix_world @ hip.matrix).to_quaternion()
        hip_delta = hip_cur @ hip_rest.inverted()
        ref = source.pose.bones['B-spine' if same else 'Body']
        rest_len = (source.matrix_world.to_3x3() @ (ref.bone.tail_local - ref.bone.head_local)).length
        posed_len = ((corr @ source.matrix_world).to_3x3() @ (ref.tail - ref.head)).length
        uc = posed_len / rest_len
        translation = (corr @ source.matrix_world @ hip.head - align @ source.matrix_world.translation) / uc + align @ source.matrix_world.translation - hip_origin
        target.matrix_world = Matrix.LocRotScale(target_base.translation + translation, hip_delta @ target_base.to_quaternion(), target_base.to_scale())
        for pb in target.pose.bones:
            pb.rotation_mode = 'QUATERNION'; pb.rotation_quaternion.identity()
        bpy.context.view_layer.update()
        for pb in target.pose.bones:
            if pb.name not in bone_map: continue
            sb = source.pose.bones[bone_map[pb.name]]
            cur = (corr @ source.matrix_world @ sb.matrix).to_quaternion()
            desired = cur @ source_rest[sb.name].inverted() @ target_rest[pb.name]
            desired_arm = target.matrix_world.to_quaternion().inverted() @ desired
            rest_local = pb.parent.bone.matrix_local.inverted() @ pb.bone.matrix_local if pb.parent else pb.bone.matrix_local
            parent_rot = pb.parent.matrix.to_quaternion() if pb.parent else Matrix.Identity(4).to_quaternion()
            pb.rotation_quaternion = rest_local.to_quaternion().inverted() @ parent_rot.inverted() @ desired_arm
            bpy.context.view_layer.update()
        if RENDER and i in (N // 4, N // 2):
            hipw = target.matrix_world.translation
            cam.location = (hipw.x + 4.0, hipw.y, 1.0)
            scene.render.filepath = str(out / ('render_%s_%s_%02d.png' % (label, 'lab' if not AUTO_ALIGN else 'aligned', i)))
            bpy.ops.render.render(write_still=True)
        sp, tp = {}, {}
        for k, names in J.items():
            sb = source.pose.bones[names[col]]
            sp[k] = (corr @ source.matrix_world) @ sb.head
            if names[0] in target.pose.bones:
                tp[k] = target.matrix_world @ target.pose.bones[names[0]].head
            else:  # the body's hips are the armature object itself (the lab drives it as the root)
                tp[k] = target.matrix_world.translation.copy()
        spts.append(sp); tpts.append(tp)
        su.append(((sp['chest'] - sp['spine']).length) or 1e-6)
        tu.append(((tp['chest'] - tp['spine']).length) or 1e-6)
    report[label] = {'source': analyse(spts, su), 'target': analyse(tpts, tu),
                     'spine_len_source_m_mean': sum(su) / len(su), 'align_deg': align_deg, 'spine_len_target_m': tu[0],
                     'target_foot_floor_abs_m': min(tpts[0][k].z for k in ('footL', 'footR', 'toeL', 'toeR'))}
    for o in added: bpy.data.objects.remove(o, do_unlink=True)
json.dump(report, open(out / ('spatial_audit_noalign.json' if not AUTO_ALIGN else 'spatial_audit.json'), 'w', encoding='utf-8'), indent=1)
print('SPATIAL_AUDIT_DONE', len(report))
