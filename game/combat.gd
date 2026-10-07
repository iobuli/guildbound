class_name Combat
extends Node
## Weapon combat for one fighter (the player or an AI; both drive it through their Intent).
##   LMB      light attack; pressing again inside the chain window carries the combo on
##   hold LMB heavy attack            E  special (shield kick, leap attack, flying kick...)
##   RMB      melee: guard (tap it as a blow lands = PARRY); bow / staff: aim (LMB fires / casts)
##   Q        next weapon style (data/weapons.json: sword and shield, great sword, daggers, bow, staff, fists)
## A swing is dangerous only while its blade is moving fastest (measured from the clip); the blade is
## swept between frames against each opponent's body. Moves carry the fighter by their own root motion.
## Poise: every blow wears it down; at zero the fighter staggers. The player is built for a power
## fantasy: light hits never interrupt your attacks (hyper armour), and you are only ever staggered,
## never knocked down. Hits resolve through both stat sheets (Attack Damage, crits, Armor, Lifesteal).

enum S { IDLE, ATTACK, GUARD, AIM, STAGGER, DEAD }

const BUFFER := 0.3
const HEAVY_HOLD := 0.32
const COMBO_RESET := 1.0
const PARRY_WINDOW := 0.3
const GUARD_COS := 0.34          # cos 70 deg: blows from inside this arc in front are guarded
const BODY_RADIUS := 0.42
const BLADE_RADIUS := 0.12        # generous on purpose: a blow that looks like it lands, lands
const POISE_REGEN := 25.0        # per second, after 1.5 s without being hit
const PLAYER_POISE_MULT := 2.5
const STAGGER_TIME := 0.7
const PARRIED_TIME := 1.1
const PVP_DAMAGE := 0.6

static var _stop_until := 0
static var log_events := false

var p: Player
var state: int = S.IDLE
var stance_id := "sword_shield"
var stance := {}
var move := {}
var move_kind := ""              # "light" / "heavy" / "special" / "sprint"
var combo_i := 0
var t := 0.0
var dur := 0.0
var speed := 1.0
var window := Vector2(-1, -1)    # dangerous stretch of the move, seconds of playback
var _victims := {}
var _prev_seg: Array = []
var buffered := ""
var buffer_t := 0.0
var press_t := -1.0
var heavy_fired := false
var guard_t := 9.0
var poise := 60.0
var poise_t := 9.0
var stagger_left := 0.0
var aim_t := 0.0
var since_attack := 9.0
var target: Player = null
var hits_landed := 0
var hits_taken := 0
var last_result := ""             # what the last blow that reached US did ("hit", "guard", "parry"...)
var last_given := ""              # what OUR last blow did to its target
var assist_v := Vector3.ZERO      # a short step toward the target while winding up (closes small gaps)
var last_damage := 0.0
var _cast_done := false
var crits := true
var rng := RandomNumberGenerator.new()


func setup(fighter: Player) -> void:
	p = fighter
	HeroAnim.load_data()
	set_stance(stance_id)


func set_stance(id: String) -> void:
	stance_id = id
	stance = HeroAnim.data[id]
	poise = _poise_max()
	if p.model != null:
		p.model.set_stance(id)
	reset()


func next_stance() -> void:
	var order: Array = HeroAnim.data["_order"]
	set_stance(order[(order.find(stance_id) + 1) % order.size()])
	p.toast.emit("Weapon: " + str(stance["name"]))
	if str(stance.get("draw", "")) != "":
		p.model.play(str(stance["draw"]), 1.3)


func _poise_max() -> float:
	return float(stance.get("poise", 50)) * (PLAYER_POISE_MULT if not p.is_ai else 1.0)


func ranged() -> bool:
	return stance_id == "bow" or stance_id == "magic"


func busy() -> bool:
	return state == S.ATTACK or state == S.STAGGER or state == S.DEAD


func attacking() -> bool:
	return state == S.ATTACK


func guarding() -> bool:
	return state == S.GUARD


func aiming() -> bool:
	return state == S.AIM


## How fast the fighter may walk this frame (fraction of normal).
func move_scale() -> float:
	match state:
		S.ATTACK:
			return 0.0
		S.GUARD:
			return 0.45
		S.AIM:
			return 0.5
		S.STAGGER, S.DEAD:
			return 0.0
	return 1.0


# ---- tick (called from Player._physics_process with the fresh intent) ------------------------

