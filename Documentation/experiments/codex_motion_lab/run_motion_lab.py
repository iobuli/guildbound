"""Owned FBX key-frame sampling and a proposed pose-continuous light phrase.
Blender-only retarget study, NOT Unity Humanoid/contact/support acceptance.
Run blender -b --factory-startup -t 4 -P THIS -- PROJECT LOCAL_BODY_BLEND.
"""
import bpy, sys, json, math, hashlib
from pathlib import Path
from mathutils import Vector, Matrix

root, lab = map(Path, sys.argv[sys.argv.index('--')+1:])
out = root/'Documentation/experiments/codex_motion_lab'
out.mkdir(parents=True, exist_ok=True)
bpy.ops.wm.open_mainfile(filepath=str(lab))
scene = bpy.context.scene
target = next(o for o in scene.objects if o.type=='ARMATURE')
target.animation_data_clear()
target_base = target.matrix_world.copy()
target_rest = {b.name:(target_base @ b.matrix_local).to_quaternion() for b in target.data.bones}
for o in scene.objects:
    if o.type=='MESH' and o.data.shape_keys:
        for key in o.data.shape_keys.key_blocks[1:]: key.value = 0
        # The derived vest in the earlier static lab had no armature parent.
        # Preserve its bind-world matrix while making root motion follow the body.
        if o.parent is None and any(m.type=='ARMATURE' and m.object==target for m in o.modifiers):
            bind_world=o.matrix_world.copy()
            o.parent=target
            o.matrix_parent_inverse=target_base.inverted()
            o.matrix_world=bind_world
scene.render.resolution_x, scene.render.resolution_y = 400, 500
scene.render.resolution_percentage = 100
fight = root/'Assets/FightingMotionsVolume1/FBX'
spell = root/'Assets/Kevin Iglesias/Human Animations/Animations/Male/Combat/Spellcasting'
direction = spell/'MagicAttacks/Directional'
clips = {'stance':fight/'idle_A.fbx', 'light_left':fight/'hp_straight_left_A.fbx',
         'light_right':fight/'nomove/hp_straight_right_A2.fbx',
         'heavy':fight/'nomove/bk_rh_right_A2.fbx', 'guard':fight/'hb_front_A.fbx',
         'hit':fight/'hd_front_A.fbx', 'get_up':fight/'getup_A.fbx',
         'plain_fire':direction/'HumanM@MagicAttackDirect1H01_R.fbx',
         'charge':direction/'HumanM@MagicAttackDirect2H01 - Load.fbx',
         'release':direction/'HumanM@MagicAttackDirect2H01 - Cast.fbx',
         'cast_interrupt':spell/'HumanM@CastingDamage01.fbx'}
brawler=root/'Assets/Brawler Animations Sample/Animations'
clips.update({label:brawler/(filename+'.fbx') for label,filename in [
    ('brawler_stance','Fight_Idle'),('brawler_light1','Atk_P_1'),('brawler_light2','Atk_P_2'),
    ('brawler_heavy','Atk_K_1'),('brawler_guard','Guard_Idle'),('brawler_step','Quickstep_B'),
    ('throw_reach','Grab_Start'),('throw_actor','Grab_Throw_A'),('throw_receiver','Grab_Throw_B')]})
mapping = {'B-spine':'Body', 'B-chest':'Chest', 'B-neck':'Neck', 'B-head':'Head'}
for side, suffix in [('L','.L'),('R','.R')]:
    for dest,src in [('shoulder','Collar'),('upperArm','Shoulder'),('forearm','ForeArm'),
                     ('hand','Hand'),('thigh','Thigh'),('shin','Shin'),('foot','Foot'),('toe','Toe')]:
        mapping['B-'+dest+suffix] = src+'_'+side

def capture_pose():
    return {'root':target.matrix_world.copy(), 'bones':{b.name:b.rotation_quaternion.copy() for b in target.pose.bones}}

def apply_pose(pose):
    target.matrix_world = pose['root']
    for pb in target.pose.bones:
        pb.rotation_mode='QUATERNION'
        pb.rotation_quaternion = pose['bones'][pb.name]
    bpy.context.view_layer.update()

def point(name):
    return target.matrix_world @ target.pose.bones[name].head

