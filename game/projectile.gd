class_name Projectile
extends Node3D
## Arrows and spell bolts. Flies with gravity (arrows) or homes in a little (spells), is swept against
## the world (arrows stick in walls) and against every other fighter's body each frame; a fire orb bursts.

var shooter: Player
var kind := "arrow"
var vel := Vector3.ZERO
var gravity := 0.0
var dmg := 1.0
var poise := 10.0
var radius := 0.06
var color := Color(1, 1, 1)
var homing := 0.0
var homing_target: Player = null
var blast := 0.0
var life := 4.0
var _stuck := false
var _mesh: MeshInstance3D


static func make(from: Player, p_kind: String, at: Vector3, v: Vector3, g: float, p_dmg: float, p_poise: float, r: float) -> Projectile:
	var pr := Projectile.new()
	pr.shooter = from
	pr.kind = p_kind
	pr.vel = v
	pr.gravity = g
	pr.dmg = p_dmg
	pr.poise = p_poise
	pr.radius = r
	pr.position = at
	return pr


func _ready() -> void:
	_mesh = MeshInstance3D.new()
	var mat := StandardMaterial3D.new()
	if kind == "arrow":
		var cm := CylinderMesh.new()
		cm.top_radius = 0.008
		cm.bottom_radius = 0.008
		cm.height = 0.75
		mat.albedo_color = Color(0.55, 0.4, 0.25)
		cm.material = mat
		_mesh.mesh = cm
		_mesh.rotation.x = PI / 2.0
	else:
		var sm := SphereMesh.new()
		sm.radius = radius
		sm.height = radius * 2.0
		mat.albedo_color = color
		mat.emission_enabled = true
		mat.emission = color
		mat.emission_energy_multiplier = 3.0
		sm.material = mat
		_mesh.mesh = sm
		var trail := CPUParticles3D.new()
		trail.amount = 40
		trail.lifetime = 0.35
		trail.local_coords = false
		trail.emission_shape = CPUParticles3D.EMISSION_SHAPE_SPHERE
		trail.emission_sphere_radius = radius * 0.6
		trail.gravity = Vector3.ZERO
		trail.scale_amount_min = 0.5
		trail.scale_amount_max = 1.0
		var pm := SphereMesh.new()
		pm.radius = radius * 0.35
		pm.height = radius * 0.7
		var pmat := StandardMaterial3D.new()
		pmat.albedo_color = color
		pmat.emission_enabled = true
		pmat.emission = color
		pmat.emission_energy_multiplier = 2.0
		pmat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
		pm.material = pmat
		trail.mesh = pm
		add_child(trail)
		var light := OmniLight3D.new()
		light.light_color = color
		light.omni_range = 3.0
		light.light_energy = 1.5
		add_child(light)
	add_child(_mesh)
	_orient()


func _orient() -> void:
	if vel.length() > 0.1:
		look_at(global_position + vel, Vector3.UP if absf(vel.normalized().y) < 0.99 else Vector3.FORWARD)


func _physics_process(delta: float) -> void:
	life -= delta
	if life <= 0.0:
		queue_free()
		return
	if _stuck:
		return
	if homing > 0.0 and homing_target != null and is_instance_valid(homing_target) and LockOn.alive(homing_target):
		var want := (homing_target.model.chest_position() - global_position).normalized() * vel.length()
		vel = vel.move_toward(want, homing * vel.length() * delta)
	vel.y -= gravity * delta
	var from := global_position
	var to := from + vel * delta
	# fighters first (the body is the target), then the world
	for f in Player.fighters:
		if f == shooter or not LockOn.alive(f):
			continue
		var pts := Geometry3D.get_closest_points_between_segments(from, to, f.global_position + Vector3.UP * 0.35, f.global_position + Vector3.UP * 1.55)
		if (pts[0] as Vector3).distance_to(pts[1]) <= Combat.BODY_RADIUS + radius:
			_impact(pts[0], f)
			return
	var q := PhysicsRayQueryParameters3D.create(from, to, Layers.WORLD | Layers.PROPS)
	var hit := get_world_3d().direct_space_state.intersect_ray(q)
	if not hit.is_empty():
		global_position = hit["position"]
		_impact(hit["position"], null)
		return
	global_position = to
	_orient()


func _impact(at: Vector3, victim: Player) -> void:
	if victim != null and is_instance_valid(shooter):
		shooter.combat.apply_hit(victim, dmg, poise, at, false, kind != "arrow")
	if blast > 0.0 and is_instance_valid(shooter):
		Vfx.ring(shooter, at, color, blast * 0.5)
		for f in Player.fighters:
			if f != shooter and f != victim and LockOn.alive(f) and f.global_position.distance_to(at) <= blast:
				shooter.combat.apply_hit(f, dmg * 0.6, poise * 0.6, f.model.chest_position(), false, true)
	if kind == "arrow" and victim == null:
		_stuck = true  # it stays in the wall for a moment
		life = 3.0
		Sfx.at(self, at, "block", 1.5, -6.0)
		return
	if kind != "arrow" and is_instance_valid(shooter):
		Vfx.spark(shooter, at, -vel.normalized(), color, 24)
	queue_free()
