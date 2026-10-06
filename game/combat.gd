class_name Combat
extends Node
## Hand-to-hand fighting for one fighter (the player or an AI; both drive it through their Intent).
##
## Strikes are a timeline (startup -> active -> recovery, scaled by Attack Speed). Which strike comes
## out is read from the input: LMB punches, E kicks (while fighting), and the way the mouse was moving
## as you pressed picks the variant (up = uppercut / knee, down = body blow / sweep, left / right =
## hooks / roundhouses; no flick = the jab-cross-hook combo / front kick).
## Punches: the fist is placed by maths (MotionIK aims it at the opponent's surface), the captured clip
## is a style layer. Kicks: the captured clip drives the leg. Either way a hit is the limb's PHYSICAL
## body reaching one of the opponent's body parts (swept each physics step); physics stops it on the
## surface, so nothing passes through.
## Frame-perfect chain: after a hit a beat follows; striking again on it (+-50 ms) is PERFECT (faster,
## x1.3 damage and impact); three in a row, then E, is the finisher.
## Hits resolve through both stat sheets (Attack Damage, crits, Armor, Lifesteal, Power vs Tenacity) and
## the target reacts with its own body: flinch, stagger by stepping, or knockdown into the ragdoll and
## the get-up. Player vs player hits are scaled (PvP tuning, diminishing returns on knockdowns).

enum S { IDLE, WINDUP, ACTIVE, RECOVER, CHARGING, GUARD, STAGGER }

const REF_ATTACK_SPEED := 1.5   # move times in data/moves.json are at this attack speed
const BUFFER := 0.12
const CHARGE_START := 0.35      # LMB held this long turns the combo into a charge
const CHARGE_MAX := 1.0
const PARRY_WINDOW := 0.15
const GUARD_COS := 0.342        # cos 70 deg: hits from inside this arc in front are guarded
const GUARD_CHIP := 0.2
const GUARD_STAMINA := 0.5      # stamina lost per point of raw damage guarded
const FLINCH_BELOW := 1.2
const KNOCKDOWN_AT := 3.5
const KNOCKDOWN_STAGGERED := 2.0
const STAGGER_TIME := 0.45
const GUARD_BREAK_TIME := 0.8
const PARRIED_TIME := 0.9
const FLURRY_TIME := 1.5
const PERFECT_DODGE := 0.2
const PERFECT_WINDOW := 0.05    # +- seconds around the beat
const PERFECT_BEAT := 0.3       # the beat falls this far into the recovery after a hit
const PERFECT_BONUS := 1.3
const FINISHER_CHAIN := 3
const FLICK_MIN := 30.0         # pixels of mouse travel (last ~0.15 s) that count as a direction
const LIMB_RADIUS := {"hand": 0.11, "foot": 0.12}  # limb box plus ~2 cm: a fist arriving on the surface counts
const KNUCKLES := 0.10          # wrist to the front of the fist (less 1 cm: the knuckles press in)
const SURFACE := {"head": 0.12, "chest": 0.11, "pelvis": 0.11}  # struck part's radius / half depth toward the attacker
const ZONE := {"head": 1.15, "chest": 1.0, "pelvis": 1.0}
const COMBO := ["jab", "cross", "hook", "uppercut"]
## Plain LMB presses run a string; each new chain takes the next one, so it never looks the same twice
const STRINGS := [["jab", "cross", "hook", "uppercut"], ["jab", "jab_r", "cross", "hook_r"],
	["jab", "cross", "body", "hook"], ["jab_r", "cross_l", "hook_r", "uppercut_l"]]
const KICK_STRINGS := [["front_kick", "roundhouse_r", "back_kick"], ["side_kick", "roundhouse_l", "front_kick"],
	["front_kick", "side_kick", "roundhouse_r"]]
