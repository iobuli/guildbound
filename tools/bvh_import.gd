extends SceneTree
## Offline converter: motion-capture BVH -> Godot Animation resources retargeted onto our Quaternius
## 65-bone rig. Two sources:
##   CMU (free, no restrictions)              Godot --headless --path <guildbound> --script res://tools/bvh_import.gd
##   Motifect combat pack (free for use in projects; the raw files are NOT redistributed and stay in
##   D:/Godot/assets/motifect)                ... --script res://tools/bvh_import.gd -- --set=motifect
##
## Retarget: both skeletons are Y-up, facing +Z, left = +X. Each mapped bone copies the source bone's
## *direction* and twist: target_global = source_global * align * target_rest, where `align` turns the
## target's rest bone direction onto the source's rest bone direction (CMU's rest pose has splayed legs).

var SRC := "D:/Godot/assets/cmu/"
var OUT := "res://assets/animations/cmu/"
var LEG := ["LeftUpLeg", "LeftFoot"]   # source hip / ankle joints (sets the scale)
const CHAR := "res://assets/characters/quaternius_ubc/Superhero_Male_FullBody.gltf"
const FPS := 30.0

## source joint -> [target bone, source joint whose rest position gives the bone direction]
var MAP := {}
const MAP_CMU := {
	"Hips": ["pelvis", "Spine"], "LowerBack": ["spine_01", "Spine"], "Spine": ["spine_02", "Spine1"],
	"Spine1": ["spine_03", "Neck1"], "Neck1": ["neck_01", "Head"], "Head": ["Head", "Head_end"],
	"LeftShoulder": ["clavicle_l", "LeftArm"], "LeftArm": ["upperarm_l", "LeftForeArm"],
	"LeftForeArm": ["lowerarm_l", "LeftHand"], "LeftHand": ["hand_l", "LeftHandIndex1"],
	"RightShoulder": ["clavicle_r", "RightArm"], "RightArm": ["upperarm_r", "RightForeArm"],
	"RightForeArm": ["lowerarm_r", "RightHand"], "RightHand": ["hand_r", "RightHandIndex1"],
	"LeftUpLeg": ["thigh_l", "LeftLeg"], "LeftLeg": ["calf_l", "LeftFoot"], "LeftFoot": ["foot_l", "LeftToeBase"],
	"LeftToeBase": ["ball_l", "LeftToeBase_end"],
	"RightUpLeg": ["thigh_r", "RightLeg"], "RightLeg": ["calf_r", "RightFoot"], "RightFoot": ["foot_r", "RightToeBase"],
	"RightToeBase": ["ball_r", "RightToeBase_end"],
}

