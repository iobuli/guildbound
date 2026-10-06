class_name Player
extends CharacterBody3D
## BotW-style third-person controller. Kinematic capsule + explicit state machine.
## Movement numbers come from the stat engine (CharModel), so buffs and gear change how it moves.
## The visible body also has ball-and-box physics parts (BodyRig) that follow the animation and
## turn into a ragdoll on hard falls, knockouts, or G.

enum State { GROUND, AIR, CLIMB, SWIM, RAGDOLL, RECOVER }

signal toast(text: String)

const GRAVITY := 20.0
const TERMINAL := 40.0
const STEP_HEIGHT := 0.4
const COYOTE := 0.12
const JUMP_BUFFER := 0.12
const SPRINT_MULT := 1.35
const SPRINT_CLAMP := 12.0
const CROUCH_MULT := 0.5
const GROUND_ACCEL := 45.0
const GROUND_DECEL := 60.0
const AIR_CONTROL := 0.6
const TURN_RATE := 15.7        # max 900 deg/s
const TURN_SMOOTH := 14.0      # exponential ease into the new heading
const STEP_SMOOTH := 0.07      # seconds for the body to catch up after a step snap
const DODGE_SPEED := 9.0
const DODGE_TIME := 0.35
const DODGE_IFRAMES := 0.2
const SAFE_FALL := 8.0
const RAGDOLL_FALL := 10.0     # falls longer than this knock you down
const FALL_DMG_PER_M := 0.06
const KO_TIME := 2.5
const CLIMB_MAX_NORMAL_Y := 0.643  # steeper than 50 deg is not walkable -> climbable
const CLIMB_FAST_MULT := 1.45
const WET_MULT := 2.0
const STAMINA_REGEN := 18.0
const STAMINA_DELAY := 0.6
const COST_SPRINT := 10.0
const COST_CLIMB := 6.0
const COST_CLIMB_FAST := 10.0
const COST_JUMP := 8.0
const COST_SWIM := 4.0
const COST_DODGE := 15.0
const COST_SIDE_HOP := 8.0
const LOCK_STRAFE := 0.75      # move speed while circling a locked target
const SIDE_HOP_SPEED := 4.8
const SIDE_HOP_TIME := 0.25
const PAGE1 := ["gale_step", "bright_valor", "aegis", "shadowrush"]
const PAGE2 := ["quick_step", "strength_surge", "stone_skin", "bone_carapace"]

## Live fighters (lock-on and AI queries). Entered on _ready, left on _exit_tree.
static var fighters: Array[Player] = []

var sheet: CharModel
var cam: CameraRig       # null for AI / remote fighters
var intent := Intent.new()
var is_ai := false       # true: no camera, no mouse; `intent` comes from an `ai` child (think) or from outside
var arena: Arena

var state: int = State.AIR
var stamina := 120.0
var stamina_delay := 0.0
var exhausted := false
var wet := false
var crouching := false

var coyote_t := 0.0
var jump_buf_t := 0.0
var jumping := false
var air_time := 0.0
var peak_y := 0.0
var was_on_floor := false
var air_velocity := Vector3.ZERO
var dodge_t := 0.0
var dodge_cd := 0.0
var invuln_t := 0.0
var dodge_dir := Vector3.ZERO
var climb_normal := Vector3.BACK
var climb_block_t := 0.0
var climb_hop_t := 0.0
var carried: RigidBody3D = null
var interact_target: RigidBody3D = null
var interact_t := 0.0
var facing := Vector3(0.0, 0.0, -1.0)
var respawn_pos := Vector3(0.0, 0.1, 0.0)
var last_h_speed := 0.0
var sprinting := false
var pushing := false
var push_body: RigidBody3D = null
var _push_hold := 0.0  # a rolling ball breaks contact for a frame now and then; don't flicker
var carry_lift := 1.0
var _lift_from := Transform3D.IDENTITY
var push_limit := INF
var sliding := false
var on_stairs := false
var _stairs_t := 0.0
var move_intent := Vector3.ZERO
var step_offset := 0.0
var ragdoll_t := 0.0
var settle_t := 0.0
var knocked_out := false
var recover_t := 0.0
var recover_kind := ""
var seen_states := {}  # used by the self-test
var model: CharacterVisual
var combat: Combat
var lock: LockOn
var dodge_speed := DODGE_SPEED
var _auras := {}  # active effect key -> aura particles
var body_id := "Superhero_Male_FullBody"
var hair_id := "Hair_SimpleParted"
var capsule: CapsuleShape3D
var shape: CollisionShape3D
var _released: Array = []  # [[body, seconds]] recently dropped props, still excluded from our colliders