const REACT := {"head": "ACT_react_face", "chest": "ACT_react_chest", "pelvis": "ACT_react_body"}
const PVP_DAMAGE := 0.6
const PVP_CONTROL := 0.6
const KNOCKDOWN_DR := 15.0      # seconds a knockdown counts toward diminishing returns (PvP)

static var _moves := {}
static var log_events := false  # self-test diagnostics
static var _stop_until := 0

var p: Player
var state: int = S.IDLE
var move_id := ""
var move := {}
var t := 0.0                # time in the current state (combat clock)
var startup := 0.0
var active := 0.0
var recovery := 0.0
var charge := 0.0           # 0..1 at release
var combo_i := 0
var string_i := -1
var kick_i := 0
var kick_string := -1
var _flip := false
var buffered := 0.0
var buffered_kind := ""     # "punch" / "kick"
var buffered_flick := Vector2.ZERO
var press_t := -1.0         # how long LMB has been held, -1 when up
var guard_t := 9.0          # since guard was pressed (parry window)
var stagger_left := 0.0
var slow_left := 0.0        # an opponent perfect-dodged us: our combat clock runs at half speed
var hit_done := false
var target: Player = null   # who this strike is aimed at
var aim_point := Vector3.ZERO
var push_v := Vector3.ZERO  # stagger / lunge velocity the controller applies
var dodge_age := 9.0        # since our own dodge started (perfect dodge)
var crits := true
var rng := RandomNumberGenerator.new()
var hits_landed := 0
var hits_taken := 0
var last_result := ""       # "hit", "guard", "parry", "dodge", "perfect", "knockdown" (tests / HUD)
var last_damage := 0.0
var last_raw := 0.0         # attacker side: raw damage of our last landed strike (before the target's armor)
var last_zone := ""
var last_hit_time := -99.0
var perfect := false        # the strike under way was timed on the beat
var perfect_chain := 0
var beat_at := -1.0         # sheet clock of the next beat (-1 = none)
var beat_from := -1.0       # when the beat started counting down (HUD ring)
var knockdowns: Array[float] = []  # recent knockdown times (PvP diminishing returns)
var _hand_prev := Vector3.ZERO
var _ext_cap := 9.0         # after contact the fist holds where it met the body


func setup(fighter: Player) -> void:
	p = fighter
	rng.seed = hash(fighter.name) + 1234
	_load()


static func _load() -> void:
	if _moves.is_empty():
		_moves = JSON.parse_string(FileAccess.get_file_as_string("res://data/moves.json"))
		_moves.erase("_note")


static func move_data(id: String) -> Dictionary:
	_load()
	return _moves[id]


static func move_ids() -> Array:
	_load()
	return _moves.keys()


## Can this fighter fight right now (on its feet, hands free)?
func can_fight() -> bool:
	return p.state == Player.State.GROUND and p.carried == null and not p.knocked_out


func busy() -> bool:
	return state in [S.WINDUP, S.ACTIVE, S.RECOVER, S.CHARGING, S.STAGGER]


func striking() -> bool:
	return state in [S.WINDUP, S.ACTIVE, S.RECOVER]


func guarding() -> bool:
	return state == S.GUARD


func kicking() -> bool:
	return striking() and str(move.get("kind", "")) == "kick"


## Engaged: hands up in the fighting stance (locked on, guarding, striking or just hit).
func engaged() -> bool:
	return can_fight() and (state != S.IDLE or (p.lock != null and p.lock.target != null) or p.sheet.clock - last_hit_time < 2.0)


## Ready to fight: E kicks instead of picking up / carrying.
func fighting_ready() -> bool:
	if not can_fight() or p.lock == null:
		return false
	return p.lock.target != null or (p.lock.soft != null and p.lock.soft.global_position.distance_to(p.global_position) < 2.5)


## How fast the fighter may move this frame (fraction of normal).
func move_scale() -> float:
	match state:
		S.WINDUP, S.ACTIVE:
			return 0.2
		S.RECOVER:
			return 0.5
		S.CHARGING:
			return 0.35
		S.GUARD:
			return 0.5
		S.STAGGER:
			return 0.0
	return 1.0


