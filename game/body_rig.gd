class_name BodyRig
extends Node3D
## The character's body as real physics: ball / box / capsule rigid bodies on the bones, pinned
## together at the joints. What you see is drawn FROM these bodies (RagdollPose), so nothing the body
## touches can be passed through.
##
## Every physics frame each body is driven toward its animated target (the pose the AnimationTree and
## MotionIK want) with strength 0..1:
##   1.0  active: follows the animation, but walls, steps and props still stop it
##   0.0  ragdoll: gravity, momentum and joint limits only
##   ramp recovery: the body pulls itself from where it lies into the get-up (GetUpPose)
##
## Collision layers (shared with Arena / Player):
const L_WORLD := 1
const L_PLAYER := 2
const L_BODY := 4
const L_PROPS := 8

## part, bone, end bone (sets length), shape, size (radius in x for ball/capsule), mass kg, parent part,
## ragdoll joint swing / twist limits in degrees
const PARTS := [
	["pelvis", "pelvis", "spine_01", "box", Vector3(0.34, 0.20, 0.24), 12.0, "", 0, 0],
	["chest", "spine_02", "neck_01", "box", Vector3(0.30, 0.0, 0.22), 16.0, "pelvis", 30, 20],
	["head", "Head", "", "ball", Vector3(0.12, 0.0, 0.0), 5.0, "chest", 40, 35],
	["upperarm_l", "upperarm_l", "lowerarm_l", "capsule", Vector3(0.055, 0.0, 0.0), 2.5, "chest", 85, 40],
	["lowerarm_l", "lowerarm_l", "hand_l", "capsule", Vector3(0.045, 0.0, 0.0), 1.5, "upperarm_l", 75, 10],
	["hand_l", "hand_l", "middle_01_l", "box", Vector3(0.09, 0.0, 0.04), 0.5, "lowerarm_l", 45, 20],
	["upperarm_r", "upperarm_r", "lowerarm_r", "capsule", Vector3(0.055, 0.0, 0.0), 2.5, "chest", 85, 40],
	["lowerarm_r", "lowerarm_r", "hand_r", "capsule", Vector3(0.045, 0.0, 0.0), 1.5, "upperarm_r", 75, 10],
	["hand_r", "hand_r", "middle_01_r", "box", Vector3(0.09, 0.0, 0.04), 0.5, "lowerarm_r", 45, 20],
	["thigh_l", "thigh_l", "calf_l", "capsule", Vector3(0.075, 0.0, 0.0), 8.0, "pelvis", 70, 20],
	["calf_l", "calf_l", "foot_l", "capsule", Vector3(0.06, 0.0, 0.0), 4.0, "thigh_l", 75, 5],
	["foot_l", "foot_l", "ball_l", "box", Vector3(0.10, 0.0, 0.07), 1.0, "calf_l", 30, 10],
	["thigh_r", "thigh_r", "calf_r", "capsule", Vector3(0.075, 0.0, 0.0), 8.0, "pelvis", 70, 20],
	["calf_r", "calf_r", "foot_r", "capsule", Vector3(0.06, 0.0, 0.0), 4.0, "thigh_r", 75, 5],
	["foot_r", "foot_r", "ball_r", "box", Vector3(0.10, 0.0, 0.07), 1.0, "calf_r", 30, 10],
]
## Physical foot: world up and the floor point under the ankle, in the foot bone's frame, with the foot
## flat on the floor (Idle clip); heel / toe reach along the sole from under the ankle.
const FOOT_UP_LOCAL := Vector3(0.0, -0.511, -0.860)
const FOOT_SOLE_LOCAL := Vector3(0.0, 0.0258, 0.0435)
const FOOT_HEEL := -0.05
const FOOT_TOE := 0.155    # ends at the ball joint: the toes bend, a rigid toe would dig in on tiptoe
const FOOT_BOX := Vector3(0.10, 0.205, 0.07)
const SNAP_DISTANCE := 0.8  # a body this far from its target (teleport) is just placed there
const MAX_DRIVE_SPEED := 40.0
const RECOVER_TIME := 0.7    # seconds for strength to ramp 0 -> 1 when getting up