func setup(p_arena: Arena, p_sheet: CharModel) -> void:
	arena = p_arena
	sheet = p_sheet
	collision_layer = BodyRig.L_PLAYER
	collision_mask = BodyRig.L_WORLD | BodyRig.L_PROPS | BodyRig.L_PLAYER  # fighters can't walk through each other
	floor_max_angle = deg_to_rad(50.0)
	floor_snap_length = 0.3
	floor_stop_on_slope = true
	max_slides = 6
	capsule = CapsuleShape3D.new()
	capsule.radius = 0.35
	capsule.height = 1.8
	shape = CollisionShape3D.new()
	shape.shape = capsule
	shape.position = Vector3(0.0, 0.9, 0.0)
	add_child(shape)
	model = CharacterVisual.new()
	add_child(model)
	model.build(body_id, hair_id)
	for part in model.rig.parts:
		var b: RigidBody3D = part["body"]
		b.set_meta("fighter", self)
		b.set_meta("part", part["name"])
	combat = Combat.new()
	combat.name = "combat"
	add_child(combat)
	combat.setup(self)
	lock = LockOn.new()
	lock.name = "lock"
	add_child(lock)
	lock.setup(self)
	if not is_ai:
		cam = CameraRig.new()
		add_child(cam)
		cam.setup(self, get_rid())
	stamina = sheet.v("stamina")
	peak_y = global_position.y


# ---- helpers ---------------------------------------------------------------------------------

func _ray(from: Vector3, to: Vector3, mask: int = BodyRig.L_WORLD) -> Dictionary:
	var q := PhysicsRayQueryParameters3D.create(from, to, mask)
	q.exclude = [get_rid()]
	return get_world_3d().direct_space_state.intersect_ray(q)


func _water_surface_y_at(p: Vector3) -> float:
	for a in arena.water_volumes:
		if p.x > a.position.x and p.x < a.end.x and p.z > a.position.z and p.z < a.end.z and p.y > a.position.y:
			return a.end.y
	return NAN


func _water_surface_y() -> float:
	return _water_surface_y_at(global_position)


func _enter_tree() -> void:
	if not fighters.has(self):
		fighters.append(self)


func _exit_tree() -> void:
	fighters.erase(self)


## This frame's intent: the local keyboard, an `ai` child's think(player, delta), or whatever was set from outside.
func _read_intent(delta: float) -> void:
	var ai := get_node_or_null("ai")
	if ai != null and ai.has_method("think"):
		intent = ai.think(self, delta)
	elif not is_ai:
		intent = Intent.from_input(cam)


func _enter(s: int) -> void:
	state = s
	seen_states[s] = true
	match s:
		State.CLIMB:
			floor_snap_length = 0.0
			velocity = Vector3.ZERO
			peak_y = global_position.y
		State.SWIM:
			floor_snap_length = 0.0
			peak_y = global_position.y
		_:
			floor_snap_length = 0.3


func _spend(rate: float, delta: float) -> void:
	stamina = maxf(0.0, stamina - rate * delta)
	stamina_delay = STAMINA_DELAY
	if stamina <= 0.0:
		exhausted = true


func _spend_once(cost: float) -> void:
	stamina = maxf(0.0, stamina - cost)
	stamina_delay = STAMINA_DELAY
	if stamina <= 0.0:
		exhausted = true


func _climbable(col: Object) -> bool:
	if col == null or col is RigidBody3D:
		return false
	var tag: String = col.get_meta("tag", "rock") if col.has_meta("tag") else "rock"
	return tag != "ice" and tag != "glass"


func _set_crouch(on: bool) -> void:
	if on == crouching:
		return
	if not on and test_move(global_transform, Vector3.UP * 0.75):
		return  # no headroom
	crouching = on
	capsule.height = 1.1 if on else 1.8
	shape.position.y = 0.55 if on else 0.9
	if cam != null:
		cam.height = 1.0 if on else 1.55


func teleport(pos: Vector3) -> void:
	_drop_carried(Vector3.ZERO)
	if state == State.RAGDOLL or state == State.RECOVER:
		model.cancel_ragdoll()
	shape.disabled = false
	knocked_out = false
	global_position = pos
	velocity = Vector3.ZERO
	_enter(State.AIR)
	sheet.restore()
	stamina = sheet.v("stamina")
	exhausted = false
	peak_y = pos.y
	climb_block_t = 0.0
	dodge_t = 0.0
	jumping = false
	step_offset = 0.0
	reset_physics_interpolation()
	model.rig.reset()
	combat.reset()
	lock.target = null


func _respawn() -> void:
	toast.emit("Knocked out. Back to the spawn point.")
	teleport(respawn_pos)