## Motifect skeleton (Mixamo-like names, fingers included: fists come from the capture)
const MAP_MOTIFECT := {
	"Hips": ["pelvis", "Spine1"], "Spine1": ["spine_01", "Spine2"], "Spine2": ["spine_02", "Chest"],
	"Chest": ["spine_03", "Neck1"], "Neck1": ["neck_01", "Head"], "Head": ["Head", "HeadEnd"],
	"LeftShoulder": ["clavicle_l", "LeftArm"], "LeftArm": ["upperarm_l", "LeftForeArm"],
	"LeftForeArm": ["lowerarm_l", "LeftHand"], "LeftHand": ["hand_l", "LeftHandMiddle1"],
	"RightShoulder": ["clavicle_r", "RightArm"], "RightArm": ["upperarm_r", "RightForeArm"],
	"RightForeArm": ["lowerarm_r", "RightHand"], "RightHand": ["hand_r", "RightHandMiddle1"],
	"LeftLeg": ["thigh_l", "LeftShin"], "LeftShin": ["calf_l", "LeftFoot"], "LeftFoot": ["foot_l", "LeftToeBase"],
	"LeftToeBase": ["ball_l", "LeftToeEnd"],
	"RightLeg": ["thigh_r", "RightShin"], "RightShin": ["calf_r", "RightFoot"], "RightFoot": ["foot_r", "RightToeBase"],
	"RightToeBase": ["ball_r", "RightToeEnd"],
	"LeftHandIndex1": ["index_01_l", "LeftHandIndex2"], "LeftHandIndex2": ["index_02_l", "LeftHandIndex3"], "LeftHandIndex3": ["index_03_l", "LeftHandIndex4"],
	"LeftHandMiddle1": ["middle_01_l", "LeftHandMiddle2"], "LeftHandMiddle2": ["middle_02_l", "LeftHandMiddle3"], "LeftHandMiddle3": ["middle_03_l", "LeftHandMiddle4"],
	"LeftHandRing1": ["ring_01_l", "LeftHandRing2"], "LeftHandRing2": ["ring_02_l", "LeftHandRing3"], "LeftHandRing3": ["ring_03_l", "LeftHandRing4"],
	"LeftHandPinky1": ["pinky_01_l", "LeftHandPinky2"], "LeftHandPinky2": ["pinky_02_l", "LeftHandPinky3"], "LeftHandPinky3": ["pinky_03_l", "LeftHandPinky4"],
	"LeftHandThumb1": ["thumb_01_l", "LeftHandThumb2"], "LeftHandThumb2": ["thumb_02_l", "LeftHandThumb3"], "LeftHandThumb3": ["thumb_03_l", "LeftHandThumbEnd"],
	"RightHandIndex1": ["index_01_r", "RightHandIndex2"], "RightHandIndex2": ["index_02_r", "RightHandIndex3"], "RightHandIndex3": ["index_03_r", "RightHandIndex4"],
	"RightHandMiddle1": ["middle_01_r", "RightHandMiddle2"], "RightHandMiddle2": ["middle_02_r", "RightHandMiddle3"], "RightHandMiddle3": ["middle_03_r", "RightHandMiddle4"],
	"RightHandRing1": ["ring_01_r", "RightHandRing2"], "RightHandRing2": ["ring_02_r", "RightHandRing3"], "RightHandRing3": ["ring_03_r", "RightHandRing4"],
	"RightHandPinky1": ["pinky_01_r", "RightHandPinky2"], "RightHandPinky2": ["pinky_02_r", "RightHandPinky3"], "RightHandPinky3": ["pinky_03_r", "RightHandPinky4"],
	"RightHandThumb1": ["thumb_01_r", "RightHandThumb2"], "RightHandThumb2": ["thumb_02_r", "RightHandThumb3"], "RightHandThumb3": ["thumb_03_r", "RightHandThumbEnd"],
}

## clip name -> [bvh file, kind]
##   getup: trimmed to the rise from the ground, root relative to start, ends facing +Z
##   loop:  one clean cycle, travel removed (in place), faces its travel/facing direction
var CLIPS := {}
const CLIPS_CMU := {
	"CMU_GetUp_FaceDown": ["140_01", "getup"],
	"CMU_GetUp_FaceDown2": ["140_02", "getup"],
	"CMU_GetUp_A": ["139_16", "getup"],
	"CMU_GetUp_B": ["139_17", "getup"],
	"CMU_GetUp_Side": ["140_03", "getup"],
	"CMU_GetUp_Back": ["140_08", "getup"],
	"CMU_Climb": ["13_33", "loop_climb"],
	"CMU_StairsUp": ["83_27", "loop_walk"],
	"CMU_PushHeavy": ["82_06", "loop_walk"],
}

var J := []       # joints: {name, parent, offset, channels}
var jidx := {}
var frames: Array = []
var frame_time := 0.0083333


