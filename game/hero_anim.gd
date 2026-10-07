class_name HeroAnim
extends Node3D
## The visible character: the Universal Base Characters body (CC0) with hair, weapons in the hands, and an
## AnimationTree over the clip library in assets/anims (Mixamo animations retargeted onto this body by
## tools/retarget.gd). Animation drives the skeleton directly; nothing is simulated in between.
##
## Tree, bottom to top:
##   idle    the stance's idle loop, at its own pace
##   loco    BlendSpace2D of the stance's walk / run / strafe / back loops on two diamond rings (walk at 1,
##           run at 2, the sprint at 3 ahead), picked by their measured travel direction. Every loop is
##           normalised to a one-second cycle that starts on its left heel strike and they all advance
##           together, so blending a walk into a run never crosses the legs; the cycle rate comes from the
##           ground speed over the blended stride (feet planted, no sliding). `move` mixes idle -> loco.
##   crouch  BlendSpace2D of the crouch walk set
##   mode    Transition: ground / crouch / air
##   upper   Blend2, spine and up: a held pose over the legs (guard, aiming)
##   uact    Blend2, spine and up: one-shot upper-body actions over whatever the legs do (play_upper)
##   action  three cross-faded slots, whole body: attacks, dodges, casts, hits, deaths.
##           Their ground travel comes out as root motion (the root bone), for the controller to move by.
## The tree is evaluated every RENDERED frame (smooth limbs on a 144 Hz screen); its root motion is
## gathered between physics ticks and handed to the controller as a velocity (root_motion_world).
## A skeleton modifier leans the body into turns and with acceleration (a few degrees) and keeps the feet
## from sinking through the floor where two poses are mixed.

const BASE := "res://assets/characters/base/"
const ANIMS := "res://assets/anims/"
const WEAPONS := "res://assets/weapons/"
const RUN_REF := 4.2     # m/s the stance's run clips are picked against (they cover ~3.5-4.5)
const WALK_REF := 1.4
const BLEND_HZ := 12.0   # how fast the locomotion blend follows the speed (critically damped spring)
const MOVE_HZ := 34.0    # ... the idle <-> moving mix

static var meta := {}
static var data := {}
static var _feet := {}   # clip -> ground speed its planted feet imply (m/s), measured once

var inst: Node3D
var skeleton: Skeleton3D
var tree: AnimationTree
var bt: AnimationNodeBlendTree
var stance := {}
var stance_id := ""
const SLOTS := ["a", "b", "c"]
const USLOTS := ["a", "b"]
const ACT_XFADE := 0.2     # seconds a new action takes to blend in over the last one
const WARP := 3.0          # a chained hit plays its wind-up this much faster (no seeking: no pops)
var _warp_left := 0.0      # seconds of fast wind-up left on the current slot
var _speed := 1.0
var _slot := 0
var _slot_w := [1.0, 0.0, 0.0]  # weight of each action slot (they sum to 1)
var _act_left := 0.0
var _act_w := 0.0
var _act_fade := 0.15
var act_clip := ""
var _hold := ""
var _hold_w := 0.0
var _uslot := 0
var _uslot_w := [1.0, 0.0]
var _u_left := 0.0
var _u_w := 0.0
var upper_clip := ""
var _hands := {}          # "r" / "l" -> BoneAttachment3D
var _weapon: Node3D
var _offhand: Node3D
var _swing := {}          # clip -> [active start, active end] (s at speed 1)
var _sk_path := ""
static var default_body_style := "mannequin"
var body_style := default_body_style   # "mannequin" (capsules on the bones) or "skinned" (the UBC mesh); set before build()
var hurt: Array = []                   # BodyParts segments bound to this skeleton (hurtboxes, mannequin, armour)
var parts := {}                        # segment name -> its mannequin MeshInstance3D
var _attach := {}                      # bone name -> BoneAttachment3D
var _armour: Array[MeshInstance3D] = []
var _armour_sig := -1
## Per-segment visual proportions (mannequin + armour only; the hurtboxes keep their size): a segment
## name or its side-less stem ("hand", "thigh") -> radius scale; UNIFORM_PARTS also scale in length.
var proportions: Dictionary = {}
const UNIFORM_PARTS := ["head", "hand"]
const GOBLIN_PROPORTIONS := {"head": 1.35, "hand": 1.3, "forearm": 0.8, "upperarm": 0.8, "chest": 1.1, "shin": 0.85, "thigh": 0.85}
var _mat_body: StandardMaterial3D
var _mat_joint: StandardMaterial3D
var _tint := Color.WHITE
const DUMMY_COL := Color(0.6, 0.6, 0.59)    # matte mid grey (reads light under the sun)
const JOINT_COL := Color(0.34, 0.34, 0.36)
# locomotion state (targets come from the controller at physics rate, everything moves per frame)
var _want := Vector2.ZERO          # local ground velocity: x right, y forward (m/s), this physics tick
var _want_prev := Vector2.ZERO     # ... the tick before (drawn frames sit between the two)
var _sprinting := false
var _mode := "ground"
var _ring := {}                    # loco point name -> [feet speed m/s, clip length s]
var _bp := Vector2(0, 1)           # loco blend position (from _dir and _gait)
var _dir := 0.0                    # its direction: 0 ahead, + to the right (rad)
var _gait := 1.0                   # its ring: 1 walk, 2 run, 3 sprint
var _gait_v := 0.0
var grounded := true               # set by the controller: feet may plant (not in the air)
var _move := 0.0                   # idle -> moving mix (damped)
var _move_v := 0.0
var _rate := 1.0
# root motion gathered by the frames since the last physics tick
var _rm_acc := Vector3.ZERO
var _rm_t := 0.0
var _rm_v := Vector3.ZERO
var _rm_tick := -1
var _eval_dt := 0.0                # the time the tree's next evaluation covers
# level of detail
const FAR_LOD := 32.0              # m from the camera: beyond, ~30 Hz animation and no foot planting
var _far := false
var _far_t := 0.0
# lean
var _lean: PoseMod
var _yaw_prev := NAN      # NAN until the first frame
var _lean_pos := Vector3.ZERO
var _lean_vel := Vector3.ZERO
var _lean_acc := Vector3.ZERO