# ---- main loop -------------------------------------------------------------------------------

func _physics_process(delta: float) -> void:
	var events := sheet.tick(delta)
	for e in events:
		toast.emit(e)
	if sheet.hp <= 0.0 and not knocked_out:
		knocked_out = true
		toast.emit("Knocked out!")
		start_ragdoll(Vector3.ZERO)
	climb_block_t = maxf(0.0, climb_block_t - delta)
	dodge_cd = maxf(0.0, dodge_cd - delta)
	invuln_t = maxf(0.0, invuln_t - delta)
	_read_intent(delta)
	if intent.just_jump:
		jump_buf_t = JUMP_BUFFER
	else:
		jump_buf_t = maxf(0.0, jump_buf_t - delta)
	_tick_abilities()
	lock.tick(delta)
	combat.tick(delta)
	if cam != null:
		cam.lock_target = lock.target

	var in_dir := intent.world_move()
	if lock.target != null and (state == State.GROUND or state == State.AIR):
		# locked on: forward / back / left / right are relative to the opponent, not the camera
		var to := lock.target.global_position - global_position
		to.y = 0.0
		if to.length() > 0.05:
			var f := to.normalized()
			in_dir = f.cross(Vector3.UP) * intent.move.x + f * intent.move.y
	move_intent = in_dir
	_tick_stamina(delta)
	match state:
		State.GROUND, State.AIR:
			_tick_ground_air(delta, in_dir)
		State.CLIMB:
			_tick_climb(delta)
		State.SWIM:
			_tick_swim(delta, in_dir)
		State.RAGDOLL:
			_tick_ragdoll(delta)
		State.RECOVER:
			_tick_recover(delta)
	if state != State.RAGDOLL:
		_tick_carry(delta)
		_push_props()
	_tick_released(delta)
	_update_model(delta)
	last_h_speed = Vector2(velocity.x, velocity.z).length()
	intent.clear_once()  # a press acts once, even when the intent was handed in from outside


func _tick_stamina(delta: float) -> void:
	var smax := sheet.v("stamina")
	stamina_delay -= delta
	if stamina_delay <= 0.0:
		stamina = minf(smax, stamina + STAMINA_REGEN * delta)
	stamina = minf(stamina, smax)
	if exhausted and stamina >= 0.3 * smax:
		exhausted = false