var skeleton: Skeleton3D
var parts: Array = []        # [{name, bone, body, parent, swing, twist}]
var by_name := {}
var targets := {}            # bone -> skeleton-space Transform3D the animation wants (filled by RagdollPose)
var _prev_targets := {}      # the update before that
var _since := 0              # physics steps since the last update
var _span := 1               # physics steps the last update covered
var extrapolate := false
var soft_catch := false       # getting up: follow the target's motion, close gaps at CATCH_SPEED
const CATCH_SPEED := 1.6
const CATCH_SPIN := 9.0     # carry the pose's motion across physics steps between drawn frames (get-up)
var radius := {}             # part name -> rough half-thickness, to keep targets out of walls
var strength := 1.0
## 0: stiff, each part is put on its target within one step (locomotion). > 0 (getting up): parts move
## with their target and close any gap over this many seconds, so a limb that comes free of something
## it was caught on eases back instead of jumping.
var catch_up_time := 0.0
var _last_basis := {}
var ragdolling := false
var _strength_target := 1.0
var _last_target := {}
var tracking_error := 0.0    # worst miss (m) of a part vs the target it was given last step (diagnostic)
var tracking_part := ""
var tracking_by_part := {}
var _limits: Array = []
var _give := {}              # part -> [how much the drive lets go 0..1, seconds left, seconds total] (blows)
## Fists and forearms go soft while they touch another fighter, so they rest on a body instead of two
## stiff drives pushing into each other.
const SOFT_ON_TOUCH := ["hand_l", "hand_r", "lowerarm_l", "lowerarm_r"]
const TOUCH_HOLD := 0.0
var _loose_pairs: Array = []  # [body, body] own-part pairs that may touch while ragdolled / getting up
var _self_collide := false