## Runs after the animation every frame (the skeleton restores the animated pose before the next one, so
## nothing accumulates):
##   lean    roll of the whole body about the feet into a turn, pitch of the spine with acceleration
##   lock    a foot that comes down and holds still stays exactly where it landed in the world until the
##           clip lifts it (or the body has moved off it): no skating while blends shift or the body turns
##   ground  no foot below the floor (two poses mixed mid-stride bend a planted leg through it)
## Feet are placed by bending the leg (two-bone IK; the foot keeps its own rotation).
## Skeleton space: the clips face +Z, +X is the character's left, the floor is y = 0.
class PoseMod extends SkeletonModifier3D:
	const LOCK_SPEED := 0.45     # m/s: an animated foot slower than this near the floor plants
	const LOCK_REACH := 0.3      # m: a planted foot this far from where the clip has it lets go (safety)
	const LOCK_BLEND := 0.1      # s to blend into / out of a plant (out: while the clip lifts the foot)
	var roll := 0.0
	var pitch := 0.0
	var floor_y := 0.086     # the ankle bone's lowest allowed height (the clips' contacts: 0.088-0.117)
	var feet_lock := true
	var _root := 0
	var _spine := -1
	var _legs: Array = []    # [thigh, calf, foot] per side
	var _last_origin := Vector3.INF
	var _st: Array = []      # per side: {"on": planted, "w": plant (world), "b": plant weight, "prev": clip's foot last
	                         # frame (world), "shown": where the foot was drawn last frame (world)}

	func setup(sk: Skeleton3D) -> void:
		_root = maxi(0, sk.find_bone("root"))
		_spine = sk.find_bone("spine_01")
		for side in ["l", "r"]:
			var leg := [sk.find_bone("thigh_" + side), sk.find_bone("calf_" + side), sk.find_bone("foot_" + side)]
			if leg.min() >= 0:
				_legs.append(leg)
				_st.append({"on": false, "w": Vector3.ZERO, "b": 0.0, "prev": Vector3.INF, "shown": Vector3.ZERO})

	func _process_modification_with_delta(delta: float) -> void:
		var sk := get_skeleton()
		if sk == null:
			return
		if absf(roll) > 0.0003:
			sk.set_bone_pose_rotation(_root, Quaternion(Vector3(0, 0, 1), roll) * sk.get_bone_pose_rotation(_root))
		if absf(pitch) > 0.0003 and _spine >= 0:
			var pg := sk.get_bone_global_pose(sk.get_bone_parent(_spine)).basis.get_rotation_quaternion()
			var r := Quaternion(Vector3(1, 0, 0), pitch)
			sk.set_bone_pose_rotation(_spine, pg.inverse() * r * pg * sk.get_bone_pose_rotation(_spine))
		var x := sk.get_global_transform_interpolated()
		var xi := x.affine_inverse()
		var jumped := x.origin.distance_to(_last_origin) > 1.0  # teleported: drop the plants
		_last_origin = x.origin
		for i in _legs.size():
			var leg: Array = _legs[i]
			var st: Dictionary = _st[i]
			var p := sk.get_bone_global_pose(int(leg[2])).origin
			var w := x * p
			var prev: Vector3 = st["prev"]
			var spd := (w - prev).length() / maxf(delta, 0.0001) if prev != Vector3.INF else INF
			st["prev"] = w
			var near := p.y < floor_y + 0.025
			var lw: Vector3 = st["w"]
			if jumped:
				st["on"] = false
				st["b"] = 0.0
			elif bool(st["on"]):
				if not near or lw.distance_to(w) > LOCK_REACH:
					st["on"] = false
			elif feet_lock and near and spd < LOCK_SPEED:
				st["on"] = true
				# still letting go of the last plant: plant where the foot is shown, not where the clip has it
				st["w"] = (st["shown"] as Vector3) if float(st["b"]) > 0.001 else w
			st["b"] = move_toward(float(st["b"]), 1.0 if bool(st["on"]) else 0.0, delta / LOCK_BLEND)
			var t := p
			if float(st["b"]) > 0.0:
				var bl := float(st["b"])
				bl = bl * bl * (3.0 - 2.0 * bl)
				t = p.lerp(xi * (st["w"] as Vector3), bl)
			t.y = maxf(t.y, floor_y)
			st["shown"] = x * t
			if t.distance_squared_to(p) > 0.0000001:
				_reach(sk, int(leg[0]), int(leg[1]), int(leg[2]), t)

	## Bend the leg so the ankle lands on `t` (skeleton space); the knee stays on its side.
	func _reach(sk: Skeleton3D, th: int, ca: int, ft: int, t: Vector3) -> void:
		var th_g := sk.get_bone_global_pose(th)
		var ca_g := sk.get_bone_global_pose(ca)
		var ft_g := sk.get_bone_global_pose(ft)
		var a := th_g.origin
		var b := ca_g.origin
		var c := ft_g.origin
		var l1 := a.distance_to(b)
		var l2 := b.distance_to(c)
		var reach := clampf(a.distance_to(t), absf(l1 - l2) + 0.001, l1 + l2 - 0.001)
		var u := (t - a).normalized()
		t = a + u * reach
		var v := (b - a) - u * (b - a).dot(u)  # the knee keeps its side of the hip -> foot line
		if v.length() < 0.0001:
			v = u.cross(Vector3(1, 0, 0))
		v = v.normalized()
		var x := (l1 * l1 - l2 * l2 + reach * reach) / (2.0 * reach)
		var k := a + u * x + v * sqrt(maxf(0.0, l1 * l1 - x * x))
		var r1 := Quaternion((b - a).normalized(), (k - a).normalized())
		var r2 := Quaternion((r1 * (c - b)).normalized(), (t - k).normalized())
		var q_th := th_g.basis.get_rotation_quaternion()
		var q_ca := ca_g.basis.get_rotation_quaternion()
		var q_ft := ft_g.basis.get_rotation_quaternion()
		var pg := sk.get_bone_global_pose(sk.get_bone_parent(th)).basis.get_rotation_quaternion()
		var th_new := r1 * q_th
		var ca_new := r2 * r1 * q_ca
		sk.set_bone_pose_rotation(th, pg.inverse() * th_new)
		sk.set_bone_pose_rotation(ca, th_new.inverse() * ca_new)
		sk.set_bone_pose_rotation(ft, ca_new.inverse() * q_ft)


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
	if hair != "" and body_style != "mannequin":
		_attach_hair(hair)
	for side in ["r", "l"]:
		_hands[side] = _bone_attach("hand_" + side)
	hurt = BodyParts.bind_all(skeleton)
	if body_style == "mannequin":
		_build_mannequin()
	_build_tree()
	var watch := Timer.new()  # equipment can change from anywhere (bag, shop, quest): look now and then
	watch.wait_time = 0.25
	watch.autostart = true
	watch.timeout.connect(_watch_armour)
	add_child(watch)


