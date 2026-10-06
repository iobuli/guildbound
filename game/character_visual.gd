class_name CharacterVisual
extends Node3D
## The visible body: a Quaternius Universal Base Character (CC0, 65-bone humanoid rig), animated by an
## AnimationTree built from the Universal Animation Library 1 + 2 (CC0, same rig and bone names) plus
## CMU motion capture (free, no restrictions) retargeted by tools/bvh_import.gd: climbing, heavy pushing.
##
## Layers, bottom to top:
##   mode       Transition: ground / crouch / air / fall / swim / push / slide
##   loco_speed TimeScale so feet match real speed
##   full       OneShot, whole body: roll, ledge pull-up, landing
##   carry      Blend2, spine and up: the pack's Walk_Carry arms while holding something
##   upper      OneShot, spine and up: cast, pick up, throw, flinch (so you can do them while running)
##   MotionIK        (SkeletonModifier3D): foot IK on stairs/slopes, landing dip, hit recoil, hands on
##                   pushed props, lean into pushes and turns - all maths, no canned clips
##   GetUpPose       (SkeletonModifier3D): getting up from wherever the ragdoll lies, on planted
##                   palms / knees / feet - no clip
##   RagdollPose     (SkeletonModifier3D): records that pose as targets for the BodyRig physics body,
##                   then draws the character FROM the physics body (solid: no clipping, real momentum)
##
## Animation is driven only from the player's state, never from input, so a remote co-op player would
## animate the same way.

const BASE := "res://assets/characters/quaternius_ubc/"
const LIB1 := "res://assets/animations/quaternius_ual1/UAL1_Standard.glb"
const LIB2 := "res://assets/animations/quaternius_ual2/UAL2_Standard.glb"
const CMU := "res://assets/animations/cmu/"
const FROM_CMU := ["CMU_Climb", "CMU_PushHeavy"]
const NOM_CLIMB := 0.5     # m/s the ladder clip reads as at normal speed
const NOM_PUSH := 0.83

const FROM_LIB1 := ["Idle", "Walk", "Jog_Fwd", "Sprint", "Crouch_Idle", "Crouch_Fwd", "Jump", "Jump_Land", "Roll",
	"Swim_Idle", "Swim_Fwd", "Push", "PickUp_Table", "Hit_Chest", "Death01", "Interact", "Spell_Simple_Shoot",
	"Sword_Idle", "Sword_Attack", "Punch_Jab", "Punch_Cross"]
const FROM_LIB2 := ["Melee_Hook", "Walk_Carry", "ClimbUp_1m", "Slide", "Slide_Start", "Slide_Exit", "LayToIdle", "NinjaJump_Start",
	"NinjaJump_Idle", "NinjaJump_Land", "Hit_Knockback", "OverhandThrow", "Consume", "Chest_Open", "Sword_Block",
	"Sword_Regular_A", "Sword_Regular_B", "Sword_Regular_C", "Idle_Shield"]
const LOOPING := ["Idle", "Walk", "Jog_Fwd", "Sprint", "Crouch_Idle", "Crouch_Fwd", "Jump", "Swim_Idle", "Swim_Fwd",
	"Push", "Walk_Carry", "Slide", "NinjaJump_Idle", "Sword_Idle", "Idle_Shield"]