func _tick_ground_air(delta: float, in_dir: Vector3) -> void:
	var ms := sheet.v("move_speed")
	var on_floor := is_on_floor()

	var sy := _water_surface_y()
	if not is_nan(sy) and global_position.y < sy - 1.0 and velocity.y < 1.0:
		_enter(State.SWIM)
		return

	if on_floor:
		coyote_t = COYOTE
		air_time = 0.0
		if not was_on_floor and _on_land():
			return
	else:
		coyote_t -= delta
		air_time += delta
		if was_on_floor:
			peak_y = global_position.y
		peak_y = maxf(peak_y, global_position.y)

	# crouch
	if intent.just_crouch and on_floor:
		_set_crouch(not crouching)
	if crouching and intent.sprint:
		_set_crouch(false)

	# climb entry from the air
	if not on_floor and in_dir.length() > 0.1 and climb_block_t <= 0.0 and _try_grab(in_dir):
		return

	# dodge
	var can_dodge := combat.state != Combat.S.WINDUP and combat.state != Combat.S.ACTIVE and combat.state != Combat.S.STAGGER
	if intent.just_dodge and on_floor and dodge_cd <= 0.0 and not exhausted and stamina >= COST_DODGE and can_dodge:
		_spend_once(COST_DODGE)
		combat.reset()
		dodge_t = DODGE_TIME
		dodge_speed = DODGE_SPEED
		invuln_t = DODGE_IFRAMES
		dodge_cd = DODGE_TIME + 0.2
		dodge_dir = in_dir.normalized() if in_dir.length() > 0.1 else -facing
		if lock.target == null:
			facing = dodge_dir

	# speed for this frame
	sprinting = false
	var speed := ms
	if crouching:
		speed *= CROUCH_MULT
	elif intent.sprint and in_dir.length() > 0.1 and on_floor and not exhausted and stamina > 0.0:
		sprinting = true
		speed = minf(ms * SPRINT_MULT, SPRINT_CLAMP)
		_spend(COST_SPRINT, delta)
	if carried != null:
		speed *= 1.0 - 0.4 * minf(1.0, carried.mass / maxf(1.0, sheet.v("carry")))
	if lock.target != null and not sprinting:
		speed *= LOCK_STRAFE
	speed *= combat.move_scale()
	speed = minf(speed, push_limit)  # leaning on a heavy prop: move at the prop's pace, no stop-go

	# horizontal velocity
	var h := Vector3(velocity.x, 0.0, velocity.z)
	if dodge_t > 0.0:
		dodge_t -= delta
		h = dodge_dir * dodge_speed
	elif combat.push_v.length() > 0.01 and on_floor:
		h = combat.push_v  # lunging into a punch / knocked back
	elif on_floor:
		var target := in_dir * speed
		var accel := GROUND_DECEL if in_dir.length() < 0.01 else GROUND_ACCEL
		h = h.move_toward(target, accel * delta)
		if lock.target != null and not sprinting:
			# circling: each straight step along the tangent drifts outward; pull in by exactly that much
			var r := lock.target.global_position - global_position
			r.y = 0.0
			var rl := r.length()
			if rl > 0.3:
				var tang := h - r / rl * h.dot(r / rl)
				h += r / rl * (tang.length_squared() * delta / (2.0 * rl))
	else:
		# keep momentum in the air (sprint jumps carry), steer with reduced authority
		var keep := maxf(speed, h.length())
		var target := in_dir * keep if in_dir.length() > 0.01 else h
		h = h.move_toward(target, GROUND_ACCEL * AIR_CONTROL * delta)
	velocity.x = h.x
	velocity.z = h.z
	var face := _fight_facing()
	if face != Vector3.ZERO:
		facing = face
	elif h.length() > 0.5:
		facing = h.normalized()

	# locked on, Space is a side-hop instead of a jump
	if jump_buf_t > 0.0 and on_floor and lock.target != null and in_dir.length() > 0.1 and dodge_t <= 0.0 			and not combat.busy() and stamina >= COST_SIDE_HOP and not exhausted:
		jump_buf_t = 0.0
		_spend_once(COST_SIDE_HOP)
		dodge_t = SIDE_HOP_TIME
		dodge_speed = SIDE_HOP_SPEED
		dodge_dir = in_dir.normalized()
		velocity.y = 3.5
		on_floor = false
	# jump
	if jump_buf_t > 0.0 and (on_floor or coyote_t > 0.0) and dodge_t <= 0.0:
		if crouching:
			_set_crouch(false)
		jump_buf_t = 0.0
		coyote_t = 0.0
		jumping = true
		velocity.y = sqrt(2.0 * GRAVITY * sheet.v("jump_height"))
		_spend_once(COST_JUMP)
		on_floor = false
		peak_y = global_position.y
	elif on_floor:
		velocity.y = 0.0
		if sprinting and _try_vault(in_dir):
			jumping = false
	if not on_floor:
		velocity.y = maxf(velocity.y - GRAVITY * delta, -TERMINAL)
	if jumping and velocity.y > 0.0 and not intent.jump:
		velocity.y *= 0.5
		jumping = false
	if velocity.y <= 0.0:
		jumping = false

	_step_up(Vector3(velocity.x, 0.0, velocity.z) * delta)
	was_on_floor = on_floor
	air_velocity = velocity
	var y0 := global_position.y
	move_and_slide()
	# floor snapping down a step moves us instantly; let the body ease down instead
	if on_floor and is_on_floor():
		var dy := global_position.y - y0
		if dy < -0.05:
			step_offset = clampf(step_offset - dy, -0.5, 0.5)
	state = State.GROUND if is_on_floor() else State.AIR
	seen_states[state] = true
	_stairs_t -= delta
	on_stairs = _stairs_t > 0.0 and state == State.GROUND

	# sliding down a slope too steep to stand on
	sliding = false
	if state == State.AIR and is_on_wall() and velocity.y <= 0.5:
		var n := get_wall_normal()
		if n.y > 0.3 and n.y <= CLIMB_MAX_NORMAL_Y:
			sliding = true
			facing = Vector3(n.x, 0.0, n.z).normalized()


## Which way to face while fighting: at the strike's target, else at the locked target (unless
## sprinting away), else at whoever we are guarding against. ZERO = face the way we move.
func _fight_facing() -> Vector3:
	var who: Player = null
	if combat.striking() or combat.state == Combat.S.CHARGING:
		who = combat.target
	elif lock.target != null and not sprinting:
		who = lock.target
	elif combat.guarding():
		who = lock.current()
	if who == null or not is_instance_valid(who):
		return Vector3.ZERO
	var to := who.global_position - global_position
	to.y = 0.0
	return to.normalized() if to.length() > 0.05 else Vector3.ZERO