func _init() -> void:
	MAP = MAP_CMU
	CLIPS = CLIPS_CMU
	if "--set=motifect" in OS.get_cmdline_user_args():
		SRC = "D:/Godot/assets/motifect/"
		OUT = "res://assets/animations/motifect/"
		MAP = MAP_MOTIFECT
		LEG = ["LeftLeg", "LeftFoot"]
		CLIPS = {}
		for f in DirAccess.get_files_at(SRC):
			if f.ends_with(".bvh"):
				CLIPS["MF_" + f.get_basename()] = [f.get_basename(), "action"]
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(OUT))
	var inst: Node = (load(CHAR) as PackedScene).instantiate()
	var sk: Skeleton3D = inst.find_child("Skeleton3D", true, false)
	var meta := {}
	for name in CLIPS:
		var spec: Array = CLIPS[name]
		_parse(SRC + spec[0] + ".bvh")
		var result := _convert(name, spec[1], sk)
		var anim: Animation = result[0]
		ResourceSaver.save(anim, OUT + name + ".res")
		meta[name] = result[1]
		print("CLIP %-20s %5.2fs  %s" % [name, anim.length, JSON.stringify(result[1])])
	var f := FileAccess.open(OUT + "clips.json", FileAccess.WRITE)
	f.store_string(JSON.stringify(meta, "  "))
	inst.free()
	quit()


# ---- BVH parsing ---------------------------------------------------------------------------------

func _parse(path: String) -> void:
	J.clear()
	jidx.clear()
	frames.clear()
	var text := FileAccess.get_file_as_string(path)
	var words := PackedStringArray()
	for w in text.replace("\t", " ").replace("\r", " ").replace("\n", " ").split(" ", false):
		words.append(w)
	var i := 0
	var stack: Array = []
	var cur := -1
	while i < words.size():
		var w := words[i]
		if w == "ROOT" or w == "JOINT":
			var j := {"name": words[i + 1], "parent": cur, "offset": Vector3.ZERO, "channels": []}
			J.append(j)
			cur = J.size() - 1
			jidx[j["name"]] = cur
			i += 2
		elif w == "End":
			var parent_name: String = J[cur]["name"]
			var j := {"name": parent_name + "_end", "parent": cur, "offset": Vector3.ZERO, "channels": []}
			J.append(j)
			cur = J.size() - 1
			jidx[j["name"]] = cur
			i += 2
		elif w == "{":
			stack.append(cur)
			i += 1
		elif w == "}":
			stack.pop_back()
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
	# Frames: N   Frame Time: t
	var nframes := int(words[i + 1])
	frame_time = float(words[i + 4])
	i += 5
	var per := 0
	for j in J:
		per += j["channels"].size()
	for f in nframes:
		var row := PackedFloat32Array()
		row.resize(per)
		for k in per:
			row[k] = float(words[i + k])
		i += per
		frames.append(row)


## Global rotations and root position for one frame.
func _pose(row: PackedFloat32Array) -> Array:
	var G := []
	G.resize(J.size())
	var root_pos := Vector3.ZERO
	var c := 0
	for ji in J.size():
		var j: Dictionary = J[ji]
		var q := Quaternion.IDENTITY
		for ch in j["channels"]:
			var v := row[c]
			c += 1
			match ch:
				"Xposition":
					root_pos.x = v
				"Yposition":
					root_pos.y = v
				"Zposition":
					root_pos.z = v
				"Xrotation":
					q = q * Quaternion(Vector3.RIGHT, deg_to_rad(v))
				"Yrotation":
					q = q * Quaternion(Vector3.UP, deg_to_rad(v))
				"Zrotation":
					q = q * Quaternion(Vector3.BACK, deg_to_rad(v))
		G[ji] = q if j["parent"] < 0 else (G[j["parent"]] * q)
	return [G, root_pos]


func _rest_pos(ji: int) -> Vector3:
	var p := Vector3.ZERO
	var j := ji
	while j >= 0:
		p += J[j]["offset"]
		j = J[j]["parent"]
	return p


# ---- retarget -----------------------------------------------------------------------------------

