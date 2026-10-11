"""Codex-owned Blender experiment. Reads an owned FBX; writes derivatives outside Assets.
Run: blender -b --factory-startup -t 4 -P run_lab.py -- SOURCE REPORT_DIR LOCAL_DERIVATIVE_DIR
Manual poses and bounded shape samples are diagnostic, not Unity animation/contact proof.
"""
import bpy, sys, math, json, hashlib
from pathlib import Path
from collections import Counter
from mathutils import Vector
from mathutils.kdtree import KDTree

src, out, local = map(Path, sys.argv[sys.argv.index('--') + 1:])
out.mkdir(parents=True, exist_ok=True)
local.mkdir(parents=True, exist_ok=True)
source_hash = hashlib.sha256(src.read_bytes()).hexdigest()
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.fbx(filepath=str(src), automatic_bone_orientation=True)
arm = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
body = max((o for o in bpy.context.scene.objects if o.type == 'MESH'), key=lambda o: len(o.data.vertices))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'codex_motion_lab'))
from codex_skin_root import restore_imported_hips
skin_root = restore_imported_hips(src, arm, body)
original_vertices = len(body.data.vertices)
for o in bpy.context.scene.objects:
    o.select_set(o == body)
bpy.context.view_layer.objects.active = body
decimate = body.modifiers.new('Codex facet copy', 'DECIMATE')
decimate.ratio = 0.35
bpy.ops.object.modifier_apply(modifier=decimate.name)
mesh = body.data
for p in mesh.polygons:
    p.use_smooth = False
mw, inv = body.matrix_world.copy(), body.matrix_world.inverted()
base = [mw @ v.co for v in mesh.vertices]
normals = [(mw.to_3x3().inverted().transposed() @ v.normal).normalized() for v in mesh.vertices]
names = {g.index: g.name.lower() for g in body.vertex_groups}
protected = []
for v in mesh.vertices:
    protected.append(any(g.weight > 0.01 and any(k in names[g.group] for k in ('hand', 'finger', 'thumb', 'pinky', 'index', 'middle', 'ring', 'foot', 'toe', 'head', 'jaw')) for g in v.groups))

# Connected components are actual topology islands, not assumed coincident seams.
parent = list(range(len(base)))
def root(i):
    while parent[i] != i:
        parent[i] = parent[parent[i]]
        i = parent[i]
    return i
for edge in mesh.edges:
    a, b = edge.vertices
    parent[root(a)] = root(b)
component = [root(i) for i in range(len(base))]
counts = Counter(component)
coincident = Counter(tuple(round(c, 5) for c in p) for p in base)
# Nearby cross-island vertex pairs, <= 3cm in rest space. These are diagnostics,
# not an automatic weld map and not all are intended garment/body seams.
pairs_by_id = {}
for i, a in enumerate(base):
    if not (0.80 < a.z < 1.57):
        continue
    best = None
    for j, b in enumerate(base):
        if component[i] == component[j] or j == i:
            continue
        d = (a - b).length
        if d <= 0.03 and (best is None or d < best[0]):
            best = (d, j)
    if best:
        pairs_by_id[tuple(sorted((i, best[1])))] = best[0]
pairs = [(a,b,d) for (a,b),d in sorted(pairs_by_id.items())]
seam_indices = sorted(set(i for a,b,d in pairs for i in (a,b)))
seam_tree = KDTree(len(seam_indices))
for n,i in enumerate(seam_indices):
    seam_tree.insert(base[i], n)
seam_tree.balance()

# One spatial field in world metres. Canonical rig/limb lengths untouched.
# Joint endpoints and contact extremities are protected; gradients attenuate
# near shoulder/wrist/knee/ankle connections, rather than puffing each bone.
joints = []
for bone in arm.data.bones:
    if any(k in bone.name.lower() for k in ('upperarm', 'forearm', 'hand', 'shin', 'foot', 'neck')):
        joints.append(arm.matrix_world @ bone.head_local)
def gaussian(x, center, width):
    return math.exp(-((x - center) / width) ** 2)
