"""Build a master character: a Quaternius Universal Base Character head + a Modular Character Outfit on the
UAL skeleton, its bones renamed to the RPG hero names the game uses, with clips retargeted onto it.

D:/blender.exe -b --factory-startup -P Tools/blender/build_master.py -- --outfit Male_Ranger --out assets/packs/hero/fighter.glb
D:/blender.exe -b --factory-startup -P Tools/blender/build_master.py -- --outfit Female_Peasant --set npc --out assets/packs/npc/female_peasant.glb

--set hero: every clip of the RPG hero library (--source, 130 clips) + Human Basic Motions non-combat set.
--set npc:  townsfolk idles and walks from Quaternius UAL1/UAL2.
Reuses build_hero.py: retarget (rest-relative world deltas), 30 fps re-timing, loop closure, NLA export.
"""
import argparse
import glob
import os
import shutil
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(__file__))
import build_hero as hero  # noqa: E402

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
Q = os.path.join(ROOT, "assets", "packs", "_sources", "quaternius")
STAGE = os.path.join(ROOT, "assets", "packs", "_sources", "chars_staged")
UAL1 = os.path.join(Q, "Universal_Animation_Library_1", "UAL1_Standard.glb")
UAL2 = os.path.join(Q, "Universal_Animation_Library_2", "Universal Animation Library 2[Standard]", "Unreal-Godot", "UAL2_Standard.glb")
HBM = os.path.join(ROOT, "assets", "packs", "_sources", "kevin_human_basic", "Animations", "Male")

# UAL (UE mannequin) bone -> RPG hero name; every name the game code reads is covered
RENAME = {"root": "Motion", "pelvis": "B_Pelvis", "spine_01": "B_Spine", "spine_02": "B_Spine1", "spine_03": "B_Spine2",
          "neck_01": "B_Neck", "Head": "B_Head", "hand_l": "B_L_Forearm.001", "hand_r": "B_R_Hand",
          "middle_01_l": "B_L_Finger2", "middle_01_r": "B_R_Finger2"}
for s, S in (("l", "L"), ("r", "R")):
    RENAME.update({f"clavicle_{s}": f"B_{S}_Clavicle", f"upperarm_{s}": f"B_{S}_UpperArm", f"lowerarm_{s}": f"B_{S}_Forearm",
                   f"thigh_{s}": f"B_{S}_Thigh", f"calf_{s}": f"B_{S}_Calf", f"foot_{s}": f"B_{S}_Foot", f"ball_{s}": f"B_{S}_Toe0"})
FINGERS = [f"{f}_0{i}_{s}" for f in ("thumb", "index", "middle", "ring", "pinky") for i in (1, 2, 3) for s in ("l", "r")]
# UAL clips onto our renamed rig: same skeleton, so the body bones map by RENAME and the fingers keep their names
UAL_MAP = {**{k: v for k, v in RENAME.items() if k not in ("root", "middle_01_l", "middle_01_r")}, **{f: f for f in FINGERS if f not in ("middle_01_l", "middle_01_r")}}
RPG_BONES = sorted({v for v in RENAME.values() if v not in ("Motion", "B_L_Finger2", "B_R_Finger2")})

NPC_CLIPS = {UAL1: {"npc_idle": "Idle_Loop", "npc_talk": "Idle_Talking_Loop", "npc_walk": "Walk_Loop", "npc_interact": "Interact"},
             UAL2: {"npc_fold_arms": "Idle_FoldArms_Loop", "npc_farm": "Farm_Harvest", "npc_carry": "Walk_Carry_Loop",
                    "npc_lantern": "Idle_Lantern_Loop", "npc_yes": "Yes"}}


def stage():
    """Copy the character folders to bracket-free paths (the glTF importer rejects '[' in paths)."""
    pairs = {
        "outfits": glob.glob(os.path.join(Q, "Modular_Character_Outfits", "*", "Exports", "glTF*", "Outfits"))[0],
        "parts": glob.glob(os.path.join(Q, "Modular_Character_Outfits", "*", "Exports", "glTF*", "Modular Parts"))[0],
        "base": glob.glob(os.path.join(Q, "Universal_Base_Characters", "*", "Base Characters", "Godot - UE"))[0],
        "hair": glob.glob(os.path.join(Q, "Universal_Base_Characters", "*", "Hairstyles", "Rigged to Head Bone", "glTF*"))[0],
    }
    for name, src in pairs.items():
        dst = os.path.join(STAGE, name)
        if not os.path.isdir(dst):
            shutil.copytree(src, dst)
    # hair textures live one level up
    tex = glob.glob(os.path.join(Q, "Universal_Base_Characters", "*", "Hairstyles", "Textures"))
    if tex and not os.path.isdir(os.path.join(STAGE, "Textures")):
        shutil.copytree(tex[0], os.path.join(STAGE, "Textures"))