func tick(delta: float) -> void:
	var i := p.intent
	guard_t += delta
	poise_t += delta
	since_attack += delta
	buffer_t = maxf(0.0, buffer_t - delta)
	if buffer_t <= 0.0:
		buffered = ""
	if poise_t > 1.5:
		poise = minf(_poise_max(), poise + POISE_REGEN * delta)
	if i.just_switch and state == S.IDLE:
		next_stance()
	if i.just_guard:
		guard_t = 0.0
	# light on release, heavy when held past HEAVY_HOLD
	if i.just_attack and not ranged():
		press_t = 0.0
		heavy_fired = false
	if press_t >= 0.0:
		if i.attack:
			press_t += delta
			if press_t >= HEAVY_HOLD and not heavy_fired:
				heavy_fired = true
				_buffer("heavy")
		else:
			if not heavy_fired:
				_buffer("sprint" if p.sprinting and stance.has("sprint_attack") and state == S.IDLE else "light")
			press_t = -1.0
	if i.just_special:
		_buffer("special")

	if p.state != Player.State.GROUND:
		if state == S.ATTACK or state == S.GUARD or state == S.AIM:
			_end_move()
		return
	t += delta
	match state:
		S.IDLE:
			if ranged() and i.guard and not p.exhausted:
				_enter_aim()
			elif not ranged() and i.guard and not p.exhausted:
				state = S.GUARD
				p.model.hold(str(stance.get("guard", "")))
			elif buffered != "":
				_start(buffered)
			elif ranged() and i.just_attack:
				_fire_ranged(false)
			if since_attack > COMBO_RESET:
				combo_i = 0
		S.ATTACK:
			_track()
			if window.x >= 0.0 and t >= window.x and t <= window.y + 0.02:
				_sweep()
			elif _prev_seg.size() > 0 and t > window.y:
				_prev_seg = []
			_cast_check()
			# chain: after the dangerous part, a buffered press carries the combo on
			if buffered != "" and t >= window.y and t >= dur * 0.45:
				_start(buffered)
			elif t >= dur:
				_end_move()
		S.GUARD:
			if not i.guard or p.exhausted:
				_end_move()
			elif buffered == "special":
				_end_move()
				_start("special")
		S.AIM:
			aim_t += delta
			if not i.guard:
				_end_move()
			elif i.just_attack:
				_fire_ranged(true)
		S.STAGGER:
			stagger_left -= delta
			if stagger_left <= 0.0:
				state = S.IDLE


func _buffer(kind: String) -> void:
	buffered = kind
	buffer_t = BUFFER


func _start(kind: String) -> void:
	var m: Dictionary
	match kind:
		"light":
			var lights: Array = stance["light"]
			m = lights[combo_i % lights.size()]
			combo_i += 1
		"heavy":
			m = stance["heavy"]
			combo_i = 0
		"special":
			m = stance["special"]
			combo_i = 0
		"sprint":
			m = stance.get("sprint_attack", stance["heavy"])
			combo_i = 0
	var cost := float(m.get("stamina", 8))
	if p.stamina < cost * 0.5:
		p.toast.emit("Too tired")
		buffered = ""
		return
	p._spend_once(cost)
	if m.has("mana"):
		if p.sheet.mana < float(m["mana"]):
			p.toast.emit("Not enough mana")
			buffered = ""
			return
		p.sheet.mana -= float(m["mana"])
	move = m
	move_kind = kind
	buffered = ""
	state = S.ATTACK
	t = 0.0
	since_attack = 0.0
	_victims.clear()
	_prev_seg = []
	_cast_done = false
	p.model.hold("")
	speed = float(m.get("speed", 1.0)) * clampf(p.sheet.v("attack_speed") / 1.5, 0.75, 1.4)
	dur = p.model.play(str(m["clip"]), speed, 0.2)
	var w := p.model.swing_window(str(m["clip"]), _limb())
	window = w / speed
	target = p.lock.current() if p.lock != null else null
	_face_target(1.0)
	if log_events:
		print("   [%s] start %s %s: dur %.2f window %.2f-%.2f, target %s at %.2f m" % [p.sheet.display_name, kind, str(m["clip"]), dur,
			window.x, window.y, target.sheet.display_name if target else "-", target.global_position.distance_to(p.global_position) if target else -1.0])
	Sfx.at(p, p.model.chest_position(), "swing_kick" if _limb().begins_with("foot") else "swing", 1.0 + 0.1 * combo_i)


func _limb() -> String:
	if move.has("limb"):
		return str(move["limb"])
	return "hand_" + str(stance.get("hand", "r"))


