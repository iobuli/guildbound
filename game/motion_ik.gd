class_name MotionIK
extends SkeletonModifier3D
## Maths-driven motion on top of the animation clips, so reactions are continuous instead of canned:
##   feet      two-bone leg IK plants each foot on the real ground (stairs, slopes); hips drop to the lower foot;
##             a swinging foot looks ahead and lifts onto the next step instead of cutting into it.
##             A foot that comes down is LOCKED where it landed until the pose lifts it again, so it never
##             skates; when the body turns on the spot (or comes to rest) a locked foot that has been left
##             behind takes a small procedural step back under the body.
##   stride    feet can be pushed further fore/aft of the hips (crouch-running covers more ground per step)
##   landing   a damped spring dips the hips on impact, scaled by fall speed
##   hit       a damped spring recoils the chest
##   push      two-bone arm IK puts the hands on the prop's surface wherever it is; torso leans by effort
##   bank      the torso leans into turns (turn rate x speed)
##   climb     hands go wide (beyond the shoulders) and the PALMS are placed flat on the wall surface
##   fight     guard stance (fists by the chin), strikes: the fist is driven along a computed path to a
##             point on the opponent (straight for jab / cross, an arc for the hook) with the torso
##             twisting into it; fingers curl into fists. The physical fist stops on the target's body.
## Runs right after the AnimationTree. Inputs are set by CharacterVisual from player state only.

const MASK := BodyRig.L_WORLD | BodyRig.L_PROPS

var grounded := false
var push_targets: Array = []   # two world points (left hand, right hand) or empty
var push_effort := 0.0         # 0..1: how heavy the push is for you
var turn_bank := 0.0           # radians, + leans to the character's left
var climbing := false
var climb_normal := Vector3.BACK  # wall's outward normal (world)
const CLIMB_SPREAD := 0.55        # metres from the body centre line to each palm
const PALM_FROM_WRIST := 0.08
var exclude: Array[RID] = []
var stride := 1.0               # >1 lengthens each step around the body's centre
var body_speed := 0.0           # m/s over the ground (from player state)

const ANKLE_REST := 0.05        # animated ankle / ball height when a foot is flat on the floor
const BALL_REST := -0.03
const DOWN_H := 0.045           # an animated foot this close to the floor counts as planted
const MAX_DRIFT := 0.16         # a locked foot this far from where the pose wants it takes a step
const MAX_TWIST := 0.5          # ... or twisted this much (rad) against the body
const MAX_PIVOT := 0.6          # a planted foot turns on its ball / heel beyond this instead of twisting the leg
const STEP_TIME := 0.2
const STEP_LIFT := 0.07
var _ft := {}
var _ball_local := {}
var _reach_drop := 0.0
var _still_t := 0.0
var _yaw_rate := 0.0
var _last_yaw := 0.0

var _feet_w := 0.0
var _push_w := 0.0
var _dl := 0.0
var _dr := 0.0
var _dip := 0.0
var _dip_v := 0.0
var _recoil := 0.0
var _recoil_v := 0.0
var _bank := 0.0
var _lean := 0.0
var _hands: Array = []
var _climb_w := 0.0
var _palm_local := {}   # hand bone -> palm normal in the bone's own frame (palms face down in T-pose)

# fighting (set by CharacterVisual from the fighter's Combat)
var fight_on := false
var guard_high := false
var strike_side := ""            # "l" / "r" while a strike (or a charge) is under way
var strike_ext := 0.0            # -0.3 pulled back .. 0 at guard .. 1 at the aim point
var strike_point := Vector3.ZERO # world point the fist is driven to
var strike_hook := false
var style_w := 0.0               # 0..1: a fighting clip (action layer) owns the arms and torso
var strike_ik := true            # the clip shapes the strike, the maths lands the fist on the aim point
const GUARD_FWD := 0.27          # fists this far in front of the face ...
const GUARD_SIDE := 0.12         # ... this far either side of the centre line
const HOOK_ARC := 0.32           # the hook swings this far out before coming in
var _fight_w := 0.0
var _twist := 0.0
var _assist_yaw := 0.0           # extra torso turn / lean so a clip's punch still reaches its target
var _assist_lean := 0.0
var _fingers: Array = []         # [bone, curl axis in the bone's own frame, max angle]
var B := {}
var _len := {}