## Returns true if the landing knocked us down.
func _on_land() -> bool:
	var fall := peak_y - global_position.y
	peak_y = global_position.y
	if fall <= SAFE_FALL or not is_nan(_water_surface_y()):
		return false
	var dmg := sheet.damage_percent_of_max(FALL_DMG_PER_M * (fall - SAFE_FALL))
	toast.emit("Hard landing from %.1f m: -%d HP" % [fall, int(dmg)])
	if fall > RAGDOLL_FALL:
		var v := air_velocity
		start_ragdoll(Vector3(v.x, -v.y * 0.15, v.z) * 0.6)
		return true
	return false


func _step_up(motion: Vector3) -> void:
	if not is_on_floor() or motion.length() < 0.0005:
		return
	var from := global_transform
	if not test_move(from, motion):
		return
	var lift := Vector3.UP * STEP_HEIGHT
	if test_move(from, lift):
		return
	if test_move(from.translated(lift), motion):
		return
	global_position += lift
	step_offset = clampf(step_offset - STEP_HEIGHT, -0.5, 0.5)  # body eases up instead of popping
	_stairs_t = 0.5


func _try_vault(in_dir: Vector3) -> bool:
	if in_dir.length() < 0.1:
		return false
	var d := in_dir.normalized()
	var low := global_position + Vector3.UP * 0.3
	var hit := _ray(low, low + d * 0.7)
	if hit.is_empty() or absf(hit["normal"].y) > 0.3 or not _climbable(hit["collider"]):
		return false
	var top_from: Vector3 = hit["position"] + d * 0.3 + Vector3.UP * 1.6
	var top := _ray(top_from, top_from + Vector3.DOWN * 1.8)
	if top.is_empty() or top["normal"].y < 0.7:
		return false
	var height: float = top["position"].y - global_position.y
	if height < 0.45 or height > 1.2:
		return false
	velocity.y = sqrt(2.0 * GRAVITY * (height + 0.35))
	toast.emit("Vault")
	return true


# ---- climbing --------------------------------------------------------------------------------

func _try_grab(dir: Vector3) -> bool:
	if exhausted or stamina <= 0.0 or carried != null:
		return false
	var d := dir.normalized()
	var from := global_position + Vector3.UP * 1.0
	var hit := _ray(from, from + d * 0.75)
	if hit.is_empty():
		return false
	var n: Vector3 = hit["normal"]
	if n.y > CLIMB_MAX_NORMAL_Y or n.y < -0.3 or not _climbable(hit["collider"]):
		return false
	climb_normal = n
	_enter(State.CLIMB)
	toast.emit("Climbing")
	return true


func _tick_climb(delta: float) -> void:
	var n := climb_normal
	var v := intent.move
	var fast := intent.sprint
	var spd := sheet.v("climb_speed") * (CLIMB_FAST_MULT if fast else 1.0)
	var up := (Vector3.UP - n * n.y).normalized()
	var right := (-n).cross(Vector3.UP).normalized()
	var move := right * v.x + up * v.y
	if move.length() > 0.05:
		var rate := (COST_CLIMB_FAST if fast else COST_CLIMB) * (WET_MULT if wet else 1.0)
		_spend(rate, delta)
	if intent.just_jump and stamina >= COST_JUMP and not exhausted:
		_spend_once(COST_JUMP)
		climb_hop_t = 0.25
	velocity = move.limit_length(1.0) * spd + (-n) * 1.5
	if climb_hop_t > 0.0:
		climb_hop_t -= delta
		velocity += up * 4.5
	if exhausted or stamina <= 0.0:
		toast.emit("Exhausted! You slip off.")
		climb_block_t = 0.6
		_enter(State.AIR)
		return
	if intent.just_crouch:
		climb_block_t = 0.4
		_enter(State.AIR)
		return
	move_and_slide()
	peak_y = global_position.y
	facing = Vector3(-n.x, 0.0, -n.z).normalized() if Vector2(n.x, n.z).length() > 0.01 else facing

	if is_on_floor() and v.y < -0.1:
		climb_block_t = 0.3
		_enter(State.GROUND)
		return
	var from := global_position + Vector3.UP * 1.0
	var hit := _ray(from, from + (-n) * 1.1)
	if not hit.is_empty() and _climbable(hit["collider"]) and hit["normal"].y <= CLIMB_MAX_NORMAL_Y:
		climb_normal = climb_normal.lerp(hit["normal"], 0.5).normalized()
	else:
		_end_climb_at_edge(n)


func _end_climb_at_edge(n: Vector3) -> void:
	var nh := Vector3(n.x, 0.0, n.z).normalized()
	var probe := global_position + Vector3.UP * 1.9 - nh * 0.6
	var top := _ray(probe, probe + Vector3.DOWN * 2.6)
	climb_block_t = 0.6
	if not top.is_empty() and top["normal"].y > 0.7:
		velocity = -nh * 3.0 + Vector3.UP * 6.5
		toast.emit("Mantle")
	_enter(State.AIR)