func _convert(name: String, kind: String, sk: Skeleton3D) -> Array:
	var step := maxi(1, int(round((1.0 / FPS) / frame_time)))
	var poses: Array = []
	for f in range(1, frames.size(), step):  # frame 0 is often a calibration pose
		poses.append(_pose(frames[f]))
	var n := poses.size()
	var dt := frame_time * step

	# scale: leg length (hip to ankle) target / source
	var src_leg := _rest_pos(jidx[LEG[0]]).distance_to(_rest_pos(jidx[LEG[1]]))
	var tgt_leg := sk.get_bone_global_rest(sk.find_bone("thigh_l")).origin.distance_to(sk.get_bone_global_rest(sk.find_bone("foot_l")).origin)
	var scale := tgt_leg / src_leg

	var heights := PackedFloat32Array()
	var pos := []
	for p in poses:
		pos.append(p[1] * scale)
		heights.append(p[1].y * scale)

	# ---- choose frames ----
	var a := 0
	var b := n - 1
	var meta := {"kind": kind}
	if kind == "action":
		pass  # the whole clip
	elif kind == "getup":
		var lo := INF
		var hi := -INF
		for h in heights:
			lo = minf(lo, h)
			hi = maxf(hi, h)
		var top := 0
		for k in n:
			if heights[k] >= hi - 0.01:
				top = k
				break
		var rise_start := 0
		for k in range(top, -1, -1):
			if heights[k] < lo + 0.12 * (hi - lo):
				rise_start = k
				break
		var rise_end := top
		for k in range(rise_start, n):
			if heights[k] > lo + 0.97 * (hi - lo):
				rise_end = k
				break
		a = maxi(0, rise_start - int(0.25 / dt))
		b = mini(n - 1, rise_end + int(0.45 / dt))
	else:
		# pick where the action is: steepest climb (climb, stairs) or steadiest travel (walk/push),
		# then the most similar pose 0.7..2.5 s later so the loop closes cleanly
		var win := int(1.0 / dt)
		var best_a := int(n * 0.4)
		var best_s := -INF
		for k in range(0, n - win - int(2.5 / dt)):
			var d: Vector3 = pos[k + win] - pos[k]
			var sc := d.y if (kind == "loop_climb" or name.contains("Stairs")) else Vector2(d.x, d.z).length()
			if sc > best_s:
				best_s = sc
				best_a = k
		a = best_a
		var best := INF
		for k in range(a + int(0.7 / dt), mini(n - 1, a + int(2.5 / dt))):
			var dd := _pose_dist(poses[a][0], poses[k][0])
			if dd < best:
				best = dd
				b = k
	var count := b - a + 1
	# standing height reference so the clip's hips sit where our rig's hips stand
	var ref := 0.0
	if kind == "action":
		ref = heights[a]  # starts standing (falls / knockdowns go down from there)
	elif kind == "getup":
		for k in range(a, b + 1):
			ref = maxf(ref, heights[k])
	else:
		for k in range(a, b + 1):
			ref += heights[k] / float(count)

	# ---- yaw alignment: rotate the whole clip so it faces +Z ----
	var yaw := 0.0
	if kind == "action":
		yaw = _facing_yaw(poses[a][0])          # faces +Z at the start
	elif kind == "getup":
		yaw = _facing_yaw(poses[b][0])          # end standing, facing +Z
	elif kind == "loop_climb":
		yaw = _facing_yaw(poses[a][0])          # face the wall
	else:
		var trav: Vector3 = pos[b] - pos[a]
		trav.y = 0.0
		yaw = atan2(trav.x, trav.z) if trav.length() > 0.05 else _facing_yaw(poses[a][0])
	var Ryaw := Quaternion(Vector3.UP, -yaw)

	# ---- root track ----
	var root_track := []
	var p0: Vector3 = pos[a]
	var travel: Vector3 = pos[b] - pos[a]
	for k in count:
		var t := float(k) / float(maxi(1, count - 1))
		var p: Vector3 = pos[a + k] - p0
		if kind != "getup":
			p -= travel * t                     # in place: remove travel, keep the bob and sway
			p.y += pos[a].y
		else:
			p.y += pos[a].y
		root_track.append(Ryaw * Vector3(p.x, 0.0, p.z) + Vector3(0.0, p.y, 0.0))
	var T := float(count - 1) * dt
	if kind == "action":
		meta.merge(_strike_meta(name, poses, pos, a, b, Ryaw, scale, dt))
	elif kind != "getup":
		var h := Vector3(travel.x, 0.0, travel.z).length()
		meta["speed_h"] = snappedf(h / T, 0.01)
		meta["speed_v"] = snappedf(travel.y / T, 0.01)
	else:
		# where the head points (skeleton space) at the start: recovery lines this up with the ragdoll
		var ga: Array = poses[a][0]
		var head: Vector3 = _jpos(ga, pos[a], jidx["Head"], scale) - pos[a]
		head = Ryaw * head
		meta["head_dir"] = [snappedf(head.x, 0.01), snappedf(head.z, 0.01)]
		var chest_fwd: Vector3 = Ryaw * (ga[jidx["Spine1"]] * Vector3(0, 0, 1))
		meta["face_up"] = chest_fwd.y > 0.0
	meta["length"] = snappedf(T, 0.01)

	# ---- target rest data ----
	var tgt_rest_g := {}
	for bi in sk.get_bone_count():
		tgt_rest_g[bi] = sk.get_bone_global_rest(bi).basis.get_rotation_quaternion()
	var align := {}
	for sj in MAP:
		var bi := sk.find_bone(MAP[sj][0])
		var d_s := (_rest_pos(jidx[MAP[sj][1]]) - _rest_pos(jidx[sj])).normalized()
		var d_t: Vector3 = (tgt_rest_g[bi] * Vector3.UP).normalized()
		align[sj] = Quaternion(d_t, d_s) if d_s.length() > 0.5 else Quaternion.IDENTITY

	# ---- build the animation ----
	var anim := Animation.new()
	anim.length = T
	anim.loop_mode = Animation.LOOP_NONE if kind == "getup" else Animation.LOOP_LINEAR
	var tracks := {}
	for sj in MAP:
		var tr := anim.add_track(Animation.TYPE_ROTATION_3D)
		anim.track_set_path(tr, NodePath("Armature/Skeleton3D:" + MAP[sj][0]))
		tracks[sj] = tr
	var ptr := anim.add_track(Animation.TYPE_POSITION_3D)
	anim.track_set_path(ptr, NodePath("Armature/Skeleton3D:pelvis"))
	var pelvis_rest := sk.get_bone_rest(sk.find_bone("pelvis")).origin
	var target_of := {}
	for sj in MAP:
		target_of[sk.find_bone(MAP[sj][0])] = sj
	var prev_y := 0.0
	var end_xz: Vector3 = root_track[count - 1] if kind == "getup" else Vector3.ZERO  # get-ups end standing on the origin
	end_xz.y = 0.0
	for k in count:
		var G: Array = poses[a + k][0]
		var tg := {}
		for bi in sk.get_bone_count():
			var parent := sk.get_bone_parent(bi)
			if target_of.has(bi):
				var sj: String = target_of[bi]
				tg[bi] = Ryaw * G[jidx[sj]] * align[sj] * tgt_rest_g[bi]
			else:
				var pg: Quaternion = tg[parent] if parent >= 0 else Quaternion.IDENTITY
				tg[bi] = pg * sk.get_bone_rest(bi).basis.get_rotation_quaternion()
		var t := float(k) * dt
		for sj in MAP:
			var bi := sk.find_bone(MAP[sj][0])
			var parent := sk.get_bone_parent(bi)
			var pg: Quaternion = tg[parent] if parent >= 0 else Quaternion.IDENTITY
			anim.rotation_track_insert_key(tracks[sj], t, (pg.inverse() * tg[bi]).normalized())
		var rp: Vector3 = root_track[k]
		var y := rp.y + (pelvis_rest.y - ref)
		if kind == "getup":
			# ground contact: put the lowest body point on the floor (source hip heights are unreliable)
			var low := _lowest(sk, tg)
			var want := -low + 0.03
			y = want if k == 0 else move_toward(prev_y, want, 1.5 * dt)
			prev_y = y
		anim.position_track_insert_key(ptr, t, Vector3(pelvis_rest.x + rp.x - end_xz.x, y, pelvis_rest.z + rp.z - end_xz.z))
	return [anim, meta]