func setup() -> void:
	var sk := get_skeleton()
	for n in ["pelvis", "spine_01", "thigh_l", "calf_l", "foot_l", "thigh_r", "calf_r", "foot_r",
			"upperarm_l", "lowerarm_l", "hand_l", "upperarm_r", "lowerarm_r", "hand_r", "Head"]:
		B[n] = sk.find_bone(n)
	for pair in [["thigh_l", "calf_l"], ["calf_l", "foot_l"], ["thigh_r", "calf_r"], ["calf_r", "foot_r"],
			["upperarm_l", "lowerarm_l"], ["lowerarm_l", "hand_l"], ["upperarm_r", "lowerarm_r"], ["lowerarm_r", "hand_r"]]:
		_len[pair[0]] = sk.get_bone_global_rest(B[pair[0]]).origin.distance_to(sk.get_bone_global_rest(B[pair[1]]).origin)
	for h in ["hand_l", "hand_r"]:
		_palm_local[h] = (sk.get_bone_global_rest(B[h]).basis.inverse() * Vector3.DOWN).normalized()
	# fist: each finger joint turns about the axis that carries it toward the palm (T-pose palms face down)
	for side in ["l", "r"]:
		for f in ["index", "middle", "ring", "pinky", "thumb"]:
			for k in 3:
				var b := sk.find_bone("%s_0%d_%s" % [f, k + 1, side])
				var c := sk.find_bone("%s_0%d_%s" % [f, k + 2, side])
				if b < 0:
					continue
				var bg := sk.get_bone_global_rest(b)
				var tip: Vector3 = sk.get_bone_global_rest(c).origin if c >= 0 else bg * Vector3(0.0, 0.03, 0.0)
				var d := (tip - bg.origin).normalized()
				var axis := d.cross(Vector3.DOWN)
				if f == "thumb":
					axis = d.cross(Vector3.DOWN + Vector3(0.0, 0.0, 0.6)).normalized()
				if axis.length() < 0.01:
					continue
				var ang: float = [1.25, 1.45, 1.1][k] if f != "thumb" else 0.5
				_fingers.append([b, (bg.basis.inverse() * axis.normalized()).normalized(), ang])
	for side in ["l", "r"]:
		_ball_local[side] = sk.get_bone_rest(sk.find_bone("ball_" + side)).origin
		_ft[side] = {"planted": false, "plant": Vector3.ZERO, "yaw": 0.0, "yoff": 0.0, "step": -1.0,
			"from": Vector3.ZERO, "from_yoff": 0.0, "off": Vector3.ZERO, "on_ball": false, "wait_up": false, "h": Vector3.ZERO, "aw": Vector3.ZERO, "has_aw": false, "clear": 0.0}


## Forget where the feet were locked (after a ragdoll / get-up the body is somewhere else).
func reset_feet() -> void:
	for side in _ft:
		var st: Dictionary = _ft[side]
		st["planted"] = false
		st["step"] = -1.0
		st["off"] = Vector3.ZERO
		st["yoff"] = 0.0
		st["has_aw"] = false
		st["wait_up"] = false
		st["clear"] = 0.0
	_reach_drop = 0.0
	_dip = 0.0
	_dip_v = 0.0


func land(impact_speed: float) -> void:
	_dip_v -= clampf(impact_speed * 0.09, 0.0, 1.6)


func hit(strength: float) -> void:
	_recoil_v -= strength


func _spring(x: float, v: float, k: float, c: float, delta: float) -> Vector2:
	v += (-k * x - c * v) * delta
	return Vector2(x + v * delta, v)


func _ground(sk: Skeleton3D, p: Vector3, root_y: float) -> float:
	var space := sk.get_world_3d().direct_space_state
	var q := PhysicsRayQueryParameters3D.create(Vector3(p.x, root_y + 0.6, p.z), Vector3(p.x, root_y - 0.6, p.z), MASK)
	q.exclude = exclude
	var hit := space.intersect_ray(q)
	return hit["position"].y if not hit.is_empty() else NAN