## Ground speed each clip covers at normal playback, measured from the planted foot's speed in the
## clip itself (tools/clip_probe.gd), so feet stay put when playback rate = speed / nominal.
const NOM_WALK := 1.0
const NOM_JOG := 6.25
const NOM_SPRINT := 8.42
const NOM_CROUCH := 0.68
const NOM_SWIM := 2.18
## Walk / jog / sprint are resampled to ONE cycle per second, all starting at the left heel strike, so the
## synced blendspace mixes matching phases (no crossed legs mid-blend). Cycle start (s) and metres per cycle:
const CYCLES := {"Walk": [0.0, 1.0], "Jog_Fwd": [0.025, 6.25], "Sprint": [0.03, 8.42]}
const STRIDE_WALK := 1.333    # NOM * original clip length
const STRIDE_JOG := 5.833
const STRIDE_SPRINT := 5.613
const WALK_REF := 1.5       # game speed that shows a full walk (faster blends toward the jog)
const PUSH_DROP := 0.24     # the CMU push clip holds both feet ~0.25 m off the floor: lower its hips
const CROUCH_DROP := 0.04   # the crouch clips hover their feet 4 cm (tools/clip_probe.gd "sole min")
const CROUCH_MAX_STRIDE := 1.6
const PUSH_MAX := 2.1       # m/s the push clip's legs can show (2.5x its natural pace)
## Fighting reactions from the Motifect pack: name -> [source clip, from, to] (seconds in the source).
## Played on the action layer (spine and up) so the legs keep stepping; ACT_guard is held while blocking.
const ACTS := {
	"ACT_guard": ["MF_block_mid", 1.0, 1.3],
	"ACT_parry": ["MF_parry_right", 0.3, 1.0],
	"ACT_react_face": ["MF_hit_react_face", 0.25, 1.0],
	"ACT_react_chest": ["MF_hit_react_chest", 1.2, 2.4],
	"ACT_react_body": ["MF_hit_react_body", 0.85, 2.2],
	"ACT_dodge_l": ["MF_dodge_left", 0.85, 2.2],
	"ACT_dodge_r": ["MF_dodge_right", 0.25, 1.1],
	"ACT_dodge_b": ["MF_dodge_back", 0.0, 0.9],
}
const HARD_LANDING := -9.0  # vertical speed (m/s) that earns a landing animation
const FALL_AFTER := 0.45    # seconds of falling before the long-fall pose takes over

var skeleton: Skeleton3D
var body_id := ""
var inst: Node3D
var tree: AnimationTree
var bt: AnimationNodeBlendTree
var ik: MotionIK
var rig: BodyRig
var getup: GetUpPose
var ragdoll_pose: RagdollPose

var _full_t := 0.0
var _act_slot := 0          # which of the two action slots plays the newest clip
var _act_left := 0.0        # seconds of the action clip still to play
var _act_w := 0.0
var _act_hold := false      # the held guard pose is up
var act_name := ""          # the action clip playing (tests / diagnostics)
var _carry_w := 0.0
var _prev_state := -1
var _min_vy := 0.0
var _prev_hp := -1.0
var _prev_carry := false
var _prev_dodge := false
var _prev_pos := Vector3.ZERO
var _prev_yaw := 0.0
var _prev_mode := "ground"
var _gait := 0.0


func build(body: String = "Superhero_Male_FullBody", hair: String = "Hair_SimpleParted") -> void:
	body_id = body
	inst = (load(BASE + body + ".gltf") as PackedScene).instantiate()
	inst.rotation.y = PI  # glTF faces +Z; the controller's forward is -Z
	add_child(inst)
	skeleton = inst.find_child("Skeleton3D", true, false)
	if hair != "":
		attach_hair(hair)
	_build_tree()
	ik = MotionIK.new()
	skeleton.add_child(ik)  # modifiers run in child order: IK, then get-up, then ragdoll
	ik.setup()
	rig = BodyRig.new()
	add_child(rig)
	rig.setup(skeleton)
	getup = GetUpPose.new()
	getup.rig = rig
	skeleton.add_child(getup)
	getup.setup(clip_pose("Idle", 0.0))
	ragdoll_pose = RagdollPose.new()
	ragdoll_pose.rig = rig
	skeleton.add_child(ragdoll_pose)  # after MotionIK: modifiers run in child order


## Hair is rigged to the Head bone; move its meshes onto our skeleton so it follows the head.
func attach_hair(hair: String) -> void:
	var h: Node3D = (load(BASE + "hair/" + hair + ".gltf") as PackedScene).instantiate()
	for mi in h.find_children("*", "MeshInstance3D", true, false):
		mi.owner = null
		mi.get_parent().remove_child(mi)
		skeleton.add_child(mi)
		mi.skeleton = NodePath("..")
		mi.transform = Transform3D.IDENTITY
	h.free()


# ---- tree construction -------------------------------------------------------------------------