## Tint the body and hair (goblins: green). Each surface gets its own copy of its material; the
## mannequin's two materials are this body's own already. Armour keeps its colour.
func tint(c: Color) -> void:
	_tint = c
	if _mat_body != null:
		_mat_body.albedo_color = DUMMY_COL * c
		_mat_joint.albedo_color = JOINT_COL * c
	for mi in inst.find_children("*", "MeshInstance3D", true, false):
		var m3 := mi as MeshInstance3D
		if m3.mesh == null or m3.has_meta("body_part") or m3.has_meta("armour"):
			continue
		for k in m3.mesh.get_surface_count():
			var base := m3.get_active_material(k) as BaseMaterial3D
			if base == null:
				continue
			var own := base.duplicate() as BaseMaterial3D
			own.albedo_color = base.albedo_color * c
			m3.set_surface_override_material(k, own)


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


## A locomotion loop on the shared gait timeline: one cycle = one second, starting on the left heel
## strike, so every loop in a blend space is at the same point of the stride at the same time.
func _gait_clip(n: String) -> AnimationNodeAnimation:
	var a := _clip(n)
	var m: Dictionary = meta.get(n, {})
	var length := float(m.get("length", 1.0))
	a.use_custom_timeline = true
	a.timeline_length = 1.0
	a.stretch_time_scale = true
	a.loop_mode = Animation.LOOP_LINEAR
	a.start_offset = fposmod(float(m.get("lstrike", 0.0)) / maxf(0.01, length), 1.0)
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


## A clip slot that restarts on request and plays at a set speed: <id> -> <id>_seek -> <id>_ts.
func _slot_chain(id: String) -> String:
	bt.add_node(id, _clip("LOCO_idle"))
	bt.add_node(id + "_seek", AnimationNodeTimeSeek.new())
	bt.connect_node(id + "_seek", 0, id)
	bt.add_node(id + "_ts", AnimationNodeTimeScale.new())
	bt.connect_node(id + "_ts", 0, id + "_seek")
	return id + "_ts"


func _build_tree() -> void:
	var lib := AnimationLibrary.new()
	for n: String in meta:
		var a: Animation = load(ANIMS + n + ".res")
		if a != null:
			lib.add_animation(n, a)
	bt = AnimationNodeBlendTree.new()
	bt.add_node("idle", _clip("LOCO_idle"))
	var loco := AnimationNodeBlendSpace2D.new()
	loco.blend_mode = AnimationNodeBlendSpace2D.BLEND_MODE_INTERPOLATED
	loco.sync = true  # every loop advances, weighted or not: they stay on the shared stride
	bt.add_node("loco", loco)
	bt.add_node("loco_ts", AnimationNodeTimeScale.new())
	bt.connect_node("loco_ts", 0, "loco")
	var move := AnimationNodeBlend2.new()
	bt.add_node("move", move)
	bt.connect_node("move", 0, "idle")
	bt.connect_node("move", 1, "loco_ts")
	var crouch := AnimationNodeBlendSpace2D.new()
	bt.add_node("crouch", crouch)
	bt.add_node("crouch_ts", AnimationNodeTimeScale.new())
	bt.connect_node("crouch_ts", 0, "crouch")
	bt.add_node("air", _clip(str(data["_air"]["fall"])))
	var mode := AnimationNodeTransition.new()
	mode.input_count = 3
	for i in 3:
		mode.set_input_name(i, ["ground", "crouch", "air"][i])
		mode.set_input_reset(i, false)  # coming back to the ground keeps the stride where it was
	mode.xfade_time = 0.2
	bt.add_node("mode", mode)
	bt.connect_node("mode", 0, "move")
	bt.connect_node("mode", 1, "crouch_ts")
	bt.connect_node("mode", 2, "air")
	var upper := AnimationNodeBlend2.new()
	_upper_filter(upper)
	bt.add_node("upper", upper)
	bt.add_node("hold", _clip("LOCO_idle"))
	bt.connect_node("upper", 0, "mode")
	bt.connect_node("upper", 1, "hold")
	# one-shot upper-body actions (casting on the move): two cross-faded slots over the spine and up
	bt.add_node("u_x", AnimationNodeBlend2.new())
	bt.connect_node("u_x", 0, _slot_chain("u_a"))
	bt.connect_node("u_x", 1, _slot_chain("u_b"))
	var uact := AnimationNodeBlend2.new()
	_upper_filter(uact)
	bt.add_node("uact", uact)
	bt.connect_node("uact", 0, "upper")
	bt.connect_node("uact", 1, "u_x")
	# three action slots mixed by weight: a new action takes the quietest slot and fades in over the
	# others, so a combo chained faster than the cross-fade never snaps a still-visible pose away
	bt.add_node("act_ab", AnimationNodeBlend2.new())
	bt.add_node("act", AnimationNodeBlend2.new())
	var ts := {}
	for sl in SLOTS:
		ts[sl] = _slot_chain("act_" + sl)
	bt.connect_node("act_ab", 0, ts["a"])
	bt.connect_node("act_ab", 1, ts["b"])
	bt.connect_node("act", 0, "act_ab")
	bt.connect_node("act", 1, ts["c"])
	var action := AnimationNodeBlend2.new()
	bt.add_node("action", action)
	bt.connect_node("action", 0, "uact")
	bt.connect_node("action", 1, "act")
	bt.connect_node("output", 0, "action")
	tree = AnimationTree.new()
	inst.add_child(tree)
	tree.add_animation_library("", lib)
	tree.tree_root = bt
	# evaluated every rendered frame, not at the 60 Hz physics rate: limbs move on every frame of a
	# 144 Hz screen. Root motion is collected per frame and handed to physics (root_motion_world).
	tree.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_IDLE
	tree.root_motion_track = NodePath(_sk_path + ":root")
	tree.mixer_applied.connect(_on_mixer_applied)
	tree.active = true
	process_priority = -1  # drive the tree's parameters before it evaluates this frame
	_fill_crouch(crouch)
	tree.set("parameters/action/blend_amount", 0.0)
	tree.set("parameters/upper/blend_amount", 0.0)
	tree.set("parameters/uact/blend_amount", 0.0)
	tree.set("parameters/move/blend_amount", 0.0)
	_lean = PoseMod.new()
	_lean.setup(skeleton)
	skeleton.add_child(_lean)


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
	loco.min_space = Vector2(-2.5, -2.5)
	loco.max_space = Vector2(2.5, 3.5)
	# two diamond rings: walks at 1, runs at 2 (a diagonal sits on the edge between its two neighbours),
	# the sprint ahead at 3. The idle is not in here: `move` mixes it in below walking pace.
	var pts := {"f_walk": Vector2(0, 1), "f_run": Vector2(0, 2), "b_walk": Vector2(0, -1), "b_run": Vector2(0, -2),
		"l_walk": Vector2(-1, 0), "l_run": Vector2(-2, 0), "r_walk": Vector2(1, 0), "r_run": Vector2(2, 0)}
	_ring.clear()
	mine["sprint"] = str(data["_sprint"])
	pts["sprint"] = Vector2(0, 3)
	for k: String in pts:
		if not mine.has(k):
			continue
		var clip := str(mine[k])
		loco.add_blend_point(_gait_clip(clip), pts[k], -1, k)
		_ring[k] = [feet_speed(clip), float(meta[clip]["length"])]
	(bt.get_node("idle") as AnimationNodeAnimation).animation = str(mine.get("idle", "LOCO_idle"))
	_equip_models()