## How far (up/down) this foot should move from its animated height. A foot in the air (swinging)
## also checks 0.3 m ahead and rises to the higher ground there, so it lands ON the next step.
func _foot_delta(sk: Skeleton3D, foot: Vector3, fwd: Vector3, root_y: float) -> float:
	var here := _ground(sk, foot, root_y)
	var d := here - root_y if not is_nan(here) else 0.0
	if foot.y - root_y > 0.06:
		var ahead := _ground(sk, foot + fwd * 0.3, root_y)
		if not is_nan(ahead):
			d = maxf(d, ahead - root_y)
	return clampf(d, -0.45, 0.45)


func _process_modification_with_delta(delta: float) -> void:
	var sk := get_skeleton()
	if sk == null or B.is_empty():
		return
	var s := _spring(_dip, _dip_v, 220.0, 26.0, delta)
	_dip = clampf(s.x, -0.4, 0.1)
	_dip_v = s.y
	s = _spring(_recoil, _recoil_v, 160.0, 18.0, delta)
	_recoil = clampf(s.x, -0.5, 0.5)
	_recoil_v = s.y
	_feet_w = move_toward(_feet_w, 1.0 if grounded else 0.0, delta * 6.0)
	_push_w = move_toward(_push_w, 1.0 if push_targets.size() == 2 else 0.0, delta * 5.0)
	# the stance eases in; a strike brings it in at once (a punch is always fully aimed)
	_fight_w = move_toward(_fight_w, 1.0 if fight_on and not climbing and push_targets.size() < 2 else 0.0,
		delta * (25.0 if strike_side != "" else 5.0))
	var twist_to := 0.0
	if strike_side != "":
		# right hand forward = the torso turns left (+ about up), left hand = the other way
		var sgn := 1.0 if strike_side == "r" else -1.0
		twist_to = sgn * (0.6 if strike_hook else (0.55 if strike_side == "r" else 0.35)) * strike_ext * (1.0 - style_w)
	_twist = lerpf(_twist, twist_to, 1.0 - exp(-delta * 22.0))
	_climb_w = move_toward(_climb_w, 1.0 if climbing else 0.0, delta * 6.0)
	if push_targets.size() == 2:
		_hands = push_targets.duplicate()
	_bank = lerpf(_bank, turn_bank, 1.0 - exp(-delta * 8.0))
	var lean_to := push_effort * 0.45 * (1.0 if push_targets.size() == 2 else 0.0)
	if strike_side != "":
		# weight forward into the punch (a clip brings its own)
		lean_to = maxf(lean_to, 0.22 * clampf(strike_ext, 0.0, 1.0) * (1.0 - style_w))
	_lean = lerpf(_lean, lean_to, 1.0 - exp(-delta * (18.0 if strike_side != "" else 6.0)))

	var gt := sk.global_transform
	var inv := gt.affine_inverse()
	var root_y := gt.origin.y
	var fwd := (gt.basis * Vector3(0.0, 0.0, 1.0)).normalized()
	var left := (gt.basis * Vector3(1.0, 0.0, 0.0)).normalized()

	# --- read the animated pose first (world space) ---
	var W := {}
	for n in B:
		W[n] = gt * sk.get_bone_global_pose(B[n])

	# --- feet: where should each foot be (lock / step / stride), and where is the ground there? ---
	# A planted foot is pinned at its CONTACT point: the heel while it is down, then the ball once the
	# heel peels off, so the foot rolls over its toes instead of dragging them.
	var yaw := atan2(fwd.x, fwd.z)
	_yaw_rate = lerpf(_yaw_rate, wrapf(yaw - _last_yaw, -PI, PI) / maxf(delta, 0.0001), 1.0 - exp(-delta * 20.0))
	_last_yaw = yaw
	_still_t = _still_t + delta if body_speed < 0.15 else 0.0
	var hor := {}
	var lift := {}
	var cen: Vector3 = ((W["foot_l"] as Transform3D).origin + (W["foot_r"] as Transform3D).origin) * 0.5
	for side in ["l", "r"]:
		var st: Dictionary = _ft[side]
		var other: Dictionary = _ft["r" if side == "l" else "l"]
		var ft: Transform3D = W["foot_" + side]
		var F := ft.origin
		var ball: Vector3 = ft * (_ball_local[side] as Vector3)
		var Fw := F + fwd * ((F - cen).dot(fwd) * (stride - 1.0))
		var want := Vector3(Fw.x, 0.0, Fw.z)
		var heel_h := F.y - root_y - ANKLE_REST
		var ball_h := ball.y - root_y - BALL_REST
		# down = near the floor AND no longer sweeping (a swinging foot that skims the floor is not planted)
		var aw := Vector3(Fw.x, 0.0, Fw.z)
		var sweep := ((aw - (st["aw"] as Vector3)) / maxf(delta, 0.0001)).length() if st["has_aw"] else 0.0
		st["aw"] = aw
		st["has_aw"] = true
		var down: bool = minf(heel_h, ball_h) < DOWN_H and (st["planted"] or sweep < maxf(1.2, 0.35 * body_speed))
		var on_ball := heel_h > ball_h + 0.015
		var to_c := Vector3(ball.x - F.x, 0.0, ball.z - F.z) if on_ball else Vector3.ZERO  # ankle -> contact
		var h := want + (st["off"] as Vector3)
		lift[side] = 0.0
		if not down:
			st["wait_up"] = false
		if not grounded:
			st["planted"] = false
			st["step"] = -1.0
			st["wait_up"] = false
		if not grounded or (not st["planted"] and st["step"] < 0.0):
			var k := exp(-delta * 12.0)
			st["off"] = (st["off"] as Vector3) * k
			st["yoff"] = st["yoff"] * k
			h = want + (st["off"] as Vector3)
			if grounded and down and not st["wait_up"]:
				st["planted"] = true
				st["yaw"] = yaw + st["yoff"]
				st["plant"] = h + Basis(Vector3.UP, st["yoff"]) * to_c
				st["on_ball"] = on_ball
		elif st["step"] >= 0.0:
			# quicker steps while the body is spinning round; the foot travels in the first 75 % of the
			# step and spends the rest coming straight down, so it lands without skidding
			st["step"] = minf(1.0, st["step"] + delta / lerpf(STEP_TIME, STEP_TIME * 0.6, clampf(absf(_yaw_rate) / 8.0, 0.0, 1.0)))
			var u := smoothstep(0.0, 0.75, st["step"])
			h = (st["from"] as Vector3).lerp(want, u)
			st["yoff"] = lerpf(st["from_yoff"], 0.0, u)
			lift[side] = STEP_LIFT * sin(PI * st["step"])
			if st["step"] >= 1.0:
				st["step"] = -1.0
				st["planted"] = true
				st["yaw"] = yaw
				st["yoff"] = 0.0
				st["off"] = Vector3.ZERO
				st["plant"] = h + to_c
				st["on_ball"] = on_ball
		else:
			# a planted foot can pivot on its contact point (ball or heel) up to MAX_PIVOT against the body
			var yo := wrapf(st["yaw"] - yaw, -PI, PI)
			if absf(yo) > MAX_PIVOT:
				yo = clampf(yo, -MAX_PIVOT, MAX_PIVOT)
				st["yaw"] = yaw + yo
			st["yoff"] = yo
			var rot := Basis(Vector3.UP, st["yoff"])
			if on_ball != st["on_ball"]:
				# the contact moved (heel -> ball): re-anchor at where that point is now, no jump
				st["plant"] = (st["h"] as Vector3) + rot * to_c
				st["on_ball"] = on_ball
			var plant: Vector3 = st["plant"]
			h = plant - rot * to_c
			var drift := Vector2(h.x - want.x, h.z - want.z).length()
			var free_to_step: bool = other["step"] < 0.0 or other["step"] > 0.45
			var settle: bool = _still_t > 0.3 and (drift > 0.05 or absf(st["yoff"]) > 0.3)
			if drift > 1.0:
				st["planted"] = false  # teleported
				st["off"] = Vector3.ZERO
				st["yoff"] = 0.0
				h = want
			elif not down:
				st["planted"] = false
				st["off"] = h - want
			elif drift > MAX_DRIFT or absf(st["yoff"]) > MAX_TWIST or settle:
				if body_speed > 0.6:
					st["planted"] = false  # moving on: let it catch up through the swing
					st["wait_up"] = true
					st["off"] = h - want
				elif free_to_step and (other["planted"] or settle):
					st["planted"] = false
					st["step"] = 0.0
					st["from"] = h
					st["from_yoff"] = st["yoff"]
		# swing clearance: a foot travelling fast over the floor that the pose keeps too low (blended or
		# sped-up clips skim it) is lifted clear, so the toes never drag
		var clear := 0.0
		if grounded and not st["planted"] and st["step"] < 0.0:
			var c := 0.06 * clampf((sweep - 1.5) / 2.0, 0.0, 1.0)
			clear = maxf(0.0, c - minf(heel_h, ball_h))
		st["clear"] = lerpf(st["clear"], clear, 1.0 - exp(-delta * 25.0))
		lift[side] = (lift[side] as float) + (st["clear"] as float)
		st["h"] = h
		hor[side] = h
	if _feet_w > 0.001:
		var fl: Vector3 = (W["foot_l"] as Transform3D).origin
		var fr: Vector3 = (W["foot_r"] as Transform3D).origin
		var hl: Vector3 = hor["l"]
		var hr: Vector3 = hor["r"]
		_dl = lerpf(_dl, _foot_delta(sk, Vector3(hl.x, fl.y, hl.z), fwd, root_y), 1.0 - exp(-delta * 18.0))
		_dr = lerpf(_dr, _foot_delta(sk, Vector3(hr.x, fr.y, hr.z), fwd, root_y), 1.0 - exp(-delta * 18.0))
	else:
		_dl = lerpf(_dl, 0.0, 1.0 - exp(-delta * 10.0))
		_dr = lerpf(_dr, 0.0, 1.0 - exp(-delta * 10.0))
	var shift := minf(minf(_dl, _dr), 0.0) * _feet_w + _dip - _reach_drop * _feet_w

	# --- hips ---
	if absf(shift) > 0.0005:
		var pg: Transform3D = W["pelvis"]
		pg.origin += Vector3.UP * shift
		sk.set_bone_global_pose(B["pelvis"], inv * pg)

	# --- torso: lean into a push, bank into turns, recoil from hits (pivot at spine_01) ---
	var torso_rot := Basis.IDENTITY
	var pivot: Vector3 = W["spine_01"].origin + Vector3.UP * shift
	_reach_assist(W, pivot, shift, left, delta)
	var t_yaw := _twist * _fight_w + _assist_yaw
	var t_lean := _lean - _recoil + _assist_lean
	if absf(t_lean) > 0.001 or absf(_bank) > 0.001 or absf(t_yaw) > 0.001:
		torso_rot = Basis(Vector3.UP, t_yaw) * Basis(fwd, -_bank) * Basis(left, t_lean)
		var sp: Transform3D = W["spine_01"]
		sk.set_bone_global_pose(B["spine_01"], inv * Transform3D(torso_rot * sp.basis, pivot))

	# --- legs ---
	var need_drop := 0.0
	if _feet_w > 0.001 or absf(shift) > 0.0005:
		for side in ["l", "r"]:
			var st: Dictionary = _ft[side]
			var ft: Transform3D = W["foot_" + side]
			var d := (_dl if side == "l" else _dr) * _feet_w
			var hip: Vector3 = W["thigh_" + side].origin + Vector3.UP * shift
			var h: Vector3 = hor[side]
			var flat := Vector3(ft.origin.x, 0.0, ft.origin.z).lerp(h, _feet_w)
			var target := Vector3(flat.x, ft.origin.y + d + (lift[side] as float) * _feet_w, flat.z)
			var yoff: float = st["yoff"] * _feet_w
			var fb := Basis(Vector3.UP, yoff) * ft.basis
			var pole: Vector3 = W["calf_" + side].origin + Basis(Vector3.UP, yoff) * fwd * 0.3 + Vector3.UP * shift
			# a locked foot the hips have moved too far from: let go (it catches up through the swing)
			var reach: float = _len["thigh_" + side] + _len["calf_" + side]
			var anim_d: float = (W["thigh_" + side].origin as Vector3).distance_to(ft.origin)
			var lim := maxf(0.97 * reach, anim_d)
			var hd := Vector2(target.x - hip.x, target.z - hip.z).length()
			if hd < lim:
				need_drop = maxf(need_drop, (hip.y - target.y) - sqrt(lim * lim - hd * hd))
			_two_bone(sk, inv, W, "thigh_" + side, "calf_" + side, "foot_" + side, hip, target, pole, fb)
	need_drop = clampf(need_drop, 0.0, 0.25) if grounded else 0.0
	_reach_drop = lerpf(_reach_drop, need_drop, 1.0 - exp(-delta * (20.0 if need_drop > _reach_drop else 5.0)))

	# --- climbing: wide arms, palms flat on the wall ---
	if _climb_w > 0.001:
		var n := climb_normal.normalized()
		var up_w := (Vector3.UP - n * n.y).normalized()
		var right_w := (-n).cross(Vector3.UP).normalized()
		var space := sk.get_world_3d().direct_space_state
		for side in ["l", "r"]:
			var sgn := -1.0 if side == "l" else 1.0
			var sh: Vector3 = pivot + torso_rot * (W["upperarm_" + side].origin + Vector3.UP * shift - pivot)
			var anim_hand: Vector3 = pivot + torso_rot * (W["hand_" + side].origin + Vector3.UP * shift - pivot)
			var h := clampf((anim_hand - sh).dot(up_w), -0.15, 0.5)
			var centre: Vector3 = W["spine_01"].origin
			var probe := centre + right_w * sgn * CLIMB_SPREAD + up_w * ((sh - centre).dot(up_w) + h) + n * 0.4
			var q := PhysicsRayQueryParameters3D.create(probe, probe - n * 1.4, MASK)
			q.exclude = exclude
			var hit := space.intersect_ray(q)
			if hit.is_empty():
				continue
			var wn: Vector3 = hit["normal"]
			var fingers := (up_w * 0.9 + right_w * sgn * 0.35).normalized()
			fingers = (fingers - wn * fingers.dot(wn)).normalized()
			var palm: Vector3 = hit["position"] + wn * 0.025
			var wrist := palm - fingers * PALM_FROM_WRIST
			var target: Vector3 = anim_hand.lerp(wrist, _climb_w)
			var hb: Basis = (W["hand_" + side] as Transform3D).basis.slerp(_palm_basis("hand_" + side, -wn, fingers), _climb_w)
			var elbow: Vector3 = pivot + torso_rot * (W["lowerarm_" + side].origin + Vector3.UP * shift - pivot)
			var pole: Vector3 = elbow + right_w * sgn * 0.35 + Vector3.DOWN * 0.25 + wn * 0.15
			_two_bone(sk, inv, W, "upperarm_" + side, "lowerarm_" + side, "hand_" + side, sh, target, pole, hb)

	# --- fighting: guard stance, strikes, fists ---
	if _fight_w > 0.001:
		_fight(sk, inv, W, pivot, torso_rot, shift, fwd, left)

	# --- arms on the pushed prop ---
	if _push_w > 0.001 and _hands.size() == 2:
		for i in 2:
			var side: String = ["l", "r"][i]
			var sh: Vector3 = pivot + torso_rot * (W["upperarm_" + side].origin + Vector3.UP * shift - pivot)
			var anim_hand: Vector3 = pivot + torso_rot * (W["hand_" + side].origin + Vector3.UP * shift - pivot)
			var target: Vector3 = anim_hand.lerp(_hands[i], _push_w)
			var elbow: Vector3 = pivot + torso_rot * (W["lowerarm_" + side].origin + Vector3.UP * shift - pivot)
			var pole: Vector3 = elbow + Vector3.DOWN * 0.3 + (left if side == "l" else -left) * 0.2
			_two_bone(sk, inv, W, "upperarm_" + side, "lowerarm_" + side, "hand_" + side, sh, target, pole)


