class_name FighterAI
extends Node
## A fighter's brain: fills the same Intent the keyboard fills for the player.
##   dummy  stands there and takes it; heals 3 s after the last hit
##   spar   closes in (sprinting when far), circles at its weapon's reach, throws combos and the odd
##          heavy, guards or dodges some of your swings, takes a breather when hurt, then comes back
##   pack   one of a pack (goblins). The pack shares a foe: the first to see you calls the rest. Each
##          takes a place on a ring around the foe (spread evenly, the ring slowly turning), and only
##          `max_attackers` of them may strike at once (attack tokens). The others circle, feint and
##          wait for an opening - the foe swinging, staggered, aiming or turned away - then dart in
##          for a quick one-two and skip back out. A hurt one backs off before it comes again; they
##          give up when the foe leads them more than `leash` m from where the chase began, and walk
##          home deaf to everyone until they are there.
##   puppet does exactly what `puppet` says (the self-test drives it)

const ENGAGE := 18.0
const SPRINT_FROM := 5.0
const PACK_CALL := 26.0        # a packmate's foe this close pulls the whole pack in
const PACK_CIRCLE := 0.25      # how far ahead of its place on the ring a member aims (rad): the ring turns
const PACK_SPACE := 1.4        # packmates closer than this push apart
const PACK_HURT_R := 2.6       # a hurt member backs off this much further out
# the pack's shared clock is the physics frame count, so hit-stop and hitches never lapse a token
const TOKEN_FRAMES := 20       # a token lapses when its holder has not renewed it for this many frames
const RING_FRAMES := 20        # a member that stopped reporting leaves the ring
const CALL_FRAMES := 240       # a sighting keeps the pack hunting this long

## pack -> {"id": foe instance id, "until": frame}: whom the pack is hunting
static var _hunt := {}
## "pack/foe id" -> {brain id: frame it lapses}: the attack tokens
static var _tokens := {}
## "pack/foe id" -> {brain id: [bearing around the foe, position, frame]}: the ring
static var _ring := {}

var pack := ""                 # pack mode: the pack's name (shared foe, tokens, ring)
var max_attackers := 2
var ring := 3.6                # the ring's radius around the foe (m)
var leash := 30.0
var role := "ring"             # pack mode: ring / go / strike / out / feint / hurt
var _role_t := 0.0
var _token_cd := 0.8
var _feint_cd := 2.5
var _hits_seen := 0
var _idle_t := 0.0
var _chase_from := Vector3.ZERO  # where this chase began (the leash is measured from here)
var _giving_up := false        # leashed: heading home, deaf to everyone until there
var _stuck_t := 0.0            # walking home but not getting anywhere (a rock in the way)
var _detour_t := 0.0           # sidestepping round it
var _detour_side := 1.0

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
	if mode == "pack":
		return _think_pack(p, delta, i)
	_foe = _pick_foe(p)
	if _foe == null:
		p.lock.target = null
		var back := home - p.global_position
		back.y = 0.0
		if back.length() > 1.0:
			_walk_home(p, i, back, 1.0, delta)
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


# ---- pack ------------------------------------------------------------------------------------------