## For a strike: which hand / foot does it, and when does it start out, reach full extension (the hit)
## and come back. Measured as how far that end reaches forward of the hips.
func _strike_meta(name: String, poses: Array, pos: Array, a: int, b: int, Ryaw: Quaternion, scale: float, dt: float) -> Dictionary:
	var n := name.to_lower()
	var cands: Array = []
	var feet := n.contains("kick") or n.contains("knee") or n.contains("stomp")
	for side in ["Left", "Right"]:
		var named := n.contains("left") or n.contains("right")
		if named and not n.contains(side.to_lower()):
			continue
		cands.append(side + ("Foot" if feet else "Hand"))
	var best_j := ""
	var best_v := -INF
	var best_k := a
	var series := {}
	for j in cands:
		var sr := PackedFloat32Array()
		for k in range(a, b + 1):
			var G: Array = poses[k][0]
			var rel: Vector3 = Ryaw * (_jpos(G, pos[k], jidx[j], scale) - _jpos(G, pos[k], jidx["Hips"], scale))
			var reach := rel.z + (0.5 * rel.y if feet else 0.0)  # kicks also rise
			sr.append(reach)
			if reach > best_v:
				best_v = reach
				best_j = j
				best_k = k
		series[j] = sr
	if best_j == "":
		return {}
	var sr: PackedFloat32Array = series[best_j]
	var rest := sr[0]
	var peak := sr[best_k - a]
	var thr := rest + 0.25 * (peak - rest)
	var k0 := best_k - a
	while k0 > 0 and sr[k0] > thr:
		k0 -= 1
	var k1 := best_k - a
	while k1 < sr.size() - 1 and sr[k1] > thr:
		k1 += 1
	var limb := ("hand_" if best_j.ends_with("Hand") else "foot_") + ("l" if best_j.begins_with("Left") else "r")
	return {"limb": limb, "start": snappedf(k0 * dt, 0.01), "hit": snappedf((best_k - a) * dt, 0.01),
		"back": snappedf(k1 * dt, 0.01), "reach": snappedf(peak - rest, 0.01)}


