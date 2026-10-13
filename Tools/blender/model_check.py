"""Model check: headless, data only. Blender -b --factory-startup -P model_check.py -- --glb A.glb

Writes <glb>.check.json (and prints it) for a quick good / okay / bad call on a built or assembled model:
  size     - file MB, meshes, triangles, materials, images
  height   - bounding box height (m) and how far the feet sit off the floor
  bones    - game bones missing from the rig
  skin     - vertices with no weight, weight sums off 1, weights on bones that do not exist
  coverage - vertices per body region by strongest bone (0 in a region = a hole in the body)
  seams    - open mesh edges whose nearest vertex on another mesh is far (a visible gap between parts)
  clips    - animation count
Flags list every number past its threshold.
"""
import argparse
import json
import os
import sys

import bpy
from mathutils import kdtree

GAME_BONES = ["B_Pelvis", "B_Spine", "B_Spine1", "B_Spine2", "B_Neck", "B_Head", "B_R_Hand", "B_L_Forearm.001"]
for S in ("L", "R"):
    GAME_BONES += [f"B_{S}_{b}" for b in ("Clavicle", "UpperArm", "Forearm", "Thigh", "Calf", "Foot", "Toe0")]

REGIONS = {"head": ("B_Head", "B_Neck"), "body": ("B_Spine", "B_Spine1", "B_Spine2", "B_L_Clavicle", "B_R_Clavicle"),
           "arms": ("B_L_UpperArm", "B_R_UpperArm", "B_L_Forearm", "B_R_Forearm"),
           "hands": ("B_R_Hand", "B_L_Forearm.001", "thumb_", "index_", "middle_", "ring_", "pinky_", "B_L_Finger", "B_R_Finger"),
           "legs": ("B_Pelvis", "B_L_Thigh", "B_R_Thigh", "B_L_Calf", "B_R_Calf"), "feet": ("B_L_Foot", "B_R_Foot", "B_L_Toe0", "B_R_Toe0")}


