extends SceneTree
## Animation library builder. Every source - BVH motion capture (CMU, ACCAD, 100STYLE, Motifect) and
## glTF clip libraries (Quaternius UAL) - is retargeted onto our humanoid skeleton: Godot's
## SkeletonProfileHumanoid bone names on the VRoid base rest pose (godot-vrm normalises every VRM to
## it, so the clips play on any VRoid character).
##   Godot --headless --path <guildbound> --script res://tools/retarget.gd [-- --only=Name,Name] [--takes=src]
## Jobs: data/anim_sources.json ("sources" = where the raw files are, "clips" = what to make).
## Output: assets/anims/<Name>.res and assets/anims/clips.json (length, speed, strike timing...).
##
## Retarget: per mapped bone, the source's rotation away from ITS rest pose (global) is applied to the
## target bone after `align` turns the target's rest bone direction onto the source's rest direction
## (rests differ: T-pose vs A-pose, splayed legs):  G_t = yaw * (G_s * G_s_rest^-1) * align * R_t.
## Bones the source lacks (fingers in most mocap) take a relaxed hand from a reference clip.
## Ground: after retargeting, the hips are lifted / lowered so the planted foot meets the floor.

const JOBS := "res://data/anim_sources.json"
const OUT := "res://assets/anims/"
const FPS := 30.0