func setup(sk: Skeleton3D) -> void:
	skeleton = sk
	for spec in PARTS:
		var bone := sk.find_bone(spec[1])
		if bone < 0:
			push_warning("BodyRig: missing bone " + spec[1])
			continue
		var length := 0.0
		if spec[2] != "":
			var end := sk.find_bone(spec[2])
			length = sk.get_bone_global_rest(bone).origin.distance_to(sk.get_bone_global_rest(end).origin)
		var body := RigidBody3D.new()
		body.name = "Part_" + spec[0]
		body.top_level = true
		body.mass = spec[5]
		body.gravity_scale = 0.0
		body.can_sleep = false
		body.continuous_cd = true
		body.collision_layer = L_BODY
		body.collision_mask = L_WORLD | L_PROPS | L_BODY  # other fighters' bodies; our own parts are excepted
		var pm := PhysicsMaterial.new()
		pm.friction = 0.0  # following the animation: slide along surfaces, don't drag on them
		pm.bounce = 0.0
		body.physics_material_override = pm
		var cs := CollisionShape3D.new()
		var size: Vector3 = spec[4]
		match spec[3]:
			"ball":
				var s := SphereShape3D.new()
				s.radius = size.x
				cs.shape = s
				cs.position = Vector3(0.0, 0.1, 0.0)
			"capsule":
				var c := CapsuleShape3D.new()
				c.radius = size.x
				c.height = maxf(length + size.x, size.x * 2.0 + 0.01)
				cs.shape = c
				cs.position = Vector3(0.0, length * 0.5, 0.0)
			_:
				var b := BoxShape3D.new()
				b.size = Vector3(size.x, maxf(size.y, length + 0.04), size.z)
				cs.shape = b
				cs.position = Vector3(0.0, b.size.y * 0.5 - 0.02, 0.0)
				if spec[0].begins_with("foot"):
					# the foot bone runs ankle -> ball, pitched ~31 deg down when the foot is flat; a box on
					# that line would dig its toe into the floor and tip the physical foot heel-down. Lay the
					# box along the SOLE instead (measured from the Idle clip by tools/foot_probe.gd), so the
					# physical foot is flat exactly when the animated one is.
					var up := FOOT_UP_LOCAL.normalized()
					var fwd := (Vector3.UP - up * up.y).normalized()
					var side := up.cross(fwd).normalized()
					b.size = FOOT_BOX
					cs.transform = Transform3D(Basis(side, fwd, -up),
						FOOT_SOLE_LOCAL + fwd * (FOOT_TOE + FOOT_HEEL) * 0.5 + up * FOOT_BOX.z * 0.5)
		body.add_child(cs)
		add_child(body)
		radius[spec[0]] = size.x * 0.5 if spec[3] == "box" else size.x
		var part := {"name": spec[0], "bone": bone, "body": body, "parent": spec[6], "swing": spec[7], "twist": spec[8]}
		parts.append(part)
		by_name[spec[0]] = part
	snap_to(func(bone: int) -> Transform3D: return skeleton.global_transform * skeleton.get_bone_global_pose(bone))
	# pin parts that hang directly off their parent's bone (hip, knee, ankle, elbow, wrist) so limbs
	# stay attached even when blocked. Parts joined through animated in-between bones (clavicle,
	# spine, neck) are only linked in ragdoll, where the joint is built from the pose at that moment.
	# Own parts never touch each other while driven (all 105 pairs excepted). Parent / grandparent pairs
	# stay excepted for good; the rest are let loose while ragdolled and getting up (_set_self_collide).
	var permanent := {}
	for p in parts:
		if p["parent"] == "":
			continue
		permanent[p["name"] + "|" + p["parent"]] = true
		# a joint folded right up (heel to backside, hand to shoulder) brings a part against its
		# grandparent; flesh squashes there, colliders would jam
		var gp: String = by_name[p["parent"]]["parent"]
		if gp != "" and p["name"] != "head" and not p["name"].begins_with("upperarm"):
			permanent[p["name"] + "|" + gp] = true
	for a in parts.size():
		for b in range(a + 1, parts.size()):
			var ba: RigidBody3D = parts[a]["body"]
			var bb: RigidBody3D = parts[b]["body"]
			ba.add_collision_exception_with(bb)
			bb.add_collision_exception_with(ba)
			var na: String = parts[a]["name"]
			var nb: String = parts[b]["name"]
			if not permanent.has(na + "|" + nb) and not permanent.has(nb + "|" + na):
				_loose_pairs.append([ba, bb])
	for p in parts:
		if p["parent"] == "":
			continue
		if skeleton.get_bone_parent(p["bone"]) != by_name[p["parent"]]["bone"]:
			continue
		var pin := PinJoint3D.new()
		add_child(pin)
		pin.top_level = true
		pin.global_position = p["body"].global_position
		pin.node_a = pin.get_path_to(by_name[p["parent"]]["body"])
		pin.node_b = pin.get_path_to(p["body"])


func snap_to(target_of: Callable) -> void:
	for p in parts:
		var b: RigidBody3D = p["body"]
		b.global_transform = (target_of.call(p["bone"]) as Transform3D).orthonormalized()
		b.linear_velocity = Vector3.ZERO
		b.angular_velocity = Vector3.ZERO
		b.reset_physics_interpolation()


## New pose from the animation (once per drawn frame). Below 60 fps one drawn frame spans several
## physics steps; the steps in between carry the pose's motion on instead of standing still and then
## jumping (which shook every part).
func set_targets(fresh: Dictionary) -> void:
	_prev_targets = targets
	_span = clampi(_since, 1, 4)
	targets = fresh
	_since = 0


func _target(bone: int) -> Transform3D:
	var t: Transform3D = targets[bone]
	if not extrapolate or _since == 0 or not _prev_targets.has(bone):
		return t
	var w := 1.0 + minf(float(_since), float(_span)) / float(_span)
	var pv: Transform3D = _prev_targets[bone]
	var q := pv.basis.get_rotation_quaternion().slerp(t.basis.get_rotation_quaternion(), w)
	return Transform3D(Basis(q), pv.origin.lerp(t.origin, w))