## 0 at guard, < 0 pulled back, 1 fully extended at the aim point. The fist travels during startup
## (a short draw back, then out) so it is already arriving as the active window opens, and drives a
## little past the aim point while active - until it meets a body, where it stops.
func extension() -> float:
	return minf(_extension(), _ext_cap)


func _extension() -> float:
	match state:
		S.CHARGING:
			return -0.3 * smoothstep(0.0, 0.25, t)
		S.WINDUP:
			var back := -0.3 * charge if move_id == "charged" else -0.12
			var u := t / maxf(0.001, startup)
			return lerpf(0.0, back, smoothstep(0.0, 0.3, u)) if u < 0.3 else lerpf(back, 0.85, smoothstep(0.3, 1.0, u))
		S.ACTIVE:
			return lerpf(0.85, 1.04, smoothstep(0.0, active * 0.6, t))
		S.RECOVER:
			return 1.04 * (1.0 - smoothstep(0.0, recovery * 0.7, t))
	return 0.0


## The punching hand ("l" / "r") while a punch (or a charge) is under way, else "".
func strike_side() -> String:
	if not (striking() or state == S.CHARGING) or str(move.get("kind", "")) != "punch":
		return ""
	return str(move["limb"]).right(1)


# ---- tick (called from Player._physics_process with the fresh intent) ------------------------

func tick(delta: float) -> void:
	var i := p.intent
	slow_left = maxf(0.0, slow_left - delta)
	var dt := delta * (0.5 if slow_left > 0.0 else 1.0)
	guard_t += delta
	dodge_age += delta
	buffered = maxf(0.0, buffered - delta)
	if i.just_guard:
		guard_t = 0.0
	if p.dodge_t > 0.0 and p.invuln_t > 0.0 and dodge_age > Player.DODGE_TIME:
		dodge_age = 0.0
		_check_perfect_dodge()
	# E kicks while fighting (nothing to pick up): the press is ours, not the carry code's
	var kick := false
	if i.just_interact and fighting_ready() and ItemPickup.nearest(p.global_position + p.facing * 0.4) == null:
		kick = true
		i.just_interact = false
	if (i.just_attack and p.carried == null) or kick:
		buffered = BUFFER
		buffered_kind = "kick" if kick else "punch"
		buffered_flick = i.flick
		_judge_beat()
		if not kick:
			press_t = 0.0
	elif i.attack and press_t >= 0.0:
		press_t += delta
	var released := press_t >= 0.0 and not i.attack
	if released:
		press_t = -1.0
	push_v = push_v.move_toward(Vector3.ZERO, 6.0 * delta)
	if beat_at > 0.0 and p.sheet.clock > beat_at + PERFECT_WINDOW * 3.0:
		beat_at = -1.0
	if p.sheet.clock - last_hit_time > 2.0 and not striking():
		perfect_chain = 0

	if not can_fight():
		if state != S.IDLE:
			_enter(S.IDLE)
		return
	t += dt
	match state:
		S.IDLE, S.GUARD:
			if i.guard and not p.exhausted:
				if state != S.GUARD:
					_enter(S.GUARD)
			elif state == S.GUARD:
				_enter(S.IDLE)
			if buffered > 0.0 and state == S.IDLE:
				_start(_choose())
			elif press_t >= CHARGE_START and state == S.IDLE:
				_charge()
			if state == S.IDLE and p.sheet.clock - last_hit_time > 1.2:
				combo_i = 0
				kick_i = 0
		S.WINDUP:
			_lunge()
			# already this close, the limb can arrive before the active window: it lands (and stops) early
			if not hit_done and t >= startup * 0.3:
				_sweep()
			if t >= startup:
				_enter(S.ACTIVE, t - startup)
		S.ACTIVE:
			_lunge()
			if not hit_done:
				_sweep()
			if t >= active:
				_enter(S.RECOVER, t - active)
				if hit_done:
					beat_from = p.sheet.clock
					beat_at = p.sheet.clock + recovery * PERFECT_BEAT
				if buffered > 0.0 and move_id != "charged":  # chained on this very frame
					_start(_choose(), t)
		S.RECOVER:
			# the next strike follows from a press inside the first 60 % of recovery; holding turns into a
			# charge; guarding cancels what is left of the recovery
			if i.guard and not p.exhausted:
				_enter(S.GUARD)
			elif t <= recovery * 0.6 and buffered > 0.0 and move_id != "charged":
				_start(_choose())
			elif press_t >= CHARGE_START and move_id != "charged":
				_charge()
			elif t >= recovery:
				_enter(S.IDLE)
		S.CHARGING:
			_aim()
			if released or t >= CHARGE_MAX + 0.5:
				charge = clampf(t / CHARGE_MAX, 0.0, 1.0)
				_start("charged")
		S.STAGGER:
			stagger_left -= dt
			if stagger_left <= 0.0:
				_enter(S.IDLE)