## Source joint naming -> humanoid profile bone, and the joint whose rest position gives the direction.
const MAPS := {
	"mixamo": {  # ACCAD, Motifect-like and most Mixamo-style BVH
		"Hips": ["Hips", "Spine"], "Spine": ["Spine", "Spine1"], "Spine1": ["Chest", "Neck"],
		"Neck": ["Neck", "Head"], "Head": ["Head", "Head_end"],
		"LeftShoulder": ["LeftShoulder", "LeftArm"], "LeftArm": ["LeftUpperArm", "LeftForeArm"],
		"LeftForeArm": ["LeftLowerArm", "LeftHand"], "LeftHand": ["LeftHand", "LeftHand_end"],
		"RightShoulder": ["RightShoulder", "RightArm"], "RightArm": ["RightUpperArm", "RightForeArm"],
		"RightForeArm": ["RightLowerArm", "RightHand"], "RightHand": ["RightHand", "RightHand_end"],
		"LeftUpLeg": ["LeftUpperLeg", "LeftLeg"], "LeftLeg": ["LeftLowerLeg", "LeftFoot"],
		"LeftFoot": ["LeftFoot", "LeftToeBase"], "LeftToeBase": ["LeftToes", "LeftToeBase_end"],
		"RightUpLeg": ["RightUpperLeg", "RightLeg"], "RightLeg": ["RightLowerLeg", "RightFoot"],
		"RightFoot": ["RightFoot", "RightToeBase"], "RightToeBase": ["RightToes", "RightToeBase_end"],
	},
	"style100": {  # 100STYLE (Mason et al., CC BY 4.0)
		"Hips": ["Hips", "Chest"], "Chest": ["Spine", "Chest2"], "Chest3": ["Chest", "Chest4"],
		"Chest4": ["UpperChest", "Neck"], "Neck": ["Neck", "Head"], "Head": ["Head", "Head_end"],
		"LeftCollar": ["LeftShoulder", "LeftShoulder"], "LeftShoulder": ["LeftUpperArm", "LeftElbow"],
		"LeftElbow": ["LeftLowerArm", "LeftWrist"], "LeftWrist": ["LeftHand", "LeftWrist_end"],
		"RightCollar": ["RightShoulder", "RightShoulder"], "RightShoulder": ["RightUpperArm", "RightElbow"],
		"RightElbow": ["RightLowerArm", "RightWrist"], "RightWrist": ["RightHand", "RightWrist_end"],
		"LeftHip": ["LeftUpperLeg", "LeftKnee"], "LeftKnee": ["LeftLowerLeg", "LeftAnkle"],
		"LeftAnkle": ["LeftFoot", "LeftToe"], "LeftToe": ["LeftToes", "LeftToe_end"],
		"RightHip": ["RightUpperLeg", "RightKnee"], "RightKnee": ["RightLowerLeg", "RightAnkle"],
		"RightAnkle": ["RightFoot", "RightToe"], "RightToe": ["RightToes", "RightToe_end"],
	},
	"amc": {  # ACCAD older takes (ASF/AMC naming)
		"root": ["Hips", "lowerback"], "lowerback": ["Spine", "upperback"], "upperback": ["Chest", "thorax"],
		"thorax": ["UpperChest", "neck"], "neck": ["Neck", "head"], "head": ["Head", "head_end"],
		"lshoulderjoint": ["LeftShoulder", "lhumerus"], "lhumerus": ["LeftUpperArm", "lradius"],
		"lradius": ["LeftLowerArm", "lhand"], "lhand": ["LeftHand", "lhand_end"],
		"rshoulderjoint": ["RightShoulder", "rhumerus"], "rhumerus": ["RightUpperArm", "rradius"],
		"rradius": ["RightLowerArm", "rhand"], "rhand": ["RightHand", "rhand_end"],
		"lfemur": ["LeftUpperLeg", "ltibia"], "ltibia": ["LeftLowerLeg", "lfoot"], "lfoot": ["LeftFoot", "ltoes"],
		"ltoes": ["LeftToes", "ltoes_end"],
		"rfemur": ["RightUpperLeg", "rtibia"], "rtibia": ["RightLowerLeg", "rfoot"], "rfoot": ["RightFoot", "rtoes"],
		"rtoes": ["RightToes", "rtoes_end"],
	},
	"mixamo_fbx": {  # Mixamo FBX as Godot loads it (mixamorig: -> mixamorig_)
		"mixamorig_Hips": ["Hips", "mixamorig_Spine"], "mixamorig_Spine": ["Spine", "mixamorig_Spine1"],
		"mixamorig_Spine1": ["Chest", "mixamorig_Spine2"], "mixamorig_Spine2": ["UpperChest", "mixamorig_Neck"],
		"mixamorig_Neck": ["Neck", "mixamorig_Head"], "mixamorig_Head": ["Head", "mixamorig_HeadTop_End"],
		"mixamorig_LeftShoulder": ["LeftShoulder", "mixamorig_LeftArm"], "mixamorig_LeftArm": ["LeftUpperArm", "mixamorig_LeftForeArm"],
		"mixamorig_LeftForeArm": ["LeftLowerArm", "mixamorig_LeftHand"], "mixamorig_LeftHand": ["LeftHand", "mixamorig_LeftHandMiddle1"],
		"mixamorig_RightShoulder": ["RightShoulder", "mixamorig_RightArm"], "mixamorig_RightArm": ["RightUpperArm", "mixamorig_RightForeArm"],
		"mixamorig_RightForeArm": ["RightLowerArm", "mixamorig_RightHand"], "mixamorig_RightHand": ["RightHand", "mixamorig_RightHandMiddle1"],
		"mixamorig_LeftUpLeg": ["LeftUpperLeg", "mixamorig_LeftLeg"], "mixamorig_LeftLeg": ["LeftLowerLeg", "mixamorig_LeftFoot"],
		"mixamorig_LeftFoot": ["LeftFoot", "mixamorig_LeftToeBase"], "mixamorig_LeftToeBase": ["LeftToes", "mixamorig_LeftToe_End"],
		"mixamorig_RightUpLeg": ["RightUpperLeg", "mixamorig_RightLeg"], "mixamorig_RightLeg": ["RightLowerLeg", "mixamorig_RightFoot"],
		"mixamorig_RightFoot": ["RightFoot", "mixamorig_RightToeBase"], "mixamorig_RightToeBase": ["RightToes", "mixamorig_RightToe_End"],
		"mixamorig_LeftHandThumb1": ["LeftThumbMetacarpal", "mixamorig_LeftHandThumb2"],
		"mixamorig_LeftHandThumb2": ["LeftThumbProximal", "mixamorig_LeftHandThumb3"],
		"mixamorig_LeftHandThumb3": ["LeftThumbDistal", "mixamorig_LeftHandThumb4"],
		"mixamorig_LeftHandIndex1": ["LeftIndexProximal", "mixamorig_LeftHandIndex2"],
		"mixamorig_LeftHandIndex2": ["LeftIndexIntermediate", "mixamorig_LeftHandIndex3"],
		"mixamorig_LeftHandIndex3": ["LeftIndexDistal", "mixamorig_LeftHandIndex4"],
		"mixamorig_LeftHandMiddle1": ["LeftMiddleProximal", "mixamorig_LeftHandMiddle2"],
		"mixamorig_LeftHandMiddle2": ["LeftMiddleIntermediate", "mixamorig_LeftHandMiddle3"],
		"mixamorig_LeftHandMiddle3": ["LeftMiddleDistal", "mixamorig_LeftHandMiddle4"],
		"mixamorig_LeftHandRing1": ["LeftRingProximal", "mixamorig_LeftHandRing2"],
		"mixamorig_LeftHandRing2": ["LeftRingIntermediate", "mixamorig_LeftHandRing3"],
		"mixamorig_LeftHandRing3": ["LeftRingDistal", "mixamorig_LeftHandRing4"],
		"mixamorig_LeftHandPinky1": ["LeftLittleProximal", "mixamorig_LeftHandPinky2"],
		"mixamorig_LeftHandPinky2": ["LeftLittleIntermediate", "mixamorig_LeftHandPinky3"],
		"mixamorig_LeftHandPinky3": ["LeftLittleDistal", "mixamorig_LeftHandPinky4"],
		"mixamorig_RightHandThumb1": ["RightThumbMetacarpal", "mixamorig_RightHandThumb2"],
		"mixamorig_RightHandThumb2": ["RightThumbProximal", "mixamorig_RightHandThumb3"],
		"mixamorig_RightHandThumb3": ["RightThumbDistal", "mixamorig_RightHandThumb4"],
		"mixamorig_RightHandIndex1": ["RightIndexProximal", "mixamorig_RightHandIndex2"],
		"mixamorig_RightHandIndex2": ["RightIndexIntermediate", "mixamorig_RightHandIndex3"],
		"mixamorig_RightHandIndex3": ["RightIndexDistal", "mixamorig_RightHandIndex4"],
		"mixamorig_RightHandMiddle1": ["RightMiddleProximal", "mixamorig_RightHandMiddle2"],
		"mixamorig_RightHandMiddle2": ["RightMiddleIntermediate", "mixamorig_RightHandMiddle3"],
		"mixamorig_RightHandMiddle3": ["RightMiddleDistal", "mixamorig_RightHandMiddle4"],
		"mixamorig_RightHandRing1": ["RightRingProximal", "mixamorig_RightHandRing2"],
		"mixamorig_RightHandRing2": ["RightRingIntermediate", "mixamorig_RightHandRing3"],
		"mixamorig_RightHandRing3": ["RightRingDistal", "mixamorig_RightHandRing4"],
		"mixamorig_RightHandPinky1": ["RightLittleProximal", "mixamorig_RightHandPinky2"],
		"mixamorig_RightHandPinky2": ["RightLittleIntermediate", "mixamorig_RightHandPinky3"],
		"mixamorig_RightHandPinky3": ["RightLittleDistal", "mixamorig_RightHandPinky4"],
	},
	"cmu": {
		"Hips": ["Hips", "LowerBack"], "LowerBack": ["Spine", "Spine"], "Spine": ["Chest", "Spine1"],
		"Spine1": ["UpperChest", "Neck1"], "Neck1": ["Neck", "Head"], "Head": ["Head", "Head_end"],
		"LeftShoulder": ["LeftShoulder", "LeftArm"], "LeftArm": ["LeftUpperArm", "LeftForeArm"],
		"LeftForeArm": ["LeftLowerArm", "LeftHand"], "LeftHand": ["LeftHand", "LeftHandIndex1"],
		"RightShoulder": ["RightShoulder", "RightArm"], "RightArm": ["RightUpperArm", "RightForeArm"],
		"RightForeArm": ["RightLowerArm", "RightHand"], "RightHand": ["RightHand", "RightHandIndex1"],
		"LeftUpLeg": ["LeftUpperLeg", "LeftLeg"], "LeftLeg": ["LeftLowerLeg", "LeftFoot"],
		"LeftFoot": ["LeftFoot", "LeftToeBase"], "LeftToeBase": ["LeftToes", "LeftToeBase_end"],
		"RightUpLeg": ["RightUpperLeg", "RightLeg"], "RightLeg": ["RightLowerLeg", "RightFoot"],
		"RightFoot": ["RightFoot", "RightToeBase"], "RightToeBase": ["RightToes", "RightToeBase_end"],
	},
	"motifect": {
		"Hips": ["Hips", "Spine1"], "Spine1": ["Spine", "Spine2"], "Spine2": ["Chest", "Chest"],
		"Chest": ["UpperChest", "Neck1"], "Neck1": ["Neck", "Head"], "Head": ["Head", "HeadEnd"],
		"LeftShoulder": ["LeftShoulder", "LeftArm"], "LeftArm": ["LeftUpperArm", "LeftForeArm"],
		"LeftForeArm": ["LeftLowerArm", "LeftHand"], "LeftHand": ["LeftHand", "LeftHandMiddle1"],
		"RightShoulder": ["RightShoulder", "RightArm"], "RightArm": ["RightUpperArm", "RightForeArm"],
		"RightForeArm": ["RightLowerArm", "RightHand"], "RightHand": ["RightHand", "RightHandMiddle1"],
		"LeftLeg": ["LeftUpperLeg", "LeftShin"], "LeftShin": ["LeftLowerLeg", "LeftFoot"],
		"LeftFoot": ["LeftFoot", "LeftToeBase"], "LeftToeBase": ["LeftToes", "LeftToeEnd"],
		"RightLeg": ["RightUpperLeg", "RightShin"], "RightShin": ["RightLowerLeg", "RightFoot"],
		"RightFoot": ["RightFoot", "RightToeBase"], "RightToeBase": ["RightToes", "RightToeEnd"],
		"LeftHandIndex1": ["LeftIndexProximal", "LeftHandIndex2"], "LeftHandIndex2": ["LeftIndexIntermediate", "LeftHandIndex3"],
		"LeftHandIndex3": ["LeftIndexDistal", "LeftHandIndex4"],
		"LeftHandMiddle1": ["LeftMiddleProximal", "LeftHandMiddle2"], "LeftHandMiddle2": ["LeftMiddleIntermediate", "LeftHandMiddle3"],
		"LeftHandMiddle3": ["LeftMiddleDistal", "LeftHandMiddle4"],
		"LeftHandRing1": ["LeftRingProximal", "LeftHandRing2"], "LeftHandRing2": ["LeftRingIntermediate", "LeftHandRing3"],
		"LeftHandRing3": ["LeftRingDistal", "LeftHandRing4"],
		"LeftHandPinky1": ["LeftLittleProximal", "LeftHandPinky2"], "LeftHandPinky2": ["LeftLittleIntermediate", "LeftHandPinky3"],
		"LeftHandPinky3": ["LeftLittleDistal", "LeftHandPinky4"],
		"LeftHandThumb1": ["LeftThumbMetacarpal", "LeftHandThumb2"], "LeftHandThumb2": ["LeftThumbProximal", "LeftHandThumb3"],
		"LeftHandThumb3": ["LeftThumbDistal", "LeftHandThumbEnd"],
		"RightHandIndex1": ["RightIndexProximal", "RightHandIndex2"], "RightHandIndex2": ["RightIndexIntermediate", "RightHandIndex3"],
		"RightHandIndex3": ["RightIndexDistal", "RightHandIndex4"],
		"RightHandMiddle1": ["RightMiddleProximal", "RightHandMiddle2"], "RightHandMiddle2": ["RightMiddleIntermediate", "RightHandMiddle3"],
		"RightHandMiddle3": ["RightMiddleDistal", "RightHandMiddle4"],
		"RightHandRing1": ["RightRingProximal", "RightHandRing2"], "RightHandRing2": ["RightRingIntermediate", "RightHandRing3"],
		"RightHandRing3": ["RightRingDistal", "RightHandRing4"],
		"RightHandPinky1": ["RightLittleProximal", "RightHandPinky2"], "RightHandPinky2": ["RightLittleIntermediate", "RightHandPinky3"],
		"RightHandPinky3": ["RightLittleDistal", "RightHandPinky4"],
		"RightHandThumb1": ["RightThumbMetacarpal", "RightHandThumb2"], "RightHandThumb2": ["RightThumbProximal", "RightHandThumb3"],
		"RightHandThumb3": ["RightThumbDistal", "RightHandThumbEnd"],
	},
	"ue": {  # Quaternius Universal Animation Library (glTF, Unreal-style names)
		"pelvis": ["Hips", "spine_01"], "spine_01": ["Spine", "spine_02"], "spine_02": ["Chest", "spine_03"],
		"spine_03": ["UpperChest", "neck_01"], "neck_01": ["Neck", "Head"], "Head": ["Head", ""],
		"clavicle_l": ["LeftShoulder", "upperarm_l"], "upperarm_l": ["LeftUpperArm", "lowerarm_l"],
		"lowerarm_l": ["LeftLowerArm", "hand_l"], "hand_l": ["LeftHand", "middle_01_l"],
		"clavicle_r": ["RightShoulder", "upperarm_r"], "upperarm_r": ["RightUpperArm", "lowerarm_r"],
		"lowerarm_r": ["RightLowerArm", "hand_r"], "hand_r": ["RightHand", "middle_01_r"],
		"thigh_l": ["LeftUpperLeg", "calf_l"], "calf_l": ["LeftLowerLeg", "foot_l"], "foot_l": ["LeftFoot", "ball_l"],
		"ball_l": ["LeftToes", ""],
		"thigh_r": ["RightUpperLeg", "calf_r"], "calf_r": ["RightLowerLeg", "foot_r"], "foot_r": ["RightFoot", "ball_r"],
		"ball_r": ["RightToes", ""],
		"index_01_l": ["LeftIndexProximal", "index_02_l"], "index_02_l": ["LeftIndexIntermediate", "index_03_l"], "index_03_l": ["LeftIndexDistal", ""],
		"middle_01_l": ["LeftMiddleProximal", "middle_02_l"], "middle_02_l": ["LeftMiddleIntermediate", "middle_03_l"], "middle_03_l": ["LeftMiddleDistal", ""],
		"ring_01_l": ["LeftRingProximal", "ring_02_l"], "ring_02_l": ["LeftRingIntermediate", "ring_03_l"], "ring_03_l": ["LeftRingDistal", ""],
		"pinky_01_l": ["LeftLittleProximal", "pinky_02_l"], "pinky_02_l": ["LeftLittleIntermediate", "pinky_03_l"], "pinky_03_l": ["LeftLittleDistal", ""],
		"thumb_01_l": ["LeftThumbMetacarpal", "thumb_02_l"], "thumb_02_l": ["LeftThumbProximal", "thumb_03_l"], "thumb_03_l": ["LeftThumbDistal", ""],
		"index_01_r": ["RightIndexProximal", "index_02_r"], "index_02_r": ["RightIndexIntermediate", "index_03_r"], "index_03_r": ["RightIndexDistal", ""],
		"middle_01_r": ["RightMiddleProximal", "middle_02_r"], "middle_02_r": ["RightMiddleIntermediate", "middle_03_r"], "middle_03_r": ["RightMiddleDistal", ""],
		"ring_01_r": ["RightRingProximal", "ring_02_r"], "ring_02_r": ["RightRingIntermediate", "ring_03_r"], "ring_03_r": ["RightRingDistal", ""],
		"pinky_01_r": ["RightLittleProximal", "pinky_02_r"], "pinky_02_r": ["RightLittleIntermediate", "pinky_03_r"], "pinky_03_r": ["RightLittleDistal", ""],
		"thumb_01_r": ["RightThumbMetacarpal", "thumb_02_r"], "thumb_02_r": ["RightThumbProximal", "thumb_03_r"], "thumb_03_r": ["RightThumbDistal", ""],
	},
}
const FINGERS := ["Thumb", "Index", "Middle", "Ring", "Little"]
const FLAT_REST := ["LeftFoot", "RightFoot", "LeftToes", "RightToes"]