## How far the maths pulls a clip's striking fist onto the aim point: none in the wind-up (the clip's
## own chamber and arc), all of it by contact.
func _corr() -> float:
	return smoothstep(0.3, 0.95, strike_ext) if strike_ik else 0.0


## A captured punch was thrown at a different distance (and the clip turns the shoulders its own way):
## if the fist cannot reach the aim point from where the clip puts the shoulder, turn the torso about
## the spine toward it (the smallest turn that reaches), then lean in.
func _reach_assist(W: Dictionary, pivot: Vector3, shift: float, left: Vector3, delta: float) -> void:
	var want_yaw := 0.0
	var want_lean := 0.0
	var c := _corr() * style_w * _fight_w
	if strike_side != "" and c > 0.001:
		var side := strike_side
		var reach: float = (_len["upperarm_" + side] + _len["lowerarm_" + side]) * 0.97
		var sh: Vector3 = W["upperarm_" + side].origin + Vector3.UP * shift
		var best := INF
		for k in 15:
			var yaw := -0.7 + 0.1 * k
			for j in 5:
				var ln := 0.1 * j
				var r := Basis(Vector3.UP, yaw) * Basis(left, ln)
				var d := (pivot + r * (sh - pivot)).distance_to(strike_point)
				# reach it with the least turn and lean
				var cost := maxf(0.0, d - reach) * 10.0 + absf(yaw) * 0.3 + ln * 0.4
				if cost < best:
					best = cost
					want_yaw = yaw
					want_lean = ln
		want_yaw *= c
		want_lean *= c
	var k := 1.0 - exp(-delta * 25.0)
	_assist_yaw = lerpf(_assist_yaw, want_yaw, k)
	_assist_lean = lerpf(_assist_lean, want_lean, k)


