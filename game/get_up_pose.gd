class_name GetUpPose
extends SkeletonModifier3D
## Getting up, built from wherever the ragdoll came to rest. No clip.
##
## The body moves only by turning its joints (every joint is a fixed pivot, bones keep their length),
## and whatever pushes on the floor - palms, knees, feet - is a CONTACT: it stays exactly where it was
## put down until the sequence lifts it. The feet you end up standing on are planted once and never
## slide, and the last pose IS the standing idle pose, so the animation takes over without a jump.
##
##   face down: chest up onto the palms -> knees under the hips (all fours) -> right foot forward
##              (lunge) -> left foot in (squat) -> stand
##   face up:   sit up onto the palms, knees bent, both feet planted -> rock forward over the feet
##              (squat) -> stand
##
## Phase one turns every part from its exact ragdoll orientation into the first support pose, so
## there is no snap; after that the limbs are two-bone IK onto their contacts.
##
## While ragdolling it also bends the spine and neck between the physical pelvis, chest and head, so
## the drawn ragdoll and the first get-up frame are the same pose.
##
## Runs after MotionIK and before RagdollPose: it writes the pose the physics body is driven toward.

const TORSO := ["pelvis", "spine_01", "spine_02", "spine_03", "neck_01", "Head", "clavicle_l", "clavicle_r"]
const LIMBS := {
	"arm_l": ["upperarm_l", "lowerarm_l", "hand_l"], "arm_r": ["upperarm_r", "lowerarm_r", "hand_r"],
	"leg_l": ["thigh_l", "calf_l", "foot_l"], "leg_r": ["thigh_r", "calf_r", "foot_r"],
}
const PALM_H := 0.05       # rough wrist height over the floor with the palm flat (planning only)
const PALM_FROM_WRIST := 0.08
const KNEE_R := 0.075      # knee (calf capsule) radius: a kneeling knee's height over the floor
const LIFT := {"arm_l": 0.06, "arm_r": 0.06, "leg_l": 0.2, "leg_r": 0.2}
const MASK := BodyRig.L_WORLD | BodyRig.L_PROPS

var rig: BodyRig
var running := false
var kind := ""             # "front" (face down) or "back"
var t := 0.0
var duration := 0.0

var B := {}                # bone name -> index
var ORDER: Array = []      # every bone we write, parents first
var par := {}              # bone -> parent bone
var loc := {}              # bone -> standing pose relative to its parent
var idle := {}             # bone -> standing (Idle) pose, skeleton space
var _len := {}             # upper / lower limb bone -> length to its child
var _pole_idle := {}       # limb -> which way its knee / elbow points when standing
var _palm_local := {}      # hand -> palm normal in the hand bone's frame
var _clav_rest := {}       # clavicle -> bind-pose basis relative to spine_03 (left / right symmetric)
var _clav_w := 1.0         # 0 = bind-pose shoulders, 1 = the Idle clip's (which holds the right one back)
var _s1_from_p: Basis
var _s1_from_c: Basis
var _neck_from_h: Basis
var arm_len := 0.0
var leg_len := 0.0
var short := {}            # limb -> how far (m) its contact is out of reach right now (diagnostic)

var _keys: Array = []      # skeleton space once started
var _start := {}           # {"o", "b"} the ragdoll's pose at the start, skeleton space
var _start_end := {}       # limb -> where its wrist / ankle was at the start
var _start_pole := {}      # limb -> which way its elbow / knee pointed at the start
var _start_turn := {}      # limb -> how far (rad, signed) its elbow / knee turns round the limb in phase one
var _wh := Transform3D.IDENTITY   # heading frame (origin on the floor under the pelvis) -> world
var _space: PhysicsDirectSpaceState3D
var _rate := 1.0


## `stand` is the Idle clip's pose (bone name -> skeleton-space Transform3D).
func setup(stand: Dictionary) -> void:
	var sk := get_skeleton()
	ORDER = TORSO.duplicate()
	for limb in LIMBS:
		ORDER.append_array(LIMBS[limb])
	for n in ORDER:
		B[n] = sk.find_bone(n)
		idle[n] = stand[n]
	for n in ORDER:
		if n == "pelvis":
			continue
		par[n] = sk.get_bone_name(sk.get_bone_parent(B[n]))
		loc[n] = (idle[par[n]] as Transform3D).affine_inverse() * (idle[n] as Transform3D)
	for c: String in ["clavicle_l", "clavicle_r"]:
		_clav_rest[c] = sk.get_bone_rest(B[c]).basis
	_s1_from_p = _b("pelvis").inverse() * _b("spine_01")
	_s1_from_c = _b("spine_02").inverse() * _b("spine_01")
	_neck_from_h = _b("Head").inverse() * _b("neck_01")
	for limb in LIMBS:
		var u: String = LIMBS[limb][0]
		var l: String = LIMBS[limb][1]
		var e: String = LIMBS[limb][2]
		_len[u] = (loc[l] as Transform3D).origin.length()
		_len[l] = (loc[e] as Transform3D).origin.length()
		var root := _o(u)
		var reach := _o(e) - root
		var bend := _o(l) - root
		bend -= reach.normalized() * bend.dot(reach.normalized())
		if bend.length() > 0.01:
			_pole_idle[limb] = bend.normalized()
		else:
			_pole_idle[limb] = Vector3(0, 0, 1) if limb.begins_with("leg") else Vector3(0, 0, -1)
	arm_len = _len["upperarm_l"] + _len["lowerarm_l"]
	leg_len = _len["thigh_l"] + _len["calf_l"]
	for h in ["hand_l", "hand_r"]:
		_palm_local[h] = (sk.get_bone_global_rest(B[h]).basis.inverse() * Vector3.DOWN).normalized()