var jobs := {}
var tgt: Skeleton3D
var tgt_root: Node3D
var relaxed := {}          # bone -> local rotation of a relaxed hand (for sources without fingers)
var _glb_cache := {}       # source id -> [scene root, skeleton, AnimationPlayer]
var _zip_cache := {}
var order: Array[int] = []  # target bones, parents first (VRoid's Root bone comes last in the list)
var tname := {}            # humanoid profile bone -> the target skeleton's name for it
var _fbx_cache := {}


func _initialize() -> void:
	_run()


func _run() -> void:
	jobs = JSON.parse_string(FileAccess.get_file_as_string(JOBS))
	var args := OS.get_cmdline_user_args()
	var only: Array = []
	for a in args:
		if a.begins_with("--only="):
			only = Array(a.substr(7).split(","))
		if a.begins_with("--takes="):
			_list_takes(a.substr(8))
			quit()
			return
	tgt_root = (load(str(jobs["_target"])) as PackedScene).instantiate()
	root.add_child(tgt_root)
	tgt = tgt_root.find_child("GeneralSkeleton", true, false)
	if tgt == null:
		tgt = tgt_root.find_children("*", "Skeleton3D", true, false)[0]
	# profile name -> target name (profile names directly, else the Unreal-style names of the "ue" map)
	var prof := SkeletonProfileHumanoid.new()
	for i in prof.bone_size:
		var pn := str(prof.get_bone_name(i))
		tname[pn] = pn
		if tgt.find_bone(pn) < 0:
			for un: String in MAPS["ue"]:
				if MAPS["ue"][un][0] == pn:
					tname[pn] = un
	tname["Root"] = "Root" if tgt.find_bone("Root") >= 0 else "root"
	var placed := {}
	while order.size() < tgt.get_bone_count():
		for bi in tgt.get_bone_count():
			var par := tgt.get_bone_parent(bi)
			if not placed.has(bi) and (par < 0 or placed.has(par)):
				placed[bi] = true
				order.append(bi)
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(OUT))
	await process_frame
	var meta := {}
	var mpath := OUT + "clips.json"
	if FileAccess.file_exists(mpath):
		meta = JSON.parse_string(FileAccess.get_file_as_string(mpath))
	if jobs.has("_relaxed_hand"):
		await _make_relaxed(jobs["_relaxed_hand"])
	for name: String in jobs["clips"]:
		if name.begins_with("_") or (not only.is_empty() and not only.has(name)):
			continue
		var job: Dictionary = jobs["clips"][name]
		var data := await _sample(job)
		if data.is_empty():
			push_error("clip %s: source not found (%s)" % [name, str(job)])
			continue
		var result := _build(name, job, data)
		ResourceSaver.save(result[0], OUT + name + ".res")
		meta[name] = result[1]
		print("CLIP %-24s %5.2fs  %s" % [name, (result[0] as Animation).length, JSON.stringify(result[1])])
	var f := FileAccess.open(mpath, FileAccess.WRITE)
	f.store_string(JSON.stringify(meta, "  ", true))
	quit()