func _clip(name: String) -> AnimationNodeAnimation:
	var n := AnimationNodeAnimation.new()
	n.animation = name
	return n


func _space(points: Array, sync: bool = true) -> AnimationNodeBlendSpace1D:
	var s := AnimationNodeBlendSpace1D.new()
	s.min_space = 0.0
	s.max_space = float(points.size() - 1)
	s.sync = sync
	for i in points.size():
		s.add_blend_point(_clip(points[i]), float(i), -1, points[i])
	return s


func _upper_filter(node: AnimationNode) -> void:
	node.filter_enabled = true
	var spine := skeleton.find_bone("spine_01")
	for i in skeleton.get_bone_count():
		var j := i
		while j >= 0 and j != spine:
			j = skeleton.get_bone_parent(j)
		if j == spine:
			node.set_filter_path(NodePath("Armature/Skeleton3D:" + skeleton.get_bone_name(i)), true)


## Resample a looping clip to exactly one cycle per second, starting at `t0`.
static func _one_cycle(src: Animation, t0: float, samples: int = 60) -> Animation:
	var L := src.length
	var a := Animation.new()
	a.length = 1.0
	a.loop_mode = Animation.LOOP_LINEAR
	for tr in src.get_track_count():
		var type := src.track_get_type(tr)
		if type != Animation.TYPE_POSITION_3D and type != Animation.TYPE_ROTATION_3D and type != Animation.TYPE_SCALE_3D:
			continue
		var nt := a.add_track(type)
		a.track_set_path(nt, src.track_get_path(tr))
		for k in samples:
			var u := float(k) / samples
			var ts := fposmod(t0 + u * L, L)
			match type:
				Animation.TYPE_POSITION_3D:
					a.position_track_insert_key(nt, u, src.position_track_interpolate(tr, ts))
				Animation.TYPE_ROTATION_3D:
					a.rotation_track_insert_key(nt, u, src.rotation_track_interpolate(tr, ts))
				_:
					a.scale_track_insert_key(nt, u, src.scale_track_interpolate(tr, ts))
	return a


## The part of a clip between t0 and t1 (resampled at 30 fps), played once.
static func _slice(src: Animation, t0: float, t1: float) -> Animation:
	var a := Animation.new()
	a.length = t1 - t0
	a.loop_mode = Animation.LOOP_NONE
	var n := maxi(2, int(ceil((t1 - t0) * 30.0)) + 1)
	for tr in src.get_track_count():
		var type := src.track_get_type(tr)
		if type != Animation.TYPE_POSITION_3D and type != Animation.TYPE_ROTATION_3D and type != Animation.TYPE_SCALE_3D:
			continue
		var nt := a.add_track(type)
		a.track_set_path(nt, src.track_get_path(tr))
		for k in n:
			var u := minf(float(k) / 30.0, t1 - t0)
			var ts := t0 + u
			match type:
				Animation.TYPE_POSITION_3D:
					a.position_track_insert_key(nt, u, src.position_track_interpolate(tr, ts))
				Animation.TYPE_ROTATION_3D:
					a.rotation_track_insert_key(nt, u, src.rotation_track_interpolate(tr, ts))
				_:
					a.scale_track_insert_key(nt, u, src.scale_track_interpolate(tr, ts))
	return a


## Copy of a clip with the pelvis moved by `by` (skeleton space; the track is in the parent bone's frame).
func _shift_root(src: Animation, by: Vector3) -> Animation:
	var a: Animation = src.duplicate(true)
	var parent := skeleton.get_bone_parent(skeleton.find_bone("pelvis"))
	var local := by if parent < 0 else skeleton.get_bone_global_rest(parent).basis.inverse() * by
	for tr in a.get_track_count():
		if a.track_get_type(tr) == Animation.TYPE_POSITION_3D and str(a.track_get_path(tr)).ends_with(":pelvis"):
			for k in a.track_get_key_count(tr):
				a.track_set_key_value(tr, k, (a.track_get_key_value(tr, k) as Vector3) + local)
	return a


