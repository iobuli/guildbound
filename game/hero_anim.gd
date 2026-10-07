class_name HeroAnim
extends Node3D
## The visible character: the Universal Base Characters body (CC0) with hair, weapons in the hands, and an
## AnimationTree over the clip library in assets/anims (Mixamo animations retargeted onto this body by
## tools/retarget.gd). Animation drives the skeleton directly; nothing is simulated in between.
##
## Tree, bottom to top:
##   loco    BlendSpace2D of the current stance's idle / walk / run / strafe / back loops (picked by
##           their measured travel direction), the stance's sprint on top
##   crouch  BlendSpace2D of the crouch walk set
##   mode    Transition: ground / crouch / air
##   upper   Blend2, spine and up: a held pose over the legs (guard, aiming)
##   action  Blend2 over two cross-faded slots, whole body: attacks, dodges, casts, hits, deaths.
##           Their ground travel comes out as root motion (the root bone), for the controller to move by.

const BASE := "res://assets/characters/base/"
const ANIMS := "res://assets/anims/"
const WEAPONS := "res://assets/weapons/"
const RUN_REF := 4.2     # m/s the stance's run clips are blended against (they cover ~3.5-4.5)
const WALK_REF := 1.4

static var meta := {}
static var data := {}

var inst: Node3D
var skeleton: Skeleton3D
var tree: AnimationTree
var bt: AnimationNodeBlendTree
var stance := {}
var stance_id := ""
var _slot := 0
var _act_left := 0.0
var _act_w := 0.0
var _act_fade := 0.15
var act_clip := ""
var _hold := ""
var _hold_w := 0.0
var _hands := {}          # "r" / "l" -> BoneAttachment3D
var _weapon: Node3D
var _offhand: Node3D
var _swing := {}          # clip -> [active start, active end] (s at speed 1)
var _sk_path := ""


static func load_data() -> void:
	if meta.is_empty():
		meta = JSON.parse_string(FileAccess.get_file_as_string(ANIMS + "clips.json"))
		data = JSON.parse_string(FileAccess.get_file_as_string("res://data/weapons.json"))


func build(body: String = "Superhero_Male_FullBody", hair: String = "Hair_SimpleParted") -> void:
	load_data()
	inst = (load(BASE + body + ".gltf") as PackedScene).instantiate()
	inst.rotation.y = PI  # glTF faces +Z; the controller's forward is -Z
	add_child(inst)
	skeleton = inst.find_child("Skeleton3D", true, false)
	_sk_path = str(inst.get_path_to(skeleton))
	if hair != "":
		_attach_hair(hair)
	for side in ["r", "l"]:
		var ba := BoneAttachment3D.new()
		ba.bone_name = "hand_" + side
		skeleton.add_child(ba)
		_hands[side] = ba
	_build_tree()


func _attach_hair(hair: String) -> void:
	var h: Node3D = (load(BASE + hair + ".gltf") as PackedScene).instantiate()
	for mi in h.find_children("*", "MeshInstance3D", true, false):
		mi.owner = null
		mi.get_parent().remove_child(mi)
		skeleton.add_child(mi)
		(mi as MeshInstance3D).skeleton = NodePath("..")
		mi.transform = Transform3D.IDENTITY
	h.free()


# ---- tree --------------------------------------------------------------------------------------

func _clip(n: String) -> AnimationNodeAnimation:
	var a := AnimationNodeAnimation.new()
	a.animation = n
	return a


func _upper_filter(node: AnimationNode) -> void:
	node.filter_enabled = true
	var spine := skeleton.find_bone("spine_01")
	for i in skeleton.get_bone_count():
		var j := i
		while j >= 0 and j != spine:
			j = skeleton.get_bone_parent(j)
		if j == spine:
			node.set_filter_path(NodePath(_sk_path + ":" + skeleton.get_bone_name(i)), true)