func _b(n: String) -> Basis:
	return (idle[n] as Transform3D).basis


func _o(n: String) -> Vector3:
	return (idle[n] as Transform3D).origin


# ---- planning ----------------------------------------------------------------------------------

## Plan a get-up for a ragdoll lying with its pelvis over `floor_pt`, standing up facing `fwd`.
## Returns how far along `fwd` the standing spot is (the caller puts the character there, then calls
## start()). `force_d` >= 0 overrides that distance (no room where the plan wanted to stand).
func plan(face_down: bool, floor_pt: Vector3, fwd: Vector3, force_d: float = -1.0) -> float:
	_space = get_skeleton().get_world_3d().direct_space_state
	_clav_w = 0.0
	kind = "front" if face_down else "back"
	var f := Vector3(fwd.x, 0.0, fwd.z).normalized()
	var left := Vector3.UP.cross(f).normalized()
	_wh = Transform3D(Basis(left, Vector3.UP, f), floor_pt)
	var py := clampf((_wh.affine_inverse() * rig.pelvis().global_position).y, 0.11, 0.3)
	return _build_front(py, force_d) if face_down else _build_back(py, force_d)


## How many of the planned contacts (palms, knees / feet, the standing feet) are blocked: inside a wall
## or a prop, or with something solid between the body and them. 0 = the plan fits this place.
func blocked() -> int:
	var n := 0
	var seen := {}
	var from := _wh * Vector3(0.0, 0.35, 0.0)
	for k in _keys:
		for limb in k["limbs"]:
			var lk: Dictionary = k["limbs"][limb]
			if lk["mode"] != "ik":
				continue
			var at: Vector3 = _wh * (lk["end"] as Vector3)
			var key := "%s %.2f %.2f %.2f" % [limb, at.x, at.y, at.z]
			if seen.has(key):
				continue
			seen[key] = true
			var q := PhysicsShapeQueryParameters3D.new()
			var sph := SphereShape3D.new()
			sph.radius = 0.05
			q.shape = sph
			q.transform = Transform3D(Basis.IDENTITY, at + Vector3.UP * 0.04)
			q.collision_mask = MASK
			var through := _space.intersect_ray(PhysicsRayQueryParameters3D.create(from, at + Vector3.UP * 0.1, MASK))
			if not _space.intersect_shape(q, 1).is_empty() or not through.is_empty():
				n += 1
	return n


## Begin, with the character already standing at the planned spot (skeleton space is now final).
func start() -> float:
	var sk := get_skeleton()
	var to_s := sk.global_transform.affine_inverse() * _wh   # heading frame -> skeleton space
	var keys: Array = []
	for k in _keys:
		var limbs := {}
		for limb in k["limbs"]:
			var lk: Dictionary = k["limbs"][limb]
			if lk["mode"] == "ik":
				limbs[limb] = {"mode": "ik", "end": to_s * (lk["end"] as Vector3),
					"eb": to_s.basis * (lk["eb"] as Basis), "pole": to_s.basis * (lk["pole"] as Vector3)}
			else:
				limbs[limb] = lk
		keys.append({"t": k["t"], "o": to_s * (k["o"] as Vector3), "pb": to_s.basis * (k["pb"] as Basis),
			"cb": to_s.basis * (k["cb"] as Basis), "hb": to_s.basis * (k["hb"] as Basis), "limbs": limbs})
	_keys = keys
	# the ragdoll exactly as it lies (parts from physics, spine and neck between them as drawn)
	var inv := sk.global_transform.affine_inverse()
	var pose := {}
	for p in rig.parts:
		pose[sk.get_bone_name(p["bone"])] = (inv * (p["body"] as RigidBody3D).global_transform).basis.orthonormalized()
	var bases := {}
	_clav_w = 1.0
	_torso(pose["pelvis"], pose["spine_02"], pose["Head"], bases)
	for limb in LIMBS:
		for n in LIMBS[limb]:
			bases[n] = pose[n]
	_start = {"o": inv * rig.pelvis().global_position, "b": bases}
	var T0 := _fk(_start["o"], bases)
	for limb in LIMBS:
		var names: Array = LIMBS[limb]
		_start_end[limb] = (T0[names[2]] as Transform3D).origin
		_start_pole[limb] = (bases[names[0]] as Basis) * (_b(names[0]).inverse() * (_pole_idle[limb] as Vector3))
		# which way (and how far) the knee / elbow turns round the limb on the way to the first pose:
		# decided once, so it can never flip sides half way
		var root0: Vector3 = (T0[par[names[0]]] as Transform3D) * (loc[names[0]] as Transform3D).origin
		var ax := ((_start_end[limb] as Vector3) - root0).normalized()
		var q0: Vector3 = _start_pole[limb]
		var q1: Vector3 = ((_keys[0] as Dictionary)["limbs"][limb] as Dictionary).get("pole", q0)
		q0 = (q0 - ax * q0.dot(ax)).normalized()
		q1 = (q1 - ax * q1.dot(ax)).normalized()
		_start_turn[limb] = atan2(q0.cross(q1).dot(ax), q0.dot(q1)) if q0.is_normalized() and q1.is_normalized() else 0.0
	t = 0.0
	_rate = 1.0
	duration = (_keys.back() as Dictionary)["t"]
	running = true
	return duration