## Was the press on the beat? Sets `perfect` for the strike it starts and keeps the chain count.
func _judge_beat() -> void:
	perfect = false
	if beat_at < 0.0:
		return
	if absf(p.sheet.clock - beat_at) <= PERFECT_WINDOW:
		perfect = true
		perfect_chain += 1
		p.toast.emit("PERFECT x%d" % perfect_chain)
	else:
		perfect_chain = 0
	beat_at = -1.0


## Which strike the buffered press asks for (input kind + mouse flick + combo position).
func _choose() -> String:
	var f := buffered_flick
	var dirn := ""
	if f.length() >= FLICK_MIN:
		if absf(f.y) > absf(f.x):
			dirn = "up" if f.y < 0.0 else "down"
		else:
			dirn = "left" if f.x < 0.0 else "right"
	if buffered_kind == "kick":
		if perfect and perfect_chain >= FINISHER_CHAIN:
			perfect_chain = 0
			return "finisher"
		var dist := _foe_distance()
		if p.sprinting and dist > 1.6 and dist < 5.0:
			return "jump_kick"   # sprinting in: a flying kick closes the gap
		match dirn:
			"left":
				return "roundhouse_l"
			"right":
				return "roundhouse_r"
			"down":
				return "sweep"
			"up":
				return "knee"
		if kick_i == 0:
			kick_string = (kick_string + 1) % KICK_STRINGS.size()
		var ks: Array = KICK_STRINGS[kick_string]
		var kid: String = ks[kick_i % ks.size()]
		kick_i += 1
		return kid
	match dirn:
		"up":
			_flip = not _flip
			return "uppercut" if _flip else "uppercut_l"
		"down":
			return "body"
		"left":
			return "hook"
		"right":
			return "hook_r"
	if combo_i > 0 and _foe_distance() < 0.62:
		combo_i += 1
		return "elbow"       # in too close for a straight punch
	if combo_i == 0:
		string_i = (string_i + 1) % STRINGS.size()
	var st: Array = STRINGS[string_i]
	var id: String = st[combo_i % st.size()]
	combo_i += 1
	return id


func _foe_distance() -> float:
	var f: Player = p.lock.current() if p.lock != null else null
	if f == null or not is_instance_valid(f):
		return 99.0
	return Vector2(f.global_position.x - p.global_position.x, f.global_position.z - p.global_position.z).length()


func _enter(s: int, carry: float = 0.0) -> void:
	state = s
	t = carry


func _charge() -> void:
	move_id = "charged"
	move = _moves["charged"]
	_aim()
	_enter(S.CHARGING)