func _build_tree() -> void:
	var lib := AnimationLibrary.new()
	for n: String in meta:
		var a: Animation = load(ANIMS + n + ".res")
		if a != null:
			lib.add_animation(n, a)
	bt = AnimationNodeBlendTree.new()
	var loco := AnimationNodeBlendSpace2D.new()
	loco.blend_mode = AnimationNodeBlendSpace2D.BLEND_MODE_INTERPOLATED
	bt.add_node("loco", loco)
	bt.add_node("loco_ts", AnimationNodeTimeScale.new())
	bt.connect_node("loco_ts", 0, "loco")
	var crouch := AnimationNodeBlendSpace2D.new()
	bt.add_node("crouch", crouch)
	bt.add_node("crouch_ts", AnimationNodeTimeScale.new())
	bt.connect_node("crouch_ts", 0, "crouch")
	bt.add_node("air", _clip(str(data["_air"]["fall"])))
	var mode := AnimationNodeTransition.new()
	mode.input_count = 3
	for i in 3:
		mode.set_input_name(i, ["ground", "crouch", "air"][i])
	mode.xfade_time = 0.2
	bt.add_node("mode", mode)
	bt.connect_node("mode", 0, "loco_ts")
	bt.connect_node("mode", 1, "crouch_ts")
	bt.connect_node("mode", 2, "air")
	var upper := AnimationNodeBlend2.new()
	_upper_filter(upper)
	bt.add_node("upper", upper)
	bt.add_node("hold", _clip("LOCO_idle"))
	bt.connect_node("upper", 0, "mode")
	bt.connect_node("upper", 1, "hold")
	var act := AnimationNodeTransition.new()
	act.input_count = 2
	act.set_input_name(0, "a")
	act.set_input_name(1, "b")
	act.xfade_time = 0.12
	bt.add_node("act", act)
	for sl in ["a", "b"]:
		bt.add_node("act_" + sl, _clip("LOCO_idle"))
		bt.add_node("act_ts_" + sl, AnimationNodeTimeScale.new())
		bt.connect_node("act_ts_" + sl, 0, "act_" + sl)
	bt.connect_node("act", 0, "act_ts_a")
	bt.connect_node("act", 1, "act_ts_b")
	var action := AnimationNodeBlend2.new()
	bt.add_node("action", action)
	bt.connect_node("action", 0, "upper")
	bt.connect_node("action", 1, "act")
	bt.connect_node("output", 0, "action")
	tree = AnimationTree.new()
	inst.add_child(tree)
	tree.add_animation_library("", lib)
	tree.tree_root = bt
	tree.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_PHYSICS
	tree.root_motion_track = NodePath(_sk_path + ":root")
	tree.active = true
	_fill_crouch(crouch)
	tree.set("parameters/action/blend_amount", 0.0)
	tree.set("parameters/upper/blend_amount", 0.0)


## The loops whose names start with `prefix`, sorted into idle / directions by their travel.
static func loco_set(prefix: String) -> Dictionary:
	var out := {}
	for n: String in meta:
		if not n.begins_with(prefix):
			continue
		var m: Dictionary = meta[n]
		var low := n.to_lower()
		if str(m.get("kind", "")) != "loop" or low.contains("crouch") or low.contains("block") or low.contains("fall") or low.contains("aim") or low.contains("sneak"):
			continue
		var spd := float(m.get("speed", 0.0))
		if spd < 0.2:
			if low.contains("idle") and (not out.has("idle") or n.length() < str(out["idle"]).length()):
				out["idle"] = n
			continue
		var d: Array = m.get("dir", [0, 1])
		var key := ""
		if float(d[1]) > 0.7:
			key = "f"
		elif float(d[1]) < -0.7:
			key = "b"
		elif float(d[0]) > 0.7:
			key = "l"
		elif float(d[0]) < -0.7:
			key = "r"
		else:
			continue
		key += "_run" if spd >= 2.3 else "_walk"
		if not out.has(key) or abs(spd - (RUN_REF if key.ends_with("run") else WALK_REF)) < abs(float(meta[out[key]]["speed"]) - (RUN_REF if key.ends_with("run") else WALK_REF)):
			out[key] = n
	return out


## Point the locomotion blend space at a stance's loops (fall back on the plain locomotion set).
func set_stance(id: String) -> void:
	stance_id = id
	stance = data[id]
	var mine := loco_set(str(stance["loco"]))
	var base := loco_set("LOCO_")
	for k in base:
		if not mine.has(k):
			mine[k] = base[k]
	for k in loco_set("MAG_"):
		if not mine.has(k):
			mine[k] = loco_set("MAG_")[k]
	var loco: AnimationNodeBlendSpace2D = bt.get_node("loco")
	while loco.get_blend_point_count() > 0:
		loco.remove_blend_point(0)
	loco.min_space = Vector2(-2.0, -2.0)
	loco.max_space = Vector2(2.0, 3.0)
	var pts := {"idle": Vector2.ZERO, "f_walk": Vector2(0, 1), "f_run": Vector2(0, 2), "b_walk": Vector2(0, -1), "b_run": Vector2(0, -2),
		"l_walk": Vector2(-1, 0), "l_run": Vector2(-2, 0), "r_walk": Vector2(1, 0), "r_run": Vector2(2, 0)}
	for k in pts:
		if mine.has(k):
			loco.add_blend_point(_clip(mine[k]), pts[k], -1, k)
	loco.add_blend_point(_clip(str(data["_sprint"])), Vector2(0, 3), -1, "sprint")
	_equip_models()


