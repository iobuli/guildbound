class_name CameraRig
extends Node3D
## Third-person camera: over the shoulder, smooth.
##   follow     the pivot chases the character with a short critically-damped lag (no jitter on steps)
##   look       mouse yaw / pitch, lightly smoothed; recentres behind you after a while of running
##   speed      field of view widens and the camera eases back as you run and sprint
##   aim        (bow, spells) closer, further over the shoulder, narrower view
##   lock-on    the view turns to keep you and the target in frame, the mouse nudges it
##   collision  a sphere cast keeps the camera out of walls, easing back out instead of snapping
## Everything runs per RENDERED frame (_process) and follows the body's interpolated transform, so on a
## 144 Hz screen the camera moves every frame with the body instead of in 60 Hz physics steps. The rig is
## opted out of physics interpolation: it is placed directly each frame.

var yaw := 0.0
var pitch := -0.25
var sens := 0.0025
var zoom := 4.2
var height := 1.55
var view_offset := 0.0           # look-only yaw (screenshots); movement still uses `yaw`
var lock_target: Node3D = null
var aiming := false
var speed := 0.0                 # the character's ground speed (set by the player)
var wheel_steps := 0             # wheel turns while locked (cycle targets), read by Intent.from_input
var flick := Vector2.ZERO        # recent mouse travel (pixels, decays ~0.15 s)
const SHOULDER := 0.45
const AIM_SHOULDER := 0.62
const AIM_ZOOM := 2.0
const BASE_FOV := 68.0
const LOCK_EASE := 6.0
const MARGIN := 0.2
var target: Node3D
var arm: Node3D                  # the boom: shoulder offset; the camera sits `_dist` behind it
var camera: Camera3D
var _probe := SphereShape3D.new()
var _probe_q := PhysicsShapeQueryParameters3D.new()
var _dist := 4.2                 # current boom length after collision
var _pivot := Vector3.ZERO
var _pivot_v := Vector3.ZERO
var _yaw_s := 0.0
var _pitch_s := -0.25
var _shoulder := SHOULDER
var _len := 4.2
var _nudge := 0.0
var _shake := Vector3.ZERO
var _idle_t := 0.0


func setup(p_target: Node3D, player_rid: RID) -> void:
	target = p_target
	top_level = true
	physics_interpolation_mode = Node.PHYSICS_INTERPOLATION_MODE_OFF
	arm = Node3D.new()
	add_child(arm)
	_probe.radius = 0.25
	_probe_q.shape = _probe
	_probe_q.exclude = [player_rid]
	_probe_q.collision_mask = Layers.WORLD | Layers.PROPS
	camera = Camera3D.new()
	camera.fov = BASE_FOV
	camera.current = true
	arm.add_child(camera)
	_pivot = _target_pos() + Vector3.UP * height
	yaw = target.global_rotation.y
	_yaw_s = yaw


## Where the body is drawn this frame (between physics ticks), not where physics last left it.
func _target_pos() -> Vector3:
	return target.get_global_transform_interpolated().origin if target.is_inside_tree() else target.global_position


