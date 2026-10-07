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
const PARRY_WINDOW := 0.25        # guard pressed this long before a blow (any blow) lands: parry
const PARRY_PERFECT := 0.10       # ... this long: PERFECT parry (long stagger, slow motion, deflects fly faster)
const GUARD_COS := 0.34          # cos 70 deg: blows from inside this arc in front are guarded
const BODY_RADIUS := 0.42
const BLADE_RADIUS := 0.12        # generous on purpose: a blow that looks like it lands, lands
const HURT_PAD := 0.3             # ... and more against a fighter's slim capsules: moves and AI ranges are spaced for the old thick band (tighten after retuning)
const POISE_REGEN := 25.0        # per second, after 1.5 s without being hit
const PLAYER_POISE_MULT := 2.5
const STAGGER_TIME := 0.7
const PARRIED_TIME := 0.9
const PERFECT_PARRIED_TIME := 1.8
const SLOW_SCALE := 0.3           # the world's speed during a perfect parry's slow motion
const PVP_DAMAGE := 0.6

static var _stop_until := 0
static var log_events := false
static var _slow_until := 0

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
var element := "fire"           # the magic element (X): F spells (and the old staff) and the Awakening's touch
var awaken := 0.0               # Awakening meter, 0..100: landing and taking hits fill it
var awaken_t := 0.0             # seconds of Awakening left
var last_dealt := 0.0           # the raw damage of our last blow (lifesteal)
var last_zone := ""             # where our last blow landed on a fighter: head / torso / arm / leg
var rng := RandomNumberGenerator.new()
# magic for everyone (F, any weapon): tap = the element's light spell, hold = its heavy one
var cast_cd := {"light": 0.0, "heavy": 0.0}   # seconds of cooldown left
var cast_press := -1.0
var cast_held := false
var cast_left := 0.0            # seconds of the upper-body cast clip still playing
var casts := 0
var last_cast_from := Vector3.ZERO  # where the last spell left (the casting hand, or the staff tip)
var _cast_release := -1.0       # seconds until the spell leaves the hand (-1: nothing pending)
var _cast_kind := ""
# parry
var last_parry := ""            # "perfect" / "parry": how the last parried blow was met
var last_parry_stagger := 0.0   # how long that attacker was staggered
var parries := 0
var deflects := 0
var _clock := 0.0
var _hand_hist: Array = []      # [time, world position] of the guarding hand over the last frames


## Everything a blow can land on: the fighters and the monsters (slimes).
static func targets() -> Array:
	var out: Array = []
	out.append_array(Player.fighters)
	out.append_array(Slime.all)
	return out


static func alive(f: Node3D) -> bool:
	if f is Player:
		return LockOn.alive(f)
	return f is Slime and (f as Slime).alive()


## The vertical stretch of a body a blade is checked against, and how thick it is.
static func body_band(f: Node3D) -> Array:
	if f is Slime:
		var sl := f as Slime
		return [f.global_position + Vector3.UP * 0.15, f.global_position + Vector3.UP * sl.height * 0.95, sl.radius]
	return [f.global_position + Vector3.UP * 0.35, f.global_position + Vector3.UP * 1.55, BODY_RADIUS]