records, bank = [], {}
for label,path in clips.items():
    assert path.is_file(), str(path)
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=str(path), automatic_bone_orientation=True)
    added = set(bpy.data.objects)-before
    source = next(o for o in added if o.type=='ARMATURE')
    action = source.animation_data.action
    lo,hi = action.frame_range
    source_rest = {b.name:(source.matrix_world @ b.matrix_local).to_quaternion() for b in source.data.bones}
    same_names = 'B-spine' in source.pose.bones
    brawler_names='Left_UpperArm' in source.pose.bones
    bone_map = {name:name for name in target.pose.bones.keys() if name in source.pose.bones} if same_names else dict(mapping)
    if brawler_names:
        bone_map={'B-spine':'Spine','B-chest':'Chest','B-neck':'Neck','B-head':'Head'}
        for side,suffix in [('Left','.L'),('Right','.R')]:
            for dest,src in [('shoulder','Shoulder'),('upperArm','UpperArm'),('forearm','LowerArm'),
                             ('hand','Hand'),('thigh','UpperLeg'),('shin','LowerLeg'),('foot','Foot'),('toe','Toes')]:
                bone_map['B-'+dest+suffix]=side+'_'+src
            for dest,src in [('indexFinger','Index'),('middleFinger','Middle'),('ringFinger','Ring'),('pinky','Pinky'),('thumb','Thumb')]:
                for number,part in [('01','Proximal'),('02','Intermediate'),('03','Distal')]:
                    bone_map['B-'+dest+number+suffix]=side+'_'+src+part
    hip = source.pose.bones['B-hips' if same_names else 'Hips' if brawler_names else 'Hip']
    hip_rest = (source.matrix_world @ hip.bone.matrix_local).to_quaternion()
    hip_origin = source.matrix_world @ hip.bone.head_local
    alignment=Matrix.Identity(4).to_quaternion()
    if brawler_names:
        sf=(source.matrix_world @ source.data.bones['Left_Toes'].head_local)-(source.matrix_world @ source.data.bones['Left_Foot'].head_local)
        tf=(target_base @ target.data.bones['B-toe.L'].head_local)-(target_base @ target.data.bones['B-foot.L'].head_local)
        sf.z=tf.z=0
        alignment=sf.normalized().rotation_difference(tf.normalized())
    for o in added: o.hide_render=True
    scene.frame_set(math.floor(lo), subframe=lo-math.floor(lo))
    bpy.context.view_layer.update()
    basis_align=Matrix.Identity(4)
    if same_names and 'B-root' in source.pose.bones:
        up=(source.matrix_world @ source.data.bones['B-head'].head_local)-(source.matrix_world @ source.data.bones['B-hips'].head_local)
        basis_align=up.normalized().rotation_difference(Vector((0,0,1))).to_matrix().to_4x4()
    # Capture evaluated authored object transforms. FBX object channels may
    # differ from the importer's unevaluated rest transform (notably scale).
    source_rest = {b.name:(basis_align @ source.matrix_world @ b.matrix_local).to_quaternion() for b in source.data.bones}
    hip_rest = (basis_align @ source.matrix_world @ hip.bone.matrix_local).to_quaternion()
    hip_origin = basis_align @ source.matrix_world @ hip.bone.head_local
    keyframes = sorted(set(float(k.co.x) for layer in action.layers for strip in layer.strips
                           for bag in strip.channelbags for curve in bag.fcurves for k in curve.keyframe_points))
    poses, points, pose_by_frame, anatomical_samples = [], [], {}, []
    key_samples=[min(keyframes,key=lambda x:abs(x-(lo+(hi-lo)*fraction))) for fraction in [0,.25,.5,.75,1]]
    uniform_frames=[lo+(hi-lo)*i/60 for i in range(61)]
    # Dense sampling evaluates the imported authored curves, not hand-authored poses.
    for i,frame in enumerate(sorted(set(uniform_frames+key_samples))):
        scene.frame_set(math.floor(frame), subframe=frame-math.floor(frame))
        bpy.context.view_layer.update()
        hip_current = (source.matrix_world @ hip.matrix).to_quaternion()
        # B-root carries FBX axis/scale conversion channels. Remove that
        # animated container basis before comparing anatomical rest/pose.
        root_correction = Matrix.Identity(4)
        if same_names and 'B-root' in source.pose.bones:
            container=source.pose.bones['B-root']
            container_delta=(source.matrix_world @ container.matrix) @ (source.matrix_world @ container.bone.matrix_local).inverted()
            root_correction=container_delta.inverted()
        root_correction=basis_align @ root_correction
        hip_current = (root_correction @ source.matrix_world @ hip.matrix).to_quaternion()
        hip_delta = hip_current @ hip_rest.inverted()
        if brawler_names: hip_delta=alignment @ hip_delta @ alignment.inverted()
        # Keep canonical target lengths; root rotation comes from the authored hips.
        # Some FBX files animate a 100x root scale on top of a .01 object
        # scale. Normalize positional motion by the evaluated/rest spine
        # length ratio; copy rotations only, never the animated bone scales.
        reference_bone = source.pose.bones['B-spine' if same_names else 'Spine' if brawler_names else 'Body']
        rest_length = (source.matrix_world.to_3x3() @ (reference_bone.bone.tail_local-reference_bone.bone.head_local)).length
        posed_length = ((root_correction @ source.matrix_world).to_3x3() @ (reference_bone.tail-reference_bone.head)).length
        unit_correction = posed_length/rest_length
        object_origin=basis_align @ source.matrix_world.translation
        translation = (root_correction @ source.matrix_world @ hip.head-object_origin)/unit_correction + object_origin - hip_origin
        if brawler_names: translation=alignment @ translation
        target.matrix_world = Matrix.LocRotScale(target_base.translation+translation,
                                               hip_delta @ target_base.to_quaternion(),
                                               target_base.to_scale())
        for pb in target.pose.bones:
            pb.rotation_mode='QUATERNION'; pb.rotation_quaternion.identity()
        bpy.context.view_layer.update()
        for pb in target.pose.bones:
            if pb.name not in bone_map: continue
            sb = source.pose.bones[bone_map[pb.name]]
            current = (root_correction @ source.matrix_world @ sb.matrix).to_quaternion()
            desired_world = current @ source_rest[sb.name].inverted() @ target_rest[pb.name]
            if brawler_names:
                # This pack's bind pose is not the target's T pose. A rest-delta
                # transfer incorrectly turned its home guard into a T pose.
                # Match authored world-space bone directions, keeping target
                # lengths; twist transfer remains a Unity review requirement.
                source_direction=alignment @ ((root_correction @ source.matrix_world).to_3x3() @ (sb.tail-sb.head)).normalized()
                target_direction=target_rest[pb.name] @ Vector((0,1,0))
                desired_world=target_direction.rotation_difference(source_direction) @ target_rest[pb.name]
            desired_arm = target.matrix_world.to_quaternion().inverted() @ desired_world
            rest_local = pb.parent.bone.matrix_local.inverted() @ pb.bone.matrix_local if pb.parent else pb.bone.matrix_local
            parent_rot = pb.parent.matrix.to_quaternion() if pb.parent else Matrix.Identity(4).to_quaternion()
            pb.rotation_quaternion = rest_local.to_quaternion().inverted() @ parent_rot.inverted() @ desired_arm
            bpy.context.view_layer.update()
        if i==0:
            print('DEBUG_TRANSFORM',label,'position_unit_correction',unit_correction,'hip',list(translation))
        poses.append(capture_pose())
        pose_by_frame[frame]=poses[-1]
        points.append({name:list(point(name)) for name in ['B-hand.L','B-hand.R','B-foot.L','B-foot.R']})
        anatomical_samples.append({'source_frame':frame,
            'source':{n:list(root_correction @ source.matrix_world @ source.pose.bones[bone_map[n]].head) for n in mapping},
            'target':{n:list(point(n)) for n in mapping},
            'source_hips':list(root_correction @ source.matrix_world @ hip.head),
            'target_hips':list(target.matrix_world.translation)})
    bank[label]=[pose_by_frame[f] for f in uniform_frames]
    renders=[]
    for sample,frame in zip([0,15,30,45,60],key_samples):
        apply_pose(pose_by_frame[frame])
        filename=f'{label}_{sample:02d}.png'
        scene.render.filepath=str(out/filename)
        bpy.ops.render.render(write_still=True)
        renders.append({'file':filename,'normalized_time':(frame-lo)/(hi-lo),'source_frame':frame,'authored_key_timestamp':True})
    records.append({'label':label,'source':str(path.relative_to(root)), 'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),
                    'action':action.name,'source_frame_range':[lo,hi], 'source_key_frames':keyframes,
                    'source_fps':scene.render.fps/scene.render.fps_base, 'mapped_bones':bone_map,
                    'renders':renders,'limb_samples':points,'anatomical_samples':anatomical_samples,
                    'source_basis_alignment_degrees':math.degrees(basis_align.to_quaternion().angle)})
    for o in added: bpy.data.objects.remove(o,do_unlink=True)

