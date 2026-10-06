class_name Intent
extends RefCounted
## What a fighter wants to do this physics frame. The Player reads only this, never `Input`, so the
## same body can be driven by the local keyboard (from_input), an AI brain, or later the network.

var move := Vector2.ZERO     # x right, y forward, -1..1
var view_yaw := 0.0          # the yaw that `move` is relative to (camera yaw for the local player)
var flick := Vector2.ZERO    # recent mouse travel (screen pixels, y down): picks the strike's direction

# held
var sprint := false
var crouch := false
var guard := false
var attack := false
var jump := false            # variable jump height, swim up
var interact := false        # carrying is hold-to-carry
var alt_page := false        # Ctrl: casts use the second hotbar page

# one frame
var just_jump := false
var just_dodge := false
var just_interact := false
var just_attack := false
var just_attack_released := false
var just_guard := false
var just_lock := false
var just_lock_next := false
var just_lock_prev := false
var just_ragdoll := false
var just_crouch := false
var just_cast := [false, false, false, false]  # cast_1..4


## The local player: everything from the named input actions, movement relative to the camera.
static func from_input(cam: CameraRig) -> Intent:
	var i := Intent.new()
	var v := Input.get_vector("move_left", "move_right", "move_forward", "move_back")
	i.move = Vector2(v.x, -v.y)
	i.view_yaw = cam.yaw if cam != null else 0.0
	i.flick = cam.flick if cam != null else Vector2.ZERO
	i.sprint = Input.is_action_pressed("sprint")
	i.crouch = Input.is_action_pressed("crouch")
	i.guard = Input.is_action_pressed("guard")
	i.attack = Input.is_action_pressed("attack")
	i.jump = Input.is_action_pressed("jump")
	i.interact = Input.is_action_pressed("interact")
	i.alt_page = Input.is_key_pressed(KEY_CTRL)
	i.just_jump = Input.is_action_just_pressed("jump")
	i.just_dodge = Input.is_action_just_pressed("dodge")
	i.just_interact = Input.is_action_just_pressed("interact")
	i.just_attack = Input.is_action_just_pressed("attack")
	i.just_attack_released = Input.is_action_just_released("attack")
	i.just_guard = Input.is_action_just_pressed("guard")
	i.just_lock = Input.is_action_just_pressed("lock")
	i.just_lock_next = Input.is_action_just_pressed("lock_next")
	i.just_lock_prev = Input.is_action_just_pressed("lock_prev")
	if cam != null and cam.wheel_steps != 0:  # a wheel click presses and releases in one frame: latched by the camera
		i.just_lock_next = cam.wheel_steps > 0
		i.just_lock_prev = cam.wheel_steps < 0
		cam.wheel_steps = 0
	i.just_ragdoll = Input.is_action_just_pressed("ragdoll")
	i.just_crouch = Input.is_action_just_pressed("crouch")
	for k in 4:
		i.just_cast[k] = Input.is_action_just_pressed("cast_%d" % (k + 1))
	return i


## World-space movement direction on the ground plane (length 0..1).
func world_move() -> Vector3:
	return Basis(Vector3.UP, view_yaw) * Vector3(move.x, 0.0, -move.y)


## After a tick has consumed them (intent supplied from outside), so a press acts once.
func clear_once() -> void:
	just_jump = false
	just_dodge = false
	just_interact = false
	just_attack = false
	just_attack_released = false
	just_guard = false
	just_lock = false
	just_lock_next = false
	just_lock_prev = false
	just_ragdoll = false
	just_crouch = false
	just_cast = [false, false, false, false]