def delta(p, kind, index):
    if protected[index]:
        return Vector()
    x, y, z = p
    joint_distance = min((p - q).length for q in joints)
    joint_mask = min(1.0, max(0.0, (joint_distance - 0.045) / 0.065))
    # No weld correspondence established: conservatively preserve the nearby
    # articulated interfaces, then fade the field over 4-8 cm around them.
    seam_distance = seam_tree.find(p)[2] if seam_indices else 1.0
    joint_mask *= min(1.0, max(0.0, (seam_distance - 0.04) / 0.04))
    torso = gaussian(z, 1.28, 0.20) * gaussian(x, 0, 0.25)
    lower = gaussian(z, 0.91, 0.22) * gaussian(x, 0, 0.25)
    thigh = gaussian(z, 0.62, 0.19) * gaussian(abs(x), 0.115, 0.12)
    arm_mass = gaussian(z, 1.42, 0.14) * gaussian(abs(x), 0.37, 0.20)
    if kind == 'fighter':
        sx, sy = 0.020 * torso + 0.016 * arm_mass, 0.014 * torso
    elif kind == 'caster':
        sx, sy = -0.012 * torso - 0.006 * thigh, -0.012 * torso
    else:
        sx, sy = 0.024 * lower + 0.018 * thigh + 0.009 * torso, 0.025 * lower + 0.012 * torso
    # Continuous sign function avoids discontinuity across the midline.
    return Vector((math.tanh(x / 0.08) * sx, math.tanh(y / 0.075) * sy, 0)) * joint_mask

body.shape_key_add(name='Basis')
for kind in ('fighter', 'caster', 'heavy'):
    key = body.shape_key_add(name='build_' + kind)
    for i, p in enumerate(base):
        key.data[i].co = inv @ (p + delta(p, kind, i))

# Actual outfit feasibility: a vest shell selected from the owned torso faces,
# not a new humanoid. Keep source topology/weights and carry the same keys.
selected = [p for p in mesh.polygons if 0.96 < (mw @ p.center).z < 1.42 and all(abs(base[i].x) < 0.245 for i in p.vertices)]
indices = sorted(set(i for p in selected for i in p.vertices))
mapping = {old: new for new, old in enumerate(indices)}
cloth_mesh = bpy.data.meshes.new('Derived torso shell')
cloth_mesh.from_pydata([inv @ (base[i] + normals[i] * 0.012) for i in indices], [], [[mapping[i] for i in p.vertices] for p in selected])
cloth_mesh.update()
cloth = bpy.data.objects.new('Codex vest shell proof', cloth_mesh)
bpy.context.collection.objects.link(cloth)
cloth.matrix_world = mw
for vg in body.vertex_groups:
    cloth.vertex_groups.new(name=vg.name)
for old, new in mapping.items():
    for g in mesh.vertices[old].groups:
        cloth.vertex_groups[g.group].add([new], g.weight, 'REPLACE')
cloth.modifiers.new('Owned skin binding', 'ARMATURE').object = arm
cloth.shape_key_add(name='Basis')
for kind in ('fighter', 'caster', 'heavy'):
    key = cloth.shape_key_add(name='build_' + kind)
    for old, new in mapping.items():
        key.data[new].co = inv @ (base[old] + normals[old] * 0.012 + delta(base[old], kind, old))

def material(name, color):
    m = bpy.data.materials.new(name)
    m.diffuse_color = (*color, 1)
    return m
skin = material('Skin', (0.70, 0.49, 0.33))
shirt = material('Bone shirt', (0.80, 0.75, 0.61))
trousers = material('Brown cloth', (0.20, 0.135, 0.08))
sash = material('Guild red', (0.40, 0.075, 0.055))
mesh.materials.clear()
for m in (skin, shirt, trousers, sash):
    mesh.materials.append(m)
for p in mesh.polygons:
    z = (mw @ p.center).z
    p.material_index = 2 if z < 0.89 else 3 if z < 0.97 else 1 if z < 1.48 else 0