func _fill_crouch(cs: AnimationNodeBlendSpace2D) -> void:
	var pre := str(data["_crouch"])
	var pts := {"idle": Vector2.ZERO, "walk_forward": Vector2(0, 1), "walk_back": Vector2(0, -1), "walk_left": Vector2(-1, 0), "walk_right": Vector2(1, 0)}
	cs.min_space = Vector2(-1, -1)
	cs.max_space = Vector2(1, 1)
	for k in pts:
		if meta.has(pre + k):
			cs.add_blend_point(_clip(pre + k), pts[k], -1, k)


func _equip_models() -> void:
	for n in [_weapon, _offhand]:
		if n != null and is_instance_valid(n):
			n.queue_free()
	_weapon = null
	_offhand = null
	var hand := str(stance.get("hand", "r"))
	var w := str(stance.get("weapon", ""))
	if w == "bow":
		_weapon = _make_bow()
	elif w != "":
		_weapon = _load_model(w)
	if _weapon != null:
		(_hands[hand] as Node3D).add_child(_weapon)
	var o := str(stance.get("offhand", ""))
	if o != "":
		_offhand = _load_model(o)
		(_hands["l" if hand == "r" else "r"] as Node3D).add_child(_offhand)


## KayKit weapons (CC0) hold their grip at the origin with the blade up +Y, at chibi scale: sized to
## real ones and turned so the blade leaves the fist on the thumb side.
const WEAPON_SCALE := {"sword_1handed": 0.5, "sword_2handed": 0.6, "dagger": 0.35, "staff": 0.75, "wand": 0.6,
	"axe_1handed": 0.55, "axe_2handed": 0.6, "shield_round": 0.62, "shield_badge": 0.62, "shield_spikes": 0.62}
var grip_rot := Vector3(90, 0, 0)     # weapon +Y -> out of the fist (hand bone frame), degrees
var shield_rot := Vector3(0, 0, 90)


func _load_model(n: String) -> Node3D:
	if not ResourceLoader.exists(WEAPONS + n + ".gltf"):
		return null
	var holder := Node3D.new()
	var m: Node3D = (load(WEAPONS + n + ".gltf") as PackedScene).instantiate()
	m.scale = Vector3.ONE * float(WEAPON_SCALE.get(n, 0.5))
	holder.add_child(m)
	if n.begins_with("shield"):
		holder.rotation_degrees = shield_rot
		holder.position = Vector3(0.0, 0.06, 0.0)
	else:
		holder.rotation_degrees = grip_rot
		holder.position = Vector3(0.0, 0.07, 0.0)
	return holder


func _make_bow() -> Node3D:
	# a longbow from the grip: two limbs bending back to the tips, and the string between them
	var root := Node3D.new()
	var wood := StandardMaterial3D.new()
	wood.albedo_color = Color(0.42, 0.26, 0.13)
	var tips := [Vector3(0.0, 0.72, -0.16), Vector3(0.0, -0.72, -0.16)]
	for tip: Vector3 in tips:
		for half in 2:  # each limb in two pieces so it reads as a curve
			var a: Vector3 = Vector3.ZERO if half == 0 else Vector3(0.0, tip.y * 0.55, tip.z * 0.25)
			var b: Vector3 = Vector3(0.0, tip.y * 0.55, tip.z * 0.25) if half == 0 else tip
			var seg := MeshInstance3D.new()
			var cm := CylinderMesh.new()
			cm.top_radius = 0.011
			cm.bottom_radius = 0.016
			cm.height = a.distance_to(b)
			cm.material = wood
			seg.mesh = cm
			seg.position = (a + b) * 0.5
			seg.basis = Basis(Quaternion(Vector3.UP, (b - a).normalized()))
			root.add_child(seg)
	var string := MeshInstance3D.new()
	var sm := CylinderMesh.new()
	sm.top_radius = 0.0025
	sm.bottom_radius = 0.0025
	sm.height = 1.44
	string.mesh = sm
	string.position = Vector3(0.0, 0.0, -0.16)
	root.add_child(string)
	var holder := Node3D.new()
	holder.add_child(root)
	holder.rotation_degrees = grip_rot
	holder.position = Vector3(0.0, 0.07, 0.0)
	return holder