func _start(id: String, carry: float = 0.0) -> void:
	move_id = id
	move = _moves[id]
	if not p.exhausted:
		p._spend_once(float(move["stamina"]))
	var k := clampf(REF_ATTACK_SPEED / maxf(0.1, p.sheet.v("attack_speed")), 0.5, 1.5)
	startup = float(move["startup"]) * k * (0.6 if perfect else 1.0)
	active = float(move["active"])
	recovery = float(move["recovery"]) * k
	if id != "charged":
		charge = 0.0
	hit_done = false
	_ext_cap = 9.0
	buffered = 0.0
	_aim()
	_enter(S.WINDUP, carry)
	_hand_prev = _limb_point()
	if log_events:
		print("   [%s] %s timing: AS %.2f startup %.3f active %.3f recovery %.3f%s, clock %.3f" % [p.sheet.display_name, id,
			p.sheet.v("attack_speed"), startup, active, recovery, " PERFECT" if perfect else "", p.sheet.clock])
		print("   [%s] %s start: target %s at %.2f m" % [p.sheet.display_name, id, target.sheet.display_name if target else "-",
			target.global_position.distance_to(p.global_position) if target else -1.0])
	# the captured clip is timed so its impact frame lands as the active window opens
	var clip := "MOVE_" + id
	if p.model.tree.has_animation(clip):
		var speed := (float(move["hit"]) - float(move["from"])) / maxf(0.05, startup)
		if move["kind"] == "kick":
			p.model.act_stop()
			p.model.play_full(clip, speed)
		else:
			p.model.act(clip, speed)  # upper body: the clip shapes the punch, MotionIK lands the fist
	if perfect:
		Vfx.ring(p, _limb_body().global_position, Color(1.0, 0.85, 0.3), 0.35)


## Who and where: the locked / soft target's head (chin, belly...), else straight ahead.
func _aim() -> void:
	target = p.lock.current() if p.lock != null else null
	var zone: String = move.get("aim", "head") if not move.is_empty() else "head"
	if target != null and is_instance_valid(target) and target.state != Player.State.RAGDOLL:
		var key := "head" if zone in ["head", "chin"] else "chest"
		var part: RigidBody3D = target.model.rig.by_name[key]["body"]
		# the wrist goes where the knuckles meet the surface facing us, not into the body
		var toward := p.global_position - target.global_position
		toward.y = 0.0
		toward = toward.normalized() if toward.length() > 0.01 else -p.facing
		var lift := {"head": 0.02, "chin": -0.08, "chest": 0.12, "belly": -0.12, "feet": -0.8}.get(zone, 0.0) as float
		aim_point = part.global_position + toward * (float(SURFACE[key]) + KNUCKLES) + Vector3.UP * lift
		var to := target.global_position - p.global_position
		to.y = 0.0
		if to.length() > 0.05:
			p.facing = to.normalized()
	else:
		target = null
		aim_point = p.global_position + p.facing * float(move.get("range", 0.74)) + Vector3.UP * (1.6 if zone == "head" else 1.2)


## Close the gap during startup / active so the strike arrives at its range.
func _lunge() -> void:
	var step := float(move.get("step", 0.0))
	if target == null or not is_instance_valid(target):
		if step > 0.0 and state == S.WINDUP:
			push_v = p.facing * step / maxf(0.05, startup + active)
		return
	_aim()
	if target == null:
		push_v = Vector3.ZERO
		return
	var to := target.global_position - p.global_position
	to.y = 0.0
	var gap := to.length() - float(move.get("range", 0.74)) + step
	if gap > 0.02:
		# in range by the time the limb arrives (the end of startup), not after it
		var left := startup - t if state == S.WINDUP else active - t
		push_v = to.normalized() * minf(gap / maxf(0.04, left), float(move.get("lunge", 4.5)))
	else:
		push_v = Vector3.ZERO


func _limb_body() -> RigidBody3D:
	return p.model.rig.by_name[str(move.get("limb", "hand_r"))]["body"]


## Centre of the striking fist / foot (its collision box), not the wrist / ankle joint.
func _limb_point() -> Vector3:
	var b := _limb_body()
	return (b.get_child(0) as Node3D).global_position