cloth_mesh.materials.append(material('Derived vest', (0.24, 0.145, 0.08)))
for o in bpy.context.scene.objects:
    if o.type == 'MESH' and o not in (body, cloth):
        o.hide_render = True

poses = {
    'rest': {},
    'lowered_arms': {'B-upperArm.L': (.20, -.10, -1), 'B-upperArm.R': (-.20, -.10, -1)},
    'guard_diagnostic': {'B-upperArm.L': (.10, -.65, -.45), 'B-upperArm.R': (-.10, -.65, -.45), 'B-forearm.L': (.10, -.25, .80), 'B-forearm.R': (-.10, -.25, .80)},
    'cast_diagnostic': {'B-upperArm.L': (.15, -.75, -.30), 'B-upperArm.R': (-.15, -.75, -.30), 'B-forearm.L': (.05, -1, .05), 'B-forearm.R': (-.05, -1, .05)},
    'step_diagnostic': {'B-upperArm.L': (.20, -.10, -1), 'B-upperArm.R': (-.20, -.10, -1), 'B-thigh.L': (.08, -.40, -1), 'B-shin.L': (0, .40, -1)},
}
blends = {'base': (0, 0, 0), 'fighter': (1, 0, 0), 'caster': (0, 1, 0), 'heavy': (0, 0, 1), 'fighter_heavy': (.5, 0, .5), 'caster_heavy': (0, .5, .5), 'balanced': (1/3, 1/3, 1/3)}
def apply(pose, blend):
    for pb in arm.pose.bones:
        pb.rotation_mode = 'QUATERNION'
        pb.rotation_quaternion.identity()
    bpy.context.view_layer.update()
    for name, world_direction in poses[pose].items():
        pb = arm.pose.bones[name]
        rest_local = pb.parent.bone.matrix_local.inverted() @ pb.bone.matrix_local if pb.parent else pb.bone.matrix_local
        base_rotation = pb.parent.matrix.to_quaternion() @ rest_local.to_quaternion() if pb.parent else rest_local.to_quaternion()
        desired = (arm.matrix_world.to_3x3().inverted() @ Vector(world_direction)).normalized()
        global_rotation = (base_rotation @ Vector((0, 1, 0))).rotation_difference(desired) @ base_rotation
        pb.rotation_quaternion = base_rotation.inverted() @ global_rotation
        bpy.context.view_layer.update()
    for obj in (body, cloth):
        for key, value in zip(obj.data.shape_keys.key_blocks[1:], blends[blend]):
            key.value = value
    bpy.context.view_layer.update()
def evaluated(obj):
    dg = bpy.context.evaluated_depsgraph_get()
    ev = obj.evaluated_get(dg)
    me = ev.to_mesh()
    points = [obj.matrix_world @ v.co for v in me.vertices]
    ev.to_mesh_clear()
    return points
samples = []
for pose in poses:
    apply(pose, 'base')
    reference = evaluated(body)
    bone_reference = {b.name: (arm.matrix_world @ b.head, arm.matrix_world @ b.tail) for b in arm.pose.bones}
    for blend in blends:
        apply(pose, blend)
        points, shell = evaluated(body), evaluated(cloth)
        samples.append({'pose': pose, 'blend': blend,
            'max_surface_delta_m': max((a-b).length for a,b in zip(points, reference)),
            'protected_surface_delta_m': max((points[i]-reference[i]).length for i,p in enumerate(protected) if p),
            'bone_endpoint_delta_m': max(max((arm.matrix_world @ b.head-bone_reference[b.name][0]).length, (arm.matrix_world @ b.tail-bone_reference[b.name][1]).length) for b in arm.pose.bones),
            'near_island_pair_gap_increase_m': max([0.0] + [(points[i]-points[j]).length-(reference[i]-reference[j]).length for i,j,d in pairs]),
            'shell_corresponding_offset_min_m': min((shell[new]-points[old]).length for old,new in mapping.items()),
            'shell_corresponding_offset_max_m': max((shell[new]-points[old]).length for old,new in mapping.items()),
        })