func done() -> bool:
	return not running or t >= duration


## Floor height (skeleton space) under a skeleton-space point.
func _floor_s(p: Vector3) -> float:
	var gt := get_skeleton().global_transform
	var w := gt * p
	var q := PhysicsRayQueryParameters3D.create(w + Vector3.UP * 0.6, w + Vector3.DOWN * 1.5, MASK)
	var hit := _space.intersect_ray(q)
	if hit.is_empty():
		return -INF
	return (gt.affine_inverse() * (hit["position"] as Vector3)).y


func stop() -> void:
	running = false


## Is this limb's contact (palm / knee / foot) held still right now?
func is_planted(limb: String) -> bool:
	if not running or _keys.is_empty() or t < (_keys[0] as Dictionary)["t"]:
		return false
	var i := _interval()
	var a: Dictionary = (_keys[i] as Dictionary)["limbs"][limb]
	var b: Dictionary = (_keys[mini(i + 1, _keys.size() - 1)] as Dictionary)["limbs"][limb]
	return a["mode"] == "ik" and b["mode"] == "ik" and (a["end"] as Vector3).distance_to(b["end"]) < 0.005


static func _pitch(deg: float) -> Basis:
	return Basis(Vector3(1, 0, 0), deg_to_rad(deg))  # + tips the body forward (head toward +Z)


func _g(p: Vector3) -> float:
	var from := _wh * Vector3(p.x, p.y + 0.9, p.z)
	var to := _wh * Vector3(p.x, p.y - 1.2, p.z)
	var q := PhysicsRayQueryParameters3D.create(from, to, MASK)
	var hit := _space.intersect_ray(q)
	if hit.is_empty():
		return 0.0
	return (_wh.affine_inverse() * (hit["position"] as Vector3)).y


func _ik_key(end: Vector3, eb: Basis, pole: Vector3) -> Dictionary:
	return {"mode": "ik", "end": end, "eb": eb, "pole": pole}


## A stage pose. The torso is moved (if it has to be) until every hand / foot on the floor is within
## reach, so on slopes, steps or bumps the stage still fits the ground it is on.
var settle := true


func _key(time: float, o: Vector3, pb: Basis, cb: Basis, hb: Basis, limbs: Dictionary) -> Dictionary:
	for it in (6 if settle else 0):
		var r := _roots(o, pb, cb, hb)
		var pull := Vector3.ZERO
		for limb in limbs:
			var lk: Dictionary = limbs[limb]
			if lk["mode"] != "ik":
				continue
			var names: Array = LIMBS[limb]
			var to: Vector3 = (lk["end"] as Vector3) - (r[limb] as Vector3)
			var reach: float = (_len[names[0]] as float) + (_len[names[1]] as float)
			var ex := to.length() - 0.98 * reach
			if ex > 0.0:
				pull += to.normalized() * ex
		if pull.length() < 0.002:
			break
		o += pull * 0.7
	return {"t": time, "o": o, "pb": pb, "cb": cb, "hb": hb, "limbs": limbs}


## Hand basis with the palm flat on the floor and the fingers toward `fingers`.
func _palm_basis(hand: String, fingers: Vector3) -> Basis:
	var ly := Vector3.UP
	var lp: Vector3 = _palm_local[hand]
	lp = (lp - ly * lp.dot(ly)).normalized()
	var wy := Vector3(fingers.x, 0.0, fingers.z).normalized()
	var wp := Vector3.DOWN
	return (Basis(wy.cross(wp), wy, wp) * Basis(ly.cross(lp), ly, lp).inverse()).orthonormalized()


## Foot laid top-down behind a kneeling shin: the standing foot turned over, sole up, toes back.
func _kneel_foot(foot: String) -> Basis:
	return _pitch(180.0) * _b(foot)


## How far `bone`'s collision shape, placed at T (heading frame), dips below floor height g (plus a few
## mm). Raise a contact by this and it RESTS on the floor instead of being driven into it.
func _sink(bone: String, T: Transform3D, g: float) -> float:
	for p in rig.parts:
		if p["bone"] == B[bone]:
			var cs: CollisionShape3D = (p["body"] as RigidBody3D).get_child(0)
			return g + 0.004 - _lowest(T * cs.transform, cs.shape)
	return 0.0


