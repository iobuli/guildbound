class_name LockOn
extends Node
## Target lock for one fighter. Lock (MMB) picks the fighter nearest the view centre within reach; the
## wheel cycles to the next one left / right; the lock drops when the target is too far, out of sight
## for a second, or down. While not locked a soft target (narrower cone) still aims strikes.

const ACQUIRE_RANGE := 15.0
const ACQUIRE_HALF_DEG := 30.0  # 60 deg cone around where you look
const SOFT_HALF_DEG := 35.0
const SOFT_RANGE := 4.0
const DROP_RANGE := 20.0
const LOS_GRACE := 1.0

var p: Player
var target: Player = null
var soft: Player = null
var _blind := 0.0


func setup(fighter: Player) -> void:
	p = fighter


## The fighter strikes are aimed at: the lock, else the soft target.
func current() -> Player:
	return target if target != null else soft


func view_forward() -> Vector3:
	return Basis(Vector3.UP, p.intent.view_yaw) * Vector3(0.0, 0.0, -1.0)


static func alive(f: Player) -> bool:
	return is_instance_valid(f) and not f.knocked_out and f.sheet.hp > 0.0


## Fighters in reach and inside the cone, best (nearest the view centre, then nearest) first.
func candidates(half_deg: float, reach: float) -> Array:
	var fwd := view_forward()
	var out: Array = []
	for f in Player.fighters:
		if f == p or not alive(f):
			continue
		var to := f.global_position - p.global_position
		to.y = 0.0
		var d := to.length()
		if d > reach or d < 0.01:
			continue
		var ang := rad_to_deg(fwd.angle_to(to / d))
		if ang <= half_deg:
			out.append([ang + d * 2.0, f])
	out.sort_custom(func(a: Array, b: Array) -> bool: return a[0] < b[0])
	return out.map(func(a: Array) -> Player: return a[1])


func tick(delta: float) -> void:
	var i := p.intent
	if i.just_lock:
		if target != null:
			target = null
		else:
			var c := candidates(ACQUIRE_HALF_DEG, ACQUIRE_RANGE)
			target = c[0] if not c.is_empty() else null
	if target != null and (i.just_lock_next or i.just_lock_prev):
		_cycle(1 if i.just_lock_next else -1)
	if target != null:
		var keep := alive(target) and target.global_position.distance_to(p.global_position) <= DROP_RANGE
		if keep:
			var from := p.global_position + Vector3.UP * 1.5
			var hit := p._ray(from, target.global_position + Vector3.UP * 1.3)
			_blind = _blind + delta if not hit.is_empty() else 0.0
			keep = _blind <= LOS_GRACE
		if not keep:
			_blind = 0.0
			var c := candidates(ACQUIRE_HALF_DEG, ACQUIRE_RANGE).filter(func(f: Player) -> bool: return f != target)
			target = c[0] if not c.is_empty() else null
	var s := candidates(SOFT_HALF_DEG, SOFT_RANGE)
	soft = s[0] if not s.is_empty() else null


## Next target by angle around you, to the right (+1) or left (-1) of the current one.
func _cycle(dir: int) -> void:
	var base := (target.global_position - p.global_position)
	var base_yaw := atan2(base.x, base.z)
	var best: Player = null
	var best_a := INF
	for f in Player.fighters:
		if f == p or f == target or not alive(f):
			continue
		var to := f.global_position - p.global_position
		if to.length() > ACQUIRE_RANGE:
			continue
		# clockwise from above = to the right
		var a := wrapf(base_yaw - atan2(to.x, to.z), 0.0, TAU) if dir > 0 else wrapf(atan2(to.x, to.z) - base_yaw, 0.0, TAU)
		if a < best_a:
			best_a = a
			best = f
	if best != null:
		target = best
