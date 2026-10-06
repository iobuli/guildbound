class_name CameraRig
extends Node3D
## Over-the-shoulder orbit camera. Child of the player so physics interpolation keeps it smooth.

var yaw := 0.0
var pitch := -0.30
var sens := 0.0025
var zoom := 4.5
var height := 1.55
var view_offset := 0.0  # look-only yaw (screenshots); movement still uses `yaw`
var lock_target: Node3D = null  # locked on: the view frames you and this fighter
var wheel_steps := 0            # wheel turns while locked (cycle targets), read by Intent.from_input
var _nudge := 0.0               # mouse look while locked: a nudge that springs back
var flick := Vector2.ZERO       # recent mouse travel in pixels (decays in ~0.15 s): picks strike direction
var _shake := 0.0
const LOCK_EASE := 8.0
const LOCK_OFFSET := 0.32       # you to one side, the target beside you in view (not hidden behind you)
var target: Node3D
var _y := NAN
var arm: SpringArm3D
var camera: Camera3D


func setup(p_target: Node3D, player_rid: RID) -> void:
	# Mouse look must update every rendered frame, so the rig is not physics-interpolated;
	# it follows the player's *interpolated* position instead.
	target = p_target
	top_level = true
	physics_interpolation_mode = Node.PHYSICS_INTERPOLATION_MODE_OFF
	arm = SpringArm3D.new()
	var s := SphereShape3D.new()
	s.radius = 0.25
	arm.shape = s
	arm.collision_mask = 1
	arm.spring_length = zoom
	arm.add_excluded_object(player_rid)
	add_child(arm)
	camera = Camera3D.new()
	camera.fov = 72.0
	arm.add_child(camera)
	camera.make_current()


func _process(_delta: float) -> void:
	# horizontal follows exactly; height eases (steps, landings, crouch) so the view never pops
	var o := target.get_global_transform_interpolated().origin
	var step: float = target.step_offset if "step_offset" in target else 0.0
	var want: float = o.y + height + step
	_y = want if is_nan(_y) or absf(want - _y) > 4.0 else lerpf(_y, want, 1.0 - exp(-_delta * 14.0))
	global_position = Vector3(o.x, _y, o.z)
	if lock_target != null and is_instance_valid(lock_target):
		var to := lock_target.global_position - o
		to.y = 0.0
		if to.length() > 0.3:
			var want_yaw := atan2(-to.x, -to.z) + LOCK_OFFSET
			var k := 1.0 - exp(-_delta * LOCK_EASE)
			yaw += wrapf(want_yaw + _nudge - yaw, -PI, PI) * k
			pitch = lerpf(pitch, -0.3, k)
			zoom = lerpf(zoom, clampf(4.0 + 0.3 * to.length(), 4.0, 8.0), k)
		_nudge = lerpf(_nudge, 0.0, 1.0 - exp(-_delta * 2.5))
	rotation = Vector3(0.0, yaw + view_offset, 0.0)
	flick *= exp(-_delta * 7.0)
	if _shake > 0.001:
		var tt := Time.get_ticks_msec() * 0.001
		camera.h_offset = sin(tt * 61.0) * _shake * 0.5
		camera.v_offset = cos(tt * 47.0) * _shake * 0.5
		_shake *= exp(-_delta * 12.0)
	else:
		camera.h_offset = 0.0
		camera.v_offset = 0.0
	arm.rotation = Vector3(pitch, 0.0, 0.0)
	arm.spring_length = zoom


func _unhandled_input(event: InputEvent) -> void:
	var locked := lock_target != null and is_instance_valid(lock_target)
	if event is InputEventMouseMotion and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		flick += event.relative
		if locked:
			_nudge = clampf(_nudge - event.relative.x * sens, -0.52, 0.52)
		else:
			yaw -= event.relative.x * sens
		pitch = clampf(pitch - event.relative.y * sens, -1.2, 0.5)
	elif event is InputEventMouseButton and event.pressed and locked:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP:
			wheel_steps += 1
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			wheel_steps -= 1
	elif event is InputEventMouseButton and event.pressed:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP:
			zoom = clampf(zoom - 0.4, 2.0, 9.0)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			zoom = clampf(zoom + 0.4, 2.0, 9.0)


## Kick the view (impacts): `amount` metres of jitter, gone in a fraction of a second.
func shake(amount: float) -> void:
	_shake = maxf(_shake, amount)