func pelvis() -> RigidBody3D:
	return by_name["pelvis"]["body"]


## Own non-adjacent parts collide with each other (ragdoll, get-up) or not (driven by the animation).
func _set_self_collide(on: bool) -> void:
	if on == _self_collide:
		return
	_self_collide = on
	for pair in _loose_pairs:
		var a: RigidBody3D = pair[0]
		var b: RigidBody3D = pair[1]
		if on:
			a.remove_collision_exception_with(b)
			b.remove_collision_exception_with(a)
		else:
			a.add_collision_exception_with(b)
			b.add_collision_exception_with(a)


func self_colliding() -> bool:
	return _self_collide


func exclude(other: PhysicsBody3D, on: bool) -> void:
	for p in parts:
		if on:
			p["body"].add_collision_exception_with(other)
			other.add_collision_exception_with(p["body"])
		else:
			p["body"].remove_collision_exception_with(other)
			other.remove_collision_exception_with(p["body"])


# ---- drive -----------------------------------------------------------------------------------

func _physics_process(delta: float) -> void:
	if skeleton == null or targets.is_empty():
		return
	strength = move_toward(strength, _strength_target, delta / RECOVER_TIME)
	var sgt := skeleton.global_transform
	var space := get_world_3d().direct_space_state
	for p in parts:
		var b: RigidBody3D = p["body"]
		var k := strength * _hold(p["name"], delta)
		if p["name"] in SOFT_ON_TOUCH:
			if not b.contact_monitor:
				b.contact_monitor = true
				b.max_contacts_reported = 4
			for o in b.get_colliding_bodies():
				if o.has_meta("fighter") and o.get_meta("fighter") != b.get_meta("fighter", null):
					k *= TOUCH_HOLD
					break
		b.gravity_scale = 1.0 - k
		if strength <= 0.001 or not targets.has(p["bone"]):
			continue
		var target: Transform3D = (sgt * _target(p["bone"])).orthonormalized()
		# never aim a part inside the world
		if p["parent"] != "" and extrapolate:
			# getting up (limbs along the floor): lift a target that is too low straight up, push one inside a
			# wall straight out. Both move it continuously, so a target never jumps.
			var r: float = radius[p["name"]]
			var down := space.intersect_ray(PhysicsRayQueryParameters3D.create(target.origin + Vector3.UP * 0.5,
				target.origin + Vector3.DOWN * 0.5, L_WORLD))
			if not down.is_empty() and (down["normal"] as Vector3).y > 0.5:
				var fy: float = (down["position"] as Vector3).y
				# on a slope the surface is nearer than straight down: allow for the tilt
				var need := fy + r / maxf(0.5, (down["normal"] as Vector3).y)
				if target.origin.y < need:
					target.origin.y = need
			var from: Vector3 = by_name[p["parent"]]["body"].global_position
			var hit := space.intersect_ray(PhysicsRayQueryParameters3D.create(from, target.origin, L_WORLD))
			if not hit.is_empty() and absf((hit["normal"] as Vector3).y) < 0.5:
				var n: Vector3 = hit["normal"]
				var depth := ((hit["position"] as Vector3) - target.origin).dot(n) + r
				if depth > 0.0:
					target.origin += n * depth
		elif p["parent"] != "":
			# stop the target at the surface between its joint and it
			var from: Vector3 = by_name[p["parent"]]["body"].global_position
			var q0 := PhysicsRayQueryParameters3D.create(from, target.origin, L_WORLD)
			var hit := space.intersect_ray(q0)
			if not hit.is_empty():
				target.origin = hit["position"] + hit["normal"] * radius[p["name"]]
		if strength >= 0.999 and _last_target.has(p["name"]):
			var miss: float = (_last_target[p["name"]] as Vector3).distance_to(b.global_position)
			tracking_by_part[p["name"]] = maxf(tracking_by_part.get(p["name"], 0.0), miss)
			if miss > tracking_error:
				tracking_error = miss
				tracking_part = p["name"]
		# how the target itself is moving (followed while catching up softly)
		var target_v := Vector3.ZERO
		var target_w := Vector3.ZERO
		if _last_target.has(p["name"]) and _last_basis.has(p["name"]):
			target_v = (target.origin - (_last_target[p["name"]] as Vector3)) / delta
			var dq := target.basis.get_rotation_quaternion() * (_last_basis[p["name"]] as Basis).get_rotation_quaternion().inverse()
			if dq.w < 0.0:
				dq = -dq
			if dq.get_angle() > 0.0001:
				target_w = dq.get_axis() * dq.get_angle() / delta
		_last_target[p["name"]] = target.origin
		_last_basis[p["name"]] = target.basis
		var err := target.origin - b.global_position
		if err.length() > SNAP_DISTANCE and strength >= 0.999:
			b.global_transform = target
			b.linear_velocity = Vector3.ZERO
			b.angular_velocity = Vector3.ZERO
			continue
		var tau := maxf(delta, catch_up_time)
		var q := target.basis.get_rotation_quaternion() * b.global_basis.get_rotation_quaternion().inverse()
		if q.w < 0.0:
			q = -q
		var ang := q.get_angle()
		var want_v: Vector3
		var want_w: Vector3
		if soft_catch:
			# move with the target, and close any gap at a capped rate: a part that fell behind (caught on
			# something, or while the drive was still weak) catches up visibly instead of in one step
			want_v = target_v + (err / delta).limit_length(CATCH_SPEED)
			want_w = target_w + ((q.get_axis() * ang / delta).limit_length(CATCH_SPIN) if ang > 0.0005 else Vector3.ZERO)
			want_v = want_v.limit_length(MAX_DRIVE_SPEED)
		else:
			want_v = (err / tau + (target_v if catch_up_time > 0.0 else Vector3.ZERO)).limit_length(MAX_DRIVE_SPEED)
			want_w = (q.get_axis() * ang / tau).limit_length(MAX_DRIVE_SPEED) if ang > 0.0005 else Vector3.ZERO
			if catch_up_time > 0.0:
				want_w += target_w
		# a body turns about its centre of mass, which sits mid-limb, not on the joint: give the centre
		# the velocity that keeps the JOINT on its path while the part turns (else every turn drags the
		# joint off target and the pins fight it; feet could not roll onto their toes)
		var com: Vector3 = PhysicsServer3D.body_get_direct_state(b.get_rid()).center_of_mass
		want_v += want_w.cross(com)
		b.linear_velocity = b.linear_velocity.lerp(want_v, k)
		b.angular_velocity = b.angular_velocity.lerp(want_w, k)
	_since += 1