func _build_tree() -> void:
	var lib := AnimationLibrary.new()
	for pair in [[LIB1, FROM_LIB1], [LIB2, FROM_LIB2]]:
		var src: Node = (load(pair[0]) as PackedScene).instantiate()
		var sap: AnimationPlayer = src.find_child("AnimationPlayer", true, false)
		for n in pair[1]:
			if not sap.has_animation(n):
				push_warning("animation %s missing from %s" % [n, pair[0]])
				continue
			var a: Animation = sap.get_animation(n).duplicate()
			a.loop_mode = Animation.LOOP_LINEAR if n in LOOPING else Animation.LOOP_NONE
			if CYCLES.has(n):
				a = _one_cycle(a, CYCLES[n][0])
			if n.begins_with("Crouch"):
				a = _shift_root(a, Vector3(0.0, -CROUCH_DROP, 0.0))
			lib.add_animation(n, a)
		src.free()
	# strike style clips (Motifect / Quaternius), cut around each move's impact frame: MOVE_<id>
	for id in Combat.move_ids():
		var m := Combat.move_data(id)
		var src_name: String = m.get("clip", "")
		var src: Animation = null
		if src_name.begins_with("MF_"):
			src = load("res://assets/animations/motifect/%s.res" % src_name)
		elif lib.has_animation(src_name):
			src = lib.get_animation(src_name)
		if src != null:
			lib.add_animation("MOVE_" + id, _slice(src, float(m["from"]), float(m["to"])))
	for n in ACTS:
		var spec: Array = ACTS[n]
		var a: Animation = _slice(load("res://assets/animations/motifect/%s.res" % spec[0]), spec[1], spec[2])
		if n == "ACT_guard":
			a.loop_mode = Animation.LOOP_PINGPONG  # held: a slow breath in the block
		lib.add_animation(n, a)
	for n in FROM_CMU:
		var ca: Animation = load(CMU + n + ".res")
		if n == "CMU_PushHeavy":
			ca = _shift_root(ca, Vector3(0.0, -PUSH_DROP, 0.0))
		lib.add_animation(n, ca)

	bt = AnimationNodeBlendTree.new()
	bt.add_node("ground", _space(["Idle", "Walk", "Jog_Fwd", "Sprint"]))
	bt.add_node("crouch", _space(["Crouch_Idle", "Crouch_Fwd"]))
	bt.add_node("swim", _space(["Swim_Idle", "Swim_Fwd"], false))
	bt.add_node("air", _clip("Jump"))
	bt.add_node("fall", _clip("NinjaJump_Idle"))
	bt.add_node("push", _clip("CMU_PushHeavy"))
	bt.add_node("slide", _clip("Slide"))
	bt.add_node("climb", _clip("CMU_Climb"))

	var modes := ["ground", "crouch", "air", "fall", "swim", "push", "slide", "climb"]
	var tr := AnimationNodeTransition.new()
	tr.input_count = modes.size()
	for i in modes.size():
		tr.set_input_name(i, modes[i])
	tr.xfade_time = 0.2
	bt.add_node("mode", tr)
	for i in modes.size():
		bt.connect_node("mode", i, modes[i])

	bt.add_node("loco_speed", AnimationNodeTimeScale.new())
	bt.connect_node("loco_speed", 0, "mode")

	var full := AnimationNodeOneShot.new()
	full.fadein_time = 0.08
	full.fadeout_time = 0.2
	bt.add_node("full", full)
	bt.add_node("full_clip", _clip("Roll"))
	bt.add_node("full_ts", AnimationNodeTimeScale.new())
	bt.connect_node("full_ts", 0, "full_clip")
	bt.connect_node("full", 0, "loco_speed")
	bt.connect_node("full", 1, "full_ts")

	var carry := AnimationNodeBlend2.new()
	_upper_filter(carry)
	bt.add_node("carry", carry)
	bt.add_node("carry_clip", _clip("Walk_Carry"))
	bt.connect_node("carry", 0, "full")
	bt.connect_node("carry", 1, "carry_clip")

	var upper := AnimationNodeOneShot.new()
	upper.fadein_time = 0.1
	upper.fadeout_time = 0.2
	_upper_filter(upper)
	bt.add_node("upper", upper)
	bt.add_node("upper_clip", _clip("Spell_Simple_Shoot"))
	bt.add_node("upper_ts", AnimationNodeTimeScale.new())
	bt.connect_node("upper_ts", 0, "upper_clip")
	bt.connect_node("upper", 0, "carry")
	bt.connect_node("upper", 1, "upper_ts")

	# action layer (spine and up): strikes, blocks, parries, hit reactions. Two slots so each new clip
	# cross-fades from the last one (a combo flows instead of snapping back to the stance between hits)
	var act := AnimationNodeTransition.new()
	act.input_count = 3
	for i in 3:
		act.set_input_name(i, ["a", "b", "hold"][i])
	act.xfade_time = 0.07
	bt.add_node("act", act)
	for sl in ["a", "b"]:
		bt.add_node("act_" + sl, _clip("ACT_guard"))
		bt.add_node("act_ts_" + sl, AnimationNodeTimeScale.new())
		bt.connect_node("act_ts_" + sl, 0, "act_" + sl)
	bt.add_node("act_hold", _clip("ACT_guard"))
	bt.connect_node("act", 0, "act_ts_a")
	bt.connect_node("act", 1, "act_ts_b")
	bt.connect_node("act", 2, "act_hold")
	var action := AnimationNodeBlend2.new()
	_upper_filter(action)
	bt.add_node("action", action)
	bt.connect_node("action", 0, "upper")
	bt.connect_node("action", 1, "act")
	bt.connect_node("output", 0, "action")

	tree = AnimationTree.new()
	inst.add_child(tree)  # root_node defaults to its parent, so "Armature/Skeleton3D:bone" tracks resolve
	tree.add_animation_library("", lib)
	tree.tree_root = bt
	tree.active = true
	tree.set("parameters/mode/transition_request", "ground")
	tree.set("parameters/carry/blend_amount", 0.0)
	tree.set("parameters/action/blend_amount", 0.0)