func _jpos(G: Array, root: Vector3, ji: int, scale: float) -> Vector3:
	# world position of a joint (scaled) for a posed frame
	var chain: Array = []
	var j := ji
	while j >= 0:
		chain.push_front(j)
		j = J[j]["parent"]
	var p := root
	for k in range(1, chain.size()):
		var parent: int = J[chain[k]]["parent"]
		p += G[parent] * (J[chain[k]]["offset"] * scale)
	return p


func _facing_yaw(G: Array) -> float:
	var f: Vector3 = G[jidx["Hips"]] * Vector3(0, 0, 1)
	return atan2(f.x, f.z)


func _pose_dist(A: Array, Bq: Array) -> float:
	var d := 0.0
	for sj in ["LeftUpLeg", "RightUpLeg", "LeftLeg", "RightLeg", "LeftArm", "RightArm", "LeftForeArm", "RightForeArm"]:
		var qa: Quaternion = A[jidx[sj]]
		var qb: Quaternion = Bq[jidx[sj]]
		d += qa.angle_to(qb)
	return d


## Lowest point of the posed target body, relative to the pelvis at y = 0 (forward kinematics on our rig).
func _lowest(sk: Skeleton3D, tg: Dictionary) -> float:
	var P := {}
	var low := INF
	for bi in sk.get_bone_count():
		var parent := sk.get_bone_parent(bi)
		if parent < 0:
			P[bi] = Vector3.ZERO
		elif sk.get_bone_name(bi) == "pelvis":
			P[bi] = Vector3.ZERO
		else:
			P[bi] = P[parent] + (tg[parent] * sk.get_bone_rest(bi).origin)
		var nm := sk.get_bone_name(bi)
		if nm.begins_with("ball") or nm.begins_with("foot") or nm.begins_with("calf") or nm.begins_with("hand") or nm == "Head" or nm.begins_with("lowerarm") or nm == "pelvis":
			low = minf(low, P[bi].y)
	return low