## An NPC standing about: the idle becomes `clip` (a looping library clip).
func set_idle(clip: String) -> void:
	(bt.get_node("idle") as AnimationNodeAnimation).animation = clip


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


# ---- body: mannequin, armour ---------------------------------------------------------------------

func _bone_attach(bone: String) -> BoneAttachment3D:
	if _attach.has(bone):
		return _attach[bone]
	var ba := BoneAttachment3D.new()
	ba.bone_name = bone
	skeleton.add_child(ba)
	_attach[bone] = ba
	return ba


func _seg(n: String) -> Dictionary:
	for h: Dictionary in hurt:
		if h["n"] == n:
			return h
	return {}


## A mesh riding a bone, placed by its rest-space transform. `tag` marks it: "body_part" or "armour".
func _add_part(bone: int, mesh: Mesh, rest: Transform3D, tag: String, value: String) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	mi.mesh = mesh
	mi.transform = skeleton.get_bone_global_rest(bone).affine_inverse() * rest
	mi.set_meta(tag, value)
	_bone_attach(skeleton.get_bone_name(bone)).add_child(mi)
	return mi


static func _capsule(r: float, length: float, mat: Material) -> CapsuleMesh:
	var cm := CapsuleMesh.new()
	cm.radius = r
	cm.height = length + 2.0 * r
	cm.radial_segments = 20
	cm.rings = 6
	cm.material = mat
	return cm


static func _sphere(r: float, mat: Material, half: bool = false) -> SphereMesh:
	var sm := SphereMesh.new()
	sm.radius = r
	sm.height = r if half else 2.0 * r
	sm.is_hemisphere = half
	sm.radial_segments = 20
	sm.rings = 8 if half else 12
	sm.material = mat
	return sm


## The prototype body: the skinned mesh hidden, a smooth matte capsule per segment and a darker sphere at
## each joint, all riding the skeleton (a test dummy: it fits whatever pose the clips put it in).
## The visual scale of a segment (1.0 unless `proportions` says otherwise).
func prop(seg: String) -> float:
	return float(proportions.get(seg, proportions.get(seg.trim_suffix("_l").trim_suffix("_r"), 1.0)))


## Set the proportions after build: the mannequin is rebuilt and the armour refits.
func set_proportions(pr: Dictionary) -> void:
	proportions = pr.duplicate()
	if skeleton == null or _mat_body == null:
		return  # build() will use them
	for mi in skeleton.find_children("*", "MeshInstance3D", true, false):
		if mi.has_meta("body_part"):
			mi.free()
	parts.clear()
	_build_mannequin()
	_armour_sig = -1
	_watch_armour()


func _build_mannequin() -> void:
	for mi in inst.find_children("*", "MeshInstance3D", true, false):
		(mi as MeshInstance3D).visible = false
	_mat_body = StandardMaterial3D.new()
	_mat_body.albedo_color = DUMMY_COL * _tint
	_mat_body.roughness = 0.72
	_mat_body.metallic_specular = 0.35
	_mat_joint = StandardMaterial3D.new()
	_mat_joint.albedo_color = JOINT_COL * _tint
	_mat_joint.roughness = 0.6
	for h: Dictionary in hurt:
		var pa: Vector3 = h["rest_a"]
		var pb: Vector3 = h["rest_b"]
		var k := prop(str(h["n"]))
		var stem := str(h["n"]).trim_suffix("_l").trim_suffix("_r")
		var length := pa.distance_to(pb) * (k if UNIFORM_PARTS.has(stem) else 1.0)
		parts[h["n"]] = _add_part(int(h["bone"]), _capsule(float(h["r"]) * k, length, _mat_body), BodyParts.along(pa, pb), "body_part", str(h["n"]))
	for j: Array in BodyParts.joints():
		var bi := skeleton.find_bone(str(j[0]))
		var seg := _seg(str(j[1]))
		if bi < 0 or seg.is_empty():
			continue
		var at := Transform3D(Basis(), skeleton.get_bone_global_rest(bi).origin)
		_add_part(bi, _sphere(float(seg["r"]) * prop(str(j[1])) * float(j[2]), _mat_joint), at, "body_part", "joint")
	var head := _seg("head")
	if not head.is_empty():  # a dark visor band, so the dummy's facing reads at a glance
		var r := float(head["r"]) * prop("head")
		var c: Vector3 = (head["rest_a"] as Vector3).lerp(head["rest_b"], 0.7) + Vector3(0, 0, r * 0.78)
		var visor := Transform3D(Basis(Vector3.FORWARD, PI * 0.5), c)
		_add_part(int(head["bone"]), _capsule(r * 0.3, r * 0.75, _mat_joint), visor, "body_part", "visor")