## A blow: the struck part (and less so the parts it hangs from) goes slack for `time` so the impulse
## really moves it, then the drive takes it back.
func flinch(part: String, impulse: Vector3, time: float) -> void:
	var pn := part
	var amount := 0.85
	while pn != "" and by_name.has(pn) and amount > 0.1:
		_give[pn] = [amount, time, time]
		amount *= 0.5
		pn = by_name[pn]["parent"]
	if by_name.has(part):
		(by_name[part]["body"] as RigidBody3D).apply_central_impulse(impulse)


func _hold(part: String, delta: float) -> float:
	if not _give.has(part):
		return 1.0
	var g: Array = _give[part]
	g[1] = (g[1] as float) - delta
	if (g[1] as float) <= 0.0:
		_give.erase(part)
		return 1.0
	return 1.0 - (g[0] as float) * (g[1] as float) / (g[2] as float)


func rids() -> Array[RID]:
	var out: Array[RID] = []
	for p in parts:
		out.append((p["body"] as RigidBody3D).get_rid())
	return out


# ---- ragdoll ---------------------------------------------------------------------------------

## Go limp. Every part keeps the velocity it already had (real momentum); `push` is added on top.
func start_ragdoll(push: Vector3) -> void:
	if ragdolling:
		return
	ragdolling = true
	strength = 0.0
	_strength_target = 0.0
	for p in parts:
		var b: RigidBody3D = p["body"]
		b.linear_velocity += push
		b.linear_damp = 0.05
		b.angular_damp = 0.6
		b.physics_material_override.friction = 0.9      # limp bodies grip and tumble
	_set_self_collide(true)  # limbs can't pass through the torso
	for p in parts:
		if p["parent"] == "":
			continue
		var j := ConeTwistJoint3D.new()
		add_child(j)
		j.top_level = true
		var bt: Transform3D = p["body"].global_transform
		# ConeTwist twists about its X axis: line X up with the bone (bone +Y runs along the limb)
		j.global_transform = Transform3D(Basis(bt.basis.y, -bt.basis.x, bt.basis.z).orthonormalized(), bt.origin)
		j.set_param(ConeTwistJoint3D.PARAM_SWING_SPAN, deg_to_rad(p["swing"]))
		j.set_param(ConeTwistJoint3D.PARAM_TWIST_SPAN, deg_to_rad(p["twist"]))
		j.node_a = j.get_path_to(by_name[p["parent"]]["body"])
		j.node_b = j.get_path_to(p["body"])
		_limits.append(j)