## Can this fighter's blows hurt `f`? (not itself, not the dead, not its own team)
func is_foe(f: Node3D) -> bool:
	if f == p or not alive(f):
		return false
	var team: String = f.get("team")
	return p.team == "" or team != p.team


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
	return stance_id == "bow"


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
	_clock += delta
	_sample_hand()
	_tick_cast(delta, i)
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
	if i.just_element:
		next_element()
	if i.just_awaken:
		start_awakening()
	if awaken_t > 0.0:
		awaken_t -= delta
		if awaken_t <= 0.0:
			p.toast.emit("Awakening fades")
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
			elif buffered != "" and cast_left <= 0.05:
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
	var chaining := state == S.ATTACK
	state = S.ATTACK
	t = 0.0
	since_attack = 0.0
	_victims.clear()
	_prev_seg = []
	_cast_done = false
	p.model.hold("")
	speed = float(m.get("speed", 1.0)) * clampf(p.sheet.v("attack_speed") / 1.1, 0.8, 1.5)  # a fresh hero (~1.1) plays the clip as authored
	var w := p.model.swing_window(str(m["clip"]), _limb())
	# a chained hit flows out of the last one: it rushes through most of its wind-up (to ~0.22 s before
	# the blow) and cross-fades in, instead of restarting the whole swing from a standstill
	var from := maxf(0.0, w.x - 0.22) if chaining else 0.0
	# the wind-up up to `from` plays WARP x faster: combat time starts behind so that it meets the clip
	# exactly when the wind-up is done (the blow's window is always after it)
	t = from / speed - from / (speed * HeroAnim.WARP)
	dur = t + p.model.play(str(m["clip"]), speed, 0.2, from)
	window = w / speed
	target = p.lock.current() if p.lock != null else null
	_face_target(1.0)
	if log_events:
		print("   [%s] start %s %s: dur %.2f window %.2f-%.2f, target %s at %.2f m" % [p.sheet.display_name, kind, str(m["clip"]), dur,
			window.x, window.y, target.sheet.display_name if target else "-", target.global_position.distance_to(p.global_position) if target else -1.0])
	Sfx.at(p, p.model.chest_position(), _swing_cue(), 1.0 + 0.05 * combo_i)


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
		var face: Node3D = null
		for f in targets():
			if not is_foe(f):
				continue
			var d: float = f.global_position.distance_to(p.global_position)
			var to: Vector3 = (f.global_position - p.global_position).normalized()
			if d < best and p.facing.dot(to) > 0.3:
				best = d
				face = f
		target = face as Player
		if face is Slime:  # turn to a monster in front too (it is never the locked target)
			var ts := face.global_position - p.global_position
			ts.y = 0.0
			if ts.length() > 0.05:
				p.facing = p.facing.slerp(ts.normalized(), k).normalized()
		who = target
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
	for f: Node3D in targets():
		if _victims.has(f) or not is_foe(f):
			continue
		if f.global_position.distance_to(p.global_position) > reach + 2.0:
			continue
		var hit := hurt_test(f, pa, pb, a, b)
		if not hit.is_empty():
			_victims[f] = true
			var zone: String = hit[1]
			last_zone = zone
			var mult := float(move.get("dmg", 1.0)) * float(BodyParts.ZONE_MULT.get(zone, 1.0))
			apply_hit(f, mult, float(move.get("poise", 20)), hit[0], bool(move.get("guard_break", false)))


## Does a blade moving from pa-pb (last frame) to a-b (now) touch `f`? Tested at 4 moments in between:
## a fighter's body capsules (BodyParts: segment-segment distance under the radii), a monster's band.
## Returns [where on the blade, zone] or []. `r` is the blade's (or a projectile's) own radius; a still
## segment (pa-pb == a-b, e.g. a projectile's step this frame) is tested once.
static func hurt_test(f: Node3D, pa: Vector3, pb: Vector3, a: Vector3, b: Vector3, r: float = BLADE_RADIUS) -> Array:
	var caps: Array = (f as Player).model.hurtboxes() if f is Player and (f as Player).model.hurt.size() > 0 else []
	var band := body_band(f)
	for k in (1 if pa == a and pb == b else 4):
		var u := float(k) / 3.0
		var s0 := pa.lerp(a, u)
		var s1 := pb.lerp(b, u)
		if caps.is_empty():
			var pts := Geometry3D.get_closest_points_between_segments(s0, s1, band[0], band[1])
			if (pts[0] as Vector3).distance_to(pts[1]) <= float(band[2]) + r:
				return [pts[0], "torso"]
		else:
			var near: Array = BodyParts.nearest(caps, s0, s1)
			if float(near[0]) <= r + HURT_PAD:
				return [near[1], near[2]]
	return []