def interpolate(a,b,t):
    # Cubic ease gives zero blend-weight slope at both handovers. Simulation is not retimed here.
    w=t*t*(3-2*t)
    al,ar,asc=a['root'].decompose(); bl,br,bsc=b['root'].decompose()
    return {'root':Matrix.LocRotScale(al.lerp(bl,w),ar.slerp(br,w),asc.lerp(bsc,w)),
            'bones':{n:a['bones'][n].slerp(b['bones'][n],w) for n in a['bones']}}

# Review only: full authored clips, then a separately visible 120ms blend.
# This avoids hiding a bad handover by deleting the strike/recovery frames.
timeline=[]
review_fps=30
for index,label in enumerate(['brawler_light1','brawler_light2','brawler_stance']):
    if timeline:
        previous=timeline[-1]['pose']; following=bank[label][0]
        for i in range(1,5):
            timeline.append({'stage':'handover to '+label,'pose':interpolate(previous,following,i/4)})
    record=next(x for x in records if x['label']==label)
    duration=(record['source_frame_range'][1]-record['source_frame_range'][0])/record['source_fps']
    count=max(2,round(duration*review_fps))
    for j in range(count+1):
        p=j/count*60; low=math.floor(p); high=min(60,low+1)
        # Linear sample interpolation preserves uniform review time; handovers
        # above use their own cubic weight, not a retime of the source motion.
        a,b=bank[label][low],bank[label][high]; t=p-low
        al,ar,asc=a['root'].decompose(); bl,br,bsc=b['root'].decompose()
        pose={'root':Matrix.LocRotScale(al.lerp(bl,t),ar.slerp(br,t),asc.lerp(bsc,t)),
              'bones':{n:a['bones'][n].slerp(b['bones'][n],t) for n in a['bones']}}
        timeline.append({'stage':label,'pose':pose})