scene = bpy.context.scene
scene.render.engine = 'BLENDER_WORKBENCH'
scene.display.shading.light = 'STUDIO'
scene.display.shading.color_type = 'MATERIAL'
scene.display.shading.show_cavity = True
scene.display.shading.background_type = 'WORLD'
scene.world = bpy.data.worlds.new('Proof backdrop')
scene.world.color = (0.20, 0.21, 0.23)
scene.render.resolution_x, scene.render.resolution_y = 700, 920
cam_data = bpy.data.cameras.new('Proof camera')
cam_data.type, cam_data.ortho_scale = 'ORTHO', 2.2
cam = bpy.data.objects.new('Proof camera', cam_data)
bpy.context.collection.objects.link(cam)
scene.camera = cam
target = Vector((0, 0, .93))
cam.location = (2.4, -6, 1.8)
cam.rotation_euler = (target-cam.location).to_track_quat('-Z','Y').to_euler()
for blend in ('base', 'fighter', 'caster', 'heavy'):
    apply('lowered_arms', blend)
    scene.render.filepath = str(out/(blend+'_vest.png'))
    bpy.ops.render.render(write_still=True)
for pose in ('guard_diagnostic', 'cast_diagnostic', 'step_diagnostic'):
    apply(pose, 'heavy')
    scene.render.filepath = str(out/(pose+'.png'))
    bpy.ops.render.render(write_still=True)

apply('rest', 'base')
bpy.ops.wm.save_as_mainfile(filepath=str(local/'codex_body_lab.blend'))
for o in bpy.context.scene.objects:
    o.select_set(o in (body, cloth, arm))
fbx = local/'codex_body_lab.fbx'
bpy.ops.export_scene.fbx(filepath=str(fbx), use_selection=True, add_leaf_bones=False, bake_anim=False, use_mesh_modifiers=False)
expected = {'bones': len(arm.data.bones), 'body_vertices': len(mesh.vertices), 'keys': [k.name for k in mesh.shape_keys.key_blocks]}
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.fbx(filepath=str(fbx), automatic_bone_orientation=True)
import_arm = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
import_meshes = [o for o in bpy.context.scene.objects if o.type == 'MESH']
import_body = max(import_meshes, key=lambda o:len(o.data.vertices))
actual = {'bones': len(import_arm.data.bones), 'body_vertices':len(import_body.data.vertices), 'keys':[k.name for k in import_body.data.shape_keys.key_blocks]}
report = {'author':'Codex', 'blender':bpy.app.version_string, 'source':str(src), 'source_sha256_before':source_hash, 'source_sha256_after':hashlib.sha256(src.read_bytes()).hexdigest(), 'skin_root_repair':skin_root,
    'original_vertices':original_vertices, 'faceted_vertices':expected['body_vertices'], 'bones':expected['bones'],
    'topology_components':len(counts), 'component_sizes':sorted(counts.values(),reverse=True), 'coincident_clusters':sum(n>1 for n in coincident.values()),
    'near_island_pairs':len(pairs), 'pair_records':[{'a':a,'b':b,'rest_distance_m':d} for a,b,d in pairs], 'pair_selection':'For every vertex at world z 0.80-1.57 m, take its nearest vertex of another island within 3 cm, canonicalise (min,max) and deduplicate. Includes asymmetric nearest relationships. Diagnostic only; not a weld/seam map. Interface vertices protected with 4-8 cm attenuation.',
    'vest_vertices':len(indices), 'vest_faces':len(selected), 'samples':samples, 'roundtrip_expected':expected, 'roundtrip_actual':actual,
    'local_derivatives':str(local), 'limits':['Manual diagnostic poses, not owned clip playback.', 'No Unity import, retargeting, gameplay contact, grounding or frame-rate pass.', 'Corresponding shell offsets do not prove no triangle/body intersections.', 'Near-island metric does not repair pre-existing articulated segment gaps.', 'No independent reviewer acceptance yet.']}
(out/'results.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print('CODEX_RESULT',json.dumps({k:v for k,v in report.items() if k not in ('samples','component_sizes')}))