## Deal a blow to `f` (from a swing or a projectile). Returns what happened.
func apply_hit(f: Node3D, mult: float, poise_dmg: float, at: Vector3, guard_break: bool = false, spell: bool = false, ranged_hit: bool = false) -> String:
	var raw := p.sheet.v("attack_damage") * mult
	if spell:
		raw = (p.sheet.v("attack_damage") * 0.6 + p.sheet.v("spell_power")) * mult
	var crit := crits and rng.randf() < p.sheet.v("crit_chance")
	if crit:
		raw *= p.sheet.v("crit_damage")
	var dir := f.global_position - p.global_position
	dir.y = 0.0
	dir = dir.normalized() if dir.length() > 0.01 else p.facing
	last_dealt = raw
	var res: String
	if f is Slime:
		res = (f as Slime).receive_hit(self, raw, poise_dmg * p.sheet.v("power"), dir, at, crit)
	else:
		res = (f as Player).combat.receive(self, raw, poise_dmg * p.sheet.v("power"), dir, at, crit, guard_break, ranged_hit)
	last_given = res
	if res == "hit" or res == "stagger" or res == "dead" or res == "guard":
		hits_landed += 1
		gain_awaken(float(HeroAnim.data["_awakening"]["per_hit"]))
		if awakened() and not spell and res != "guard":
			_awaken_proc(f, raw, at)
		var heavy := poise_dmg >= 40.0 or res == "stagger" or res == "dead"
		hitstop(95 if heavy else 60)
		var col := Color(0.55, 0.75, 1.0) if res == "guard" else (Color(1.0, 0.85, 0.3) if crit else Color(1.0, 0.55, 0.25))
		Vfx.spark(p, at, -dir, col, 22 if heavy else 12)
		var blade := _blade() and not spell
		if res == "guard":
			Sfx.at(p, at, "clang" if blade else "block")
		else:
			Sfx.at(p, at, "hit_blade" if blade else ("hit_heavy" if heavy else "hit"))
			if blade and heavy:
				Sfx.at(p, at, "hit_heavy", 0.9, -4.0)
		if p.cam != null:
			p.cam.shake(0.06 if heavy else 0.035)
		if f is Player and (f as Player).cam != null:
			(f as Player).cam.shake(0.08 if heavy else 0.04, Vector2(-dir.x, 0.3))
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
	return _aim_from(p.model.weapon_tip())


func _aim_from(from: Vector3) -> Vector3:
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
	_launch(spell_id(str(move["spell"])), p.model.weapon_tip())


## Send spell `sid` on its way from `from` (the staff tip or a casting hand): at the locked target, else
## along the aim.
func _launch(sid: String, from: Vector3) -> void:
	var sp: Dictionary = HeroAnim.data["_spells"][sid]
	var col: Array = sp["color"]
	var c := Color(col[0], col[1], col[2])
	last_cast_from = from
	if sid == "nova":
		var r := float(sp["radius"])
		Vfx.ring(p, p.global_position + Vector3.UP * 0.2, c, r * 0.5)
		for f: Node3D in targets():
			if is_foe(f) and f.global_position.distance_to(p.global_position) <= r:
				apply_hit(f, float(sp["dmg"]), float(sp["poise"]), f.global_position + Vector3.UP * 0.8, false, true)
		Sfx.at(p, p.global_position, "ice_hit", 0.8)
		return
	var dir := _aim_from(from)
	var elem := _element_of(sid)
	Sfx.at(p, from, (elem if elem != "" else "arcane") + "_cast")
	var n := int(sp.get("count", 1))
	for k in n:  # a fan of shots for spells with a count (Daggers of Denak, Crowfeather)
		var yaw := deg_to_rad(float(sp.get("spread", 0.0)) * (k - (n - 1) * 0.5))
		var d := dir.rotated(Vector3.UP, yaw)
		var pr := Projectile.make(p, sid, from + d * 0.3, d * float(sp["speed"]), 0.0,
			float(sp["dmg"]), float(sp["poise"]), float(sp["radius"]))
		pr.configure(sp)
		pr.element = elem if elem != "" else "arcane"
		pr.color = c
		pr.homing_target = p.lock.current() if p.lock != null else null
		p.get_parent().add_child(pr)
	Vfx.cast_flare(p, "light", "might")


# ---- magic for everyone (F) --------------------------------------------------------------------

## F: light spell on release, heavy once held past HEAVY_HOLD (like the attack button).
func _tick_cast(delta: float, i: Intent) -> void:
	for k: String in ["light", "heavy"]:
		cast_cd[k] = maxf(0.0, float(cast_cd[k]) - delta)
	cast_left = maxf(0.0, cast_left - delta)
	if _cast_release >= 0.0:
		_cast_release -= delta
		if _cast_release < 0.0:
			_release_spell()
	if i.just_spell:
		cast_press = 0.0
		cast_held = false
	if cast_press >= 0.0:
		if i.spell:
			cast_press += delta
			if cast_press >= HEAVY_HOLD and not cast_held:
				cast_held = true
				cast("heavy")
		else:
			if not cast_held:
				cast("light")
			cast_press = -1.0