func _process(delta: float) -> void:
	if target == null:
		return
	# follow: a critically damped spring on the pivot (fast, never overshoots)
	var tpos := _target_pos()
	var want := tpos + Vector3.UP * height
	var k := 18.0
	var acc := (want - _pivot) * k * k - _pivot_v * 2.0 * k
	_pivot_v += acc * delta
	_pivot += _pivot_v * delta
	if _pivot.distance_to(want) > 4.0:  # teleport
		_pivot = want
		_pivot_v = Vector3.ZERO
	# lock-on: turn to keep the target in view, slightly to the side
	if lock_target != null and is_instance_valid(lock_target):
		var to := lock_target.get_global_transform_interpolated().origin - tpos
		to.y = 0.0
		if to.length() > 0.3:
			var ty := atan2(-to.x, -to.z) + _nudge
			yaw = lerp_angle(yaw, ty, 1.0 - exp(-delta * LOCK_EASE))
			var tp := clampf(-0.18 - 0.02 * to.length(), -0.45, -0.12)
			pitch = lerpf(pitch, tp, 1.0 - exp(-delta * 3.0))
		_nudge = lerpf(_nudge, 0.0, 1.0 - exp(-delta * 2.0))
	elif speed > 3.0 and not aiming:
		# running: after a moment without mouse input the view drifts back behind you
		_idle_t += delta
		if _idle_t > 1.2:
			var mdl := target.get("model") as Node3D
			var behind: float = mdl.get_global_transform_interpolated().basis.get_euler().y if mdl != null else yaw
			yaw = lerp_angle(yaw, behind, 1.0 - exp(-delta * 0.8))
	_yaw_s = lerp_angle(_yaw_s, yaw, 1.0 - exp(-delta * 25.0))
	_pitch_s = lerpf(_pitch_s, pitch, 1.0 - exp(-delta * 25.0))
	# distance, shoulder and field of view from what you are doing
	var run_f := clampf((speed - 2.0) / 6.0, 0.0, 1.0)
	var want_len := (AIM_ZOOM if aiming else zoom + 0.6 * run_f)
	_len = lerpf(_len, want_len, 1.0 - exp(-delta * 6.0))
	_shoulder = lerpf(_shoulder, AIM_SHOULDER if aiming else SHOULDER, 1.0 - exp(-delta * 8.0))
	camera.fov = lerpf(camera.fov, (52.0 if aiming else BASE_FOV + 10.0 * run_f), 1.0 - exp(-delta * 5.0))
	global_position = _pivot
	global_rotation = Vector3(_pitch_s, _yaw_s + view_offset, 0.0)
	arm.position = Vector3(_shoulder, 0.0, 0.0)
	# collision: sweep the probe from the shoulder back to the wanted distance; pull in at once when
	# something is in the way, ease back out when it clears
	var reach := _len
	var from := arm.global_transform
	_probe_q.transform = from
	_probe_q.motion = from.basis.z.normalized() * _len
	var hit := get_world_3d().direct_space_state.cast_motion(_probe_q)
	if hit.size() == 2 and hit[0] < 1.0:
		reach = maxf(0.3, _len * hit[0] - MARGIN)
	_dist = reach if reach < _dist else lerpf(_dist, reach, 1.0 - exp(-delta * 5.0))
	camera.position = Vector3(0.0, 0.0, _dist)
	# shake: a decaying kick in the direction of the hit
	camera.h_offset = _shake.x
	camera.v_offset = _shake.y
	_shake = _shake.lerp(Vector3.ZERO, 1.0 - exp(-delta * 14.0))
	flick = flick.lerp(Vector2.ZERO, 1.0 - exp(-delta * 7.0))


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		var rel: Vector2 = event.relative
		flick += rel
		_idle_t = 0.0
		var s := sens * (0.6 if aiming else 1.0)
		if lock_target != null:
			_nudge = clampf(_nudge - rel.x * s, -0.6, 0.6)
		else:
			yaw -= rel.x * s
		pitch = clampf(pitch - rel.y * s, -1.2, 0.6)
	elif event is InputEventMouseButton and event.pressed:
		if lock_target != null and (event.button_index == MOUSE_BUTTON_WHEEL_UP or event.button_index == MOUSE_BUTTON_WHEEL_DOWN):
			wheel_steps += 1 if event.button_index == MOUSE_BUTTON_WHEEL_UP else -1
		elif event.button_index == MOUSE_BUTTON_WHEEL_UP:
			zoom = clampf(zoom - 0.4, 2.2, 8.0)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			zoom = clampf(zoom + 0.4, 2.2, 8.0)


## A camera kick: `amount` in metres, along `dir` (screen-ish: x right, y up); random when ZERO.
func shake(amount: float, dir: Vector2 = Vector2.ZERO) -> void:
	var d := dir if dir != Vector2.ZERO else Vector2(randf_range(-1, 1), randf_range(-1, 1)).normalized()
	_shake += Vector3(d.x, d.y, 0.0) * amount