func _watch_armour() -> void:
	var s: Variant = get_parent().get("sheet") if get_parent() != null else null
	if s is CharModel and (s as CharModel).equipment.hash() != _armour_sig:
		refresh_armour(s)


## Dress the body in what the sheet wears (head, body, hands, feet). Call after equipping / taking off.
func refresh_armour(sheet: CharModel) -> void:
	var gear := {}
	if sheet != null:
		_armour_sig = sheet.equipment.hash()
		for slot in ["head", "body", "hands", "feet"]:
			var id: String = sheet.equipment.get(slot, "")
			if id != "":
				gear[slot] = id + " " + str(sheet.item_data(id).get("name", ""))
	dress(gear)


## Armour shells over the segments: slot -> item text (its id / name tells the material).
func dress(gear: Dictionary) -> void:
	for n in _armour:
		if is_instance_valid(n):
			n.free()
	_armour.clear()
	for slot: String in gear:
		var kind: String = BodyParts.material_of(str(gear[slot]))
		var mat: StandardMaterial3D = BodyParts.armour_material(kind)
		var metal: bool = kind != "leather" and kind != "cloth"
		match slot:
			"head":
				var h := _seg("head")
				if h.is_empty():
					continue
				var R := float(h["r"]) + 0.018
				var top: Vector3 = (h["rest_b"] as Vector3) - Vector3(0, 0.02, 0)
				_shell(h, _sphere(R, mat, true), Transform3D(Basis(), top), slot)
				var rim := CylinderMesh.new()
				rim.top_radius = R + 0.006
				rim.bottom_radius = R + 0.008
				rim.height = 0.032
				rim.radial_segments = 20
				rim.material = mat
				_shell(h, rim, Transform3D(Basis(), top), slot)
				if metal:  # nose guard
					var nose := BoxMesh.new()
					nose.size = Vector3(0.022, 0.075, 0.014)
					nose.material = mat
					_shell(h, nose, Transform3D(Basis(), top + Vector3(0, -0.04, R + 0.004)), slot)
			"body":
				_cover("chest", 0.022, 0.0, 1.0, mat, slot)
				_cover("abdomen", 0.018, 0.0, 1.0, mat, slot)
				for s in ["l", "r"]:
					var ua := _seg("upperarm_" + s)
					if ua.is_empty():
						continue
					var bi := skeleton.find_bone("clavicle_" + s)  # rides the shoulder, the arm moves under it
					if bi < 0:
						bi = int(ua["bone"])
					var at: Vector3 = (ua["rest_a"] as Vector3) + Vector3(0, 0.008, 0)
					_armour.append(_add_part(bi, _sphere(float(ua["r"]) + 0.04, mat, true), Transform3D(Basis(), at), "armour", slot))
			"hands":
				for s in ["l", "r"]:
					_cover("forearm_" + s, 0.016, 0.35, 0.92, mat, slot)
					_cover("hand_" + s, 0.01 if not metal else 0.014, 0.0, 1.0, mat, slot)
			"feet":
				for s in ["l", "r"]:
					_cover("foot_" + s, 0.016, 0.0, 1.0, mat, slot)
					_cover("shin_" + s, 0.018, 0.58, 1.0, mat, slot)


func _shell(h: Dictionary, mesh: Mesh, rest: Transform3D, slot: String) -> void:
	_armour.append(_add_part(int(h["bone"]), mesh, rest, "armour", slot))


## A capsule shell over the stretch [from, to] (0..1) of a segment, `pad` thicker than it.
func _cover(seg: String, pad: float, from: float, to: float, mat: Material, slot: String) -> void:
	var h := _seg(seg)
	if h.is_empty():
		return
	var pa: Vector3 = (h["rest_a"] as Vector3).lerp(h["rest_b"], from)
	var pb: Vector3 = (h["rest_a"] as Vector3).lerp(h["rest_b"], to)
	_shell(h, _capsule(float(h["r"]) * prop(seg) + pad, pa.distance_to(pb), mat), BodyParts.along(pa, pb), slot)


## The armour pieces worn now, optionally of one slot.
func armour_pieces(slot: String = "") -> Array:
	var out: Array = []
	for n in _armour:
		if is_instance_valid(n) and (slot == "" or n.get_meta("armour", "") == slot):
			out.append(n)
	return out


# ---- driving -----------------------------------------------------------------------------------

## Locomotion: `local` = velocity in the character's frame (x right, y forward) in m/s. Called at the
## physics rate; the blend follows it smoothly every rendered frame (_process).
func set_motion(local: Vector2, sprinting: bool, mode: String, _delta: float = 0.0) -> void:
	_want_prev = _want
	_want = local
	_sprinting = sprinting
	_mode = mode


func _process(delta: float) -> void:
	if tree == null:
		return
	_drive_loco(delta)
	_drive_layers(delta)
	_drive_lean(delta)
	# level of detail: far from the camera the tree is stepped at ~30 Hz and the pose modifier rests
	var cam := get_viewport().get_camera_3d()
	var far := cam != null and cam.global_position.distance_squared_to(global_position) > FAR_LOD * FAR_LOD
	if far != _far:
		_far = far
		tree.callback_mode_process = AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_MANUAL if far else AnimationMixer.ANIMATION_CALLBACK_MODE_PROCESS_IDLE
		_lean.active = not far
		_far_t = 0.0
	if far:
		_far_t += delta
		if _far_t >= 1.0 / 30.0:
			_eval_dt = _far_t
			_far_t = 0.0
			tree.advance(_eval_dt)
	else:
		_eval_dt = delta


