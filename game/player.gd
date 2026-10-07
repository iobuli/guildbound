class_name Player
extends CharacterBody3D
## A fighter: the local player or an AI (both fill the same Intent, so they move and fight through the
## same code; later, the network). A capsule moved by Godot's CharacterBody3D; the HeroAnim draws it.
##   ground  camera-relative movement (or relative to the locked target), accelerate / brake, turn to
##           face where you go (or the target / where you aim), run, Shift sprint, C crouch
##   air     jump, keep momentum, steer a little, land (hard landings stagger)
##   dodge   Alt: a roll forward, or a step in any direction while locked / guarding / aiming;
##           invulnerable for the first part, carried by the clip's own motion
##   attacks the Combat node plays them; their ground travel is the clip's root motion
##   water   wade, then swim at the surface
##   dead    the death clip; the player comes back at the spawn point, an AI gets back up

signal toast(text: String)

enum State { GROUND, AIR, SWIM, DODGE, DEAD }

const GRAVITY := 22.0
const TERMINAL := 40.0
const JUMP_SPEED := 7.0
const COYOTE := 0.12
const JUMP_BUFFER := 0.12
const ACCEL := 32.0
const DECEL := 40.0
const AIR_CONTROL := 0.35
const TURN_SMOOTH := 12.0
const RUN_FACTOR := 0.62       # run speed = Move Speed x this (Move Speed 7 -> ~4.3 m/s: the run clips' pace)
const SPRINT_MULT := 1.45
const WALK_SPEED := 1.8
const CROUCH_SPEED := 1.4
const DODGE_IFRAMES := 0.35
const DODGE_COOLDOWN := 0.15
const FALL_SAFE := 7.0
const FALL_DMG_PER_M := 0.05
const RESPAWN_AFTER := 4.0
const STAMINA_REGEN := 22.0
const STAMINA_DELAY := 0.6
const COST_SPRINT := 9.0
const COST_JUMP := 6.0
const COST_DODGE := 14.0
const PAGE1 := ["gale_step", "bright_valor", "aegis", "shadowrush"]
const PAGE2 := ["quick_step", "strength_surge", "stone_skin", "bone_carapace"]

static var fighters: Array[Player] = []

var sheet: CharModel
var cam: CameraRig            # null for AI / remote fighters
var intent := Intent.new()
var is_ai := false
var arena: Arena
var state: int = State.AIR
var model: HeroAnim
var combat: Combat
var lock: LockOn
var body_id := "Superhero_Male_FullBody"
var hair_id := "Hair_SimpleParted"
var stance_id := "sword_shield"
var facing := Vector3(0.0, 0.0, -1.0)
var respawn_pos := Vector3.ZERO
var stamina := 100.0
var stamina_delay := 0.0
var exhausted := false
var sprinting := false
var crouching := false
var knocked_out := false        # dead, for the lock-on / AI
var wet := false                # rain (arena flag)
var last_h_speed := 0.0
var dodge_t := 0.0
var invuln_t := 0.0
var dead_t := 0.0
var _dodge_cd := 0.0
var _coyote := 0.0
var _jump_buf := 0.0
var _peak_y := 0.0
var _land_lock := 0.0
var _auras := {}
var shape: CollisionShape3D


func setup(p_arena: Arena, p_sheet: CharModel) -> void:
	arena = p_arena
	sheet = p_sheet
	collision_layer = Layers.PLAYER
	collision_mask = Layers.WORLD | Layers.PROPS | Layers.PLAYER
	floor_max_angle = deg_to_rad(50.0)
	floor_snap_length = 0.35
	floor_stop_on_slope = true
	floor_constant_speed = true
	max_slides = 6
	var cap := CapsuleShape3D.new()
	cap.radius = 0.32
	cap.height = 1.8
	shape = CollisionShape3D.new()
	shape.shape = cap
	shape.position = Vector3(0.0, 0.9, 0.0)
	add_child(shape)
	model = HeroAnim.new()
	add_child(model)
	model.build(body_id, hair_id)
	combat = Combat.new()
	combat.name = "combat"
	combat.stance_id = stance_id
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


func _enter_tree() -> void:
	if not fighters.has(self):
		fighters.append(self)