# ---- sources -------------------------------------------------------------------------------------

## Sampled source motion at FPS: {"G": [frame -> {joint -> Quaternion delta from rest}],
## "root": [frame -> Vector3], "rest": {joint -> rest position}, "parent": {joint -> parent joint}, "map": {...}}
func _sample(job: Dictionary) -> Dictionary:
	var src: Dictionary = jobs["sources"][job["src"]]
	match str(src["type"]):
		"bvh_dir", "bvh_zip":
			var text := _bvh_text(src, str(job["take"]))
			if text == "":
				return {}
			return _sample_bvh(text, MAPS[src["map"]], job)
		"glb":
			return await _sample_glb(job["src"], src, str(job["take"]), job)
		"fbx_dir":
			var fp := str(src["path"]) + str(job["take"]) + ".fbx"
			if not FileAccess.file_exists(fp):
				return {}
			return await _sample_glb(fp, {"path": fp, "map": src["map"], "fbx": true}, "", job)
	return {}


func _bvh_text(src: Dictionary, take: String) -> String:
	var path: String = src["path"]
	if src["type"] == "bvh_dir":
		var p := path + take + ".bvh"
		return FileAccess.get_file_as_string(p) if FileAccess.file_exists(p) else ""
	if not _zip_cache.has(path):
		var z := ZIPReader.new()
		if z.open(path) != OK:
			return ""
		_zip_cache[path] = z
	var zr: ZIPReader = _zip_cache[path]
	for f in zr.get_files():
		if f.get_file().get_basename() == take:
			return zr.read_file(f).get_string_from_ascii()
	return ""