func _end_move() -> void:
	assist_v = Vector3.ZERO
	if state == S.ATTACK:
		p.model.stop_action(0.2)
	state = S.IDLE
	move = {}
	window = Vector2(-1, -1)
	_prev_seg = []
	aim_t = 0.0
	p.model.hold("")


## Turn toward the target early in a move (tracking), not once the blade is swinging; step in if it
## is just out of reach so the blow arrives on it.
func _track() -> void:
	assist_v = Vector3.ZERO
	if t < window.x:
		_face_target(0.25)
		if target != null and is_instance_valid(target):
			var to := target.global_position - p.global_position
			to.y = 0.0
			var gap := to.length() - float(move.get("reach", 1.0))
			if gap > 0.0 and gap < 2.5:
				assist_v = to.normalized() * minf(gap / maxf(0.1, window.x - t), 8.0)


func _face_target(k: float) -> void:
	var who: Player = target
	if who == null:
		# no lock: the nearest opponent in front within reach gets the swing
		var best := 3.0
		for f in Player.fighters:
			if f == p or not LockOn.alive(f):
				continue
			var d := f.global_position.distance_to(p.global_position)
			var to := (f.global_position - p.global_position).normalized()
			if d < best and p.facing.dot(to) > 0.3:
				best = d
				who = f
		target = who
	if who != null and is_instance_valid(who):
		var to := who.global_position - p.global_position
		to.y = 0.0
		if to.length() > 0.05:
			p.facing = p.facing.slerp(to.normalized(), k).normalized()


## Sweep the blade (or fist / foot) from its last position to now against every opponent's body.
func _sweep() -> void:
	var seg: Array = p.model.strike_segment(_limb())
	var a: Vector3 = seg[0]
	var b: Vector3 = seg[1]
	var pa: Vector3 = _prev_seg[0] if _prev_seg.size() == 2 else a
	var pb: Vector3 = _prev_seg[1] if _prev_seg.size() == 2 else b
	_prev_seg = [a, b]
	var reach := float(move.get("reach", 1.0))
	if log_events and target != null:
		var pts0 := Geometry3D.get_closest_points_between_segments(a, b, target.global_position + Vector3.UP * 0.35, target.global_position + Vector3.UP * 1.55)
		print("      t %.2f blade %s->%s  gap %.2f" % [t, str(a.snappedf(0.01)), str(b.snappedf(0.01)), (pts0[0] as Vector3).distance_to(pts0[1])])
	for f in Player.fighters:
		if f == p or _victims.has(f) or not LockOn.alive(f):
			continue
		if f.global_position.distance_to(p.global_position) > reach + 2.0:
			continue
		var bot := f.global_position + Vector3.UP * 0.35
		var top := f.global_position + Vector3.UP * 1.55
		var hit_at := Vector3.INF
		for k in 4:  # the blade at 4 moments between last frame and this one
			var u := float(k) / 3.0
			var s0 := pa.lerp(a, u)
			var s1 := pb.lerp(b, u)
			var pts := Geometry3D.get_closest_points_between_segments(s0, s1, bot, top)
			if (pts[0] as Vector3).distance_to(pts[1]) <= BODY_RADIUS + BLADE_RADIUS:
				hit_at = pts[0]
				break
		if hit_at != Vector3.INF:
			_victims[f] = true
			apply_hit(f, float(move.get("dmg", 1.0)), float(move.get("poise", 20)), hit_at, bool(move.get("guard_break", false)))


## Deal a blow to `f` (from a swing or a projectile). Returns what happened.
func apply_hit(f: Player, mult: float, poise_dmg: float, at: Vector3, guard_break: bool = false, spell: bool = false) -> String:
	var raw := p.sheet.v("attack_damage") * mult
	if spell:
		raw = (p.sheet.v("attack_damage") * 0.6 + p.sheet.v("spell_power")) * mult
	var crit := crits and rng.randf() < p.sheet.v("crit_chance")
	if crit:
		raw *= p.sheet.v("crit_damage")
	var dir := f.global_position - p.global_position
	dir.y = 0.0
	dir = dir.normalized() if dir.length() > 0.01 else p.facing
	var res := f.combat.receive(self, raw, poise_dmg * p.sheet.v("power"), dir, at, crit, guard_break)
	last_given = res
	if res == "hit" or res == "stagger" or res == "dead" or res == "guard":
		hits_landed += 1
		var heavy := poise_dmg >= 40.0 or res == "stagger" or res == "dead"
		hitstop(95 if heavy else 60)
		var col := Color(0.55, 0.75, 1.0) if res == "guard" else (Color(1.0, 0.85, 0.3) if crit else Color(1.0, 0.55, 0.25))
		Vfx.spark(p, at, -dir, col, 22 if heavy else 12)
		Sfx.at(p, at, "block" if res == "guard" else ("hit_heavy" if heavy else "hit"))
		if p.cam != null:
			p.cam.shake(0.06 if heavy else 0.035)
		if f.cam != null:
			f.cam.shake(0.08 if heavy else 0.04, Vector2(-dir.x, 0.3))
	elif res == "parry":
		hitstop(140)
		Sfx.at(p, at, "parry")
		Vfx.ring(f, at, Color(0.6, 0.9, 1.0), 0.7)
	if log_events:
		print("   [%s] %s %s -> %s %s" % [p.sheet.display_name, move_kind, str(move.get("clip", "")), f.sheet.display_name, res])
	return res