func _fight(sk: Skeleton3D, inv: Transform3D, W: Dictionary, pivot: Vector3, torso_rot: Basis, shift: float,
		fwd: Vector3, left: Vector3) -> void:
	var w := _fight_w
	var curl := w * (1.0 - 0.5 * style_w)
	for f in _fingers:
		var b: int = f[0]
		sk.set_bone_pose_rotation(b, sk.get_bone_pose_rotation(b) * Quaternion(f[1] as Vector3, (f[2] as float) * curl))
	var head: Vector3 = pivot + torso_rot * ((W["Head"] as Transform3D).origin + Vector3.UP * shift - pivot)
	var high := 0.04 if guard_high else -0.08
	for side in ["l", "r"]:
		var sgn := 1.0 if side == "l" else -1.0  # +left
		var sh: Vector3 = pivot + torso_rot * (W["upperarm_" + side].origin + Vector3.UP * shift - pivot)
		var anim_hand: Vector3 = pivot + torso_rot * (W["hand_" + side].origin + Vector3.UP * shift - pivot)
		var lead := 0.06 if side == "l" else 0.0  # orthodox: the left fist a little further out
		var guard := head + fwd * (GUARD_FWD + lead) + left * sgn * GUARD_SIDE + Vector3.UP * (high - 0.04)
		var target := guard
		var fingers := (fwd * 0.55 + Vector3.UP * 0.75 - left * sgn * 0.25).normalized()  # knuckles up by the face
		var palm := -left * sgn                                                           # palms facing in
		if strike_side == side:
			var e := strike_ext
			var reach: float = (_len["upperarm_" + side] + _len["lowerarm_" + side]) * 0.99
			var to := strike_point - sh
			var aim := sh + to.limit_length(reach)
			if e < 0.0:
				target = guard - fwd * 0.12 * -e + Vector3.DOWN * 0.04 * -e
			else:
				target = guard.lerp(aim, e)
				if strike_hook:
					target += left * sgn * HOOK_ARC * sin(PI * minf(e, 1.0)) * 0.6
				var dir := (aim - guard).normalized()
				fingers = fingers.slerp(dir if not strike_hook else (dir - left * sgn * 0.6).normalized(), clampf(e * 1.4, 0.0, 1.0))
				palm = palm.slerp(Vector3.DOWN, clampf(e * 1.4, 0.0, 1.0))
		var hb: Basis = (W["hand_" + side] as Transform3D).basis.slerp(_palm_basis("hand_" + side, palm, fingers), w)
		var elbow_out := 0.22 if (strike_hook and strike_side == side) else 0.12
		var pole: Vector3 = sh + Vector3.DOWN * 0.35 + left * sgn * elbow_out + fwd * 0.05
		var aw := w
		if style_w > 0.001:
			# a fighting clip is playing: its arms, except the striking fist, which the maths brings onto
			# the aim point by contact
			var anim_basis: Basis = torso_rot * (W["hand_" + side] as Transform3D).basis
			if strike_side == side and strike_ik:
				var reach: float = (_len["upperarm_" + side] + _len["lowerarm_" + side]) * 0.99
				var aim := sh + (strike_point - sh).limit_length(reach)
				var c := _corr()
				target = anim_hand.lerp(aim, c)
				hb = anim_basis.slerp(hb, c * 0.6)
				aw = w
			else:
				hb = anim_basis.slerp(hb, 1.0 - style_w)
				aw = w * (1.0 - style_w)
			var anim_elbow: Vector3 = pivot + torso_rot * (W["lowerarm_" + side].origin + Vector3.UP * shift - pivot)
			pole = pole.lerp(anim_elbow + (anim_elbow - sh).normalized() * 0.1, style_w)
		_two_bone(sk, inv, W, "upperarm_" + side, "lowerarm_" + side, "hand_" + side, sh, anim_hand.lerp(target, aw), pole, hb)