static func _lowest(t: Transform3D, sh: Shape3D) -> float:
	if sh is SphereShape3D:
		return t.origin.y - (sh as SphereShape3D).radius
	if sh is CapsuleShape3D:
		var c := sh as CapsuleShape3D
		var hh := c.height * 0.5 - c.radius
		return minf((t * Vector3(0, hh, 0)).y, (t * Vector3(0, -hh, 0)).y) - c.radius
	var e: Vector3 = (sh as BoxShape3D).size * 0.5
	var lo := INF
	for sx in [-1.0, 1.0]:
		for sy in [-1.0, 1.0]:
			for sz in [-1.0, 1.0]:
				lo = minf(lo, (t * Vector3(e.x * sx, e.y * sy, e.z * sz)).y)
	return lo


## A contact (hand / foot bone with basis `b`) put down on the floor at x, z: its bone position.
func _rest(bone: String, b: Basis, x: float, z: float) -> Vector3:
	var g := _g(Vector3(x, 0.0, z))
	return Vector3(x, g + _sink(bone, Transform3D(b, Vector3(x, g, z)), g), z)


## Where the shoulders and hips are for a torso pose (heading frame).
func _roots(o: Vector3, pb: Basis, cb: Basis, hb: Basis) -> Dictionary:
	var bases := {}
	_torso(pb, cb, hb, bases)
	var T := _fk_torso(o, bases)
	var r := {}
	for s in ["l", "r"]:
		r["arm_" + s] = (T["clavicle_" + s] as Transform3D) * (loc["upperarm_" + s] as Transform3D).origin
		r["leg_" + s] = (T["pelvis"] as Transform3D) * (loc["thigh_" + s] as Transform3D).origin
	return r


static func _mid(r: Dictionary, what: String) -> Vector3:
	return ((r[what + "_l"] as Vector3) + (r[what + "_r"] as Vector3)) * 0.5


func _stand_foot(stand: Transform3D, s: String) -> Vector3:
	var a := stand * _o("foot_" + s)
	a.y = _g(a) + _o("foot_" + s).y
	return a


func _stand_key(time: float, stand: Transform3D) -> Dictionary:
	var limbs := {"arm_l": {"mode": "free"}, "arm_r": {"mode": "free"}}
	for s in ["l", "r"]:
		limbs["leg_" + s] = _ik_key(_stand_foot(stand, s), _b("foot_" + s), _pole_idle["leg_" + s])
	return _key(time, stand * _o("pelvis"), _b("pelvis"), _b("spine_02"), _b("Head"), limbs)


