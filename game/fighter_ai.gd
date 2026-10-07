class_name FighterAI
extends Node
## A fighter's brain: fills the same Intent the keyboard fills for the player.
##   dummy  stands there and takes it; heals 3 s after the last hit
##   spar   closes in (sprinting when far), circles at its weapon's reach, throws combos and the odd
##          heavy, guards or dodges some of your swings, takes a breather when hurt, then comes back
##   puppet does exactly what `puppet` says (the self-test drives it)

const ENGAGE := 18.0
const SPRINT_FROM := 5.0

var mode := "dummy"
var home := Vector3.ZERO
var rng := RandomNumberGenerator.new()
var puppet := Intent.new()
var _foe: Player = null
var _attack_in := 1.0
var _strafe := 1.0
var _strafe_t := 0.0
var _decided := {}
var _guard_left := 0.0
var _breather := 0.0
var _hp_mark := 1.0
var _combo_left := 0
var _heavy_hold := 0.0


func _init(p_mode: String = "dummy") -> void:
	mode = p_mode
	rng.seed = hash(p_mode) + 7


func think(p: Player, delta: float) -> Intent:
	var i := Intent.new()
	i.view_yaw = atan2(-p.facing.x, -p.facing.z)
	if mode == "puppet":
		return puppet
	if mode == "dummy":
		if p.combat.hits_taken > 0 and p.combat.poise_t > 3.0 and p.sheet.hp < p.sheet.v("max_hp"):
			p.sheet.hp = p.sheet.v("max_hp")
		return i
	_foe = _pick_foe(p)
	if _foe == null:
		p.lock.target = null
		var back := home - p.global_position
		back.y = 0.0
		if back.length() > 1.0:
			i.view_yaw = atan2(-back.x, -back.z)
			i.move = Vector2(0.0, 1.0)
		if p.combat.poise_t > 6.0:
			p.sheet.hp = minf(p.sheet.v("max_hp"), p.sheet.hp + p.sheet.v("max_hp") * 0.2 * delta)
		return i
	var to := _foe.global_position - p.global_position
	to.y = 0.0
	var d := to.length()
	i.view_yaw = atan2(-to.x, -to.z)
	p.lock.target = _foe
	var reach := _reach(p)
	# hurt: a short breather each time another quarter of health goes, then straight back in
	var hpf := p.sheet.hp / p.sheet.v("max_hp")
	if hpf < _hp_mark - 0.25:
		_hp_mark = hpf
		_breather = 2.5
	_breather -= delta
	_strafe_t -= delta
	if _strafe_t <= 0.0:
		_strafe_t = rng.randf_range(1.0, 2.5)
		_strafe = -_strafe if rng.randf() < 0.6 else _strafe
	var want := reach + (3.5 if _breather > 0.0 else 0.4)
	if d > SPRINT_FROM and _breather <= 0.0:
		i.move = Vector2(0.0, 1.0)
		i.sprint = true
	else:
		i.move = Vector2(_strafe * (0.6 if d < reach + 1.5 else 0.2), clampf((d - want) * 1.5, -1.0, 1.0))
	# the foe winds up: guard, or step away, sometimes
	_guard_left -= delta
	var fc := _foe.combat
	if fc.attacking() and d < 3.5:
		var key := fc.get_instance_id() * 100 + fc.combo_i
		if not _decided.has(key):
			_decided = {key: true}
			var r := rng.randf()
			if r < 0.12:
				i.just_dodge = true
				i.move = Vector2(_strafe, -0.5)
			elif r < 0.45 and not p.combat.ranged():
				_guard_left = 0.6
				i.just_guard = true
	i.guard = _guard_left > 0.0
	# attack: combos of 2-4 at reach, a heavy now and then (held past the heavy threshold)
	_attack_in -= delta
	if _heavy_hold > 0.0:
		_heavy_hold -= delta
		i.attack = true
	if _breather <= 0.0 and not i.guard and p.combat.state == Combat.S.IDLE and d < reach + 0.5:
		if _combo_left > 0:
			i.just_attack = true
			_combo_left -= 1
		elif _attack_in <= 0.0:
			_attack_in = rng.randf_range(0.8, 1.8)
			if rng.randf() < 0.2:
				_heavy_hold = Combat.HEAVY_HOLD + 0.1
				i.attack = true
				i.just_attack = true
			else:
				i.just_attack = true
				_combo_left = rng.randi_range(1, 3)
	elif p.combat.attacking() and _combo_left > 0 and p.combat.t > p.combat.window.y:
		i.just_attack = true
		_combo_left -= 1
	return i


func _reach(p: Player) -> float:
	var lights: Array = p.combat.stance.get("light", [])
	return float((lights[0] as Dictionary).get("reach", 1.0)) + 0.3 if not lights.is_empty() else 1.3


func _pick_foe(p: Player) -> Player:
	var best: Player = null
	var best_d := ENGAGE
	for f in Player.fighters:
		if f == p or not LockOn.alive(f) or f.is_ai:
			continue
		var dd := f.global_position.distance_to(p.global_position)
		if dd < best_d:
			best_d = dd
			best = f
	return best