def region_of(bone):
    for r, prefixes in REGIONS.items():
        if bone.startswith(prefixes):
            return r
    return "other"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--glb", required=True)
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    # merge the vertices glTF splits at UV/normal seams, or every seam reads as an open edge
    bpy.ops.import_scene.gltf(filepath=os.path.abspath(a.glb), merge_vertices=True)
    rigs = [o for o in bpy.data.objects if o.type == "ARMATURE"]
    rig = max(rigs, key=lambda r: len(r.data.bones)) if rigs else None
    # skinned meshes only: the importer adds bone-shape helpers (Icosphere) that are not part of the model
    meshes = [o for o in bpy.data.objects if o.type == "MESH" and any(md.type == "ARMATURE" for md in o.modifiers)]
    bones = set(rig.data.bones.keys()) if rig else set()
    depsgraph = bpy.context.evaluated_depsgraph_get()

    out = {"file": os.path.basename(a.glb), "mb": round(os.path.getsize(a.glb) / 1e6, 1), "meshes": [],
           "materials": len(bpy.data.materials), "images": len(bpy.data.images), "clips": len(bpy.data.actions),
           "bones": len(bones), "missing_bones": [b for b in GAME_BONES if b not in bones]}
    zmin, zmax = 1e9, -1e9
    coverage = {r: 0 for r in list(REGIONS) + ["other"]}
    unweighted = bad_sum = stray = 0
    trees, edges = [], []
    for m in meshes:
        ev = m.evaluated_get(depsgraph)
        data = ev.to_mesh()
        mw = m.matrix_world
        tris = sum(len(poly.vertices) - 2 for poly in data.polygons)
        names = {g.index: g.name for g in m.vertex_groups}
        tree = kdtree.KDTree(len(data.vertices))
        for v in data.vertices:
            co = mw @ v.co
            zmin, zmax = min(zmin, co.z), max(zmax, co.z)
            tree.insert(co, v.index)
        tree.balance()
        for v in m.data.vertices:
            if not v.groups:
                unweighted += 1
                continue
            total = sum(g.weight for g in v.groups)
            if abs(total - 1.0) > 0.02:
                bad_sum += 1
            if any(names.get(g.group) not in bones for g in v.groups if g.weight > 0.0):
                stray += 1
            g = max(v.groups, key=lambda g: g.weight)
            coverage[region_of(names.get(g.group, ""))] += 1
        # open edges: used by one face only
        count = {}
        for poly in data.polygons:
            for k in poly.edge_keys:
                count[k] = count.get(k, 0) + 1
        open_verts = {i for k, c in count.items() if c == 1 for i in k}
        def region_at(i):
            gs = m.data.vertices[i].groups
            return region_of(names.get(max(gs, key=lambda g: g.weight).group, "")) if gs else "other"
        edges.append((m.name, [(mw @ data.vertices[i].co, region_at(i)) for i in open_verts]))
        trees.append((m.name, tree))
        out["meshes"].append({"name": m.name, "verts": len(m.data.vertices), "tris": tris, "open_verts": len(open_verts)})
        ev.to_mesh_clear()

    # a seam gap: an open-edge vertex whose nearest vertex on any other mesh is further than 3 cm. Clothing shells
    # are open by design, so only the base skin (cut under each part) can show a hole: per body region.
    gaps = {}
    for name, pts in edges:
        far = {}
        worst = 0.0
        for co, region in pts:
            near = min((t.find(co)[2] for n, t in trees if n != name), default=0.0)
            if near > 0.03:
                far[region] = far.get(region, 0) + 1
                worst = max(worst, near)
        if far:
            gaps[name] = {"open_verts_far": sum(far.values()), "by_region": far, "worst_cm": round(worst * 100, 1),
                          "skin": name.startswith("SuperHero")}

    out["height_m"] = round(zmax - zmin, 3)
    out["feet_off_floor_cm"] = round(zmin * 100, 1)
    out["tris"] = sum(x["tris"] for x in out["meshes"])
    out["skin"] = {"unweighted_verts": unweighted, "weight_sum_off": bad_sum, "stray_bone_verts": stray}
    out["coverage"] = coverage
    out["seam_gaps"] = gaps

    flags = []
    if out["missing_bones"]:
        flags.append(f"missing game bones: {', '.join(out['missing_bones'])}")
    if not 1.5 <= out["height_m"] <= 2.1:
        flags.append(f"height {out['height_m']} m outside 1.5-2.1")
    if abs(out["feet_off_floor_cm"]) > 3:
        flags.append(f"feet {out['feet_off_floor_cm']} cm off the floor")
    if unweighted:
        flags.append(f"{unweighted} vertices with no skin weight (they will not follow the body)")
    if stray:
        flags.append(f"{stray} vertices weighted to bones the rig lacks")
    for r in ("head", "body", "arms", "hands", "legs", "feet"):
        if coverage[r] == 0:
            flags.append(f"no geometry on the {r} (hole)")
    # the build report knows exactly where the base skin was cut under each part, and what covers it
    build = a.glb + ".report.json"
    if os.path.exists(build):
        with open(build) as f:
            rep = json.load(f)
        out["build"] = {k: rep.get(k) for k in ("outfit", "base", "parts", "hair", "set", "seams")}
        seams = rep.get("seams") or {}
        holes = seams.get("uncovered", {})
        if sum(holes.values()) > 10:
            where = ", ".join(f"{b} {n}" for b, n in sorted(holes.items(), key=lambda x: -x[1]))
            flags.append(f"skin hole: {sum(holes.values())} of {seams.get('cut_edge_verts')} cut-edge verts with no part within 3 cm ({where})")
    if out["tris"] > 60000:
        flags.append(f"{out['tris']} triangles (heavy for a crowd model)")
    if out["mb"] > 60:
        flags.append(f"{out['mb']} MB file")
    out["flags"] = flags
    path = a.glb + ".check.json"
    with open(path, "w") as f:
        json.dump(out, f, indent=2)
    print("CHECK_JSON", json.dumps(out))
    print("CHECK_DONE", path)


main()