func _exit_tree() -> void:
	fighters.erase(self)


func _ray(from: Vector3, to: Vector3, mask: int = Layers.WORLD) -> Dictionary:
	var q := PhysicsRayQueryParameters3D.create(from, to, mask)
	q.exclude = [get_rid()]
	return get_world_3d().direct_space_state.intersect_ray(q)


func _water_surface_y() -> float:
	var p := global_position
	for a in arena.water_volumes:
		if p.x > a.position.x and p.x < a.end.x and p.z > a.position.z and p.z < a.end.z and p.y > a.position.y:
			return a.end.y
	return NAN


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


func teleport(pos: Vector3) -> void:
	global_position = pos
	velocity = Vector3.ZERO
	state = State.AIR
	knocked_out = false
	dead_t = 0.0
	sheet.restore()
	stamina = sheet.v("stamina")
	exhausted = false
	_peak_y = pos.y
	dodge_t = 0.0
	invuln_t = 0.0
	combat.reset()
	model.stop_action(0.05)
	lock.target = null
	reset_physics_interpolation()


## The Combat node calls this when health runs out.
func on_death() -> void:
	state = State.DEAD
	knocked_out = true
	dead_t = 0.0
	velocity = Vector3.ZERO
	toast.emit("%s is down" % sheet.display_name)


# ---- main loop -------------------------------------------------------------------------------

func _physics_process(delta: float) -> void:
	for e in sheet.tick(delta):
		toast.emit(e)
	invuln_t = maxf(0.0, invuln_t - delta)
	_dodge_cd = maxf(0.0, _dodge_cd - delta)
	_land_lock = maxf(0.0, _land_lock - delta)
	_read_intent(delta)
	_jump_buf = JUMP_BUFFER if intent.just_jump else maxf(0.0, _jump_buf - delta)
	if state == State.DEAD:
		_tick_dead(delta)
		return
	_tick_abilities()
	lock.tick(delta)
	combat.tick(delta)
	_tick_stamina(delta)
	var in_dir := intent.world_move()
	if lock.target != null:
		var to := lock.target.global_position - global_position
		to.y = 0.0
		if to.length() > 0.05:
			var f := to.normalized()
			in_dir = f.cross(Vector3.UP) * intent.move.x + f * intent.move.y
	var water := _water_surface_y()
	if not is_nan(water) and water - global_position.y > 1.25:
		state = State.SWIM
	elif state == State.SWIM:
		state = State.GROUND if is_on_floor() else State.AIR
	match state:
		State.GROUND, State.AIR, State.SWIM:
			_tick_move(delta, in_dir, water)
		State.DODGE:
			_tick_dodge(delta)
	_update_model(delta)
	last_h_speed = Vector2(velocity.x, velocity.z).length()
	if cam != null:
		cam.lock_target = lock.target
		cam.speed = last_h_speed
		cam.aiming = combat.aiming()
	intent.clear_once()


func _read_intent(delta: float) -> void:
	var ai := get_node_or_null("ai")
	if ai != null and ai.has_method("think"):
		intent = ai.think(self, delta)
	elif not is_ai:
		intent = Intent.from_input(cam)
	# E: an item in reach goes in the bag; otherwise it is the weapon's special attack
	if intent.just_interact:
		var pick := ItemPickup.nearest(global_position + facing * 0.4)
		if pick != null:
			sheet.add_item(pick.item_id, pick.count)
			var nm: String = sheet.item_data(pick.item_id).get("name", pick.item_id)
			toast.emit("Picked up %s%s  (Tab: inventory)" % [nm, " x%d" % pick.count if pick.count > 1 else ""])
			Sfx.at(self, pick.global_position, "pickup")
			pick.queue_free()
		else:
			intent.just_special = true


func _tick_stamina(delta: float) -> void:
	var smax := sheet.v("stamina")
	stamina_delay -= delta
	if stamina_delay <= 0.0:
		stamina = minf(smax, stamina + STAMINA_REGEN * delta)
	if exhausted and stamina >= 0.3 * smax:
		exhausted = false