# ---- swimming --------------------------------------------------------------------------------

func _tick_swim(delta: float, in_dir: Vector3) -> void:
	var sy := _water_surface_y()
	if is_nan(sy) or global_position.y > sy - 0.7:
		_enter(State.AIR)
		return
	var ms := sheet.v("move_speed")
	var fast := intent.sprint and not exhausted
	var sp := ms * (0.8 if fast else 0.5)
	if exhausted:
		sp *= 0.3
	if in_dir.length() > 0.05:
		_spend(COST_SWIM * (2.0 if fast else 1.0), delta)
	var h := Vector3(velocity.x, 0.0, velocity.z).move_toward(in_dir * sp, 20.0 * delta)
	velocity.x = h.x
	velocity.z = h.z
	if exhausted:
		velocity.y = move_toward(velocity.y, -2.0, 10.0 * delta)
	elif intent.jump:
		velocity.y = move_toward(velocity.y, 3.0, 20.0 * delta)
	elif intent.crouch:
		velocity.y = move_toward(velocity.y, -3.0, 20.0 * delta)
	else:
		var target_y := sy - 1.25
		velocity.y = clampf((target_y - global_position.y) * 3.0, -3.0, 3.0)
	if intent.just_jump and global_position.y > sy - 1.5 and not exhausted:
		velocity.y = 9.5
		_enter(State.AIR)
		coyote_t = 0.0
	if h.length() > 0.3:
		facing = h.normalized()
	move_and_slide()
	peak_y = global_position.y


# ---- ragdoll and recovery ----------------------------------------------------------------------

func start_ragdoll(v: Vector3) -> void:
	if state == State.RAGDOLL:
		return
	_drop_carried(v)
	_set_crouch(false)
	_enter(State.RAGDOLL)
	combat.reset()
	shape.disabled = true
	velocity = Vector3.ZERO
	ragdoll_t = 0.0
	settle_t = 0.0
	dodge_t = 0.0
	model.rig.start_ragdoll(v)


func _ground_below(p: Vector3) -> float:
	var hit := _ray(p + Vector3.UP * 0.5, p + Vector3.DOWN * 3.0)
	return hit["position"].y if not hit.is_empty() else p.y - 0.9


func _tick_ragdoll(delta: float) -> void:
	ragdoll_t += delta
	var pel := model.rig.pelvis()
	var pp := pel.global_position
	if pp.y < -60.0:
		_respawn()
		return
	# the (disabled) capsule and camera ride along with the body
	global_position = Vector3(pp.x, _ground_below(pp), pp.z)
	var spd := model.rig.speed()
	var near_ground := pp.y - global_position.y < 0.45
	# sliding fast along the ground: hands and feet dig in to shed momentum
	model.rig.brace(clampf((spd - 2.0) / 4.0, 0.0, 1.0) if near_ground else 0.0)
	settle_t = settle_t + delta if spd < 0.7 else 0.0
	var sy := _water_surface_y_at(pp)
	if knocked_out:
		if ragdoll_t > KO_TIME:
			_respawn()
		return
	if not is_nan(sy) and pp.y < sy:
		_finish_ragdoll_into(State.SWIM)
		return
	if (ragdoll_t > 0.8 and settle_t > 0.3) or ragdoll_t > 6.0:
		_begin_recover()


func _finish_ragdoll_into(s: int) -> void:
	model.cancel_ragdoll()
	shape.disabled = false
	velocity = Vector3.ZERO
	_enter(s)