func _list_takes(src_id: String) -> void:
	jobs = JSON.parse_string(FileAccess.get_file_as_string(JOBS))
	var src: Dictionary = jobs["sources"][src_id]
	if src["type"] == "bvh_zip":
		var z := ZIPReader.new()
		z.open(src["path"])
		for f in z.get_files():
			if f.to_lower().ends_with(".bvh"):
				print(f.get_file().get_basename())
	elif src["type"] == "fbx_dir":
		for f in DirAccess.get_files_at(src["path"]):
			if f.to_lower().ends_with(".fbx"):
				print(f.get_basename())
	elif src["type"] == "bvh_dir":
		for f in DirAccess.get_files_at(src["path"]):
			if f.ends_with(".bvh"):
				print(f.get_basename())
	else:
		var g := GLTFDocument.new()
		var st := GLTFState.new()
		g.append_from_file(src["path"], st)
		var sc := g.generate_scene(st)
		var ap: AnimationPlayer = sc.find_child("AnimationPlayer", true, false)
		print(" ".join(ap.get_animation_list()))
		sc.free()


func _sample_bvh(text: String, map: Dictionary, job: Dictionary) -> Dictionary:
	var J: Array = []
	var jidx := {}
	var words := PackedStringArray()
	for w in text.replace("\t", " ").replace("\r", " ").replace("\n", " ").split(" ", false):
		words.append(w)
	var i := 0
	var cur := -1
	while i < words.size():
		var w := words[i]
		if w == "ROOT" or w == "JOINT":
			J.append({"name": words[i + 1], "parent": cur, "offset": Vector3.ZERO, "channels": []})
			cur = J.size() - 1
			jidx[words[i + 1]] = cur
			i += 2
		elif w == "End":
			var nm: String = J[cur]["name"] + "_end"
			J.append({"name": nm, "parent": cur, "offset": Vector3.ZERO, "channels": []})
			cur = J.size() - 1
			jidx[nm] = cur
			i += 2
		elif w == "{":
			i += 1
		elif w == "}":
			cur = J[cur]["parent"] if cur >= 0 else -1
			i += 1
		elif w == "OFFSET":
			J[cur]["offset"] = Vector3(float(words[i + 1]), float(words[i + 2]), float(words[i + 3]))
			i += 4
		elif w == "CHANNELS":
			var n := int(words[i + 1])
			var ch: Array = []
			for k in n:
				ch.append(words[i + 2 + k])
			J[cur]["channels"] = ch
			i += 2 + n
		elif w == "MOTION":
			i += 1
			break
		else:
			i += 1
	var nframes := int(words[i + 1])
	var ft := float(words[i + 4])
	i += 5
	var per := 0
	for j in J:
		per += (j["channels"] as Array).size()
	var step := maxi(1, int(round((1.0 / FPS) / ft)))
	var rest := {}
	var parent := {}
	for ji in J.size():
		var p := Vector3.ZERO
		var j := ji
		while j >= 0:
			p += J[j]["offset"]
			j = J[j]["parent"]
		rest[J[ji]["name"]] = p
		parent[J[ji]["name"]] = J[J[ji]["parent"]]["name"] if J[ji]["parent"] >= 0 else ""
	var Gs: Array = []
	var roots: Array = []
	var skip0 := 1 if bool(job.get("skip_first", true)) else 0  # frame 0 is often a calibration pose
	for f in range(skip0, nframes, step):
		var base := i + f * per
		if base + per > words.size():
			break
		var G := {}
		var Gi: Array = []
		Gi.resize(J.size())
		var rp := Vector3.ZERO
		var c := base
		for ji in J.size():
			var q := Quaternion.IDENTITY
			for ch in J[ji]["channels"]:
				var v := float(words[c])
				c += 1
				match ch:
					"Xposition":
						rp.x = v
					"Yposition":
						rp.y = v
					"Zposition":
						rp.z = v
					"Xrotation":
						q = q * Quaternion(Vector3.RIGHT, deg_to_rad(v))
					"Yrotation":
						q = q * Quaternion(Vector3.UP, deg_to_rad(v))
					"Zrotation":
						q = q * Quaternion(Vector3.BACK, deg_to_rad(v))
			Gi[ji] = q if J[ji]["parent"] < 0 else (Gi[J[ji]["parent"]] as Quaternion) * q
			G[J[ji]["name"]] = Gi[ji]
		Gs.append(G)
		roots.append(rp)
	var units := float(job.get("units", 1.0))  # e.g. 0.01 for centimetre files; scale is relative anyway
	return {"G": Gs, "root": roots, "rest": rest, "parent": parent, "map": map, "units": units}


func _sample_glb(src_id: String, src: Dictionary, take: String, job: Dictionary) -> Dictionary:
	if not _glb_cache.has(src_id):
		var sc: Node
		if bool(src.get("fbx", false)):
			var fd := FBXDocument.new()
			var fs := FBXState.new()
			if fd.append_from_file(src["path"], fs) != OK:
				return {}
			sc = fd.generate_scene(fs)
		else:
			var g := GLTFDocument.new()
			var st := GLTFState.new()
			if g.append_from_file(src["path"], st) != OK:
				return {}
			sc = g.generate_scene(st)
		root.add_child(sc)
		_glb_cache[src_id] = [sc, sc.find_child("Skeleton3D", true, false), sc.find_child("AnimationPlayer", true, false)]
		await process_frame
	var sk: Skeleton3D = _glb_cache[src_id][1]
	var ap: AnimationPlayer = _glb_cache[src_id][2]
	if take == "" and ap.get_animation_list().size() > 0:
		take = ap.get_animation_list()[0]  # an FBX file holds one take
	if not ap.has_animation(take):
		return {}
	var a := ap.get_animation(take)
	var map: Dictionary = MAPS[src["map"]]
	var rest := {}
	var parent := {}
	var rest_q := {}
	for bi in sk.get_bone_count():
		var nm := sk.get_bone_name(bi)
		var gr := sk.get_bone_global_rest(bi)
		rest[nm] = gr.origin
		rest_q[nm] = gr.basis.get_rotation_quaternion()
		parent[nm] = sk.get_bone_name(sk.get_bone_parent(bi)) if sk.get_bone_parent(bi) >= 0 else ""
	# bones with no child joint in the map: give a direction along their own rest +Y
	for sj in map:
		if map[sj][1] == "" and rest.has(sj):
			rest[sj + "_end"] = rest[sj] + (rest_q[sj] as Quaternion) * Vector3(0, 0.1, 0)
	ap.play(take)
	var Gs: Array = []
	var roots: Array = []
	var hips := sk.find_bone(str(map.keys()[0]))
	var t := 0.0
	while t <= a.length + 0.0001:
		ap.seek(t, true)
		sk.force_update_all_bone_transforms()
		var G := {}
		for bi in sk.get_bone_count():
			var nm := sk.get_bone_name(bi)
			G[nm] = sk.get_bone_global_pose(bi).basis.get_rotation_quaternion() * (rest_q[nm] as Quaternion).inverse()
		Gs.append(G)
		roots.append(sk.get_bone_global_pose(hips).origin)
		t += 1.0 / FPS
	ap.stop()
	if bool(src.get("fbx", false)):  # one take per file: let it go
		(_glb_cache[src_id][0] as Node).queue_free()
		_glb_cache.erase(src_id)
	return {"G": Gs, "root": roots, "rest": rest, "parent": parent, "map": map, "units": 1.0,
		"loop": a.loop_mode != Animation.LOOP_NONE}