# ---- one-shots -------------------------------------------------------------------------------

## Whole-body action. max_time cuts it short with a fade. Returns how long it will play.
func play_full(name: String, speed: float = 1.0, max_time: float = 0.0) -> float:
	(bt.get_node("full_clip") as AnimationNodeAnimation).animation = name
	tree.set("parameters/full_ts/scale", speed)
	tree.set("parameters/full/request", AnimationNodeOneShot.ONE_SHOT_REQUEST_FIRE)
	var len := tree.get_animation(name).length / speed
	_full_t = len if max_time <= 0.0 else minf(len, max_time)
	return _full_t


## Upper-body action layered over whatever the legs are doing.
func play_upper(name: String, speed: float = 1.0) -> void:
	(bt.get_node("upper_clip") as AnimationNodeAnimation).animation = name
	tree.set("parameters/upper_ts/scale", speed)
	tree.set("parameters/upper/request", AnimationNodeOneShot.ONE_SHOT_REQUEST_FIRE)


## Fighting clip on the action layer (spine and up), cross-faded from whatever it was doing.
## Returns how long it plays.
func act(name: String, speed: float = 1.0) -> float:
	if not tree.has_animation(name):
		return 0.0
	_act_slot = 1 - _act_slot
	var sl: String = ["a", "b"][_act_slot]
	(bt.get_node("act_" + sl) as AnimationNodeAnimation).animation = name
	tree.set("parameters/act_ts_%s/scale" % sl, speed)
	tree.set("parameters/act/transition_request", sl)
	_act_left = tree.get_animation(name).length / maxf(0.01, speed)
	act_name = name
	return _act_left


## Hold the block (or let it go): the guard pose stays up between strikes and reactions.
func act_hold(on: bool) -> void:
	if on == _act_hold:
		return
	_act_hold = on
	if on and _act_left <= 0.0:
		tree.set("parameters/act/transition_request", "hold")
		act_name = "ACT_guard"


func act_stop() -> void:
	_act_left = 0.0
	_act_hold = false


## How much the action layer owns the upper body right now (MotionIK yields the arms to it).
func act_weight() -> float:
	return _act_w