# ---- ranged ----------------------------------------------------------------------------------

func _enter_aim() -> void:
	state = S.AIM
	aim_t = 0.0
	if stance_id == "bow":
		p.model.hold(str(stance.get("aim", "")))


func _fire_ranged(aimed: bool) -> void:
	if stance_id == "bow":
		if not aimed:
			_enter_aim()
			return
		var arrow: Dictionary = stance["arrow"]
		var draw := clampf(aim_t / float(arrow["full_draw"]), 0.25, 1.0)
		var dir := _aim_dir()
		var pr := Projectile.make(p, "arrow", p.model.weapon_tip() + dir * 0.4, dir * float(arrow["speed"]) * (0.5 + 0.5 * draw),
			float(arrow["gravity"]), float(arrow["dmg"]) * draw, float(arrow["poise"]) * draw, 0.06)
		p.get_parent().add_child(pr)
		p.model.play(str(stance.get("fire", "")), 1.6, 0.1)
		Sfx.at(p, p.global_position, "swing", 1.6)
		aim_t = 0.0
	else:
		_start("light")


## Where an arrow / spell goes: at the locked target's chest, else along the camera's aim.
func _aim_dir() -> Vector3:
	var from := p.model.weapon_tip()
	var who := p.lock.current() if p.lock != null else null
	if who != null:
		return (who.model.chest_position() - from).normalized()
	if p.cam != null:
		var c := p.cam.camera
		var far := c.global_position - c.global_basis.z * 60.0
		var q := PhysicsRayQueryParameters3D.create(c.global_position, far, Layers.WORLD | Layers.PROPS)
		q.exclude = [p.get_rid()]
		var hit := p.get_world_3d().direct_space_state.intersect_ray(q)
		var to: Vector3 = hit["position"] if not hit.is_empty() else far
		return (to - from).normalized()
	return p.facing


## A spell leaves the staff at the moment the cast clip throws it.
func _cast_check() -> void:
	if _cast_done or not move.has("spell") or t < window.x:
		return
	_cast_done = true
	var sp: Dictionary = HeroAnim.data["_spells"][move["spell"]]
	var col: Array = sp["color"]
	var c := Color(col[0], col[1], col[2])
	if move["spell"] == "nova":
		var r := float(sp["radius"])
		Vfx.ring(p, p.global_position + Vector3.UP * 0.2, c, r * 0.5)
		for f in Player.fighters:
			if f != p and LockOn.alive(f) and f.global_position.distance_to(p.global_position) <= r:
				apply_hit(f, float(sp["dmg"]), float(sp["poise"]), f.model.chest_position(), false, true)
		Sfx.at(p, p.global_position, "parry", 0.6)
		return
	var dir := _aim_dir()
	var pr := Projectile.make(p, str(move["spell"]), p.model.weapon_tip() + dir * 0.3, dir * float(sp["speed"]), 0.0,
		float(sp["dmg"]), float(sp["poise"]), float(sp["radius"]))
	pr.color = c
	pr.homing = float(sp.get("homing", 0.0))
	pr.blast = float(sp.get("blast", 0.0))
	pr.homing_target = p.lock.current() if p.lock != null else null
	p.get_parent().add_child(pr)
	Vfx.cast_flare(p, "light", "might")


# ---- being hit -------------------------------------------------------------------------------