func _physics_process(_delta: float) -> void:
	_take_root_motion()  # a new tick: what the frames since the last one produced is this tick's share


## Speed of a loco point's clip (feet) or a fallback.
func _ring_speed(k: String, fallback: float) -> float:
	return float(_ring[k][0]) if _ring.has(k) else fallback


## Where on the rings a local velocity lands: direction on the unit diamond times the gait (1 = this
## direction's walk at its own pace, 2 = its run, 3 = the sprint). Each clip sits exactly at its own
## measured speed, so at that speed it plays at its natural rate.
func _loco_target(local: Vector2, sprinting: bool) -> Vector2:
	var sp := local.length()
	if sp < 0.01:
		return Vector2.ZERO
	var d := local / sp
	var l1 := absf(d.x) + absf(d.y)
	var side := absf(d.x) / l1
	var fb := "f_" if d.y >= 0.0 else "b_"
	var lr := "r_" if d.x >= 0.0 else "l_"
	var walk := lerpf(_ring_speed(fb + "walk", WALK_REF), _ring_speed(lr + "walk", WALK_REF), side)
	var run := lerpf(_ring_speed(fb + "run", RUN_REF), _ring_speed(lr + "run", RUN_REF), side)
	var g := sp / walk if sp <= walk else 1.0 + clampf((sp - walk) / maxf(0.2, run - walk), 0.0, 1.0)
	var sprint := _ring_speed("sprint", 5.6)
	if sprinting and d.y > 0.7 and sp > run:
		g = 2.0 + clampf((sp - run) / maxf(0.2, sprint - run), 0.0, 1.0)
	return d / l1 * g


func _drive_loco(delta: float) -> void:
	# the controller's velocity steps at 60 Hz; the frames in between see it interpolated like the body
	var want := _want_prev.lerp(_want, Engine.get_physics_interpolation_fraction())
	var sp := want.length()
	var tgt := _loco_target(want, _sprinting)
	var g := absf(tgt.x) + absf(tgt.y)  # the target sits on the diamond of radius g
	# the blend position in polar form: a direction that swings round the rings (never across the middle,
	# where a left and a right strafe would be mixed) - quickly when nearly stopped - and a gait on a
	# critically damped spring (no lurch when the speed steps, no overshoot). Below walking pace it rests on
	# the walk ring and `move` fades it out.
	var align := 1.0
	if g > 0.05:
		var dd := wrapf(atan2(tgt.x, tgt.y) - _dir, -PI, PI)
		if absf(dd) > 1.75 and _move > 0.25:
			# reversed (a strafe from right to left): the legs gather to the idle first and the direction
			# swaps under cover of it, instead of mixing a left-leading with a right-leading stride
			align = 0.0
		elif absf(dd) > 1.75:
			_dir = atan2(tgt.x, tgt.y)
		else:
			var turn := lerpf(40.0, 10.0, _move * _move) * delta
			_dir = wrapf(_dir + clampf(dd * (1.0 - exp(-delta * 30.0)), -turn, turn), -PI, PI)
	var k := BLEND_HZ
	_gait_v += ((maxf(g, 1.0) - _gait) * k * k - _gait_v * 2.0 * k) * delta
	_gait = maxf(1.0, _gait + _gait_v * delta)
	var d := Vector2(sin(_dir), cos(_dir))
	_bp = d / (absf(d.x) + absf(d.y)) * _gait
	# idle <-> moving follows the (already acceleration-limited, interpolated) speed closely: a lagging mix
	# would make the legs spin up to cover the ground with too little weight; a stiff spring only rounds
	# off corners and steps (root motion handing over to the stick)
	var mk := MOVE_HZ
	_move_v += ((clampf(g, 0.0, 1.0) * align - _move) * mk * mk - _move_v * 2.0 * mk) * delta
	_move = clampf(_move + _move_v * delta, 0.0, 1.0)
	tree.set("parameters/loco/blend_position", _bp)
	tree.set("parameters/move/blend_amount", _move)
	# stride matching: the shared cycle advances at ground speed / blended stride, so planted feet stay
	# planted whatever the mix (below walking pace the walk keeps its own rate: `move` shortens it)
	var loco: AnimationNodeBlendSpace2D = bt.get_node("loco")
	var w := _bs_weights(loco, _bp)
	var stride := 0.0
	var nat := 0.0
	for i in w.size():
		if w[i] <= 0.0:
			continue
		var r: Array = _ring.get(loco.get_blend_point_name(i), [WALK_REF, 1.0])
		stride += w[i] * float(r[0]) * float(r[1])
		nat += w[i] / float(r[1])
	if stride > 0.05:
		# planted feet move at move x stride x rate (the idle share holds still): solve for the rate.
		# Slowing faster than the blend can follow, the stride slows with the body instead of skating.
		_rate = clampf(sp / (maxf(_move, 0.05) * stride), nat * 0.3, nat * 1.4)
	tree.set("parameters/loco_ts/scale", _rate)
	# crouch: its own small blend space, rate from speed
	tree.set("parameters/crouch/blend_position", Vector2(clampf(want.x / 1.3, -1, 1), clampf(want.y / 1.3, -1, 1)))
	tree.set("parameters/crouch_ts/scale", clampf(sp / 1.3, 0.7, 1.6) if sp > 0.2 else 1.0)
	if tree.get("parameters/mode/current_state") != _mode:
		tree.set("parameters/mode/transition_request", _mode)