func _tick_act(delta: float) -> void:
	if _act_left > 0.0:
		_act_left -= delta
		if _act_left <= 0.0 and _act_hold:
			tree.set("parameters/act/transition_request", "hold")
			act_name = "ACT_guard"
	elif not _act_hold:
		act_name = ""
	# in fast (a strike is fully its clip within ~50 ms), out over the last 0.15 s
	var want := 1.0 if _act_hold or _act_left > 0.15 else (_act_left / 0.15 if _act_left > 0.0 else 0.0)
	_act_w = move_toward(_act_w, want, delta * (20.0 if want > _act_w else 7.0))
	tree.set("parameters/action/blend_amount", _act_w)


func on_cast() -> void:
	play_upper("Spell_Simple_Shoot", 1.5)


func on_throw() -> void:
	play_upper("OverhandThrow", 1.8)


## Ragdoll -> get-up, once getup.plan() has been made and the character stands on the planned spot.
## The body pulls itself up from exactly where it lies; returns how long it takes.
func begin_recovery() -> float:
	rig.recover(true)
	ik.reset_feet()
	tree.set("parameters/mode/transition_request", "ground")
	tree.set("parameters/ground/blend_position", 0.0)
	_gait = 0.0
	return getup.start()


func end_recovery() -> void:
	getup.stop()
	rig.solo()


func cancel_ragdoll() -> void:
	getup.stop()
	rig.reset()


## A clip's pose at `time`, every bone in skeleton space (bone name -> Transform3D).
func clip_pose(clip: String, time: float) -> Dictionary:
	var a: Animation = tree.get_animation(clip)
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
				x.origin = a.position_track_interpolate(tr, time)
			Animation.TYPE_ROTATION_3D:
				x.basis = Basis(a.rotation_track_interpolate(tr, time))
		local[bone] = x
	var glob: Array[Transform3D] = []
	glob.resize(local.size())
	var done: Array[bool] = []
	done.resize(local.size())
	var out := {}
	for i in local.size():
		var chain: Array[int] = []
		var j := i
		while j >= 0 and not done[j]:
			chain.push_front(j)
			j = skeleton.get_bone_parent(j)
		for k in chain:
			var pa := skeleton.get_bone_parent(k)
			glob[k] = local[k] if pa < 0 else glob[pa] * local[k]
			done[k] = true
		out[skeleton.get_bone_name(i)] = glob[i]
	return out


# ---- per-frame drive ---------------------------------------------------------------------------

static func gait(speed: float, run: float, sprint: float) -> float:
	if speed <= WALK_REF:
		return speed / WALK_REF
	if speed <= run:
		return 1.0 + (speed - WALK_REF) / maxf(0.01, run - WALK_REF)
	return 2.0 + clampf((speed - run) / maxf(0.01, sprint - run), 0.0, 1.0)


## Metres the blended ground cycle covers (clips are one cycle per second, so this is also m/s at scale 1).
static func gait_stride(g: float) -> float:
	if g <= 1.0:
		return STRIDE_WALK * maxf(g, 0.0)
	if g <= 2.0:
		return lerpf(STRIDE_WALK, STRIDE_JOG, g - 1.0)
	return lerpf(STRIDE_JOG, STRIDE_SPRINT, clampf(g - 2.0, 0.0, 1.0))


func _set_mode(mode: String) -> void:
	if tree.get("parameters/mode/current_state") != mode:
		tree.set("parameters/mode/transition_request", mode)