func receive(att: Combat, raw: float, poise_dmg: float, dir: Vector3, at: Vector3, crit: bool, guard_break: bool) -> String:
	if state == S.DEAD:
		return "none"
	if p.invuln_t > 0.0:
		last_result = "dodge"
		return "dodge"
	hits_taken += 1
	poise_t = 0.0
	if not p.is_ai and not att.p.is_ai and att.p != p:
		raw *= PVP_DAMAGE
	var from_dir := att.p.global_position - p.global_position
	from_dir.y = 0.0
	var frontal := from_dir.length() < 0.01 or p.facing.dot(from_dir.normalized()) >= GUARD_COS
	if state == S.GUARD and frontal and not guard_break:
		# parry: guard tapped just before the blow, or while it was already swinging at you
		var swinging := att.state == S.ATTACK and guard_t <= att.t - att.window.x + 0.05
		if guard_t <= PARRY_WINDOW or swinging:
			att.stagger(PARRIED_TIME, true)
			p.toast.emit("Parry!")
			last_result = "parry"
			return "parry"
		var taken := p.sheet.take_physical(raw * (1.0 - float(stance.get("block", 0.6))))
		last_damage = taken
		p._spend_once(raw * 0.25)
		p.model.play(str(stance.get("guard_hit", "")), 1.4, 0.1)
		Vfx.number(p, at + Vector3.UP * 0.3, "%d" % roundi(taken), Color(0.6, 0.8, 1.0))
		if p.exhausted:
			stagger(STAGGER_TIME, false)
			p.toast.emit("Guard broken")
		last_result = "guard"
		return "guard"
	var dmg := p.sheet.take_physical(raw)
	last_damage = dmg
	var ls := att.p.sheet.v("lifesteal")
	if ls > 0.0:
		att.p.sheet.hp = minf(att.p.sheet.v("max_hp"), att.p.sheet.hp + dmg * ls)
	Vfx.number(p, at + Vector3.UP * 0.3, ("%d!" if crit else "%d") % roundi(dmg), Color(1.0, 0.4, 0.3) if crit else Color.WHITE)
	if p.sheet.hp <= 0.0:
		die(dir)
		last_result = "dead"
		return "dead"
	poise -= poise_dmg * (1.0 - p.sheet.v("tenacity"))
	# the player keeps swinging through light hits (hyper armour); enemies flinch from any of them
	var armoured := not p.is_ai and state == S.ATTACK and poise_dmg < 40.0
	if poise <= 0.0 or guard_break:
		poise = _poise_max()
		stagger(STAGGER_TIME if p.is_ai else STAGGER_TIME * 0.6, true, dir)
		last_result = "stagger"
		return "stagger"
	if not armoured and state != S.ATTACK:
		p.model.play(_react_clip(dir, false), 1.5, 0.15)
	last_result = "hit"
	return "hit"


func _react_clip(dir: Vector3, large: bool) -> String:
	var hits: Dictionary = HeroAnim.data["_hits"]
	var side := "f"
	var local := p.facing.dot(-dir)
	var right := p.facing.cross(Vector3.UP).dot(-dir)
	if local < -0.5:
		side = "b"
	elif right > 0.5:
		side = "r"
	elif right < -0.5:
		side = "l"
	return str(hits[("large_" if large else "small_") + side])


## Knocked off balance: can't act for `time`, a big reaction from where the blow came from.
func stagger(time: float, big: bool, dir: Vector3 = Vector3.ZERO) -> void:
	if state == S.DEAD:
		return
	_end_move()
	state = S.STAGGER
	stagger_left = time
	var d := dir if dir != Vector3.ZERO else -p.facing
	p.model.play(_react_clip(d, big), 1.2, 0.2)


func die(dir: Vector3) -> void:
	_end_move()
	state = S.DEAD
	var deaths: Array = HeroAnim.data["_deaths"]
	var local := p.facing.dot(-dir)
	p.model.play(str(deaths[0] if local > 0.0 else deaths[1]), 1.0, 0.0)
	p.on_death()


## Freeze the moment of impact (single-player: the whole world), released on the real-time clock.
static func hitstop(ms: int) -> void:
	_stop_until = maxi(_stop_until, Time.get_ticks_msec() + ms)
	Engine.time_scale = 0.06


func _process(_delta: float) -> void:
	if Engine.time_scale < 1.0 and _stop_until > 0 and Time.get_ticks_msec() >= _stop_until:
		Engine.time_scale = 1.0
		_stop_until = 0


func reset() -> void:
	state = S.IDLE
	move = {}
	combo_i = 0
	buffered = ""
	press_t = -1.0
	stagger_left = 0.0
	window = Vector2(-1, -1)
	if p != null and p.model != null:
		p.model.hold("")