def import_gltf(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    return [o for o in bpy.data.objects if o not in before]


def keep_head_only(mesh, rig):
    """With an outfit only the base head is needed: delete body vertices below the neck."""
    neck = (rig.matrix_world @ rig.data.bones["neck_01"].head_local).z
    bm = bmesh.new()
    bm.from_mesh(mesh.data)
    mw = mesh.matrix_world
    doomed = [v for v in bm.verts if (mw @ v.co).z < neck - 0.02]
    bmesh.ops.delete(bm, geom=doomed, context="VERTS")
    bm.to_mesh(mesh.data)
    bm.free()


# modular part slot -> base body bones (UE names) whose skin the part replaces; uncovered slots keep the base skin
SLOT_BONES = {"Body": ("spine_01", "spine_02", "spine_03", "clavicle_"),
              "Arms": ("upperarm_", "lowerarm_", "hand_", "thumb_", "index_", "middle_", "ring_", "pinky_"),
              "Legs": ("pelvis", "thigh_", "calf_"),
              "Feet": ("foot_", "ball_")}


def slot_of(part):
    """Male_Ranger_Feet_Boots -> Feet; Female_Ranger_Head_Hood -> Head; Male_Ranger_Acc_Pauldron -> Acc."""
    words = part.split("_")[2:]
    return words[0] if words else ""


def cut_slots(mesh, slots):
    """Delete the base skin under the chosen parts: a vertex goes when its strongest bone belongs to a covered slot.
    Returns the new cut edge (world position, strongest bone of each kept vertex next to a deleted one)."""
    prefixes = tuple(b for s in slots for b in SLOT_BONES.get(s, ()))
    if not prefixes:
        return []
    names = {g.index: g.name for g in mesh.vertex_groups}
    doomed_idx = set()
    for v in mesh.data.vertices:
        if v.groups:
            g = max(v.groups, key=lambda g: g.weight)
            if names.get(g.group, "").startswith(prefixes):
                doomed_idx.add(v.index)
    bm = bmesh.new()
    bm.from_mesh(mesh.data)
    bm.verts.ensure_lookup_table()
    rim = {v.index for e in bm.edges if (e.verts[0].index in doomed_idx) != (e.verts[1].index in doomed_idx)
           for v in e.verts if v.index not in doomed_idx}
    mw = mesh.matrix_world
    edge = [(mw @ mesh.data.vertices[i].co, names.get(max(mesh.data.vertices[i].groups, key=lambda g: g.weight).group, ""))
            for i in rim]
    bmesh.ops.delete(bm, geom=[bm.verts[i] for i in doomed_idx], context="VERTS")
    bm.to_mesh(mesh.data)
    bm.free()
    return edge


def uncovered(edge, meshes):
    """Cut-edge vertices with no part vertex within 3 cm (a hole where the base skin was removed), per bone."""
    from mathutils import kdtree
    pts = [mw @ v.co for m in meshes for mw in [m.matrix_world] for v in m.data.vertices]
    tree = kdtree.KDTree(len(pts))
    for i, co in enumerate(pts):
        tree.insert(co, i)
    tree.balance()
    holes = {}
    for co, bone in edge:
        if not pts or tree.find(co)[2] > 0.03:
            holes[bone] = holes.get(bone, 0) + 1
    return {"cut_edge_verts": len(edge), "uncovered": holes}


def srgb_to_linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def tint(objs, hexrgb):
    """Multiply the base colour of these meshes by an sRGB hex colour. Each mesh gets its own material copy (outfit
    parts share one atlas material), and the multiply is a Mix node the glTF exporter writes as baseColorFactor."""
    rgb = tuple(srgb_to_linear(int(hexrgb[i:i + 2], 16) / 255.0) for i in (0, 2, 4))
    for ob in objs:
        if ob.type != "MESH":
            continue
        for ms in ob.material_slots:
            if ms.material is None or not ms.material.use_nodes:
                continue
            m = ms.material.copy()
            m.name = f"{ob.name}_{ms.material.name}"
            ms.material = m
            nt = m.node_tree
            bsdf = next((n for n in nt.nodes if n.type == "BSDF_PRINCIPLED"), None)
            if bsdf is None:
                continue
            inp = bsdf.inputs["Base Color"]
            if inp.is_linked:
                src = inp.links[0].from_socket
                mix = nt.nodes.new("ShaderNodeMix")
                mix.data_type = "RGBA"
                mix.blend_type = "MULTIPLY"
                mix.inputs[0].default_value = 1.0
                nt.links.new(src, mix.inputs[6])
                mix.inputs[7].default_value = (*rgb, 1.0)
                nt.links.new(mix.outputs[2], inp)
            else:
                c = inp.default_value
                inp.default_value = (c[0] * rgb[0], c[1] * rgb[1], c[2] * rgb[2], c[3])


def adopt(objs, rig):
    """Bind every mesh of another import to the master rig (same UAL skeleton) and drop its own rig."""
    for ob in objs:
        if ob.type == "MESH":
            for m in ob.modifiers:
                if m.type == "ARMATURE":
                    m.object = rig
            mw = ob.matrix_world.copy()
            ob.parent = rig
            ob.matrix_world = mw
    for ob in objs:
        if ob.type == "ARMATURE" and ob is not rig:
            bpy.data.objects.remove(ob, do_unlink=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--outfit", default="Male_Ranger")
    p.add_argument("--hair", default="")
    p.add_argument("--set", default="hero", choices=["hero", "npc", "none"])
    # character creator: a base body and any modular parts (file stems in Modular Parts), instead of a whole outfit
    # mannequin: the yellow UAL1 Mannequin as the whole body (no outfit, no hair)
    p.add_argument("--base", default="", choices=["", "male", "female", "mannequin"])
    p.add_argument("--parts", default="", help="comma list of modular part names, e.g. Male_Ranger_Body,Male_Peasant_Legs")
    p.add_argument("--tints", default="", help="slot=rrggbb list (Head, Body, Arms, Legs, Feet, Acc, Hair, Skin), e.g. Body=b04040,Hair=302820")
    p.add_argument("--source", default=os.path.join(ROOT, "assets", "packs", "_sources", "rpg_hero", "fighter.glb"))
    p.add_argument("--out", required=True)
    p.add_argument("--only", default="", help="comma list of clip names (quick validation builds)")
    a = p.parse_args(sys.argv[sys.argv.index("--") + 1:])
    stage()
    for ob in list(bpy.data.objects):
        bpy.data.objects.remove(ob, do_unlink=True)
    female = a.base == "female" if a.base else a.outfit.startswith("Female")
    mannequin = a.base == "mannequin"
    if mannequin:
        base = import_gltf(UAL1)
    else:
        base = import_gltf(os.path.join(STAGE, "base", "Superhero_Female_FullBody.gltf" if female else "Superhero_Male_FullBody.gltf"))
    rig = hero.armature_of(base)
    parts = [x.strip() for x in a.parts.split(",") if x.strip()]
    cut_edge = []
    slot_objs = {"Skin": [o for o in base if o.type == "MESH" and len(o.data.vertices) > 2000]}
    for ob in base:
        if ob.type == "MESH" and len(ob.data.vertices) > 2000:
            if parts:
                cut_edge += cut_slots(ob, {slot_of(x) for x in parts})
            elif not mannequin:
                keep_head_only(ob, rig)
    seams = None
    if parts:
        part_meshes = []
        for part in parts:
            path = os.path.join(STAGE, "parts", f"{part}.gltf")
            if os.path.exists(path):
                objs = import_gltf(path)
                part_meshes += [o for o in objs if o.type == "MESH"]
                slot_objs.setdefault(slot_of(part), []).extend(o for o in objs if o.type == "MESH")
                adopt(objs, rig)
            else:
                print("PART_MISSING", part)
        seams = uncovered(cut_edge, part_meshes)
    elif not mannequin:
        adopt(import_gltf(os.path.join(STAGE, "outfits", f"{a.outfit}.gltf")), rig)
    hair = "none" if mannequin else a.hair or ("Hair_Long" if female else "Hair_SimpleParted")
    # the base head already carries eyebrows; add only the hairstyle
    path = os.path.join(STAGE, "hair", f"{hair}.gltf")
    if hair != "none" and os.path.exists(path):
        objs = import_gltf(path)
        slot_objs["Hair"] = [o for o in objs if o.type == "MESH"]
        adopt(objs, rig)
    tints = dict(t.split("=", 1) for t in a.tints.split(",") if "=" in t)
    for slot, hexrgb in tints.items():
        if hexrgb.lower() != "ffffff":
            tint([o for o in slot_objs.get(slot, []) if o.name in bpy.data.objects], hexrgb)
    # helper shapes shipped in the base file (an Icosphere spanning +-1 m) would break height normalization
    for ob in [o for o in bpy.data.objects if o.type == "MESH" and (len(o.data.vertices) < 100 or o.name.startswith("Icosphere"))]:
        bpy.data.objects.remove(ob, do_unlink=True)
    for act in list(bpy.data.actions):
        bpy.data.actions.remove(act)
    # rename to the hero names the game reads (Blender renames the bound vertex groups with the bones)
    for old, new in RENAME.items():
        if old in rig.data.bones:
            rig.data.bones[old].name = new
    rig.animation_data_create()
    rig.animation_data.action = None
    model = [o for o in bpy.data.objects]
    report = {"outfit": a.outfit if not (a.parts or mannequin) else None, "base": a.base or ("female" if female else "male"), "parts": a.parts.split(",") if a.parts else [], "hair": hair, "set": a.set, "tints": a.tints, "clips": [], "skipped": [], "seams": seams}

    def take(path, clips, mapping, motion_root=None, root_bones=None):
        objs, acts, time_scale = hero.load_source(path, set(clips.values()))
        src = max((o for o in objs if o.type == "ARMATURE"), key=lambda r: len(r.data.bones))
        src.animation_data_create()
        for t in list(src.animation_data.nla_tracks):
            src.animation_data.nla_tracks.remove(t)
        for name, action in clips.items():
            act = acts.get(action)
            if act is None:
                report["skipped"].append(f"{name}: {action} not in {os.path.basename(path)}")
                continue
            src.animation_data.action = act
            if hasattr(src.animation_data, "action_slot") and len(getattr(act, "slots", [])) > 0:
                src.animation_data.action_slot = act.slots[0]
            out = hero.retarget(src, rig, name, mapping, motion_root=motion_root, time_scale=time_scale, root_bones=root_bones)
            hero.push(rig, out, name, report)
            report["clips"].append(name)
        for o in objs:
            bpy.data.objects.remove(o, do_unlink=True)
        for act in acts.values():
            if not act.use_fake_user:
                bpy.data.actions.remove(act)

    ual_map = UAL_MAP
    if a.set == "hero":
        # the full RPG hero library: same bone names, different rest pose and proportions
        names = {}
        before = set(bpy.data.actions)
        probe = import_gltf(a.source)
        # boss: only RPG idles; FightingMotions/caster/kneel idles are cut
        cut = {"idle_A", "idle_Tiramis", "mage_cast_idle", "mx_kneel"}
        names = {act.name: act.name for act in bpy.data.actions if act not in before and act.name not in cut and not act.name.startswith("idle_")}
        for o in probe:
            bpy.data.objects.remove(o, do_unlink=True)
        for act in [x for x in bpy.data.actions if x not in before]:
            bpy.data.actions.remove(act)
        if a.only:
            names = {k: v for k, v in names.items() if k in a.only.split(",")}
        take(a.source, names, {b: b for b in RPG_BONES}, root_bones=("Motion", "Motion"))
        # Human Basic Motions (Kevin Iglesias, HumanM rig): the non-combat set
        nc = {"nc_idle": "HumanM@Idle01", "nc_jump_begin": "HumanM@Jump01 - Begin", "nc_fall": "HumanM@Fall01", "nc_jump_land": "HumanM@Jump01 - Land"}
        for kind in ("Walk", "Run"):
            for d in ("Forward", "ForwardRight", "Right", "BackwardRight", "Backward", "BackwardLeft", "Left", "ForwardLeft"):
                nc[f"nc_{kind.lower()}_{d.lower()}"] = f"HumanM@{kind}01_{d}"
        for d in ("Forward", "ForwardRight", "ForwardLeft"):
            nc[f"nc_sprint_{d.lower()}"] = f"HumanM@Sprint01_{d}"
        files = {os.path.splitext(os.path.basename(f))[0]: f for f in glob.glob(os.path.join(HBM, "**", "*.fbx"), recursive=True) if "[RM]" not in f}
        for name, stem in (nc.items() if not a.only else []):
            f = files.get(stem)
            if f is None:
                report["skipped"].append(f"{name}: no {stem}")
                continue
            objs = hero.import_fbx(f)
            src = hero.armature_of(objs)
            act = hero.retarget(src, rig, name, hero.HERO_MAPPING["mage"]["bones"], motion_root="B-root") if src else None
            if act:
                hero.push(rig, act, name, report)
                report["clips"].append(name)
            else:
                report["skipped"].append(f"{name}: no armature action")
            for o in objs:
                bpy.data.objects.remove(o, do_unlink=True)
            for x in [x for x in bpy.data.actions if x.users == 0 and not x.use_fake_user]:
                bpy.data.actions.remove(x)
    elif a.set == "npc":
        for path, clips in NPC_CLIPS.items():
            take(path, clips, ual_map)
    # pauldrons follow the shoulder girdle, not only the upper arm (they cut into the chest otherwise)
    import fix_skin
    report["pauldron_reweighted"] = fix_skin.reweight_pauldrons(rig)
    report["normalize_scale"] = hero.normalize(model, 1.8)
    hero.export_hero(rig, os.path.abspath(a.out), report)


if __name__ == "__main__":
    main()