func update(p: Player, delta: float) -> void:
	var st := p.state
	if _full_t > 0.0:
		_full_t -= delta
		if _full_t <= 0.0 and st != Player.State.RECOVER:
			tree.set("parameters/full/request", AnimationNodeOneShot.ONE_SHOT_REQUEST_FADE_OUT)

	# ragdoll and get-up own the body; no other events
	if st == Player.State.RAGDOLL or st == Player.State.RECOVER:
		act_stop()
		_tick_act(delta)
		ik.grounded = false
		_carry_w = 0.0
		tree.set("parameters/carry/blend_amount", 0.0)
		if st == Player.State.RAGDOLL:
			_set_mode("fall")
		_prev_state = st
		_prev_hp = p.sheet.hp
		_prev_pos = p.global_position
		_min_vy = 0.0
		return

	var speed := p.last_h_speed
	var ms := p.sheet.v("move_speed")
	var run := ms
	var sprint := minf(ms * Player.SPRINT_MULT, Player.SPRINT_CLAMP)

	# ---- events (derived from state changes) ----
	var hp := p.sheet.hp
	if _prev_hp >= 0.0 and _prev_hp - hp > p.sheet.v("max_hp") * 0.03 and hp > 0.0:
		ik.hit(clampf((_prev_hp - hp) / p.sheet.v("max_hp") * 4.0, 0.3, 1.2))
	_prev_hp = hp

	var carrying := p.carried != null
	_prev_carry = carrying
	_carry_w = move_toward(_carry_w, 1.0 if carrying else 0.0, delta / 0.2)
	tree.set("parameters/carry/blend_amount", _carry_w)

	var dodging := p.dodge_t > 0.0
	if dodging and not _prev_dodge and p.lock.target == null and p.dodge_speed == Player.DODGE_SPEED:
		play_full("Roll", 3.2, 0.42)
	elif dodging and not _prev_dodge and p.lock.target != null:
		# locked on, a dodge is a slip / sway that keeps the eyes on the opponent (the legs step it)
		var right := p.dodge_dir.dot(p.facing.cross(Vector3.UP))
		var n := "ACT_dodge_b" if p.dodge_dir.dot(p.facing) < -0.6 else ("ACT_dodge_r" if right > 0.0 else "ACT_dodge_l")
		act(n, tree.get_animation(n).length / (p.dodge_t + 0.2))
	_prev_dodge = dodging

	if _prev_state == Player.State.CLIMB and st == Player.State.AIR and p.velocity.y > 2.0:
		play_full("ClimbUp_1m", 1.0)

	if st == Player.State.AIR:
		_min_vy = minf(_min_vy, p.velocity.y)
	elif _prev_state == Player.State.AIR and st == Player.State.GROUND:
		ik.land(-_min_vy)
		_min_vy = 0.0
	else:
		_min_vy = 0.0

	# how fast we are actually moving along the wall (hanging still = frozen clip)
	var climb_speed := 0.0
	if st == Player.State.CLIMB:
		var d := p.global_position - _prev_pos
		d -= p.climb_normal * d.dot(p.climb_normal)
		climb_speed = d.length() / maxf(delta, 0.0001)
	_prev_pos = p.global_position

	# ---- base locomotion ----
	var mode := "ground"
	var scale := 1.0
	var stride := 1.0
	match st:
		Player.State.AIR:
			if p.sliding:
				mode = "slide"
			elif p.velocity.y < -2.0 and p.air_time > FALL_AFTER:
				mode = "fall"
			else:
				mode = "air"
		Player.State.CLIMB:
			mode = "climb"
			scale = clampf(climb_speed / NOM_CLIMB, 0.0, 3.5)
		Player.State.SWIM:
			mode = "swim"
			tree.set("parameters/swim/blend_position", clampf(speed / 1.5, 0.0, 1.0))
			scale = clampf(speed / NOM_SWIM, 0.7, 2.0) if speed > 0.4 else 1.0
		_:
			if p.crouching:
				mode = "crouch"
				# fast crouching: longer steps first (up to CROUCH_MAX_STRIDE), then a quicker cadence
				var w := clampf(speed / 0.8, 0.0, 1.0)
				tree.set("parameters/crouch/blend_position", w)
				stride = clampf(speed / (NOM_CROUCH * 1.8), 1.0, CROUCH_MAX_STRIDE)
				scale = speed / (NOM_CROUCH * stride * w) if w > 0.01 else 1.0
				scale = clampf(scale, 0.8, 4.0)
			elif p.pushing and speed < (PUSH_MAX if _prev_mode == "push" else PUSH_MAX - 0.3):
				# a slow, heavy shove: the CMU push. Faster than its legs can go, the prop is rolling away
				# and we just run after it with the hands on it (IK), so the feet keep their footing.
				mode = "push"
				scale = clampf(speed / NOM_PUSH, 0.5, PUSH_MAX / NOM_PUSH)
			else:
				# ease the gait so a one-frame tap or a stop does not snap the legs into a stride
				_gait = lerpf(_gait, gait(speed, run, sprint), 1.0 - exp(-delta * 14.0))
				var g := _gait
				tree.set("parameters/ground/blend_position", g)
				scale = speed / gait_stride(g) if g > 0.01 else WALK_REF / STRIDE_WALK
				scale = clampf(scale, 0.3, 2.6)
	ik.stride = stride
	ik.body_speed = speed
	act_hold(p.combat.guarding())
	_tick_act(delta)
	_drive_ik(p, st, speed, delta)
	# swim clips lie the body flat at foot height; lift it to the waterline
	var lift := 1.2 if st == Player.State.SWIM else 0.0
	inst.position.y = move_toward(inst.position.y, lift, delta * 4.0)
	# the ladder clip keeps the chest ~0.3 m off the rungs: stand the body off the wall so hands reach it
	var off := 0.15 if st == Player.State.CLIMB else 0.0
	inst.position.z = move_toward(inst.position.z, off, delta * 2.0)
	_set_mode(mode)
	_prev_mode = mode
	tree.set("parameters/loco_speed/scale", scale)
	_prev_state = st