func _tick_move(delta: float, in_dir: Vector3, water: float) -> void:
	var on_floor := is_on_floor()
	if on_floor:
		_coyote = COYOTE
		if state == State.AIR:
			_on_land()
		if state != State.SWIM:
			state = State.GROUND
	else:
		_coyote -= delta
		if state == State.GROUND and _coyote <= 0.0:
			state = State.AIR
			_peak_y = global_position.y
	if state == State.AIR:
		_peak_y = maxf(_peak_y, global_position.y)
	if intent.just_crouch and on_floor:
		crouching = not crouching
	# speed for this frame
	var run := clampf(sheet.v("move_speed") * RUN_FACTOR, 3.0, 6.5)
	var speed := run
	sprinting = false
	var careful := lock.target != null or combat.guarding() or combat.aiming()
	if crouching:
		speed = CROUCH_SPEED
		if intent.sprint:
			crouching = false
	elif intent.sprint and in_dir.length() > 0.1 and not exhausted and stamina > 0.0 and not combat.aiming():
		sprinting = true
		speed = run * SPRINT_MULT
		_spend(COST_SPRINT, delta)
	elif careful:
		speed = minf(run, WALK_SPEED * 1.6)
	if state == State.SWIM:
		speed = 2.2
	speed *= combat.move_scale()
	if _land_lock > 0.0:
		speed *= 0.3
	# horizontal velocity: the attack / reaction clip moves us (root motion), otherwise the stick does
	var h := Vector3(velocity.x, 0.0, velocity.z)
	if (combat.busy() or _land_lock > 0.0) and model.acting():
		var rm := model.root_motion_world() / maxf(delta, 0.0001)
		h = Vector3(rm.x, 0.0, rm.z) + combat.assist_v
	elif on_floor or state == State.SWIM:
		var want := in_dir * speed
		h = h.move_toward(want, (ACCEL if want.length() > h.length() else DECEL) * delta)
	else:
		var keep := maxf(speed, h.length())
		h = h.move_toward(in_dir * keep if in_dir.length() > 0.01 else h, ACCEL * AIR_CONTROL * delta)
	velocity.x = h.x
	velocity.z = h.z
	# facing: where we go, or the target / the aim while fighting
	if lock.target != null and not sprinting:
		var to := lock.target.global_position - global_position
		to.y = 0.0
		if to.length() > 0.05:
			facing = to.normalized()
	elif (combat.aiming() or combat.guarding()) and cam != null:
		facing = Vector3(-sin(cam.yaw), 0.0, -cos(cam.yaw))
	elif not combat.busy() and h.length() > 0.4:
		facing = h.normalized()
	# vertical
	if state == State.SWIM:
		velocity.y = move_toward(velocity.y, clampf((water - 1.3 - global_position.y) * 4.0, -2.0, 2.0), 12.0 * delta)
	else:
		velocity.y = maxf(velocity.y - GRAVITY * delta, -TERMINAL)
	if _jump_buf > 0.0 and _coyote > 0.0 and not combat.busy() and state != State.SWIM and stamina >= COST_JUMP * 0.5:
		_jump_buf = 0.0
		_coyote = 0.0
		velocity.y = JUMP_SPEED
		state = State.AIR
		_peak_y = global_position.y
		crouching = false
		_spend_once(COST_JUMP)
		model.play(str(HeroAnim.data["_air"]["jump"]), 1.0, 0.1)
	# dodge
	if intent.just_dodge and (on_floor or state == State.SWIM) and _dodge_cd <= 0.0 and not exhausted \
			and stamina >= COST_DODGE * 0.5 and combat.state != Combat.S.STAGGER:
		_start_dodge(in_dir)
		return
	move_and_slide()


func _on_land() -> void:
	var fall := _peak_y - global_position.y
	if fall > FALL_SAFE:
		var dmg := sheet.v("max_hp") * FALL_DMG_PER_M * (fall - FALL_SAFE)
		sheet.hp = maxf(1.0, sheet.hp - dmg)
		toast.emit("Hard landing: -%d" % int(dmg))
		model.play(str(HeroAnim.data["_air"]["hard_land"]), 1.6, 0.2)
		_land_lock = 0.6
	elif fall > 1.2 and not combat.busy():
		var air: Dictionary = HeroAnim.data["_air"]
		var moving := Vector2(velocity.x, velocity.z).length() > 2.0
		model.play(str(air["land_run"] if moving else air["land"]), 1.4, 0.15)
		if cam != null:
			cam.shake(0.03 * minf(fall, 4.0), Vector2(0, -1))


