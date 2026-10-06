class_name Vfx
extends RefCounted
## Gameplay cues: the flare that makes hits, perfects, parries and buffs readable. All one-shot nodes
## free themselves. CPU particles (work headless and on every renderer).

const LIGHT := Color(1.0, 0.88, 0.45)
const DARK := Color(0.62, 0.3, 1.0)
const FAMILY := {"swiftness": Color(0.4, 1.0, 0.8), "might": Color(1.0, 0.45, 0.25), "ward": Color(0.45, 0.65, 1.0),
	"vigor": Color(0.5, 1.0, 0.45), "food": Color(1.0, 0.8, 0.5)}


static func _mat(col: Color, glow: float = 2.0) -> StandardMaterial3D:
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.albedo_color = col
	m.emission_enabled = true
	m.emission = col * glow
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	m.vertex_color_use_as_albedo = true
	m.billboard_mode = BaseMaterial3D.BILLBOARD_PARTICLES
	return m


static func _free_after(n: Node, secs: float) -> void:
	n.get_tree().create_timer(secs, false).timeout.connect(n.queue_free)


## A burst of sparks where a blow lands (`normal` = the direction they fly).
static func spark(host: Node, pos: Vector3, normal: Vector3, col: Color, amount: int = 12) -> void:
	if host == null or not host.is_inside_tree():
		return
	var p := CPUParticles3D.new()
	p.top_level = true
	host.add_child(p)
	p.global_position = pos
	p.one_shot = true
	p.explosiveness = 1.0
	p.amount = amount
	p.lifetime = 0.35
	var q := QuadMesh.new()
	q.size = Vector2(0.05, 0.05)
	q.material = _mat(col, 3.0)
	p.mesh = q
	p.direction = normal if normal.length() > 0.01 else Vector3.UP
	p.spread = 55.0
	p.initial_velocity_min = 2.5
	p.initial_velocity_max = 6.0
	p.gravity = Vector3(0, -9.0, 0)
	p.scale_amount_min = 0.5
	p.scale_amount_max = 1.4
	var g := Gradient.new()
	g.set_color(0, col)
	g.set_color(1, Color(col.r, col.g, col.b, 0.0))
	p.color_ramp = g
	p.emitting = true
	_free_after(p, 1.0)


## An expanding ring (perfect strikes, parries, Flurry, casts).
static func ring(host: Node, pos: Vector3, col: Color, radius: float) -> void:
	if host == null or not host.is_inside_tree():
		return
	var m := MeshInstance3D.new()
	var t := TorusMesh.new()
	t.inner_radius = 0.9
	t.outer_radius = 1.0
	t.rings = 24
	t.ring_segments = 6
	var mat := _mat(col, 3.0)
	mat.billboard_mode = BaseMaterial3D.BILLBOARD_DISABLED
	t.material = mat
	m.mesh = t
	m.top_level = true
	host.add_child(m)
	m.global_position = pos
	m.scale = Vector3.ONE * radius * 0.3
	var tw := m.create_tween()
	tw.tween_property(m, "scale", Vector3.ONE * radius, 0.25).set_ease(Tween.EASE_OUT).set_trans(Tween.TRANS_CUBIC)
	tw.parallel().tween_property(mat, "albedo_color:a", 0.0, 0.3)
	tw.tween_callback(m.queue_free)


## A number that pops up and floats away.
static func number(host: Node, pos: Vector3, text: String, col: Color) -> void:
	if host == null or not host.is_inside_tree():
		return
	var l := Label3D.new()
	l.text = text
	l.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	l.no_depth_test = true
	l.font_size = 64
	l.outline_size = 12
	l.modulate = col
	l.pixel_size = 0.004
	l.top_level = true
	host.add_child(l)
	l.global_position = pos
	var tw := l.create_tween()
	tw.tween_property(l, "global_position", pos + Vector3(randf_range(-0.2, 0.2), 0.7, 0.0), 0.7).set_ease(Tween.EASE_OUT)
	tw.parallel().tween_property(l, "modulate:a", 0.0, 0.7).set_delay(0.25)
	tw.tween_callback(l.queue_free)