## Get up from exactly where the ragdoll lies: face down you push up and stand facing where your head
## was, face up you sit up and stand facing where your feet were (GetUpPose plans it).
func _begin_recover() -> void:
	var rig := model.rig
	var pp := rig.pelvis().global_position
	var face_down := rig.facing_up() <= 0.0
	recover_kind = "front" if face_down else "back"
	var to_feet := rig.head_to_pelvis()
	var natural := -to_feet if face_down else to_feet
	var floor_pt := Vector3(pp.x, _ground_below(pp), pp.z)
	# the natural way up first; against a wall or a prop, turn until the standing spot and every palm,
	# knee and foot of the plan are clear
	var fwd := natural
	var spot := floor_pt
	var best := 99
	var best_deg := 0.0
	for deg: float in [0.0, 45.0, -45.0, 90.0, -90.0, 135.0, -135.0, 180.0]:
		var f := Basis(Vector3.UP, deg_to_rad(deg)) * natural
		var d := model.getup.plan(face_down, floor_pt, f)
		var at := floor_pt + f * d
		at.y = _ground_below(at + Vector3.UP * 0.5)
		var bad := model.getup.blocked() + (0 if _fits(at) else 10)
		if bad < best:
			best = bad
			best_deg = deg
		if bad == 0:
			break
	fwd = Basis(Vector3.UP, deg_to_rad(best_deg)) * natural
	var dd := model.getup.plan(face_down, floor_pt, fwd)
	spot = floor_pt + fwd * dd
	spot.y = _ground_below(spot + Vector3.UP * 0.5)
	if not _fits(spot):
		model.getup.plan(face_down, floor_pt, fwd, 0.0)  # no room anywhere: stand up on the spot
		spot = floor_pt
	global_position = spot
	facing = fwd
	model.rotation.y = atan2(-fwd.x, -fwd.z)
	reset_physics_interpolation()
	shape.disabled = false
	velocity = Vector3.ZERO
	recover_t = model.begin_recovery() + 0.05
	_enter(State.RECOVER)


## Is there room for the standing capsule here?
func _fits(at: Vector3) -> bool:
	var q := PhysicsShapeQueryParameters3D.new()
	q.shape = capsule
	q.transform = Transform3D(Basis.IDENTITY, at + Vector3.UP * (shape.position.y + 0.05))
	q.collision_mask = BodyRig.L_WORLD | BodyRig.L_PROPS | BodyRig.L_PLAYER
	q.exclude = [get_rid()]
	return get_world_3d().direct_space_state.intersect_shape(q, 1).is_empty()


func _tick_recover(delta: float) -> void:
	recover_t -= delta
	velocity.x = 0.0
	velocity.z = 0.0
	velocity.y = 0.0 if is_on_floor() else maxf(velocity.y - GRAVITY * delta, -TERMINAL)
	move_and_slide()
	peak_y = global_position.y
	if model.getup.done() or recover_t < -4.0:
		model.end_recovery()
		was_on_floor = is_on_floor()
		_enter(State.GROUND if is_on_floor() else State.AIR)


# ---- carry and push --------------------------------------------------------------------------

func _find_prop() -> RigidBody3D:
	for y: float in [1.0, 0.4]:
		var from: Vector3 = global_position + Vector3.UP * y
		var hit := _ray(from, from + facing * 2.2, BodyRig.L_PROPS)
		if not hit.is_empty() and hit["collider"] is RigidBody3D:
			return hit["collider"]
	return null


func _hold_transform() -> Transform3D:
	var fwd := -model.global_basis.z
	fwd.y = 0.0
	fwd = fwd.normalized()
	return Transform3D(Basis(Vector3.UP, atan2(-fwd.x, -fwd.z)), global_position + fwd * 0.75 + Vector3.UP * 1.1)


func _drop_carried(v: Vector3) -> void:
	if carried == null:
		return
	var c := carried
	carried = null
	c.freeze = false
	c.linear_velocity = v
	_released.append([c, 0.6])  # stay non-colliding with us until it has cleared our body


func _tick_released(delta: float) -> void:
	for r in _released.duplicate():
		r[1] -= delta
		if r[1] <= 0.0:
			_released.erase(r)
			var b: RigidBody3D = r[0]
			if is_instance_valid(b) and b != carried:
				remove_collision_exception_with(b)
				b.remove_collision_exception_with(self)
				model.rig.exclude(b, false)


func _tick_carry(delta: float) -> void:
	if carried != null:
		if not intent.interact:
			_drop_carried(velocity)
			return
		if intent.just_attack:
			_drop_carried(facing * 11.0 + Vector3.UP * 3.0 + velocity * 0.5)
			model.on_throw()
			toast.emit("Thrown")
			return
		# kinematic: eases up into the arms over 0.3 s, then stays locked there (no jitter)
		carry_lift = minf(1.0, carry_lift + delta / 0.3)
		var e := carry_lift * carry_lift * (3.0 - 2.0 * carry_lift)
		carried.global_transform = _lift_from.interpolate_with(_hold_transform(), e)
		return
	if intent.just_interact:
		# an item lying here goes in the bag first
		var pick := ItemPickup.nearest(global_position + facing * 0.4)
		if pick != null:
			sheet.add_item(pick.item_id, pick.count)
			var nm: String = sheet.item_data(pick.item_id).get("name", pick.item_id)
			toast.emit("Picked up %s%s  (Tab: status window)" % [nm, " x%d" % pick.count if pick.count > 1 else ""])
			pick.queue_free()
			interact_target = null
			return
		interact_target = _find_prop()
		interact_t = 0.0
		if interact_target == null:
			toast.emit("Nothing to interact with")
	elif intent.interact and interact_target != null:
		interact_t += delta
		if interact_t >= 0.25:
			var cap := sheet.v("carry")
			var t := interact_target
			interact_target = null
			if t.mass > cap:
				toast.emit("Too heavy: %s (%d kg > your carry %d kg)" % [t.get_meta("label"), int(t.mass), int(cap)])
			else:
				carried = t
				add_collision_exception_with(t)
				t.add_collision_exception_with(self)
				model.rig.exclude(t, true)
				t.freeze_mode = RigidBody3D.FREEZE_MODE_KINEMATIC
				t.freeze = true
				carry_lift = 0.0
				_lift_from = t.global_transform
				toast.emit("Carrying %s" % t.get_meta("label"))
	elif not intent.interact:
		interact_target = null