# ---- build --------------------------------------------------------------------------------------

func _build(name: String, job: Dictionary, d: Dictionary) -> Array:
	var map: Dictionary = d["map"]
	var rest: Dictionary = d["rest"]
	var Gs: Array = d["G"]
	var roots: Array = d["root"]
	var hips_j: String = map.keys()[0]
	var n := Gs.size()
	var dt := 1.0 / FPS
	# slice
	var a := clampi(int(round(float(job.get("from", 0.0)) / dt)), 0, n - 1)
	var b := clampi(int(round(float(job.get("to", 1e9)) / dt)), a, n - 1)
	var kind: String = job.get("kind", "action")
	if kind == "loop" and job.has("cycle"):  # pick a clean cycle automatically: [min_len, max_len] s
		# only where the actor is really moving (mocap takes also stand, turn and stop): the cycle's
		# travel speed must be near the take's best; then the closest start / end poses win
		var cy: Array = job["cycle"]
		var lo := int(float(cy[0]) / dt)
		var hi := int(float(cy[1]) / dt)
		var still := bool(job.get("still", false))
		var vert := bool(job.get("vertical", false))
		var vmax := 0.0
		if not still:
			for s in range(a, maxi(a + 1, b - lo)):
				var dv: Vector3 = (roots[s + lo] as Vector3) - (roots[s] as Vector3)
				vmax = maxf(vmax, (dv.y if vert else Vector2(dv.x, dv.z).length()) / (lo * dt))
		var best := [a, b, INF]
		for s in range(a, maxi(a + 1, b - lo), 2):
			for e in range(s + lo, mini(b, s + hi) + 1):
				if not still:
					var dv: Vector3 = (roots[e] as Vector3) - (roots[s] as Vector3)
					if (dv.y if vert else Vector2(dv.x, dv.z).length()) / ((e - s) * dt) < 0.85 * vmax:
						continue
				var dd := _pose_diff(Gs[s], Gs[e], map)
				if dd < best[2]:
					best = [s, e, dd]
		a = best[0]
		b = best[1]
	var count := b - a + 1
	# scale from leg length (hip -> ankle)
	var lu := _src_of(map, "LeftUpperLeg")
	var lf := _src_of(map, "LeftFoot")
	var src_leg: float = (rest[lu] as Vector3).distance_to(rest[lf])
	var tgt_leg := tgt.get_bone_global_rest(tgt.find_bone(str(tname["LeftUpperLeg"]))).origin.distance_to(tgt.get_bone_global_rest(tgt.find_bone(str(tname["LeftFoot"]))).origin)
	var scale := tgt_leg / src_leg
	# facing: rotate the clip so the character faces +Z (at the start, or along its travel)
	var face: String = job.get("face", "travel" if kind == "loop" else "start")
	var yaw := 0.0
	var travel: Vector3 = (roots[b] - roots[a]) * scale
	travel.y = 0.0
	var fwd0: Vector3 = (Gs[a][hips_j] as Quaternion) * _src_forward(d)
	if face == "travel" and travel.length() > 0.1:
		yaw = atan2(travel.x, travel.z) + deg_to_rad(float(job.get("travel_angle", 0.0)))
	elif face != "none":
		yaw = atan2(fwd0.x, fwd0.z)
	var Ryaw := Quaternion(Vector3.UP, -yaw)
	# alignment of rest directions
	var align := {}
	for sj in map:
		var tb: String = map[sj][0]
		var bi := tgt.find_bone(str(tname.get(tb, tb)))
		if bi < 0 or not rest.has(sj):
			continue
		var child: String = map[sj][1]
		if child == "" or not rest.has(child):
			child = sj + "_end"
		var d_s := ((rest.get(child, rest[sj]) as Vector3) - (rest[sj] as Vector3))
		var d_t := tgt.get_bone_global_rest(bi).basis.get_rotation_quaternion() * Vector3.UP
		align[sj] = Quaternion(d_t.normalized(), d_s.normalized()) if d_s.length() > 0.0001 else Quaternion.IDENTITY
		if tb in FLAT_REST:
			# every source stands flat-footed in its rest pose, but the ankle -> toe line points down at
			# different angles: copy the foot's change of orientation, not its direction
			align[sj] = Quaternion.IDENTITY
	var target_of := {}
	for sj in align:
		target_of[tgt.find_bone(str(tname.get(map[sj][0], map[sj][0])))] = sj
	var anim := Animation.new()
	anim.length = float(count - 1) * dt if kind != "loop" else float(count - 1) * dt
	anim.loop_mode = Animation.LOOP_LINEAR if kind == "loop" else Animation.LOOP_NONE
	# which bones get tracks: everything humanoid (fingers from the source or the relaxed hand)
	var drive: Array[int] = []
	var prof := SkeletonProfileHumanoid.new()
	for pn: String in tname:
		var bi := tgt.find_bone(str(tname[pn]))
		if bi >= 0 and pn != "Root" and not pn.ends_with("Eye") and pn != "Jaw" and not drive.has(bi):
			drive.append(bi)
	var tracks := {}
	for bi in drive:
		var tr := anim.add_track(Animation.TYPE_ROTATION_3D)
		anim.track_set_path(tr, NodePath("%s:%s" % [tgt_root.get_path_to(tgt), tgt.get_bone_name(bi)]))
		tracks[bi] = tr
	var hips := tgt.find_bone(str(tname["Hips"]))
	var ptr := anim.add_track(Animation.TYPE_POSITION_3D)
	anim.track_set_path(ptr, NodePath("%s:%s" % [tgt_root.get_path_to(tgt), tname["Hips"]]))
	# root motion: the ground travel goes on the root bone, the hips keep only their sway and height
	var root_motion := bool(job.get("root_motion", false))
	var rtr := -1
	var root_bi := tgt.find_bone(str(tname["Root"]))
	if root_motion and root_bi >= 0:
		rtr = anim.add_track(Animation.TYPE_POSITION_3D)
		anim.track_set_path(rtr, NodePath("%s:%s" % [tgt_root.get_path_to(tgt), tname["Root"]]))
	var hips_rest := tgt.get_bone_rest(hips).origin
	var trg_rest_g := {}
	for bi in tgt.get_bone_count():
		trg_rest_g[bi] = tgt.get_bone_global_rest(bi).basis.get_rotation_quaternion()
	var root0: Vector3 = roots[a]
	var in_place := bool(job.get("in_place", kind == "loop"))
	var lows := PackedFloat32Array()
	var frames_tg: Array = []
	var hip_pos: Array = []
	for k in count:
		var G: Dictionary = Gs[a + k]
		var tg := {}
		for bi in order:
			var parent := tgt.get_bone_parent(bi)
			var pg: Quaternion = tg[parent] if parent >= 0 else Quaternion.IDENTITY
			if target_of.has(bi):
				var sj: String = target_of[bi]
				tg[bi] = Ryaw * (G[sj] as Quaternion) * (align[sj] as Quaternion) * (trg_rest_g[bi] as Quaternion)
			elif relaxed.has(bi):
				tg[bi] = pg * (relaxed[bi] as Quaternion)
			else:
				tg[bi] = pg * tgt.get_bone_rest(bi).basis.get_rotation_quaternion()
		frames_tg.append(tg)
		var rp: Vector3 = ((roots[a + k] as Vector3) - root0) * scale
		if in_place:
			rp -= Vector3(travel.x, 0.0, travel.z) * (float(k) / float(maxi(1, count - 1))) if kind == "loop" else Vector3(rp.x, 0.0, rp.z)
		rp = Ryaw * Vector3(rp.x, 0.0, rp.z) + Vector3(0.0, rp.y, 0.0)
		var dy := ((roots[a + k] as Vector3).y - (roots[a] as Vector3).y) * scale
		if in_place and bool(job.get("vertical", false)):
			dy -= ((roots[b] as Vector3).y - (roots[a] as Vector3).y) * scale * float(k) / float(maxi(1, count - 1))
		var hp := Vector3(hips_rest.x + rp.x, hips_rest.y + dy, hips_rest.z + rp.z)
		hip_pos.append(hp)
		lows.append(_lowest(tg, hp))
	# "strike": turn the clip so its strike goes straight ahead (+Z) - fight takes often start in a bladed
	# stance, where the hips do not point where the blow goes
	if face == "strike":
		# the limb that moves out the most from where it started is the one striking
		var best_d := Vector3.ZERO
		var best_move := 0.0
		for c in ["LeftHand", "RightHand", "LeftFoot", "RightFoot"]:
			var cbi := tgt.find_bone(str(tname[c]))
			var rel0 := _tpos(frames_tg[0], hip_pos[0], cbi) - (hip_pos[0] as Vector3)
			rel0.y = 0.0
			for k in count:
				var rel := _tpos(frames_tg[k], hip_pos[k], cbi) - (hip_pos[k] as Vector3)
				rel.y = 0.0
				var mv := (rel - rel0).length()
				if mv > best_move:
					best_move = mv
					best_d = rel - rel0
		if best_d.length() > 0.05:
			var re := Quaternion(Vector3.UP, -atan2(best_d.x, best_d.z))
			for k in count:
				var tgk: Dictionary = frames_tg[k]
				for bi2 in tgk:
					tgk[bi2] = re * (tgk[bi2] as Quaternion)
				hip_pos[k] = hips_rest + re * ((hip_pos[k] as Vector3) - hips_rest)
	# ground: the planted foot on the floor. Standing clips: the lowest point over the clip; actions
	# that leave the ground keep their own rise above where they started.
	var ground: String = job.get("ground", "min")
	var lift := 0.0
	var sorted := lows.duplicate()
	sorted.sort()
	match ground:
		"min":
			lift = -sorted[0]
		"start":
			lift = -lows[0]
		"median_low":
			lift = -sorted[int(sorted.size() * 0.1)]
		"none":
			lift = 0.0
	lift += float(job.get("lift", 0.0))
	for k in count:
		var t := float(k) * dt
		var tg: Dictionary = frames_tg[k]
		for bi in drive:
			var parent := tgt.get_bone_parent(bi)
			var pg: Quaternion = tg[parent] if parent >= 0 else Quaternion.IDENTITY
			anim.rotation_track_insert_key(tracks[bi], t, (pg.inverse() * (tg[bi] as Quaternion)).normalized())
		var hk: Vector3 = (hip_pos[k] as Vector3) + Vector3.UP * lift
		if rtr >= 0:
			# straight-line travel to the root; the hips keep the difference (sway around the path)
			var path := hips_rest.lerp(hip_pos[count - 1] as Vector3, float(k) / float(maxi(1, count - 1)))
			var travel_k := Vector3(path.x - hips_rest.x, 0.0, path.z - hips_rest.z)
			anim.position_track_insert_key(rtr, t, travel_k)
			hk -= travel_k
		anim.position_track_insert_key(ptr, t, hk)
	var meta := {"kind": kind, "src": job["src"], "take": job["take"], "length": snappedf(anim.length, 0.01),
		"scale": snappedf(scale, 0.001)}
	if kind == "loop":
		# left heel strike: the first frame of the left foot's ground contact (synced gait blending)
		var lfoot := tgt.find_bone(str(tname["LeftFoot"]))
		var ys := PackedFloat32Array()
		for k in count:
			ys.append(_tpos(frames_tg[k], hip_pos[k], lfoot).y)
		var ymin := ys[0]
		for y in ys:
			ymin = minf(ymin, y)
		for k in count:
			var kp := (k - 1 + count) % count
			if ys[k] < ymin + 0.015 and ys[kp] >= ymin + 0.015:
				meta["lstrike"] = snappedf(k * dt, 0.001)
				break
	var T := anim.length
	if T > 0.0:
		meta["speed"] = snappedf(travel.length() / T, 0.01)
		meta["speed_v"] = snappedf(((roots[b] as Vector3).y - (roots[a] as Vector3).y) * scale / T, 0.01)
		var td := Ryaw * travel  # travel direction in the clip's facing frame (+Z forward, +X the character's left)
		if td.length() > 0.05:
			meta["dir"] = [snappedf(td.normalized().x, 0.01), snappedf(td.normalized().z, 0.01)]
	if job.has("tags"):
		meta["tags"] = job["tags"]
	if kind == "action" and bool(job.get("strike", false)):
		meta.merge(_strike_meta(frames_tg, hip_pos, dt, name))
	return [anim, meta]