## Hand basis with the palm facing `palm_dir` and the fingers (bone +Y) toward `fingers`.
func _palm_basis(hand: String, palm_dir: Vector3, fingers: Vector3) -> Basis:
	var ly := Vector3.UP
	var lp: Vector3 = _palm_local[hand]
	lp = (lp - ly * lp.dot(ly)).normalized()
	var wy := fingers.normalized()
	var wp := (palm_dir - wy * palm_dir.dot(wy)).normalized()
	var local := Basis(ly.cross(lp), ly, lp)
	var world := Basis(wy.cross(wp), wy, wp)
	return (world * local.inverse()).orthonormalized()


## Analytic two-bone IK in world space. Keeps each bone's animated twist (minimal-arc rotation).
func _two_bone(sk: Skeleton3D, inv: Transform3D, W: Dictionary, upper: String, lower: String, end: String,
		root: Vector3, target: Vector3, pole: Vector3, end_basis = null) -> void:
	var l1: float = _len[upper]
	var l2: float = _len[lower]
	var to := target - root
	var d := clampf(to.length(), 0.05, l1 + l2 - 0.001)
	var dir := to.normalized()
	var pd := pole - root
	pd = (pd - dir * pd.dot(dir))
	if pd.length() < 0.0001:
		pd = Vector3.FORWARD
	pd = pd.normalized()
	var x := (l1 * l1 - l2 * l2 + d * d) / (2.0 * d)
	var h := sqrt(maxf(l1 * l1 - x * x, 0.0))
	var joint := root + dir * x + pd * h
	var tip := root + dir * d
	var ub: Basis = W[upper].basis
	var lb: Basis = W[lower].basis
	var eb: Basis = W[end].basis if end_basis == null else end_basis
	var ub2 := Basis(Quaternion(ub.y.normalized(), (joint - root).normalized())) * ub
	var lb2 := Basis(Quaternion(lb.y.normalized(), (tip - joint).normalized())) * lb
	sk.set_bone_global_pose(B[upper], inv * Transform3D(ub2, root))
	sk.set_bone_global_pose(B[lower], inv * Transform3D(lb2, joint))
	sk.set_bone_global_pose(B[end], inv * Transform3D(eb, tip))