# ---- driving -----------------------------------------------------------------------------------

## Locomotion: `local` = velocity in the character's frame (x right, y forward) in m/s.
func set_motion(local: Vector2, sprinting: bool, mode: String, delta: float) -> void:
	var sp := local.length()
	var bx := 0.0
	var by := 0.0
	var gait := 0.0
	if sp > 0.05:
		gait = sp / WALK_REF if sp <= WALK_REF else 1.0 + clampf((sp - WALK_REF) / (RUN_REF - WALK_REF), 0.0, 1.0)
		if sprinting and local.y > 0.5 * sp:
			gait = 3.0
		var d := local / sp
		bx = d.x * gait
		by = d.y * gait
	var cur: Vector2 = tree.get("parameters/loco/blend_position")
	var tgt := Vector2(bx, by)
	tree.set("parameters/loco/blend_position", cur.lerp(tgt, 1.0 - exp(-delta * 10.0)))
	var nominal := WALK_REF if gait <= 1.0 else (RUN_REF if gait <= 2.0 else 5.6)
	var scale := clampf(sp / nominal, 0.75, 1.6) if gait > 0.5 else 1.0
	tree.set("parameters/loco_ts/scale", scale)
	tree.set("parameters/crouch/blend_position", Vector2(clampf(local.x / 1.3, -1, 1), clampf(local.y / 1.3, -1, 1)))
	tree.set("parameters/crouch_ts/scale", clampf(sp / 1.3, 0.7, 1.6) if sp > 0.2 else 1.0)
	if tree.get("parameters/mode/current_state") != mode:
		tree.set("parameters/mode/transition_request", mode)
	# held pose over the upper body, and the action layer fading out at the end of its clip
	_hold_w = move_toward(_hold_w, 1.0 if _hold != "" else 0.0, delta * 8.0)
	tree.set("parameters/upper/blend_amount", _hold_w)
	if _act_left > 0.0:
		_act_left -= delta
	var want := 1.0 if _act_left > _act_fade else maxf(0.0, _act_left / _act_fade)
	_act_w = move_toward(_act_w, want, delta * (14.0 if want > _act_w else 1.0 / maxf(0.05, _act_fade)))
	tree.set("parameters/action/blend_amount", _act_w)
	if _act_w <= 0.001 and _act_left <= 0.0:
		act_clip = ""


## Play a whole-body action; returns its duration at that speed. `fade` = blend out at the end.
func play(clip: String, speed: float = 1.0, fade: float = 0.18) -> float:
	if not tree.has_animation(clip):
		push_warning("no clip " + clip)
		return 0.0
	_slot = 1 - _slot
	var sl: String = ["a", "b"][_slot]
	(bt.get_node("act_" + sl) as AnimationNodeAnimation).animation = clip
	tree.set("parameters/act_ts_%s/scale" % sl, speed)
	tree.set("parameters/act/transition_request", sl)
	_act_left = tree.get_animation(clip).length / maxf(0.01, speed)
	_act_fade = fade
	act_clip = clip
	return _act_left


## End the action early (blend back to locomotion over `fade`).
func stop_action(fade: float = 0.15) -> void:
	_act_left = minf(_act_left, fade)
	_act_fade = fade


func acting() -> bool:
	return _act_left > 0.0


## Hold a pose over the upper body while the legs keep walking ("" lets go).
func hold(clip: String) -> void:
	if clip == _hold:
		return
	_hold = clip
	if clip != "" and tree.has_animation(clip):
		(bt.get_node("hold") as AnimationNodeAnimation).animation = clip


## This physics frame's root motion in world space (the action's own ground travel).
func root_motion_world() -> Vector3:
	var rm: Vector3 = tree.get_root_motion_position()
	return inst.global_basis * rm


# ---- queries -----------------------------------------------------------------------------------

func bone_pos(n: String) -> Vector3:
	var b := skeleton.find_bone(n)
	return skeleton.global_transform * skeleton.get_bone_global_pose(b).origin if b >= 0 else global_position