func _think_pack(p: Player, delta: float, i: Intent) -> Intent:
	var now := Engine.get_physics_frames()
	var me := get_instance_id()
	if not LockOn.alive(p):
		_drop_token(me)
		_foe = null
		role = "ring"
		return i
	var foe := _pack_foe(p, now)
	if foe != _foe:
		_drop_token(me)
		_foe = foe
		role = "ring"
		_token_cd = rng.randf_range(0.3, 1.2)
	if foe == null:
		p.lock.target = null
		role = "ring"
		var back := home - p.global_position
		back.y = 0.0
		if back.length() > 1.0:
			_walk_home(p, i, back, 1.0 if back.length() > 2.0 else 0.5, delta)
		if p.combat.poise_t > 6.0:
			p.sheet.hp = minf(p.sheet.v("max_hp"), p.sheet.hp + p.sheet.v("max_hp") * 0.2 * delta)
		return i
	var key := "%s/%d" % [pack, foe.get_instance_id()]
	var to := foe.global_position - p.global_position
	to.y = 0.0
	var d := to.length()
	var f := to / d if d > 0.001 else -p.facing
	var right := f.cross(Vector3.UP)
	i.view_yaw = atan2(-f.x, -f.z)
	p.lock.target = foe
	var reach := _reach(p)
	var hpf := p.sheet.hp / maxf(1.0, p.sheet.v("max_hp"))
	var members: Dictionary = _ring.get_or_add(key, {})
	var away := p.global_position - foe.global_position
	members[me] = [atan2(away.x, away.z), p.global_position, now]
	var slot := _slot(members, me, now)
	# hurt: back off for a while before coming again (longer when badly hurt)
	if p.combat.hits_taken > _hits_seen:
		_hits_seen = p.combat.hits_taken
		if role != "hurt" and (hpf < 0.6 or rng.randf() < 0.5):
			role = "hurt"
			_role_t = rng.randf_range(1.3, 2.2) * (1.6 if hpf < 0.35 else 1.0)
	_role_t -= delta
	_token_cd -= delta
	_feint_cd -= delta
	var fc := foe.combat
	# an opening: the foe is swinging, staggered or aiming, or has its back (or side) to me
	var opening := fc.busy() or fc.aiming() or foe.facing.dot(-f) < 0.1
	var holding := _holds_token(key, me, now)
	match role:
		"ring":
			# wait for a token: snatch one at once on an opening, otherwise now and then
			var rate := 4.0 if opening else (0.25 if hpf < 0.35 else 0.7)
			if _token_cd <= 0.0 and rng.randf() < 1.0 - exp(-rate * delta) and _take_token(key, me, now):
				role = "go"
				_role_t = 2.4
				_combo_left = rng.randi_range(0, 1)
			elif _feint_cd <= 0.0 and d < ring + 1.5:
				_feint_cd = rng.randf_range(2.5, 5.5)
				if rng.randf() < 0.55:
					role = "feint"
					_role_t = rng.randf_range(0.22, 0.38)
		"go":
			_renew_token(key, me, now)
			if d < reach + 0.35 and p.combat.state == Combat.S.IDLE:
				i.just_attack = true
				role = "strike"
				_idle_t = 0.0
			elif _role_t <= 0.0:  # could not get there in time: let a packmate try
				_drop_token(me)
				_out(i)
		"strike":
			_renew_token(key, me, now)
			if p.combat.attacking():
				_idle_t = 0.0
				if _combo_left > 0 and p.combat.t > p.combat.window.y:
					i.just_attack = true
					_combo_left -= 1
			else:
				_idle_t += delta
				if _idle_t > 0.15:  # the string is over (nothing buffered): hand the token on
					_drop_token(me)
					_token_cd = rng.randf_range(1.4, 3.0)
					_out(i)
		"feint":
			if _role_t <= 0.0:
				_out(i)
		"out", "hurt":
			if holding and not p.combat.attacking():
				_drop_token(me)
				holding = false
			if holding:
				_renew_token(key, me, now)
			if _role_t <= 0.0:
				role = "ring"
	# movement
	match role:
		"go", "feint":
			i.move = Vector2(0.0, 1.0)
			i.sprint = d > reach + 0.8
		"strike":
			pass
		"out":
			if not i.just_dodge:
				i.move = Vector2(0.0, -1.0)
		_:
			var want_r := ring + (1.2 if hpf < 0.35 else 0.0) + (PACK_HURT_R if role == "hurt" else 0.0)
			var tgt := foe.global_position + Vector3(sin(slot), 0.0, cos(slot)) * want_r
			var w := tgt - p.global_position
			w.y = 0.0
			w += _spacing(members, me, p.global_position, now)
			var e := w.length()
			if e > 0.2:
				var wn := w / e
				i.move = Vector2(wn.dot(right), wn.dot(f)) * clampf(e / 1.5, 0.35, 1.0)
				i.sprint = e > 3.5
	# the foe swings at me while I am not committed: hop out of it, or guard
	if role != "go" and role != "strike" and fc.attacking() and d < 3.2:
		var k := fc.get_instance_id() * 100 + fc.combo_i
		if not _decided.has(k):
			_decided = {k: true}
			var r := rng.randf()
			if r < 0.3:
				i.just_dodge = true
				i.move = Vector2(rng.randf_range(-1.0, 1.0), -1.0)
			elif r < 0.45:
				_guard_left = 0.5
				i.just_guard = true
	_guard_left -= delta
	i.guard = _guard_left > 0.0 and role != "go" and role != "strike"
	if i.move.length() > 1.0:
		i.move = i.move.normalized()
	return i


## Skip back out to the ring after a strike or a feint (now and then with a hop).
func _out(i: Intent) -> void:
	role = "out"
	_role_t = rng.randf_range(0.45, 0.8)
	if rng.randf() < 0.35:
		i.just_dodge = true
		i.move = Vector2(0.0, -1.0)


