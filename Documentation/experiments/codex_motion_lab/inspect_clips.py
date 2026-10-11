import bpy, json, sys
from pathlib import Path

root = Path(sys.argv[sys.argv.index('--') + 1])
paths = [root/'Assets/FightingMotionsVolume1/FBX/nomove/hp_straight_right_A2.fbx',
         root/'Assets/Kevin Iglesias/Human Animations/Models/HumanM_Model.fbx']
paths += sorted((root/'Assets/Kevin Iglesias/Human Animations/Animations/Male').rglob('*MagicAttackDirect1H01_R*.fbx'))[:1]
paths += [root/'Assets/Brawler Animations Sample/Animations/Fight_Idle.fbx',
          root/'Assets/ExplosiveLLC/RPG Character Mecanim Animation Pack FREE/Animations/Unarmed/RPG-Character@Unarmed-Attack-L1.FBX']
report = []
for path in paths:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=str(path), automatic_bone_orientation=True)
    report.append({'path':str(path), 'armatures':[
        {'name':o.name, 'bones':list(o.data.bones.keys()), 'action':o.animation_data.action.name if o.animation_data and o.animation_data.action else None}
        for o in bpy.data.objects if o.type=='ARMATURE'],
        'meshes':[{ 'name':o.name, 'vertices':len(o.data.vertices)} for o in bpy.data.objects if o.type=='MESH'],
        'actions':[{'name':a.name,'frames':list(a.frame_range),'slots':len(a.slots)} for a in bpy.data.actions]})
(root/'Documentation/experiments/codex_motion_lab/clip_inventory.json').write_text(json.dumps(report,indent=2))
print('MOTION_INVENTORY', json.dumps(report))