## The current element's spell for a tap ("light") or a hold ("heavy") of F: what the hotbar shows.
func cast_info(kind: String) -> Dictionary:
	var c: Dictionary = HeroAnim.data["_cast"][kind]
	var sid := spell_id("@" + kind)
	var col: Array = HeroAnim.data["_elements"][element]["color"]
	return {"sid": sid, "name": str(HeroAnim.data["_spells"][sid]["name"]), "mana": float(c["mana"]),
		"cooldown": float(c["cooldown"]), "left": float(cast_cd[kind]), "color": Color(col[0], col[1], col[2])}


## Cast the element's spell with whatever is in hand: the weapon style's cast clip plays on the upper
## body only (the legs keep walking), and the spell leaves the casting hand at the clip's release point.
func cast(kind: String) -> bool:
	if state == S.ATTACK or state == S.STAGGER or state == S.DEAD or _cast_release >= 0.0 or cast_left > 0.1:
		return false
	var c: Dictionary = HeroAnim.data["_cast"][kind]
	if float(cast_cd[kind]) > 0.0:
		return false
	var cost := float(c["mana"])
	if p.sheet.mana < cost:
		p.toast.emit("Not enough mana")
		return false
	p.sheet.mana -= cost
	cast_cd[kind] = float(c["cooldown"])
	if state == S.GUARD or state == S.AIM:
		_end_move()
	var dur := p.model.play_upper(str(stance.get("cast", c["clip"])), float(c["speed"]))
	cast_left = dur
	_cast_kind = kind
	_cast_release = dur * float(c["release"])
	casts += 1
	return true


func _release_spell() -> void:
	_cast_release = -1.0
	if state == S.DEAD or state == S.STAGGER:
		return  # knocked out of the cast: the spell fizzles
	_launch(spell_id("@" + _cast_kind), p.model.bone_pos(cast_hand()))


## The hand that throws: whichever is further out in front of the chest at the release.
func cast_hand() -> String:
	var c := p.model.chest_position()
	var l := (p.model.bone_pos("hand_l") - c).dot(p.facing)
	var r := (p.model.bone_pos("hand_r") - c).dot(p.facing)
	return "hand_l" if l > r else "hand_r"


# ---- parry -------------------------------------------------------------------------------------

## A shield on the arm (an offhand item, or the weapon style's own shield).
func has_shield() -> bool:
	var off: Variant = p.sheet.equipment.get("offhand", "")
	return (off != null and str(off) != "") or str(stance.get("offhand", "")).begins_with("shield")


## Can a parry send a projectile back? A shield, or enough magic in the hands.
func can_deflect() -> bool:
	return has_shield() or p.sheet.v("spell_power") >= float(HeroAnim.data["_parry"]["deflect_spell_power"])


func guard_hand() -> String:
	return "hand_l" if has_shield() else "hand_" + str(stance.get("hand", "r"))


func _sample_hand() -> void:
	if p.model == null:
		return
	_hand_hist.append([_clock, p.model.bone_pos(guard_hand())])
	while _hand_hist.size() > 8:
		_hand_hist.pop_front()


## World velocity of the guarding hand over the last ~0.1 s (m/s).
func hand_velocity() -> Vector3:
	if _hand_hist.size() < 2:
		return Vector3.ZERO
	var b: Array = _hand_hist[_hand_hist.size() - 1]
	var a: Array = b
	for h: Array in _hand_hist:  # the oldest sample at most 0.12 s back
		if float(b[0]) - float(h[0]) <= 0.12:
			a = h
			break
	var dt := float(b[0]) - float(a[0])
	return ((b[1] as Vector3) - (a[1] as Vector3)) / dt if dt > 0.001 else Vector3.ZERO


## What our guard makes of a blow arriving now from `from_dir` (pointing at its source): "perfect" or
## "parry" when guard was pressed within PARRY_PERFECT / PARRY_WINDOW before it (held or just tapped),
## "block" when it is merely held, "" when there is no guard or it comes from the side or behind.
func guard_vs(from_dir: Vector3) -> String:
	from_dir.y = 0.0
	var frontal := from_dir.length() < 0.01 or p.facing.dot(from_dir.normalized()) >= GUARD_COS
	if not frontal or not (state == S.GUARD or (state == S.IDLE and not ranged())):
		return ""
	if guard_t <= PARRY_PERFECT:
		return "perfect"
	if guard_t <= PARRY_WINDOW:
		return "parry"
	return "block" if state == S.GUARD else ""