func _build_front(py: float, force_d: float) -> float:
	var P := _b("pelvis")
	var C := _b("spine_02")
	var H := _b("Head")
	_keys = []
	# 1. chest up onto the palms: palms under the shoulders, arms a little bent, legs straight back
	var o1 := Vector3(0.0, py, 0.0)
	var pb1 := _pitch(80.0) * P
	var wy := _g(Vector3(0.0, 0.0, 0.4)) + PALM_H
	var th := 85.0
	var r := {}
	while th >= 20.0:
		r = _roots(o1, pb1, _pitch(th) * C, _pitch(th * 0.5) * H)
		if _mid(r, "arm").y - wy >= 0.82 * arm_len:
			break
		th -= 1.0
	var cb1 := _pitch(th) * C
	var hands := {}
	var arm_keys := {}
	for s in ["l", "r"]:
		var root: Vector3 = r["arm_" + s]
		var out := signf(root.x)
		var hb := _palm_basis("hand_" + s, Vector3(out * 0.25, 0.0, 1.0))
		var w := _rest("hand_" + s, hb, root.x + out * 0.04, root.z - 0.02)
		hands[s] = w
		arm_keys["arm_" + s] = _ik_key(w, hb, Vector3(out * 0.35, 0.0, -1.0))
	var limbs1 := arm_keys.duplicate()
	for s in ["l", "r"]:
		var hip: Vector3 = r["leg_" + s]
		var a := _rest("foot_" + s, _kneel_foot("foot_" + s), hip.x, hip.z - 0.96 * leg_len)
		# knees open sideways (along the floor) as the legs start to fold, never down into it
		limbs1["leg_" + s] = _ik_key(a, _kneel_foot("foot_" + s), Vector3(signf(hip.x), -0.3, 0.0))
	o1.y += maxf(0.0, _sink("pelvis", Transform3D(pb1, o1), _g(Vector3.ZERO)))
	_keys.append(_key(0.9, o1, pb1, cb1, _pitch(th * 0.5) * H, limbs1))
	var hand_z := ((hands["l"] as Vector3).z + (hands["r"] as Vector3).z) * 0.5
	wy = maxf((hands["l"] as Vector3).y, (hands["r"] as Vector3).y)

	# 2. all fours: shoulders over the (unmoved) palms, back near level, each knee on the floor under
	#    or just behind its hip, shin along the floor behind the knee. Every knee and foot sits on the
	#    ground right under it (slopes, steps), found by raycast.
	var l_thigh: float = _len["thigh_l"]
	var l_calf: float = _len["calf_l"]
	var pb2 := _pitch(78.0) * P
	var th2 := 84.0
	var cb2 := _pitch(th2) * C
	var hb2 := _pitch(th2 * 0.8) * H
	var rr := _roots(Vector3.ZERO, pb2, cb2, hb2)
	var o2 := Vector3(0.0, wy + 0.86 * arm_len - _mid(rr, "arm").y, hand_z - _mid(rr, "arm").z)
	var gk := _g(Vector3(0.0, 0.0, o2.z + _mid(rr, "leg").z))
	var knee_y := gk + KNEE_R
	o2.y = minf(o2.y, knee_y + l_thigh - _mid(rr, "leg").y)  # no higher than a vertical thigh allows
	var r2 := _roots(o2, pb2, cb2, hb2)
	var limbs2 := arm_keys.duplicate()
	var knees := {}
	for s in ["l", "r"]:
		var hip: Vector3 = r2["leg_" + s]
		var kz := hip.z
		var ky := knee_y
		for it in 3:
			ky = _g(Vector3(hip.x, 0.0, kz)) + KNEE_R
			var drop := minf(hip.y - ky, l_thigh)
			kz = hip.z - sqrt(maxf(l_thigh * l_thigh - drop * drop, 0.0))
		var knee := Vector3(hip.x, ky, kz)
		var az := knee.z - l_calf
		var a := _rest("foot_" + s, _kneel_foot("foot_" + s), hip.x, az)
		for it in 3:
			var dy := minf(absf(knee.y - a.y), l_calf * 0.95)
			az = knee.z - sqrt(maxf(l_calf * l_calf - dy * dy, 0.0))
			a = _rest("foot_" + s, _kneel_foot("foot_" + s), hip.x, az)
		knees[s] = knee
		limbs2["leg_" + s] = _ik_key(a, _kneel_foot("foot_" + s), Vector3(0.0, -1.0, 0.3))
	_keys.append(_key(1.6, o2, pb2, cb2, hb2, limbs2))

	# 3. kneel up: sit back over the knees with the torso upright; the hands come off the floor. Knees
	#    and feet stay where they are.
	var pb3 := _pitch(5.0) * P
	var cb3 := _pitch(5.0) * C
	var knee_mid := ((knees["l"] as Vector3) + (knees["r"] as Vector3)) * 0.5
	var rr3 := _roots(Vector3.ZERO, pb3, cb3, H)
	var o3 := Vector3(0.0, knee_mid.y + 0.98 * l_thigh, knee_mid.z) - _mid(rr3, "leg")
	var limbs3 := {"arm_l": {"mode": "free"}, "arm_r": {"mode": "free"}}
	for s in ["l", "r"]:
		var lk: Dictionary = limbs2["leg_" + s]
		limbs3["leg_" + s] = _ik_key(lk["end"], lk["eb"], Vector3(0.0, -0.3, 1.0))
	_keys.append(_key(2.25, o3, pb3, cb3, H, limbs3))

	# 4. half kneel: the right foot steps forward onto the spot it will stand on; left knee stays down
	var d := (knees["r"] as Vector3).z + 0.5 - _o("foot_r").z if force_d < 0.0 else force_d
	var stand := Transform3D(Basis.IDENTITY, Vector3(0.0, _g(Vector3(0.0, 0.0, d)), d))
	var ar := _stand_foot(stand, "r")
	var limbs4 := limbs3.duplicate()
	limbs4["leg_r"] = _ik_key(ar, _b("foot_r"), Vector3(0.0, 0.5, 1.0))
	_keys.append(_key(2.9, o3 + Vector3(0.0, 0.0, 0.06), pb3, cb3, H, limbs4))

	# 5. stand: exactly the idle pose on the same two planted feet
	_keys.append(_stand_key(3.7, stand))
	return d