## Sweep the striking limb's physical body from where it was last step to where it is now.
func _sweep() -> void:
	var from := _hand_prev
	var to := _limb_point()
	_hand_prev = to
	var r: float = LIMB_RADIUS["foot" if str(move["limb"]).begins_with("foot") else "hand"]
	var sh := CapsuleShape3D.new()
	sh.radius = r
	var seg := to - from
	sh.height = seg.length() + r * 2.0
	var xf := Transform3D(Basis.IDENTITY, (from + to) * 0.5)
	if seg.length() > 0.001:
		xf.basis = Basis(Quaternion(Vector3.UP, seg.normalized()))
	var q := PhysicsShapeQueryParameters3D.new()
	q.shape = sh
	q.transform = xf
	q.collision_mask = BodyRig.L_BODY
	q.exclude = p.model.rig.rids()
	var best: RigidBody3D = null
	var best_d := INF
	for res in p.get_world_3d().direct_space_state.intersect_shape(q, 16):
		var c: Object = res["collider"]
		if not (c is RigidBody3D) or not c.has_meta("fighter") or c.get_meta("fighter") == p:
			continue
		var d := (c as RigidBody3D).global_position.distance_to(to)
		if d < best_d:
			best_d = d
			best = c
	if log_events and target != null:
		var all_hits := p.get_world_3d().direct_space_state.intersect_shape(q, 16)
		var names: Array = []
		for hh in all_hits:
			names.append(str((hh["collider"] as Object).get("name")))
		var chest_: RigidBody3D = target.model.rig.by_name["chest"]["body"]
		var tp_: RigidBody3D = target.model.rig.by_name["head"]["body"]
		print("   [%s] %s %s t=%.2f limb->head %.2f m, centres %.2f" % [p.sheet.display_name, move_id, "windup" if state == S.WINDUP else "active", t,
			to.distance_to(tp_.global_position), target.global_position.distance_to(p.global_position)])
	if best == null:
		return
	hit_done = true
	_ext_cap = _extension()
	# the limb has met a body: it gives instead of driving on into it
	p.model.rig.flinch(str(move["limb"]), Vector3.ZERO, 0.12)
	var victim: Player = best.get_meta("fighter")
	var part: String = best.get_meta("part")
	var dir := victim.global_position - p.global_position
	dir.y = 0.0
	dir = dir.normalized() if dir.length() > 0.01 else p.facing
	var zone := "limb"
	if part == "head" or part == "chest" or part == "pelvis":
		zone = part
	var raw := p.sheet.v("attack_damage") * float(move["mult"]) * (1.0 + 0.8 * charge if move_id == "charged" else 1.0)
	raw *= float(ZONE.get(zone, 0.85)) * (PERFECT_BONUS if perfect else 1.0)
	var crit := crits and rng.randf() < p.sheet.v("crit_chance")
	if crit:
		raw *= p.sheet.v("crit_damage")
	var impact := float(move["impact"]) * (1.0 + 0.4 * charge if move_id == "charged" else 1.0) * p.sheet.v("power")
	impact *= PERFECT_BONUS if perfect else 1.0
	last_raw = raw
	last_zone = zone
	var res := victim.combat.receive(self, raw, impact, dir, best, crit)
	if log_events:
		print("   [%s] %s HIT %s %s -> %s" % [p.sheet.display_name, move_id, victim.sheet.display_name, part, res])
	last_result = res
	var spark_col := Color(1.0, 0.85, 0.3) if perfect else (Color(0.5, 0.75, 1.0) if res == "guard" else Color(1.0, 0.55, 0.25))
	Vfx.spark(p, to, -dir, spark_col, 18 if impact >= 2.0 else 10)
	if res == "hit" or res == "knockdown" or res == "guard":
		hits_landed += 1
		hitstop(90 if impact >= 2.0 else 70)
		var shake := clampf(impact * 0.04, 0.03, 0.2)
		for f in [p, victim]:
			if f.cam != null:
				f.cam.shake(shake)
	if res == "parry":
		hitstop(110)