## A parried blow: sparks and a clang; a PERFECT one rings the parry cue, toasts and slows the world.
func _on_parry(perfect: bool, at: Vector3, msg: String = "") -> void:
	last_parry = "perfect" if perfect else "parry"
	parries += 1
	var col := Color(1.0, 0.92, 0.55) if perfect else Color(0.6, 0.9, 1.0)
	Vfx.spark(p, at, p.facing, col, 36 if perfect else 18)
	Vfx.ring(p, at, col, 1.0 if perfect else 0.7)
	if perfect:
		p.toast.emit("PERFECT PARRY" if msg == "" else "PERFECT PARRY - " + msg)
		Sfx.at(p, at, "parry")
		gain_awaken(float(HeroAnim.data["_awakening"]["per_hit"]))
		hitstop(80)
		slowmo(450)
	else:
		p.toast.emit("Parry!" if msg == "" else msg)
		Sfx.at(p, at, "clang", 1.25)
		hitstop(140)
	if p.cam != null:
		p.cam.shake(0.05)


## A projectile reaching us. Parried in time with a shield on the arm (or enough spell power) it is
## DEFLECTED: sent along the guarding hand's swing, pulled toward its shooter the better the timing
## (perfect: mostly straight back, homing, faster), and it is ours now - it can hit whoever cast it.
## Returns "deflect", or "" to let it strike (receive() then blocks it if the guard is up).
func receive_projectile(pr: Projectile, at: Vector3) -> String:
	if state == S.DEAD or p.invuln_t > 0.0:
		return ""
	var g := guard_vs(-pr.vel)
	if (g != "perfect" and g != "parry") or not can_deflect():
		return ""
	var perfect := g == "perfect"
	var pd: Dictionary = HeroAnim.data["_parry"]
	var old := pr.shooter
	var back := -pr.vel.normalized()
	if is_instance_valid(old) and old != p:
		back = (old.model.chest_position() - at).normalized()
	var hv := hand_velocity()
	var swing := hv.normalized() if hv.length() > 0.6 else p.facing
	swing.y *= 0.35
	swing = swing.normalized()
	var dir := swing.lerp(back, float(pd["perfect_aim" if perfect else "aim"]))
	dir = dir.normalized() if dir.length() > 0.05 else back
	var spd := pr.vel.length() * (float(pd["perfect_speed"]) if perfect else 1.0)
	pr.deflect(p, dir * spd, old if perfect and is_instance_valid(old) else null)
	deflects += 1
	_on_parry(perfect, at, "Deflected!")
	Sfx.at(p, at, "deflect")
	last_result = "deflect"
	return "deflect"


# ---- being hit -------------------------------------------------------------------------------

func receive(att: Combat, raw: float, poise_dmg: float, dir: Vector3, at: Vector3, crit: bool, guard_break: bool, ranged_hit: bool = false) -> String:
	if state == S.DEAD:
		return "none"
	if p.invuln_t > 0.0:
		last_result = "dodge"
		return "dodge"
	hits_taken += 1
	gain_awaken(float(HeroAnim.data["_awakening"]["per_hit_taken"]))
	poise_t = 0.0
	if not p.is_ai and not att.p.is_ai and att.p != p:
		raw *= PVP_DAMAGE
	var from_dir := att.p.global_position - p.global_position
	from_dir.y = 0.0
	var g := guard_vs(from_dir)
	# parry: guard pressed within PARRY_WINDOW before the blow (even a guard-breaker); the last
	# PARRY_PERFECT of it is a perfect parry that leaves the attacker reeling twice as long
	if not ranged_hit and (g == "perfect" or g == "parry"):
		last_parry_stagger = PERFECT_PARRIED_TIME if g == "perfect" else PARRIED_TIME
		att.stagger(last_parry_stagger, true)
		_on_parry(g == "perfect", at)
		last_result = "parry"
		return "parry"
	# a held guard (or a parried projectile we cannot send back) blocks: chip damage
	if g != "" and (ranged_hit or not guard_break):
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
		p.last_hit_by = att.p
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


