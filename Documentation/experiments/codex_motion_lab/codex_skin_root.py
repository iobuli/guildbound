"""Recover the original FBX's hips cluster before any mesh simplification."""
import bpy
from io_scene_fbx import parse_fbx
from mathutils import Vector

def restore_imported_hips(path, arm, body):
    tree,_=parse_fbx.parse(str(path))
    objects=next(e for e in tree.elems if e.id==b'Objects')
    links=next(e for e in tree.elems if e.id==b'Connections')
    models=[e for e in objects.elems if e.id==b'Model' and e.props[1].split(b'\x00')[0]==b'B-hips']
    assert len(models)==1
    cluster_ids={c.props[2] for c in links.elems if len(c.props)>2 and c.props[1]==models[0].props[0]}
    clusters=[e for e in objects.elems if e.id==b'Deformer' and e.props[0] in cluster_ids and e.props[-1]==b'Cluster']
    assert len(clusters)==1
    cluster=clusters[0]
    indices=next(x for x in cluster.elems if x.id==b'Indexes').props[0]
    weights=next(x for x in cluster.elems if x.id==b'Weights').props[0]
    geometry=next(e for e in objects.elems if e.id==b'Geometry' and e.props[-1]==b'Mesh')
    vertices=next(x for x in geometry.elems if x.id==b'Vertices').props[0]
    assert len(vertices)==len(body.data.vertices)*3,'Source control-point ordering changed'
    max_error=max((body.data.vertices[i].co-Vector(vertices[3*i:3*i+3])).length for i in range(len(body.data.vertices)))
    assert max_error<.0001,('Source index correspondence mismatch',max_error)
    before={b.name:(b.head_local.copy(),b.tail_local.copy()) for b in arm.data.bones}
    for o in bpy.context.scene.objects: o.select_set(o==arm)
    bpy.context.view_layer.objects.active=arm
    bpy.ops.object.mode_set(mode='EDIT')
    hips=arm.data.edit_bones.new('B-hips')
    hips.head=Vector((0,0,0)); hips.tail=arm.data.edit_bones['B-spine'].head.copy()
    if (hips.tail-hips.head).length<.01: hips.tail=Vector((0,.1,0))
    for b in arm.data.edit_bones:
        if b!=hips and b.parent is None: b.parent=hips
    bpy.ops.object.mode_set(mode='OBJECT')
    arm.name='FreeGuildRig'
    group=body.vertex_groups.new(name='B-hips')
    for index,weight in zip(indices,weights): group.add([index],weight,'REPLACE')
    bone_names=set(arm.data.bones.keys())
    names={g.index:g.name for g in body.vertex_groups}
    unbound=sum(not any(g.weight>0 and names[g.group] in bone_names for g in v.groups) for v in body.data.vertices)
    max_endpoint_delta=max(max((arm.data.bones[n].head_local-a).length,(arm.data.bones[n].tail_local-b).length) for n,(a,b) in before.items())
    assert unbound==0 and max_endpoint_delta<.000001,(unbound,max_endpoint_delta)
    return {'hips_source_control_points':len(indices),'source_index_max_error':max_error,
            'unbound_source_vertices_after':unbound,'original_bones':len(before),'derived_bones':len(arm.data.bones),
            'max_original_bone_endpoint_change_m':max_endpoint_delta,
            'method':'Original FBX B-hips cluster weights restored by verified control-point index before decimation.'}
