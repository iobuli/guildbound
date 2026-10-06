class_name AnimProbe
extends RefCounted
## Measures animation quality on the PHYSICAL body (that is what gets drawn), once per physics frame.
##   slide  how fast a planted foot's contact point (heel or ball, whichever is down) moves over the
##          ground. A foot in stance should not move at all.
##   pop    a body part leaving its smooth path in one frame: |r(t) - 2 r(t-1) + r(t-2)|, where r is the
##          part's position in the character's own frame (so walking and turning are not pops). While
##          ragdolled / getting up the character frame is meaningless, so world space is used.
## Used by the self-test; reads player state and bodies only.

const STANCE_H := 0.03     # a contact point this close to its standing height counts as planted

var p: Player
var rig: BodyRig
var foot_len := 0.15
var ankle_h := 0.08
var ball_h := 0.02

var slide_dist := 0.0
var stance_t := 0.0
var slide_peak := 0.0      # worst single-frame contact speed (m/s)
var moving_t := 0.0
var _prev := {}
var _was := {}
var _which := {}

var pop := 0.0
var pop_part := ""
var pop_state := -1
var pop_at := 0
var _hist := {}
var _kind := -1
var _n := 0


func _init(player: Player) -> void:
	p = player
	rig = p.model.rig
	var sk := p.model.skeleton
	foot_len = sk.get_bone_global_rest(sk.find_bone("foot_l")).origin.distance_to(sk.get_bone_global_rest(sk.find_bone("ball_l")).origin)


func _ground(at: Vector3) -> float:
	var hit := p._ray(at + Vector3.UP * 0.6, at + Vector3.DOWN * 1.2)
	return hit["position"].y if not hit.is_empty() else NAN


func _points(side: String) -> Array:
	var fb: RigidBody3D = rig.by_name["foot_" + side]["body"]
	return [fb.global_position, fb.global_transform * Vector3(0.0, foot_len, 0.0)]


## Stand still on flat ground first: records how high heel and ball sit when planted.
func calibrate() -> void:
	var a := 0.0
	var b := 0.0
	for side in ["l", "r"]:
		var pts := _points(side)
		a += pts[0].y - _ground(pts[0])
		b += pts[1].y - _ground(pts[1])
	ankle_h = a * 0.5
	ball_h = b * 0.5


func reset() -> void:
	slide_dist = 0.0
	stance_t = 0.0
	slide_peak = 0.0
	moving_t = 0.0
	_prev.clear()
	_was.clear()
	_which.clear()
	pop = 0.0
	pop_part = ""
	pop_state = -1
	_hist.clear()
	_kind = -1


func slide_speed() -> float:
	return slide_dist / stance_t if stance_t > 0.0 else 0.0


## `count` false: track the body (for continuity) but don't add this frame to the sliding totals.
func sample(delta: float = 1.0 / 60.0, count: bool = true) -> void:
	_n += 1
	# ---- planted feet ----
	for side in ["l", "r"]:
		var pts := _points(side)
		var ha: float = pts[0].y - _ground(pts[0]) - ankle_h
		var hb: float = pts[1].y - _ground(pts[1]) - ball_h
		var i := 0 if ha <= hb else 1
		var planted := minf(ha, hb) < STANCE_H and p.state == Player.State.GROUND and count
		# only while the SAME point (heel or ball) carries the weight in both frames: when the foot rolls
		# from heel to ball the ball is still swinging down, which is not sliding
		if planted and _was.get(side, false) and _which.get(side, -1) == i:
			var prv: Vector3 = _prev[side][i]
			var d := Vector2(pts[i].x - prv.x, pts[i].z - prv.z).length()
			slide_dist += d
			stance_t += delta
			slide_peak = maxf(slide_peak, d / delta)
		_prev[side] = pts
		_was[side] = planted
		_which[side] = i
	# ---- pops ----
	var world := p.state == Player.State.RAGDOLL or p.state == Player.State.RECOVER
	var kind := 1 if world else 0
	if kind != _kind:
		_hist.clear()
		_kind = kind
	var inv := Transform3D.IDENTITY if world else p.model.global_transform.affine_inverse()
	for part in rig.parts:
		var r: Vector3 = inv * (part["body"] as RigidBody3D).global_position
		var h: Array = _hist.get(part["name"], [])
		if h.size() == 2:
			var a: float = (r - 2.0 * (h[0] as Vector3) + (h[1] as Vector3)).length()
			if a > pop:
				pop = a
				pop_part = part["name"]
				pop_state = p.state
				pop_at = _n
		_hist[part["name"]] = [r] if h.is_empty() else [r, h[0]]