## A blow from a monster that has no fighter's moves (a slime's hop). Dodging, guarding (a guard tapped
## just before it parries and knocks the monster back) and poise work as against a fighter.
func receive_monster(src: Node3D, raw: float, poise_dmg: float, at: Vector3) -> String:
	if state == S.DEAD:
		return "none"
	if p.invuln_t > 0.0:
		last_result = "dodge"
		return "dodge"
	hits_taken += 1
	gain_awaken(float(HeroAnim.data["_awakening"]["per_hit_taken"]))
	poise_t = 0.0
	var dir := p.global_position - src.global_position
	dir.y = 0.0
	dir = dir.normalized() if dir.length() > 0.01 else -p.facing
	var g := guard_vs(-dir)
	if g == "perfect" or g == "parry":
		if src.has_method("parried"):
			src.call("parried", p)
		if g == "perfect" and src.get("_stun") != null:
			src.set("_stun", PERFECT_PARRIED_TIME + 0.6)  # knocked silly: a free hit or two
		last_parry_stagger = PERFECT_PARRIED_TIME if g == "perfect" else PARRIED_TIME
		_on_parry(g == "perfect", at)
		last_result = "parry"
		return "parry"
	if g == "block":
		var taken := p.sheet.take_physical(raw * (1.0 - float(stance.get("block", 0.6))))
		last_damage = taken
		p.model.play(str(stance.get("guard_hit", "")), 1.4, 0.1)
		Vfx.number(p, at + Vector3.UP * 0.3, "%d" % roundi(taken), Color(0.6, 0.8, 1.0))
		last_result = "guard"
		return "guard"
	var dmg := p.sheet.take_physical(raw)
	last_damage = dmg
	Vfx.number(p, at + Vector3.UP * 0.3, "%d" % roundi(dmg), Color(1.0, 0.75, 0.75))
	if p.sheet.hp <= 0.0:
		p.last_hit_by = null
		die(dir)
		last_result = "dead"
		return "dead"
	poise -= poise_dmg * (1.0 - p.sheet.v("tenacity"))
	if poise <= 0.0:
		poise = _poise_max()
		stagger(STAGGER_TIME * (1.0 if p.is_ai else 0.6), true, dir)
		last_result = "stagger"
		return "stagger"
	if state != S.ATTACK:
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


## A moment of slow motion (a perfect parry), on the real-time clock like hitstop.
static func slowmo(ms: int) -> void:
	_slow_until = maxi(_slow_until, Time.get_ticks_msec() + ms)
	Engine.time_scale = minf(Engine.time_scale, SLOW_SCALE)


func _process(_delta: float) -> void:
	var now := Time.get_ticks_msec()
	if _stop_until > 0 and now >= _stop_until:
		_stop_until = 0
		Engine.time_scale = SLOW_SCALE if now < _slow_until else 1.0
	elif _stop_until == 0 and _slow_until > 0 and now >= _slow_until:
		_slow_until = 0
		Engine.time_scale = 1.0


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


# ---- elements and Awakening ----------------------------------------------------------------

## Which element a spell belongs to ("" for the old staff spells).
func _element_of(sid: String) -> String:
	for e: String in HeroAnim.data["_elements"]:
		var d: Dictionary = HeroAnim.data["_elements"][e]
		if d["light"] == sid or d["heavy"] == sid:
			return e
	return ""


## Is this blow struck with an edge (sword, dagger) rather than a fist, foot or staff?
func _blade() -> bool:
	var w := str(stance.get("weapon", ""))
	return (w.begins_with("sword") or w == "dagger" or w.begins_with("axe")) and _limb() == "hand_" + str(stance.get("hand", "r"))


## The sound of the swing: fists whoosh, kicks thump, swords swish, the great sword roars.
func _swing_cue() -> String:
	if _limb().begins_with("foot"):
		return "swing_kick"
	if not _blade():
		return "swish_light"
	return "swish_heavy" if stance_id == "greatsword" else ("swish_light" if stance_id == "dagger" else "swish_blade")


## "@light" / "@heavy" -> the current element's spell; anything else is a spell id already.
func spell_id(s: String) -> String:
	if s.begins_with("@"):
		return str(HeroAnim.data["_elements"][element][s.substr(1)])
	return s