## Feed the maths layer from player state (never input).
func _drive_ik(p: Player, st: int, speed: float, delta: float) -> void:
	if ik.exclude.is_empty():
		ik.exclude.append(p.get_rid())
	ik.grounded = st == Player.State.GROUND and (p.dodge_t <= 0.0 or p.lock.target != null) and not p.combat.kicking()
	ik.climbing = st == Player.State.CLIMB
	# fighting: stance, strikes (the maths layer moves the fists; the pack clip is a style layer)
	var c := p.combat
	ik.fight_on = c.engaged() and not c.kicking()  # a kick's clip carries its own guard
	ik.guard_high = c.guarding()
	ik.strike_side = c.strike_side()
	ik.strike_ext = c.extension()
	ik.strike_point = c.aim_point
	ik.strike_hook = bool(c.move.get("hook", false)) and c.striking()
	# the action layer's clip owns the arms; a punch's clip shapes it and the maths lands the fist
	ik.style_w = act_weight()
	ik.strike_ik = bool(c.move.get("ik", true))
	ik.climb_normal = p.climb_normal
	# lean into turns: yaw rate x speed, capped
	var yaw := rotation.y
	var rate := wrapf(yaw - _prev_yaw, -PI, PI) / maxf(delta, 0.0001)
	_prev_yaw = yaw
	ik.turn_bank = clampf(rate * speed * 0.018, -0.22, 0.22) if st == Player.State.GROUND else 0.0
	# hands on whatever we are leaning into
	ik.push_targets = []
	ik.push_effort = 0.0
	if p.pushing and p.push_body != null:
		var space := get_world_3d().direct_space_state
		var fwd := -global_basis.z
		fwd.y = 0.0
		fwd = fwd.normalized()
		var side := fwd.cross(Vector3.UP).normalized()  # character's right
		var pts: Array = []
		for s: float in [-1.0, 1.0]:  # left hand, right hand
			var from := p.global_position + Vector3.UP * 1.25 + side * s * 0.22
			var q := PhysicsRayQueryParameters3D.create(from, from + fwd * 1.3, BodyRig.L_PROPS)
			var hit := space.intersect_ray(q)
			if hit.is_empty() or hit["collider"] != p.push_body:
				pts.clear()
				break
			pts.append(hit["position"] - fwd * 0.04)
		if pts.size() == 2:
			ik.push_targets = pts
			ik.push_effort = clampf(p.push_body.mass / maxf(1.0, p.sheet.v("carry")), 0.2, 1.0)


## Climbing clip cycles per second right now (diagnostic for the self-test).
func climb_rate() -> float:
	if tree.get("parameters/mode/current_state") != "climb":
		return 0.0
	return float(tree.get("parameters/loco_speed/scale")) / tree.get_animation("CMU_Climb").length