## A streak following a body (a fist or foot mid-strike) for `secs`.
static func trail(body: Node3D, col: Color, secs: float) -> void:
	if body == null or not body.is_inside_tree():
		return
	var p := CPUParticles3D.new()
	body.add_child(p)
	p.local_coords = false
	p.amount = 24
	p.lifetime = 0.18
	var q := QuadMesh.new()
	q.size = Vector2(0.07, 0.07)
	q.material = _mat(col, 2.0)
	p.mesh = q
	p.initial_velocity_min = 0.0
	p.initial_velocity_max = 0.1
	p.gravity = Vector3.ZERO
	var g := Gradient.new()
	g.set_color(0, Color(col.r, col.g, col.b, 0.8))
	g.set_color(1, Color(col.r, col.g, col.b, 0.0))
	p.color_ramp = g
	p.emitting = true
	body.get_tree().create_timer(secs, false).timeout.connect(func() -> void:
		if is_instance_valid(p):
			p.emitting = false
			_free_after(p, 0.3))


## Casting a Boon: a flash and a column of motes in the spell's colours.
static func cast_flare(host: Node3D, aspect: String, family: String) -> void:
	var col: Color = FAMILY.get(family, Color.WHITE)
	var edge := LIGHT if aspect == "light" else (DARK if aspect == "dark" else col)
	ring(host, host.global_position + Vector3.UP * 0.1, edge, 1.4)
	ring(host, host.global_position + Vector3.UP * 1.0, col, 0.9)
	var p := CPUParticles3D.new()
	host.add_child(p)
	p.position = Vector3.UP * 0.2
	p.one_shot = true
	p.explosiveness = 0.6
	p.amount = 40
	p.lifetime = 0.9
	var q := QuadMesh.new()
	q.size = Vector2(0.06, 0.06)
	q.material = _mat(edge, 3.0)
	p.mesh = q
	p.emission_shape = CPUParticles3D.EMISSION_SHAPE_RING
	p.emission_ring_axis = Vector3.UP
	p.emission_ring_radius = 0.6
	p.emission_ring_inner_radius = 0.4
	p.emission_ring_height = 0.0
	p.direction = Vector3.UP
	p.spread = 10.0
	p.initial_velocity_min = 1.5
	p.initial_velocity_max = 3.0
	p.gravity = Vector3.ZERO
	var g := Gradient.new()
	g.set_color(0, edge)
	g.set_color(1, Color(col.r, col.g, col.b, 0.0))
	p.color_ramp = g
	p.emitting = true
	_free_after(p, 1.6)


## A lasting aura while a Boon is up (motes around the body; dark ones are slower and smoky).
static func aura(aspect: String, family: String) -> CPUParticles3D:
	var col: Color = FAMILY.get(family, Color.WHITE)
	if aspect == "dark":
		col = col.lerp(DARK, 0.6)
	elif aspect == "light":
		col = col.lerp(LIGHT, 0.4)
	var p := CPUParticles3D.new()
	p.local_coords = false
	p.amount = 18 if aspect != "dark" else 26
	p.lifetime = 1.1
	var q := QuadMesh.new()
	q.size = Vector2(0.05, 0.05) if aspect != "dark" else Vector2(0.12, 0.12)
	q.material = _mat(col, 2.0 if aspect != "dark" else 1.0)
	p.mesh = q
	p.emission_shape = CPUParticles3D.EMISSION_SHAPE_SPHERE
	p.emission_sphere_radius = 0.45
	p.direction = Vector3.UP
	p.spread = 25.0
	p.initial_velocity_min = 0.2
	p.initial_velocity_max = 0.6
	p.gravity = Vector3(0, 0.4 if aspect != "dark" else -0.2, 0)
	var g := Gradient.new()
	g.set_color(0, Color(col.r, col.g, col.b, 0.0))
	g.add_point(0.3, Color(col.r, col.g, col.b, 0.85))
	g.set_color(g.get_point_count() - 1, Color(col.r, col.g, col.b, 0.0))
	p.color_ramp = g
	p.emitting = true
	return p