func next_element() -> void:
	var order: Array = HeroAnim.data["_element_order"]
	element = order[(order.find(element) + 1) % order.size()]
	var e: Dictionary = HeroAnim.data["_elements"][element]
	p.toast.emit("Element: %s  (%s / %s)" % [e["name"], HeroAnim.data["_spells"][e["light"]]["name"], HeroAnim.data["_spells"][e["heavy"]]["name"]])


func awakened() -> bool:
	return awaken_t > 0.0


func gain_awaken(n: float) -> void:
	if awaken_t <= 0.0:
		var was := awaken
		awaken = minf(100.0, awaken + n)
		if was < 100.0 and awaken >= 100.0 and not p.is_ai:
			p.toast.emit("Awakening ready - press R")


## R with a full meter: damage and attack speed up, and melee hits carry your element.
func start_awakening() -> bool:
	if awaken < 100.0 or awaken_t > 0.0:
		return false
	var a: Dictionary = HeroAnim.data["_awakening"]
	var e: Dictionary = HeroAnim.data["_elements"][element]
	awaken = 0.0
	awaken_t = float(a["duration"])
	var s := p.sheet
	s.effects = s.effects.filter(func(x: Dictionary) -> bool: return x["kind"] != "awakening")
	s.effects.append({"key": "awakening", "name": str(e["awaken"]), "kind": "awakening", "icon": "S_Light01",
		"expires_at": s.clock + float(a["duration"]), "slots": 0, "upkeep": 0.0, "hp_drain": 0.0, "backlash": null,
		"aspect": "light", "family": "awakening", "mods": [
			Stats.make_mod("Awakening: " + str(e["awaken"]), "attack_damage", "add_pct", float(a["dmg_pct"])),
			Stats.make_mod("Awakening: " + str(e["awaken"]), "attack_speed", "add_pct", float(a["aspd_pct"]))]})
	s.invalidate()
	var col: Array = e["color"]
	Vfx.ring(p, p.global_position + Vector3.UP * 0.2, Color(col[0], col[1], col[2]), 2.2)
	Sfx.at(p, p.global_position, "awaken")
	p.toast.emit("AWAKENING: %s" % e["awaken"])
	return true


## An Awakened melee blow carries the element.
func _awaken_proc(f: Node3D, raw: float, at: Vector3) -> void:
	var a: Dictionary = HeroAnim.data["_awakening"]
	match str(HeroAnim.data["_elements"][element]["proc"]):
		"burn":
			StatusFx.apply(f, "burn", raw * float(a["proc_burn"]), 3.0, p)
		"chill":
			StatusFx.apply(f, "chill", float(a["proc_chill"]), 2.0, p)
		"hex":
			StatusFx.apply(f, "hex", float(a["proc_hex"]), 4.0, p)
		"lifesteal":
			p.sheet.hp = minf(p.sheet.v("max_hp"), p.sheet.hp + raw * float(a["proc_lifesteal"]))
		"knockback":
			push(f, (f.global_position - p.global_position).normalized(), float(a["proc_knockback"]))
		"chain":
			var next := nearest_foe(f.global_position, 6.0, [f])
			if next != null:
				Vfx.spark(p, at, Vector3.UP, Color(0.75, 0.8, 1.0), 16)
				apply_hit(next, float(move.get("dmg", 1.0)) * float(a["proc_chain"]), 10.0, next.global_position + Vector3.UP * 0.9, false, true)


## The nearest foe of ours within `r` of `from`, skipping `skip`.
func nearest_foe(from: Vector3, r: float, skip: Array) -> Node3D:
	var best: Node3D = null
	var bd := r
	for f: Node3D in targets():
		if skip.has(f) or not is_foe(f):
			continue
		var d := f.global_position.distance_to(from)
		if d < bd:
			bd = d
			best = f
	return best


## Shove a body away (wind).
static func push(f: Node3D, dir: Vector3, strength: float) -> void:
	dir.y = 0.0
	dir = dir.normalized()
	if f is Slime:
		(f as Slime).velocity += dir * strength + Vector3.UP * 3.0
	elif f is Player:
		var pl := f as Player
		pl.velocity += dir * strength + Vector3.UP * 2.5
		pl.global_position += dir * minf(0.6, strength * 0.06)