func _src_of(map: Dictionary, profile_bone: String) -> String:
	for sj in map:
		if map[sj][0] == profile_bone:
			return sj
	return ""


func _src_forward(d: Dictionary) -> Vector3:
	# the source hips' forward at rest: from the rest positions, left hip -> right hip x up
	var map: Dictionary = d["map"]
	var rest: Dictionary = d["rest"]
	var l: Vector3 = rest[_src_of(map, "LeftUpperLeg")]
	var r: Vector3 = rest[_src_of(map, "RightUpperLeg")]
	return (r - l).cross(Vector3.UP).normalized() * -1.0 if (r - l).length() > 0.001 else Vector3.BACK


func _pose_diff(A: Dictionary, B: Dictionary, map: Dictionary) -> float:
	var s := 0.0
	for pb in ["LeftUpperLeg", "RightUpperLeg", "LeftLowerLeg", "RightLowerLeg", "LeftUpperArm", "RightUpperArm"]:
		var sj := _src_of(map, pb)
		if sj != "":
			s += (A[sj] as Quaternion).angle_to(B[sj])
	return s


## Lowest point of the feet (ankle / toes) for a target pose, hips at `hp`.
func _lowest(tg: Dictionary, hp: Vector3) -> float:
	var low := INF
	for b in ["LeftFoot", "RightFoot", "LeftToes", "RightToes"]:
		var bi := tgt.find_bone(str(tname[b]))
		low = minf(low, _tpos(tg, hp, bi).y - (0.06 if b.ends_with("Foot") else 0.02))
	return low