transition=[]
for a,b in [('light_left','light_right'),('light_right','guard'),('charge','release'),
            ('brawler_light1','brawler_light2'),('brawler_light2','brawler_stance'),('brawler_light2','brawler_guard')]:
    first,last=bank[a][-1],bank[b][0]
    angle=max(2*math.acos(min(1.0,abs(first['bones'][n].normalized().dot(last['bones'][n].normalized())))) for n in mapping)
    apply_pose(first); from_points={n:point(n) for n in ['B-hand.L','B-hand.R','B-foot.L','B-foot.R']}
    apply_pose(last)
    transition.append({'from':a,'to':b,'raw_endpoint_max_angle_deg':math.degrees(angle),
                       'raw_endpoint_max_limb_jump_m':max((point(n)-v).length for n,v in from_points.items()),
                       'proposed_blend_seconds':0.12,'review_blend_seconds':4/review_fps,
                       'status':'raw authored endpoints measured; blend alone does not prove planted feet or combat timing'})
for i,entry in enumerate(timeline):
    scene.frame_set(i+1)
    apply_pose(entry['pose'])
    scene.render.filepath=str(out/f'combo_{i:03d}.png')
    bpy.ops.render.render(write_still=True)
    # Bake a review action, preserving this body and authored pose provenance locally.
    target.keyframe_insert(data_path='location')
    target.keyframe_insert(data_path='rotation_quaternion' if target.rotation_mode=='QUATERNION' else 'rotation_euler')
    for pb in target.pose.bones: pb.keyframe_insert(data_path='rotation_quaternion')
for layer in target.animation_data.action.layers:
    for strip in layer.strips:
        for bag in strip.channelbags:
            for curve in bag.fcurves:
                for key in curve.keyframe_points: key.interpolation='LINEAR'
scene.render.fps=review_fps
scene.frame_start=1; scene.frame_end=len(timeline)
scene.frame_set(1)
bpy.ops.wm.save_as_mainfile(filepath=str(lab.parent/'codex_motion_review.blend'))
report={'clips':records, 'transitions':transition, 'review_fps':review_fps,
        'combo_frames':[{'file':f'combo_{i:03d}.png','stage':x['stage']} for i,x in enumerate(timeline)],
        'limits':['Blender world-rotation retarget, not Unity Humanoid proof.',
                  'Brawler uses absolute bone-direction retarget due to a different bind pose; axial twist requires Unity verification.',
                  'No support IK, collision motor, damage, invulnerability or healing simulation.',
                  'Captures evaluate the nearest actual authored key timestamp to each quarter interval; dense samples also evaluate in-between curves.',
                  '12mm vest/contact-envelope and full-volume garment gates are not certified by these captures.',
                  'Combo montage preserves full source clips; gameplay timing and late-recovery handovers still need curation.']}
(out/'results.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print('MOTION_RESULT',json.dumps({'clips':len(records),'frames':len(timeline),'transitions':transition}))