## The blend weights Godot's BlendSpace2D gives `pos` (barycentric in its triangle, or along the nearest
## edge when outside), per blend point.
static func _bs_weights(bs: AnimationNodeBlendSpace2D, pos: Vector2) -> PackedFloat32Array:
	var w := PackedFloat32Array()
	w.resize(bs.get_blend_point_count())
	var best := INF
	var best_w := [0.0, 0.0, 0.0]
	var best_i := [0, 0, 0]
	for t in bs.get_triangle_count():
		var ii := [bs.get_triangle_point(t, 0), bs.get_triangle_point(t, 1), bs.get_triangle_point(t, 2)]
		var p: Array[Vector2] = [bs.get_blend_point_position(ii[0]), bs.get_blend_point_position(ii[1]), bs.get_blend_point_position(ii[2])]
		if Geometry2D.point_is_inside_triangle(pos, p[0], p[1], p[2]):
			var v0 := p[1] - p[0]
			var v1 := p[2] - p[0]
			var v2 := pos - p[0]
			var den := v0.x * v1.y - v1.x * v0.y
			if absf(den) < 0.000001:
				continue
			var b1 := (v2.x * v1.y - v1.x * v2.y) / den
			var b2 := (v0.x * v2.y - v2.x * v0.y) / den
			w[ii[0]] = 1.0 - b1 - b2
			w[ii[1]] = b1
			w[ii[2]] = b2
			return w
		for j in 3:
			var a: Vector2 = p[j]
			var b: Vector2 = p[(j + 1) % 3]
			var c := Geometry2D.get_closest_point_to_segment(pos, a, b)
			var d := c.distance_to(pos)
			if d < best:
				best = d
				var f := a.distance_to(c) / maxf(0.000001, a.distance_to(b))
				best_i = [ii[j], ii[(j + 1) % 3], ii[(j + 2) % 3]]
				best_w = [1.0 - f, f, 0.0]
	if best < INF:
		for j in 3:
			w[best_i[j]] += best_w[j]
	return w


func _drive_layers(delta: float) -> void:
	# held pose over the upper body
	_hold_w = move_toward(_hold_w, 1.0 if _hold != "" else 0.0, delta * 8.0)
	tree.set("parameters/upper/blend_amount", _hold_w)
	# one-shot upper-body action: in fast, out over its last 0.15 s
	if _u_left > 0.0:
		_u_left -= delta
	var uw := minf(1.0, float(_uslot_w[_uslot]) + delta / 0.12)
	_uslot_w[_uslot] = uw
	_uslot_w[1 - _uslot] = 1.0 - uw
	tree.set("parameters/u_x/blend_amount", float(_uslot_w[1]))
	var u_want := 1.0 if _u_left > 0.15 else maxf(0.0, _u_left / 0.15)
	_u_w = move_toward(_u_w, u_want, delta * (12.0 if u_want > _u_w else 7.0))
	tree.set("parameters/uact/blend_amount", _u_w)
	if _u_w <= 0.001 and _u_left <= 0.0:
		upper_clip = ""
	# the action layer, fading out at the end of its clip
	_mix_slots(delta)
	if _warp_left > 0.0:
		_warp_left -= delta
		if _warp_left <= 0.0:
			tree.set("parameters/act_%s_ts/scale" % SLOTS[_slot], _speed)
	if _act_left > 0.0:
		_act_left -= delta
	var want := 1.0 if _act_left > _act_fade else maxf(0.0, _act_left / _act_fade)
	_act_w = move_toward(_act_w, want, delta * (14.0 if want > _act_w else 1.0 / maxf(0.05, _act_fade)))
	tree.set("parameters/action/blend_amount", _act_w)
	if _act_w <= 0.001 and _act_left <= 0.0:
		act_clip = ""


## Lean like a runner: into the acceleration (centripetal in a turn, forward when speeding up, back when
## braking), from the body as it is drawn this frame. A few degrees, about the feet; off in actions.
func _drive_lean(delta: float) -> void:
	var xf := get_global_transform_interpolated()
	var pos := xf.origin
	if is_nan(_yaw_prev):
		_yaw_prev = 0.0
		_lean_pos = pos
	var f := 1.0 - exp(-delta * 18.0)
	var v := (pos - _lean_pos) / maxf(delta, 0.0001)
	v.y = 0.0
	if v.length() > 25.0:  # a teleport
		v = _lean_vel
	_lean_pos = pos
	var vs := _lean_vel.lerp(v, f)
	var acc := (vs - _lean_vel) / maxf(delta, 0.0001)
	_lean_vel = vs
	_lean_acc = _lean_acc.lerp(acc, f)
	var right := xf.basis.x.normalized()
	var fwd := -xf.basis.z.normalized()
	var on := (1.0 - _act_w) * (1.0 if _mode == "ground" else 0.0)
	var roll := clampf(-_lean_acc.dot(right) * 0.011, -0.15, 0.15) * on
	var pitch := clampf(_lean_acc.dot(fwd) * 0.006, -0.07, 0.08) * on
	var g := 1.0 - exp(-delta * 12.0)
	_lean.roll = lerpf(_lean.roll, roll, g)
	_lean.pitch = lerpf(_lean.pitch, pitch, g)
	# feet plant on the ground only, and not while an action carries the body fast (rolls, lunges)
	_lean.feet_lock = grounded and _mode != "air" and (_act_w < 0.5 or _rm_v.length() < 1.5)


## Play a whole-body action; returns how long it lasts at that speed. `fade` = blend out at the end.
## `from` > 0 (chained combo hits): the first `from` seconds of the clip (the wind-up) play WARP times
## faster, so the blow comes quickly but the motion stays continuous.
func play(clip: String, speed: float = 1.0, fade: float = 0.18, from: float = 0.0) -> float:
	if not tree.has_animation(clip):
		push_warning("no clip " + clip)
		return 0.0
	var quiet := -1
	for k in SLOTS.size():
		if k != _slot and (quiet < 0 or float(_slot_w[k]) < float(_slot_w[quiet])):
			quiet = k
	_slot = quiet
	var sl: String = SLOTS[_slot]
	(bt.get_node("act_" + sl) as AnimationNodeAnimation).animation = clip
	_speed = speed
	_warp_left = from / (speed * WARP)
	tree.set("parameters/act_%s_ts/scale" % sl, speed * (WARP if _warp_left > 0.0 else 1.0))
	tree.set("parameters/act_%s_seek/seek_request" % sl, 0.0)  # always from the start: mid-clip seeks pop
	if _act_w <= 0.001:  # nothing showing: no need to blend in over an old action
		for k in SLOTS.size():
			_slot_w[k] = 1.0 if k == _slot else 0.0
		_mix_slots(0.0)
	_act_left = _warp_left + maxf(0.05, tree.get_animation(clip).length - from) / maxf(0.01, speed)
	_act_fade = fade
	act_clip = clip
	return _act_left