## Sliding fast along the ground: drag the body down (hands and feet dig in).
func brace(amount: float) -> void:
	for p in parts:
		var b: RigidBody3D = p["body"]
		b.linear_damp = 0.05 + 2.5 * amount
		b.angular_damp = 0.6 + 3.0 * amount


## Start pulling back toward the animation (strength ramps up over RECOVER_TIME). With `self_collide`
## the limbs keep colliding with the torso and each other (getting up), else not with their own parts.
func recover(self_collide: bool = false) -> void:
	catch_up_time = 0.0
	extrapolate = self_collide
	soft_catch = false
	for j in _limits:
		j.queue_free()
	_limits.clear()
	ragdolling = false
	_strength_target = 1.0
	for p in parts:
		var b: RigidBody3D = p["body"]
		b.linear_damp = 0.0
		b.angular_damp = 0.0
		b.physics_material_override.friction = 0.0
	_set_self_collide(self_collide)


## Own limbs stop colliding with each other again (after a get-up, so running limbs can brush past).
func solo() -> void:
	soft_catch = false
	catch_up_time = 0.0
	extrapolate = false
	_set_self_collide(false)


func reset() -> void:
	if ragdolling:
		recover()
	soft_catch = false
	catch_up_time = 0.0
	extrapolate = false
	_set_self_collide(false)
	strength = 1.0
	_strength_target = 1.0
	if not targets.is_empty():
		var sgt := skeleton.global_transform
		snap_to(func(bone: int) -> Transform3D: return sgt * (targets.get(bone, Transform3D.IDENTITY) as Transform3D))


## How far the furthest-behind part is from where it was told to be (m).
func worst_miss() -> float:
	var w := 0.0
	for p in parts:
		if _last_target.has(p["name"]):
			w = maxf(w, (_last_target[p["name"]] as Vector3).distance_to(p["body"].global_position))
	return w


func speed() -> float:
	return pelvis().linear_velocity.length()


## Which way the chest faces: +1 face up, -1 face down.
func facing_up() -> float:
	var chest: RigidBody3D = by_name["chest"]["body"]
	var rest := skeleton.get_bone_global_rest(by_name["chest"]["bone"])
	var fwd: Vector3 = (chest.global_basis * rest.basis.inverse()) * Vector3(0.0, 0.0, 1.0)
	return signf(fwd.y)


func head_to_pelvis() -> Vector3:
	var h: Vector3 = pelvis().global_position - by_name["head"]["body"].global_position
	h.y = 0.0
	return h.normalized() if h.length() > 0.01 else Vector3.FORWARD