func chest_position() -> Vector3:
	return bone_pos("spine_03")


func head_position() -> Vector3:
	return bone_pos("Head")


## The held weapon's blade as a segment (hilt -> tip) in world space, or the limb for kicks / punches.
func strike_segment(limb: String) -> Array:
	var weapon_hand := "hand_" + str(stance.get("hand", "r"))
	var armed := _weapon != null and (limb == "" or limb == weapon_hand)
	if not armed:
		var b := limb if limb != "" else "hand_r"
		var p := bone_pos(b)
		var q := bone_pos(b.replace("hand", "lowerarm").replace("foot", "calf"))
		return [p, p + (p - q).normalized() * 0.12]
	var g := _weapon.global_transform
	var length := 1.15 if stance_id == "greatsword" else (0.33 if stance_id == "dagger" else (0.95 if stance_id == "magic" else 0.7))
	var up := g.basis.y.normalized()
	return [g.origin + up * 0.08, g.origin + up * length]


## A held weapon's tip (staff / wand / bow) in world space - where spells and arrows leave from.
func weapon_tip() -> Vector3:
	return strike_segment("")[1]


## When a swing is dangerous: the stretch of the clip where the striking hand / foot moves fastest
## (measured once from the clip itself). Returns [start, end] in seconds at speed 1.
func swing_window(clip: String, limb: String) -> Vector2:
	var key := clip + "|" + limb
	if _swing.has(key):
		return _swing[key]
	var a := tree.get_animation(clip)
	var bone := skeleton.find_bone(limb)
	var hips := skeleton.find_bone("pelvis")
	var prev := Vector3.ZERO
	var speeds := PackedFloat32Array()
	var fwd := PackedFloat32Array()
	var dt := 1.0 / 30.0
	var t := 0.0
	while t <= a.length:
		var pose := _pose_at(a, t)
		var p: Vector3 = (pose[bone] as Transform3D).origin - (pose[hips] as Transform3D).origin
		speeds.append((p - prev).length() / dt if t > 0.0 else 0.0)
		fwd.append(p.z)  # clips face +Z: how far in front of the hips the striking hand / foot is
		prev = p
		t += dt
	var peak := 0
	for k in speeds.size():
		if speeds[k] > speeds[peak]:
			peak = k
	var thr := speeds[peak] * 0.45
	var k0 := peak
	while k0 > 0 and speeds[k0 - 1] > thr:
		k0 -= 1
	var k1 := peak
	while k1 < speeds.size() - 1 and speeds[k1 + 1] > thr:
		k1 += 1
	# overheads and thrusts land when the hand is furthest forward, which can come after the fastest
	# part of the swing (the wind-up): the window covers both
	var kf := 0
	for k in fwd.size():
		if fwd[k] > fwd[kf]:
			kf = k
	k0 = mini(k0, kf - 2)
	k1 = maxi(k1, kf + 3)
	var w := Vector2(maxf(0.0, (k0 - 1) * dt), minf(a.length, (k1 + 1) * dt))
	if w.y - w.x < 0.1:
		w = Vector2(w.x - 0.05, w.y + 0.05)
	_swing[key] = w
	return w


## Bone poses (skeleton space) of a clip at time t, from the clip's tracks over the rest pose.
func _pose_at(a: Animation, t: float) -> Array:
	var local: Array[Transform3D] = []
	for i in skeleton.get_bone_count():
		local.append(skeleton.get_bone_rest(i))
	for tr in a.get_track_count():
		var bone := skeleton.find_bone(a.track_get_path(tr).get_concatenated_subnames())
		if bone < 0:
			continue
		var x := local[bone]
		match a.track_get_type(tr):
			Animation.TYPE_POSITION_3D:
				x.origin = a.position_track_interpolate(tr, t)
			Animation.TYPE_ROTATION_3D:
				x.basis = Basis(a.rotation_track_interpolate(tr, t))
		local[bone] = x
	var glob: Array = []
	glob.resize(local.size())
	for i in local.size():
		var chain: Array[int] = []
		var j := i
		while j >= 0 and glob[j] == null:
			chain.push_front(j)
			j = skeleton.get_bone_parent(j)
		for c in chain:
			var pa := skeleton.get_bone_parent(c)
			glob[c] = local[c] if pa < 0 else (glob[pa] as Transform3D) * local[c]
	return glob