func _build_back(py: float, force_d: float) -> float:
	var P := _b("pelvis")
	var C := _b("spine_02")
	var H := _b("Head")
	_keys = []
	# 1. sit up onto the palms (behind the hips), knees up, both feet planted where you will stand
	var o1 := Vector3(0.0, maxf(py, 0.14), 0.0)
	var pb1 := _pitch(-35.0) * P
	var best := INF
	var th := -40.0
	var deg := -5.0
	var gh := _g(Vector3(0.0, 0.0, -0.2))
	while deg >= -75.0:
		var rr := _roots(o1, pb1, _pitch(deg) * C, _pitch(deg * 0.35) * H)
		var hz := _mid(rr, "leg").z - 0.15
		var err := 0.0
		for s in ["l", "r"]:
			var root: Vector3 = rr["arm_" + s]
			var w := Vector3(root.x + signf(root.x) * 0.07, gh + PALM_H, hz)
			err = maxf(err, absf(root.distance_to(w) - 0.9 * arm_len))
		if err < best:
			best = err
			th = deg
		deg -= 1.0
	var r := _roots(o1, pb1, _pitch(th) * C, _pitch(th * 0.35) * H)
	var limbs1 := {}
	for s in ["l", "r"]:
		var root: Vector3 = r["arm_" + s]
		var out := signf(root.x)
		var hb := _palm_basis("hand_" + s, Vector3(out * 0.7, 0.0, -1.0))
		var w := _rest("hand_" + s, hb, root.x + out * 0.07, _mid(r, "leg").z - 0.15)
		limbs1["arm_" + s] = _ik_key(w, hb, Vector3(out * 0.5, 0.0, -1.0))
	var foot_z := (_o("foot_l").z + _o("foot_r").z) * 0.5
	var d := _mid(r, "leg").z + 0.5 * leg_len - foot_z if force_d < 0.0 else force_d
	var stand := Transform3D(Basis.IDENTITY, Vector3(0.0, _g(Vector3(0.0, 0.0, d)), d))
	var feet := {}
	for s in ["l", "r"]:
		feet[s] = _stand_foot(stand, s)
		limbs1["leg_" + s] = _ik_key(feet[s], _b("foot_" + s), Vector3(0.0, 1.0, 0.35))
	o1.y += maxf(0.0, _sink("pelvis", Transform3D(pb1, o1), _g(Vector3.ZERO)))
	_keys.append(_key(0.9, o1, pb1, _pitch(th) * C, _pitch(th * 0.35) * H, limbs1))

	# 2. rock forward over the planted feet into a squat; the palms push off and come up
	var limbs2 := {"arm_l": {"mode": "free"}, "arm_r": {"mode": "free"}}
	for s in ["l", "r"]:
		limbs2["leg_" + s] = _ik_key(feet[s], _b("foot_" + s), Vector3(0.0, 0.5, 1.0))
	_keys.append(_key(1.7, stand * (_o("pelvis") + Vector3(0.0, -0.38, -0.12)), _pitch(20.0) * P, _pitch(30.0) * C,
		_pitch(10.0) * H, limbs2))

	# 3. stand
	_keys.append(_stand_key(2.45, stand))
	return d


# ---- pose maths --------------------------------------------------------------------------------

static func _slerp(a: Basis, b: Basis, u: float) -> Basis:
	return Basis(a.get_rotation_quaternion().slerp(b.get_rotation_quaternion(), u))


## Spine and neck bend between the pelvis, chest and head; clavicles ride on the chest.
func _torso(pb: Basis, cb: Basis, hb: Basis, out: Dictionary) -> void:
	out["pelvis"] = pb
	out["spine_01"] = _slerp(pb * _s1_from_p, cb * _s1_from_c, 0.5)
	out["spine_02"] = cb
	out["spine_03"] = cb * (loc["spine_03"] as Transform3D).basis
	out["neck_01"] = _slerp((out["spine_03"] as Basis) * (loc["neck_01"] as Transform3D).basis, hb * _neck_from_h, 0.5)
	out["Head"] = hb
	for s in ["l", "r"]:
		var c: String = "clavicle_" + s
		out[c] = (out["spine_03"] as Basis) * _slerp(_clav_rest[c], (loc[c] as Transform3D).basis, _clav_w)


func _fk_torso(o: Vector3, bases: Dictionary) -> Dictionary:
	var T := {"pelvis": Transform3D(bases["pelvis"], o)}
	for n in TORSO:
		if n != "pelvis":
			T[n] = Transform3D(bases[n], (T[par[n]] as Transform3D) * (loc[n] as Transform3D).origin)
	return T


func _fk(o: Vector3, bases: Dictionary) -> Dictionary:
	var T := {"pelvis": Transform3D(bases["pelvis"], o)}
	for n in ORDER:
		if n != "pelvis":
			T[n] = Transform3D(bases[n], (T[par[n]] as Transform3D) * (loc[n] as Transform3D).origin)
	return T


## Rotation taking the standing bone (direction a1, bend toward the idle pole) onto direction a2 with
## the bend toward `pole`: the knee / elbow hinge always opens the same way the standing one does.
func _hinge(limb: String, i: int, a2: Vector3, pole: Vector3) -> Basis:
	var names: Array = LIMBS[limb]
	var a1 := (_o(names[i + 1]) - _o(names[i])).normalized()
	var p1: Vector3 = _pole_idle[limb]
	p1 = (p1 - a1 * p1.dot(a1)).normalized()
	var p2 := pole - a2 * pole.dot(a2)
	if p2.length() < 0.0001:
		p2 = a2.cross(Vector3(1, 0, 0))
	p2 = p2.normalized()
	return Basis(a2, p2, a2.cross(p2)) * Basis(a1, p1, a1.cross(p1)).transposed()