## Leaning into a prop moves it at a pace set by its mass against your Carry stat, and you move at
## that same pace, so there is no stop-go jitter.
func _push_props() -> void:
	var cap := sheet.v("carry")
	var want := sheet.v("move_speed")
	_push_hold -= get_physics_process_delta_time()
	if _push_hold <= 0.0:
		pushing = false
		push_body = null
	push_limit = INF
	for i in get_slide_collision_count():
		var col := get_slide_collision(i)
		var body := col.get_collider()
		if not (body is RigidBody3D) or body == carried:
			continue
		var n := col.get_normal()
		var dir := Vector3(-n.x, 0.0, -n.z)
		if dir.length() < 0.01:
			continue
		dir = dir.normalized()
		if move_intent.length() < 0.1 or move_intent.normalized().dot(dir) < 0.5:
			continue
		pushing = true
		push_body = body
		_push_hold = 0.25
		var k := clampf(cap / body.mass, 0.15, 1.0)
		var target_speed := want * k
		var along: float = body.linear_velocity.dot(dir)
		if along < target_speed:
			body.apply_central_impulse(dir * (target_speed - along) * body.mass * 0.5)
		push_limit = minf(push_limit, target_speed + 0.4)


# ---- intent: abilities -----------------------------------------------------------------------

func _tick_abilities() -> void:
	if intent.just_ragdoll and (state == State.GROUND or state == State.AIR):
		start_ragdoll(facing * 3.0 + Vector3.UP * 2.0)  # the body already carries its running momentum
		return
	for i in 4:
		if intent.just_cast[i]:
			var page: Array = PAGE2 if intent.alt_page else PAGE1
			var sid: String = page[i]
			if sheet.find_effect(sid) != null:
				sheet.dismiss(sid)
				toast.emit("Dismissed " + DataStore.spells()[sid]["name"])
			else:
				var r := sheet.cast(sid)
				if r[0]:
					model.on_cast()
					var sp: Dictionary = DataStore.spells()[sid]
					var ms: Array = []
					for m in sp["mods"]:
						ms.append(CharModel.mod_text(m))
					toast.emit("%s  -  %s" % [sp["name"], ", ".join(ms)])
					Vfx.cast_flare(self, sp["aspect"], sp["family"])
				else:
					toast.emit(r[1])


# ---- visuals ---------------------------------------------------------------------------------

func _update_model(delta: float) -> void:
	if state != State.RAGDOLL and state != State.RECOVER:
		var target := atan2(-facing.x, -facing.z)
		var diff := wrapf(target - model.rotation.y, -PI, PI)
		var step := clampf(diff * (1.0 - exp(-delta * TURN_SMOOTH)), -TURN_RATE * delta, TURN_RATE * delta)
		model.rotation.y = wrapf(model.rotation.y + step, -PI, PI)
	step_offset *= exp(-delta / STEP_SMOOTH)
	model.position.y = step_offset
	model.update(self, delta)
	_sync_auras()


## One aura per active Boon (and food), in its family's colour; light ones sparkle, dark ones smoke.
func _sync_auras() -> void:
	var live := {}
	for e in sheet.effects:
		if e["kind"] == "spell" or e["kind"] == "consumable":
			live[e["key"]] = e
	for k in _auras.keys():
		if not live.has(k):
			var old: CPUParticles3D = _auras[k]
			old.emitting = false
			get_tree().create_timer(1.2, false).timeout.connect(old.queue_free)
			_auras.erase(k)
	for k in live:
		if not _auras.has(k):
			var e: Dictionary = live[k]
			var a := Vfx.aura(str(e.get("aspect", "")), str(e.get("family", "")))
			(model.rig.by_name["chest"]["body"] as Node3D).add_child(a)
			_auras[k] = a
