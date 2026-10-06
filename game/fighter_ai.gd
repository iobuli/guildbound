class_name FighterAI
extends Node
## A fighter's brain: fills the same Intent the keyboard fills for the player, so it moves, guards,
## strikes, gets knocked down and gets up through exactly the same code.
##   dummy  stands there and takes it; back to full health 3 s after the last hit
##   spar   closes in (sprinting when far), keeps punching range, circles, mixes punches and kicks in
##          every direction, guards / parries / slips some of your wind-ups; hurt, it backs off for a
##          breather and comes back (it never just hangs at a distance)
##   puppet does exactly what `puppet` says (the self-test drives it)

const ENGAGE := 15.0
const RANGE := 0.95        # centre to centre it fights at (the punches are thrown from ~0.72 m)
const SPRINT_FROM := 3.0
const BREATHER := 2.5      # seconds it backs off when hurt, then it re-engages
const REGEN_AFTER := 5.0   # out of the fight this long, it heals back up

var mode := "dummy"
var home := Vector3.ZERO
var rng := RandomNumberGenerator.new()
var _foe: Player = null
var _attack_in := 1.0
var _strafe := 1.0
var _strafe_t := 0.0
var _decided_for := -1     # the opponent wind-up we already decided to guard / not guard
var _guard_left := 0.0
var _dodge_cd := 0.0
var _breather := 0.0
var _breathed_at := 1.0    # hp fraction we last took a breather at
var puppet := Intent.new()


func _init(p_mode: String = "dummy") -> void:
	mode = p_mode
	rng.seed = hash(p_mode) + 99


func think(p: Player, delta: float) -> Intent:
	var i := Intent.new()
	i.view_yaw = atan2(-p.facing.x, -p.facing.z)
	if mode == "puppet":
		return puppet
	var calm := p.sheet.clock - p.combat.last_hit_time
	if mode == "dummy":
		if calm > 3.0 and p.sheet.hp < p.sheet.v("max_hp"):
			p.sheet.hp = p.sheet.v("max_hp")
		return i
	_foe = _pick_foe(p)
	if calm > REGEN_AFTER and p.sheet.hp < p.sheet.v("max_hp"):
		p.sheet.hp = minf(p.sheet.v("max_hp"), p.sheet.hp + p.sheet.v("max_hp") * 0.15 * delta)
		_breathed_at = 1.0
	if _foe == null:
		p.lock.target = null
		var back := home - p.global_position
		back.y = 0.0
		if back.length() > 1.0:
			i.view_yaw = atan2(-back.x, -back.z)
			i.move = Vector2(0.0, 1.0)
		return i
	var to := _foe.global_position - p.global_position
	to.y = 0.0
	var d := to.length()
	i.view_yaw = atan2(-to.x, -to.z)  # "forward" = toward the opponent
	p.lock.target = _foe               # locked on directly (a toggle press could unlock it)
	# hurt: one short breather per 25 % of health lost, then straight back in
	var hpf := p.sheet.hp / p.sheet.v("max_hp")
	if hpf < _breathed_at - 0.25 and _breather <= 0.0:
		_breather = BREATHER
		_breathed_at = hpf
	_breather -= delta
	# close in, keep range, circle
	_strafe_t -= delta
	if _strafe_t <= 0.0:
		_strafe_t = rng.randf_range(1.0, 2.2)
		_strafe = -_strafe if rng.randf() < 0.6 else _strafe
	var want := RANGE * (3.0 if _breather > 0.0 else 1.0)
	var fwd := clampf((d - want) * 2.0, -1.0, 1.0)
	if d > SPRINT_FROM and _breather <= 0.0:
		i.move = Vector2(0.0, 1.0)
		i.sprint = true
	else:
		i.move = Vector2(_strafe * (0.5 if d < 2.0 else 0.2), fwd)
	# react to the opponent's wind-up
	_dodge_cd -= delta
	_guard_left -= delta
	var fc: Combat = _foe.combat
	if fc.state == Combat.S.WINDUP or fc.state == Combat.S.CHARGING:
		var id := fc.get_instance_id() * 1000 + fc.combo_i
		if _decided_for != id and d < 2.2:
			_decided_for = id
			var r := rng.randf()
			if r < 0.12 and _dodge_cd <= 0.0:
				i.just_dodge = true
				i.move = Vector2(-1.0 if rng.randf() < 0.5 else 1.0, -0.3)
				_dodge_cd = 1.5
			elif r < 0.45:
				_guard_left = 0.5
				i.just_guard = true
	i.guard = _guard_left > 0.0
	# attack on a rhythm set by attack speed: punches in every direction, kicks from a little further out
	_attack_in -= delta
	var ready := _breather <= 0.0 and not i.guard and p.combat.state == Combat.S.IDLE and _attack_in <= 0.0
	if ready and d < 1.5:
		if d > 1.05 or rng.randf() < 0.25:
			i.just_interact = true   # E: a kick
		else:
			i.just_attack = true
			i.attack = true
		i.flick = _random_flick()
		_attack_in = rng.randf_range(0.5, 1.3) / (p.sheet.v("attack_speed") / Combat.REF_ATTACK_SPEED)
	elif p.combat.state == Combat.S.RECOVER and p.combat.hit_done and rng.randf() < 0.08:
		i.just_attack = true  # a landed hit: carry the string on
		i.attack = true
		i.flick = _random_flick()
	return i


## Mostly the straight string, sometimes a hook / uppercut / body shot (the same mouse flick a player uses).
func _random_flick() -> Vector2:
	var r := rng.randf()
	if r < 0.55:
		return Vector2.ZERO
	var dirs := [Vector2(-60, 0), Vector2(60, 0), Vector2(0, -60), Vector2(0, 60)]
	return dirs[rng.randi() % 4]


func _pick_foe(p: Player) -> Player:
	var best: Player = null
	var best_d := ENGAGE
	for f in Player.fighters:
		if f == p or not LockOn.alive(f) or f.has_node("ai"):
			continue
		var d := f.global_position.distance_to(p.global_position)
		if d < best_d:
			best_d = d
			best = f
	return best