# ---- being hit ------------------------------------------------------------------------------

## Resolve an incoming strike. Returns what happened.
func receive(att: Combat, raw: float, impact: float, dir: Vector3, part: RigidBody3D, crit: bool) -> String:
	if p.state == Player.State.RAGDOLL or p.state == Player.State.RECOVER:
		return "none"
	last_hit_time = p.sheet.clock
	hits_taken += 1
	var pvp := not p.is_ai and not att.p.is_ai and att.p != p
	if pvp:
		raw *= PVP_DAMAGE
		impact *= PVP_CONTROL
	if p.invuln_t > 0.0:
		last_result = "dodge"
		return "dodge"
	var from_dir := att.p.global_position - p.global_position
	from_dir.y = 0.0
	var frontal := from_dir.length() < 0.01 or p.facing.dot(from_dir.normalized()) >= GUARD_COS
	if log_events:
		print("   [%s] receive: state %d guard_t %.3f frontal %s invuln %.2f" % [p.sheet.display_name, state, guard_t, frontal, p.invuln_t])
	if state == S.GUARD and frontal:
		if guard_t <= PARRY_WINDOW:
			att.stagger(PARRIED_TIME, -dir * 1.5)
			p.model.act("ACT_parry", 2.4)
			att.p.model.act("ACT_react_face", 1.6)
			p.toast.emit("Parry!")
			Vfx.ring(p, part.global_position, Color(0.6, 0.9, 1.0), 0.6)
			last_result = "parry"
			return "parry"
		last_damage = p.sheet.take_physical(raw * GUARD_CHIP)
		p.stamina = maxf(0.0, p.stamina - raw * GUARD_STAMINA)
		p.stamina_delay = Player.STAMINA_DELAY
		p.model.ik.hit(0.3)
		Vfx.number(p, part.global_position + Vector3.UP * 0.3, "%d" % roundi(last_damage), Color(0.6, 0.8, 1.0))
		if p.stamina <= 0.0:
			p.exhausted = true
			stagger(GUARD_BREAK_TIME, dir * 1.2)
			p.toast.emit("Guard broken")
		last_result = "guard"
		return "guard"
	var was_staggered := state == S.STAGGER
	last_damage = p.sheet.take_physical(raw)
	var ls := att.p.sheet.v("lifesteal")
	if ls > 0.0:
		att.p.sheet.hp = minf(att.p.sheet.v("max_hp"), att.p.sheet.hp + last_damage * ls)
	Vfx.number(p, part.global_position + Vector3.UP * 0.3, ("%d!" if crit else "%d") % roundi(last_damage),
		Color(1.0, 0.85, 0.3) if att.perfect else (Color(1.0, 0.4, 0.3) if crit else Color(1.0, 1.0, 1.0)))
	var eff := impact * (1.0 - p.sheet.v("tenacity"))
	if crit:
		p.toast.emit("Critical hit: -%d" % int(last_damage))
	var trip := bool(att.move.get("trip", false)) and eff >= 1.2
	if eff >= KNOCKDOWN_AT or (was_staggered and eff >= KNOCKDOWN_STAGGERED) or trip or p.sheet.hp <= 0.0:
		# PvP: repeated knockdowns within 15 s count less, the third is shrugged off as a stagger
		var now := p.sheet.clock
		knockdowns = knockdowns.filter(func(x: float) -> bool: return now - x < KNOCKDOWN_DR)
		if pvp and knockdowns.size() >= 2 and p.sheet.hp > 0.0:
			stagger(STAGGER_TIME, dir * 0.8)
			last_result = "hit"
			return "hit"
		knockdowns.append(now)
		_enter(S.IDLE)
		var up := 0.3 if trip else 1.5
		p.start_ragdoll(dir * (2.0 + eff) + Vector3.UP * up)
		part.apply_central_impulse(dir * eff * 3.0)
		if trip:
			# swept: the feet go out from under
			for f in ["foot_l", "foot_r", "calf_l", "calf_r"]:
				(p.model.rig.by_name[f]["body"] as RigidBody3D).apply_central_impulse(dir * 6.0 + Vector3.UP * 2.0)
		p.toast.emit("Knockdown")
		last_result = "knockdown"
		return "knockdown"
	# the struck part gives: the drive lets go of it for a moment and the blow pushes it
	p.model.rig.flinch(part.get_meta("part"), dir * (1.5 + eff) * 2.0, 0.18)
	p.model.ik.hit(clampf(eff * 0.6, 0.3, 1.2))
	# light hits do not stop a strike already under way; otherwise the body shows where it was hit
	if not striking() or eff >= FLINCH_BELOW:
		var pn := str(part.get_meta("part"))
		p.model.act(REACT.get(pn, "ACT_react_chest"), clampf(1.4 + 0.25 * eff, 1.4, 2.4))
	if eff >= FLINCH_BELOW:
		stagger(STAGGER_TIME * (PVP_CONTROL if pvp else 1.0), dir * (0.6 + 0.3 * eff))
	last_result = "hit"
	return "hit"