func _ik(limb: String, root: Vector3, target: Vector3, pole: Vector3, eb: Basis, out: Dictionary) -> void:
	var names: Array = LIMBS[limb]
	var l1: float = _len[names[0]]
	var l2: float = _len[names[1]]
	var to := target - root
	short[limb] = maxf(0.0, to.length() - (l1 + l2))
	var d := clampf(to.length(), absf(l1 - l2) + 0.01, l1 + l2 - 0.0005)
	var dir := to.normalized() if to.length() > 0.0001 else Vector3.DOWN
	var pd := pole - dir * pole.dot(dir)
	if pd.length() < 0.0001:
		pd = dir.cross(Vector3(1, 0, 0))
	pd = pd.normalized()
	var x := (l1 * l1 - l2 * l2 + d * d) / (2.0 * d)
	var hgt := sqrt(maxf(l1 * l1 - x * x, 0.0))
	var joint := root + dir * x + pd * hgt
	if running and hgt > 0.01:
		# the limb never passes into the floor or a step edge, whatever the ground's shape: if the knee /
		# elbow, shin / forearm or thigh / upper arm would, it swings round the hip-foot (shoulder-hand)
		# line toward up, just as far as it has to (found by bisection, so it moves smoothly)
		var up := Vector3.UP - dir * dir.y
		if up.length() > 0.05:
			up = up.normalized()
			var side := dir.cross(up)
			var base := root + dir * x
			var tip0 := root + dir * d
			var clear := 0.05 if limb.begins_with("leg") else 0.04
			var a0 := atan2(pd.dot(side), pd.dot(up))
			if not _limb_clear(root, base, hgt, up, side, a0, tip0, clear):
				var f_ok := 0.0
				if _limb_clear(root, base, hgt, up, side, 0.0, tip0, clear):
					var f_bad := 1.0
					for k in 8:
						var mid := (f_ok + f_bad) * 0.5
						if _limb_clear(root, base, hgt, up, side, a0 * mid, tip0, clear):
							f_ok = mid
						else:
							f_bad = mid
				var a := a0 * f_ok
				pd = (up * cos(a) + side * sin(a)).normalized()
				joint = base + pd * hgt
	var tip := root + dir * d
	out[names[0]] = _hinge(limb, 0, (joint - root).normalized(), pd) * _b(names[0])
	out[names[1]] = _hinge(limb, 1, (tip - joint).normalized(), pd) * _b(names[1])
	out[names[2]] = eb


func _limb_clear(root: Vector3, base: Vector3, hgt: float, up: Vector3, side: Vector3, a: float, tip: Vector3,
		clear: float) -> bool:
	var joint := base + (up * cos(a) + side * sin(a)) * hgt
	if joint.y < _floor_s(joint) + clear:
		return false
	# along the shin / forearm and the thigh / upper arm (a kneeling shin may lie along the floor)
	for q: Vector3 in [joint.lerp(tip, 0.35), root.lerp(joint, 0.5)]:
		if q.y < _floor_s(q) + clear * 0.6:
			return false
	return true


func _limb_bases(limb: String, lk: Dictionary, root: Vector3, cb: Basis, out: Dictionary) -> void:
	if lk["mode"] == "ik":
		_ik(limb, root, lk["end"], (lk["pole"] as Vector3).normalized(), lk["eb"], out)
		return
	var turn := cb * _b("spine_02").inverse()  # hang off the chest the way they do standing
	for n in LIMBS[limb]:
		out[n] = turn * _b(n)


## Pose between two keys at u (0..1): torso blended, contacts held or carried on an arc, limbs solved.
func _between(a: Dictionary, b: Dictionary, u: float) -> Dictionary:
	var o: Vector3 = (a["o"] as Vector3).lerp(b["o"], u)
	var cb := _slerp(a["cb"], b["cb"], u)
	var bases := {}
	_torso(_slerp(a["pb"], b["pb"], u), cb, _slerp(a["hb"], b["hb"], u), bases)
	var T := _fk_torso(o, bases)
	# where each hand / foot on the floor is headed right now (lifted on an arc if it is stepping)
	var ends := {}
	for limb in LIMBS:
		var la: Dictionary = a["limbs"][limb]
		var lb: Dictionary = b["limbs"][limb]
		if la["mode"] == "ik" and lb["mode"] == "ik":
			var ea: Vector3 = la["end"]
			var eb_: Vector3 = lb["end"]
			var end := ea.lerp(eb_, u)
			var gap := ea.distance_to(eb_)
			if gap > 0.005:
				end += Vector3.UP * (LIFT[limb] as float) * sin(PI * u) * clampf(gap / 0.25, 0.3, 1.0)
			ends[limb] = end

	for limb in LIMBS:
		var names: Array = LIMBS[limb]
		var root: Vector3 = (T[par[names[0]]] as Transform3D) * (loc[names[0]] as Transform3D).origin
		var la: Dictionary = a["limbs"][limb]
		var lb: Dictionary = b["limbs"][limb]
		if la["mode"] == "ik" and lb["mode"] == "ik":
			var end: Vector3 = ends[limb]
			var pole := (la["pole"] as Vector3).normalized().lerp((lb["pole"] as Vector3).normalized(), u)
			_ik(limb, root, end, pole, _slerp(la["eb"], lb["eb"], u), bases)
		else:
			var A := {}
			var Bb := {}
			_limb_bases(limb, la, root, cb, A)
			_limb_bases(limb, lb, root, cb, Bb)
			for n in names:
				bases[n] = _slerp(A[n], Bb[n], u)
	return {"o": o, "b": bases}