func _start_dodge(in_dir: Vector3) -> void:
	var dd: Dictionary = HeroAnim.data["_dodge"]
	_spend_once(COST_DODGE)
	combat.reset()
	model.hold("")
	state = State.DODGE
	invuln_t = DODGE_IFRAMES
	_dodge_cd = DODGE_COOLDOWN
	var dir := in_dir.normalized() if in_dir.length() > 0.1 else -facing
	var clip := str(dd["roll"])
	var careful := lock.target != null or combat.guarding() or combat.aiming()
	if careful or in_dir.length() < 0.1:
		# a step that keeps your eyes on the fight
		var fwd := facing.dot(dir)
		var right := facing.cross(Vector3.UP).dot(dir)
		clip = str(dd["f"] if fwd > 0.6 else (dd["b"] if fwd < -0.6 else (dd["r"] if right > 0.0 else dd["l"])))
	else:
		facing = dir
	dodge_t = model.play(clip, 1.6, 0.15) * 0.7
	Sfx.at(self, global_position, "swing", 0.8)


func _tick_dodge(delta: float) -> void:
	dodge_t -= delta
	var rm := model.root_motion_world() / maxf(delta, 0.0001)
	velocity.x = rm.x
	velocity.z = rm.z
	velocity.y = maxf(velocity.y - GRAVITY * delta, -TERMINAL)
	move_and_slide()
	if dodge_t <= 0.0:
		state = State.GROUND if is_on_floor() else State.AIR
		model.stop_action(0.2)


func _tick_dead(delta: float) -> void:
	dead_t += delta
	velocity.x = 0.0
	velocity.z = 0.0
	velocity.y = maxf(velocity.y - GRAVITY * delta, -TERMINAL)
	move_and_slide()
	_update_model(delta)
	if dead_t >= RESPAWN_AFTER:
		if is_ai:
			teleport(respawn_pos)
		else:
			toast.emit("You were defeated. Back at the spawn point.")
			teleport(respawn_pos)


func _update_model(delta: float) -> void:
	var target := atan2(-facing.x, -facing.z)
	model.rotation.y = lerp_angle(model.rotation.y, target, 1.0 - exp(-delta * TURN_SMOOTH))
	var local := Vector3(velocity.x, 0.0, velocity.z).rotated(Vector3.UP, -model.rotation.y)
	var mode := "ground"
	if state == State.AIR and not is_on_floor() and velocity.y < -1.0:
		mode = "air"
	elif crouching:
		mode = "crouch"
	model.set_motion(Vector2(local.x, -local.z), sprinting, mode, delta)
	_sync_auras()


# ---- abilities -------------------------------------------------------------------------------

func _tick_abilities() -> void:
	for i in 4:
		if intent.just_cast[i]:
			var page: Array = PAGE2 if intent.alt_page else PAGE1
			cast_spell(page[i])


## Cast a Boon (hotbar key or the spellbook), or dismiss it if it is already up.
func cast_spell(sid: String) -> void:
	if sheet.find_effect(sid) != null:
		sheet.dismiss(sid)
		toast.emit("Dismissed " + DataStore.spells()[sid]["name"])
		return
	var r := sheet.cast(sid)
	if r[0]:
		var sp: Dictionary = DataStore.spells()[sid]
		if not combat.busy():
			model.play(str(combat.stance.get("art", "SS_power_up")), 1.6, 0.25)
		var ms: Array = []
		for m in sp["mods"]:
			ms.append(CharModel.mod_text(m))
		toast.emit("%s  -  %s" % [sp["name"], ", ".join(ms)])
		Vfx.cast_flare(self, sp["aspect"], sp["family"])
		Sfx.at(self, global_position, "parry", 1.3, -6.0)
	else:
		toast.emit(r[1])


## One aura per active Boon (and food), in its family's colour.
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
			a.position = Vector3.UP * 1.1
			add_child(a)
			_auras[k] = a