## Head for home; when something is in the way (pushing at it, barely moving) sidestep round it,
## trying the other side the next time.
func _walk_home(p: Player, i: Intent, back: Vector3, pace: float, delta: float) -> void:
	i.view_yaw = atan2(-back.x, -back.z)
	i.move = Vector2(0.0, pace)
	if _detour_t > 0.0:
		_detour_t -= delta
		i.view_yaw += _detour_side * 1.25
		return
	if Vector2(p.velocity.x, p.velocity.z).length() < 0.4:
		_stuck_t += delta
		if _stuck_t > 0.7:
			_stuck_t = 0.0
			_detour_t = 0.8
			_detour_side = -_detour_side
	else:
		_stuck_t = 0.0


## The pack's foe: my own sighting, or a packmate's call. Nobody chases a foe more than `leash` m
## from where the chase began; one that gives up walks home and ignores everyone until it is there.
func _pack_foe(p: Player, now: int) -> Player:
	if _giving_up:
		if p.global_position.distance_to(home) > 2.5:
			return null
		_giving_up = false
	var foe: Player = _foe if is_instance_valid(_foe) else null
	if foe != null and (not LockOn.alive(foe) or foe.global_position.distance_to(p.global_position) > ENGAGE * 1.5):
		foe = null
	if foe == null:
		foe = _pick_foe(p)
	if foe == null and _hunt.has(pack):
		var h: Dictionary = _hunt[pack]
		var hf := instance_from_id(int(h["id"])) as Player
		if hf != null and int(h["until"]) > now and LockOn.alive(hf) and hf.global_position.distance_to(p.global_position) < PACK_CALL:
			foe = hf
	if foe != null and _foe == null:
		_chase_from = p.global_position
	if foe != null and foe.global_position.distance_to(_chase_from) > leash:
		_giving_up = true
		if _hunt.has(pack) and int((_hunt[pack] as Dictionary)["id"]) == foe.get_instance_id():
			_hunt.erase(pack)
		return null
	if foe != null:
		_hunt[pack] = {"id": foe.get_instance_id(), "until": now + CALL_FRAMES}
	return foe


## My place on the ring: members keep the order they stand in around the foe, spread evenly; the
## ring's turn is whatever fits where they stand best, plus a little lead so the ring keeps circling.
func _slot(members: Dictionary, me: int, now: int) -> float:
	var live: Array = []
	for id: int in members.keys():
		var m: Array = members[id]
		if now - int(m[2]) > RING_FRAMES:
			members.erase(id)
			continue
		live.append([float(m[0]), id])
	live.sort_custom(func(a: Array, b: Array) -> bool: return float(a[0]) < float(b[0]))
	var n := live.size()
	var step := TAU / maxf(1.0, n)
	var sx := 0.0
	var sy := 0.0
	var k_me := 0
	for k in n:
		var a: float = float(live[k][0]) - k * step
		sx += cos(a)
		sy += sin(a)
		if int(live[k][1]) == me:
			k_me = k
	var turn := 1.0 if sin(now / 60.0 * 0.4 + float(hash(pack) % 7)) >= 0.0 else -1.0  # the ring changes direction now and then
	return atan2(sy, sx) + k_me * step + PACK_CIRCLE * turn


## A push away from packmates standing too close.
func _spacing(members: Dictionary, me: int, at: Vector3, now: int) -> Vector3:
	var push := Vector3.ZERO
	for id: int in members:
		var m: Array = members[id]
		if id == me or now - int(m[2]) > RING_FRAMES:
			continue
		var off: Vector3 = at - (m[1] as Vector3)
		off.y = 0.0
		var dd := off.length()
		if dd < PACK_SPACE and dd > 0.001:
			push += off / dd * (PACK_SPACE - dd) * 1.5
	return push


func _holds_token(key: String, me: int, now: int) -> bool:
	var t: Dictionary = _tokens.get(key, {})
	return t.has(me) and int(t[me]) >= now


func _take_token(key: String, me: int, now: int) -> bool:
	var t: Dictionary = _tokens.get_or_add(key, {})
	for id: int in t.keys():
		if int(t[id]) < now:
			t.erase(id)
	if not t.has(me) and t.size() >= max_attackers:
		return false
	t[me] = now + TOKEN_FRAMES
	return true


func _renew_token(key: String, me: int, now: int) -> void:
	var t: Dictionary = _tokens.get(key, {})
	if t.has(me):
		t[me] = now + TOKEN_FRAMES


func _drop_token(me: int) -> void:
	for key: String in _tokens:
		(_tokens[key] as Dictionary).erase(me)


## How many of a pack hold an attack token on `foe` right now (the self-test checks the limit).
static func attackers_on(pack_name: String, foe: Player) -> int:
	var t: Dictionary = _tokens.get("%s/%d" % [pack_name, foe.get_instance_id()], {})
	var now := Engine.get_physics_frames()
	var n := 0
	for id: int in t:
		n += 1 if int(t[id]) >= now else 0
	return n


func _exit_tree() -> void:
	_drop_token(get_instance_id())