func _interval() -> int:
	var i := 0
	while i < _keys.size() - 1 and t >= (_keys[i + 1] as Dictionary)["t"]:
		i += 1
	return i


func _pose_now() -> Dictionary:
	var k0: Dictionary = _keys[0]
	var last := _keys.size() - 1
	_clav_w = 0.0
	if t < k0["t"]:
		var first := _between(k0, k0, 0.0)
		# the torso comes up first and the limbs follow, so a limb trapped under the body is lifted
		# out with it before it has to move
		var u := t / (k0["t"] as float)
		var ut := smoothstep(0.0, 0.75, u)
		var ul := smoothstep(0.25, 1.0, u)
		var bases := {}
		for n in ORDER:
			bases[n] = _slerp(_start["b"][n], first["b"][n], ut if n in TORSO else ul)
		var o := (_start["o"] as Vector3).lerp(first["o"], ut)
		# limbs don't sweep through each other or the floor: each hand / foot travels on a curve from
		# where it lay to where it goes, solved from the shoulder / hip
		var T := _fk_torso(o, bases)
		for limb in LIMBS:
			var names: Array = LIMBS[limb]
			var lk: Dictionary = k0["limbs"][limb]
			if lk["mode"] != "ik":
				continue
			var ulimb := ul
			var root: Vector3 = (T[par[names[0]]] as Transform3D) * (loc[names[0]] as Transform3D).origin
			# a curve out to the side (hands) or up (feet) and round to the new spot
			var se: Vector3 = _start_end[limb]
			var ke: Vector3 = lk["end"]
			var ctrl := (se + ke) * 0.5 + Vector3.UP * 0.15
			if limb.begins_with("arm"):
				ctrl += Vector3(signf(_o(names[0]).x) * 0.25, 0.0, 0.0)
			var end: Vector3 = se.lerp(ctrl, ulimb).lerp(ctrl.lerp(ke, ulimb), ulimb)
			# the knee / elbow turns round the limb from where it pointed to where it will point
			var axis := (end - root).normalized()
			var p0: Vector3 = _start_pole[limb]
			var p1: Vector3 = lk["pole"]
			p0 = (p0 - axis * p0.dot(axis)).normalized()
			p1 = (p1 - axis * p1.dot(axis)).normalized()
			var pole: Vector3 = p1
			if p0.is_normalized() and p1.is_normalized():
				pole = p0.rotated(axis, (_start_turn[limb] as float) * ulimb)
				pole = pole.lerp(p1, smoothstep(0.75, 1.0, ulimb)).normalized()  # lands exactly on the first pose's
			var solved := {}
			_ik(limb, root, end, pole, bases[names[2]], solved)
			var w := smoothstep(0.0, 0.25, ulimb)
			for n in names:
				bases[n] = _slerp(bases[n], solved[n], w)
		return {"o": o, "b": bases}
	var i := _interval()
	var a: Dictionary = _keys[i]
	if i >= last:
		_clav_w = 1.0
		return _between(a, a, 0.0)
	var b: Dictionary = _keys[i + 1]
	var v := smoothstep(a["t"], b["t"], t)
	if i == last - 1:
		_clav_w = v  # shoulders settle into the Idle clip's as you stand
	return _between(a, b, v)


# ---- per frame ---------------------------------------------------------------------------------

func _process_modification_with_delta(delta: float) -> void:
	var sk := get_skeleton()
	if sk == null or rig == null or B.is_empty():
		return
	if running:
		# a part held back by something (an arm caught under the chest) slows the whole get-up until it
		# has caught up, instead of the plan carrying on without it
		var rate := clampf(1.0 - (rig.worst_miss() - 0.05) / 0.1, 0.2, 1.0)
		_rate = lerpf(_rate, rate, 1.0 - exp(-delta * 6.0))
		t = minf(t + delta * _rate, duration)
		var pose := _pose_now()
		var T := _fk(pose["o"], pose["b"])
		for n in ORDER:
			sk.set_bone_global_pose(B[n], T[n])
		return
	if rig.ragdolling:
		# draw the spine and neck bent between the physical pelvis, chest and head
		var inv := sk.global_transform.affine_inverse()
		var pt := inv * rig.pelvis().get_global_transform_interpolated()
		var cb := (inv * (rig.by_name["chest"]["body"] as RigidBody3D).get_global_transform_interpolated()).basis
		var hb := (inv * (rig.by_name["head"]["body"] as RigidBody3D).get_global_transform_interpolated()).basis
		var bases := {}
		_clav_w = 1.0
		_torso(pt.basis.orthonormalized(), cb.orthonormalized(), hb.orthonormalized(), bases)
		var T := _fk_torso(pt.origin, bases)
		for n in TORSO:
			sk.set_bone_global_pose(B[n], T[n])