## Play a one-shot over the upper body (spine and up) while the legs keep doing what they do: casting
## on the move. Returns its duration at that speed.
func play_upper(clip: String, speed: float = 1.0) -> float:
	if not tree.has_animation(clip):
		push_warning("no clip " + clip)
		return 0.0
	if _u_w > 0.001:
		_uslot = 1 - _uslot  # cross-fade from the one still showing
	else:
		_uslot_w = [1.0, 0.0] if _uslot == 0 else [0.0, 1.0]
	var sl: String = USLOTS[_uslot]
	(bt.get_node("u_" + sl) as AnimationNodeAnimation).animation = clip
	tree.set("parameters/u_%s_ts/scale" % sl, speed)
	tree.set("parameters/u_%s_seek/seek_request" % sl, 0.0)
	_u_left = tree.get_animation(clip).length / maxf(0.01, speed)
	upper_clip = clip
	return _u_left


## The current slot rises to full weight over ACT_XFADE; the others share what is left.
func _mix_slots(delta: float) -> void:
	var w := minf(1.0, float(_slot_w[_slot]) + delta / ACT_XFADE)
	var others := 0.0
	for k in SLOTS.size():
		if k != _slot:
			others += float(_slot_w[k])
	for k in SLOTS.size():
		if k == _slot:
			_slot_w[k] = w
		else:
			_slot_w[k] = float(_slot_w[k]) * (1.0 - w) / others if others > 0.0001 else 0.0
	var ab := float(_slot_w[0]) + float(_slot_w[1])
	tree.set("parameters/act_ab/blend_amount", float(_slot_w[1]) / ab if ab > 0.0001 else 0.0)
	tree.set("parameters/act/blend_amount", float(_slot_w[2]))


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


## Every evaluation of the tree (once per rendered frame) adds its root motion, in world space.
func _on_mixer_applied() -> void:
	_rm_acc += inst.global_basis * tree.get_root_motion_position()
	_rm_t += _eval_dt


## Start of a physics tick: the frames since the last tick give the root motion's current velocity.
func _take_root_motion() -> void:
	var tick := Engine.get_physics_frames()
	if tick == _rm_tick:
		return
	_rm_tick = tick
	if _rm_t > 0.0:
		_rm_v = _rm_acc / _rm_t
	_rm_acc = Vector3.ZERO
	_rm_t = 0.0


## This physics tick's root motion in world space (the action's own ground travel). Frames and ticks
## interleave unevenly (2 or 3 frames per tick at 144 Hz), so it is the frames' average velocity times
## the tick: the body moves at an even pace, and over time exactly as far as the clips travel.
func root_motion_world() -> Vector3:
	_take_root_motion()
	return _rm_v * get_physics_process_delta_time()


## The ground speed a loop's planted feet imply (m/s): how fast the body has to travel for the feet to
## stay put. The root speed in clips.json is the source's travel, which can differ by 5-25 % (foot slip in
## the source); stride matching uses this. Measured once per clip from the leg chain.
func feet_speed(clip: String) -> float:
	if _feet.has(clip):
		return _feet[clip]
	var m: Dictionary = meta.get(clip, {})
	var fallback := float(m.get("speed", 0.0))
	var a := tree.get_animation(clip) if tree != null and tree.has_animation(clip) else null
	if a == null or fallback < 0.2:
		_feet[clip] = fallback
		return fallback
	var tracks := {}
	for ti in a.get_track_count():
		var bi := skeleton.find_bone(a.track_get_path(ti).get_concatenated_subnames())
		if bi < 0:
			continue
		var e: Array = tracks.get(bi, [-1, -1])
		if a.track_get_type(ti) == Animation.TYPE_POSITION_3D:
			e[0] = ti
		elif a.track_get_type(ti) == Animation.TYPE_ROTATION_3D:
			e[1] = ti
		tracks[bi] = e
	var sum := Vector2.ZERO
	var n := 0
	var dt := 1.0 / 60.0
	var steps := maxi(8, int(a.length / dt))
	for side in ["l", "r"]:
		var chain: Array[int] = []
		var b := skeleton.find_bone("foot_" + side)
		while b > 0:  # up to (not including) the root: positions relative to the root
			chain.push_front(b)
			b = skeleton.get_bone_parent(b)
		var ps: Array[Vector3] = []
		var ymin := INF
		for k in steps:
			var x := Transform3D.IDENTITY
			for c in chain:
				var l := skeleton.get_bone_rest(c)
				var e: Array = tracks.get(c, [-1, -1])
				if int(e[0]) >= 0:
					l.origin = a.position_track_interpolate(int(e[0]), k * dt)
				if int(e[1]) >= 0:
					l.basis = Basis(a.rotation_track_interpolate(int(e[1]), k * dt))
				x = x * l
			ps.append(x.origin)
			ymin = minf(ymin, x.origin.y)
		for k in range(1, steps - 1):
			var v := (ps[k + 1] - ps[k - 1]) / (2.0 * dt)
			if ps[k].y < ymin + 0.03 and absf(v.y) < 0.3:
				sum += Vector2(v.x, v.z)
				n += 1
	var fs := sum.length() / n if n > 0 else fallback
	if fs < fallback * 0.5 or fs > fallback * 1.5:
		fs = fallback  # no clean contact found: trust the root
	_feet[clip] = fs
	return fs


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
	var length := 1.15 if stance_id == "greatsword" else (0.33 if stance_id == "dagger" else 0.7)
	var up := g.basis.y.normalized()
	return [g.origin + up * 0.08, g.origin + up * length]


## The body's hurtboxes in the world, posed now: [a, b, radius, zone, name] per segment.
func hurtboxes() -> Array:
	var out: Array = []
	for h: Dictionary in hurt:
		out.append(BodyParts.to_world(skeleton, h))
	return out


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
	for ti in a.get_track_count():
		var bone := skeleton.find_bone(a.track_get_path(ti).get_concatenated_subnames())
		if bone < 0:
			continue
		var x := local[bone]
		match a.track_get_type(ti):
			Animation.TYPE_POSITION_3D:
				x.origin = a.position_track_interpolate(ti, t)
			Animation.TYPE_ROTATION_3D:
				x.basis = Basis(a.rotation_track_interpolate(ti, t))
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