func _tpos(tg: Dictionary, hp: Vector3, bi: int) -> Vector3:
	var chain: Array[int] = []
	var j := bi
	var hips := tgt.find_bone(str(tname["Hips"]))
	while j >= 0 and j != hips:
		chain.push_front(j)
		j = tgt.get_bone_parent(j)
	var p := hp
	for c in chain:
		var parent := tgt.get_bone_parent(c)
		p += (tg[parent] as Quaternion) * tgt.get_bone_rest(c).origin
	return p


## A strike: which hand / foot, when it starts out, reaches furthest (the hit) and comes back.
func _strike_meta(frames_tg: Array, hip_pos: Array, dt: float, name: String) -> Dictionary:
	var n := name.to_lower()
	var feet := n.contains("kick") or n.contains("knee") or n.contains("stomp") or n.contains("sweep") or n.contains("roundhouse") \
		or n.contains("crescent") or n.contains("capoeira") or n.contains("geri")
	var cands: Array = []
	for side in ["Left", "Right"]:
		cands.append(side + ("Foot" if feet else "Hand"))
	var best := ["", -INF, 0]
	var series := {}
	for c in cands:
		var bi := tgt.find_bone(str(tname[c]))
		var sr := PackedFloat32Array()
		for k in frames_tg.size():
			var rel: Vector3 = _tpos(frames_tg[k], hip_pos[k], bi) - (hip_pos[k] as Vector3)
			var reach := rel.z + (0.5 * rel.y if feet else 0.0)
			sr.append(reach)
			if reach > best[1]:
				best = [c, reach, k]
		series[c] = sr
	var sr: PackedFloat32Array = series[best[0]]
	var thr := sr[0] + 0.25 * (sr[best[2]] - sr[0])
	var k0: int = best[2]
	while k0 > 0 and sr[k0] > thr:
		k0 -= 1
	var k1: int = best[2]
	while k1 < sr.size() - 1 and sr[k1] > thr:
		k1 += 1
	var limb := ("hand_" if str(best[0]).ends_with("Hand") else "foot_") + ("l" if str(best[0]).begins_with("Left") else "r")
	return {"limb": limb, "start": snappedf(k0 * dt, 0.01), "hit": snappedf(int(best[2]) * dt, 0.01),
		"back": snappedf(k1 * dt, 0.01), "reach": snappedf(sr[best[2]] - sr[0], 0.01)}


## A relaxed hand (fingers gently curled) sampled from a reference clip, for sources without fingers.
func _make_relaxed(spec: Dictionary) -> void:
	var src: Dictionary = jobs["sources"][spec["src"]]
	var d := await _sample_glb(spec["src"], src, spec["take"], {})
	if d.is_empty():
		return
	var map: Dictionary = d["map"]
	var G: Dictionary = d["G"][0]
	var rest: Dictionary = d["rest"]
	# finger local rotations relative to the hand, via the same align as a clip
	var tg := {}
	for sj in map:
		var tb: String = map[sj][0]
		var bi := tgt.find_bone(str(tname.get(tb, tb)))
		if bi < 0 or not rest.has(sj):
			continue
		var child: String = map[sj][1] if map[sj][1] != "" else sj + "_end"
		var d_s: Vector3 = (rest.get(child, rest[sj]) as Vector3) - (rest[sj] as Vector3)
		var rg := tgt.get_bone_global_rest(bi).basis.get_rotation_quaternion()
		var al := Quaternion((rg * Vector3.UP).normalized(), d_s.normalized()) if d_s.length() > 0.0001 else Quaternion.IDENTITY
		tg[bi] = (G[sj] as Quaternion) * al * rg
	for bi in order:
		var bn := tgt.get_bone_name(bi)
		var is_finger := false
		for f in FINGERS:
			if bn.contains(f):
				is_finger = true
		var parent := tgt.get_bone_parent(bi)
		if is_finger and tg.has(bi) and tg.has(parent):
			relaxed[bi] = ((tg[parent] as Quaternion).inverse() * (tg[bi] as Quaternion)).normalized()
	print("relaxed hand: %d finger bones from %s" % [relaxed.size(), spec["take"]])