## Knocked off balance: can't act, pushed back by stepping (the feet plant and step, no sliding).
func stagger(time: float, push: Vector3) -> void:
	if not can_fight():
		return
	_enter(S.STAGGER)
	stagger_left = time
	push_v = Vector3(push.x, 0.0, push.z)
	p.model.ik.hit(0.8)


## A dodge that starts as a strike aimed at us is about to land (inside 0.2 s) is a perfect dodge.
func _check_perfect_dodge() -> void:
	for f in Player.fighters:
		if f == p or not is_instance_valid(f):
			continue
		var c := f.combat
		if c.target != p or f.global_position.distance_to(p.global_position) > 2.4:
			continue
		var lands_in := INF
		if c.state == S.WINDUP:
			lands_in = c.startup - c.t
		elif c.state == S.ACTIVE and not c.hit_done:
			lands_in = 0.0
		if lands_in <= PERFECT_DODGE:
			_flurry(c)
			last_result = "perfect"
			return


func _flurry(att: Combat) -> void:
	att.slow_left = FLURRY_TIME
	var s := p.sheet
	var e = s.find_effect("flurry")
	if e != null:
		s.effects.erase(e)
	var exp_at: float = s.clock + FLURRY_TIME
	s.effects.append({"key": "flurry", "name": "Flurry", "kind": "combat", "expires_at": exp_at, "slots": 0,
		"upkeep": 0.0, "hp_drain": 0.0, "backlash": null, "aspect": "", "family": "",
		"mods": [Stats.make_mod("Flurry (perfect dodge)", "attack_speed", "more", 1.3, null, exp_at)]})
	s.invalidate()
	p.toast.emit("Flurry")
	Vfx.ring(p, p.global_position + Vector3.UP * 1.0, Color(0.5, 1.0, 0.8), 0.9)


## Freeze the moment of impact (single-player: the whole world). Released on the real-time clock by
## every Combat's _process, which runs each drawn frame whatever the time scale.
static func hitstop(ms: int) -> void:
	_stop_until = maxi(_stop_until, Time.get_ticks_msec() + ms)
	Engine.time_scale = 0.05


func _process(_delta: float) -> void:
	if Engine.time_scale < 1.0 and _stop_until > 0 and Time.get_ticks_msec() >= _stop_until:
		Engine.time_scale = 1.0
		_stop_until = 0


func reset() -> void:
	_enter(S.IDLE)
	combo_i = 0
	kick_i = 0
	push_v = Vector3.ZERO
	press_t = -1.0
	buffered = 0.0
	stagger_left = 0.0
	slow_left = 0.0
	perfect = false
	perfect_chain = 0
	beat_at = -1.0
