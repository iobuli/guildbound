class_name Ragdoll
extends Node3D
## A beaten monster's last moments: its animated mannequin is hidden and every BodyParts segment becomes
## a rigid capsule where it stood, joined to its parent piece (spine and neck stiff, shoulders and hips
## loose, knees and elbows hinges). The pieces keep the body's velocity plus a shove along the killing
## blow, fall and tumble for HOLD s, then burn away (a noise dissolve with a glowing edge, embers
## rising) over DISSOLVE s. `finished` reports where the pelvis ended up: the loot lands there.
## Slimes get a cheaper ending: burst_slime() squashes the blob, sprays goo and shrinks it away.

signal finished(at: Vector3)

const MAX_LIVE := 8          # more bodies than this: the oldest start burning early
const HOLD := 1.8            # s of tumbling before the dissolve
const DISSOLVE := 1.2        # s the dissolve takes
const DENSITY := 985.0       # kg/m3 (mass by capsule volume)
const EMBER := Color(1.0, 0.55, 0.18)
## child segment -> the segment it hangs from
const PARENT := {"abdomen": "pelvis", "chest": "abdomen", "neck": "chest", "head": "neck",
	"upperarm_l": "chest", "forearm_l": "upperarm_l", "hand_l": "forearm_l",
	"upperarm_r": "chest", "forearm_r": "upperarm_r", "hand_r": "forearm_r",
	"thigh_l": "pelvis", "shin_l": "thigh_l", "foot_l": "shin_l",
	"thigh_r": "pelvis", "shin_r": "thigh_r", "foot_r": "shin_r"}
## cone-twist joints: segment stem -> [swing span, twist span] (radians)
const CONE := {"abdomen": [0.3, 0.25], "chest": [0.35, 0.3], "neck": [0.45, 0.5], "head": [0.4, 0.5],
	"upperarm": [1.3, 0.8], "hand": [0.8, 0.4], "thigh": [1.0, 0.45], "foot": [0.5, 0.2]}
## how much of the shove each part takes (the blow lands on the trunk; the limbs trail)
const SHARE := {"head": 1.0, "neck": 1.0, "chest": 1.0, "abdomen": 0.9, "pelvis": 0.7, "upperarm": 0.6,
	"forearm": 0.5, "hand": 0.45, "thigh": 0.4, "shin": 0.3, "foot": 0.25}

static var live: Array[Ragdoll] = []
static var _dmat: ShaderMaterial

var bodies: Array[RigidBody3D] = []
var pieces := {}             # segment name -> its RigidBody3D
var root_body: RigidBody3D   # the pelvis
var t := 0.0
var dissolving := false
var _dt := 0.0
var _done := false
var _meshes: Array[GeometryInstance3D] = []
var _fx: Array[CPUParticles3D] = []


## Ragdoll a mannequin fighter where it stands (and hide it). Null if it has no mannequin to copy.
static func spawn(victim: Player) -> Ragdoll:
	var mdl := victim.model
	if mdl == null or mdl.skeleton == null or mdl.parts.is_empty() or not victim.is_inside_tree():
		return null
	var rd := Ragdoll.new()
	victim.get_parent().add_child(rd)
	rd._build(victim)
	if rd.root_body == null:
		rd.queue_free()
		return null
	mdl.visible = false
	victim.collision_layer = 0  # no invisible corpse to bump into (teleport() restores it)
	live.append(rd)
	for i in maxi(0, live.size() - MAX_LIVE):
		live[i]._begin_dissolve()
	return rd


func _exit_tree() -> void:
	live.erase(self)


func _build(v: Player) -> void:
	var mdl := v.model
	var sk := mdl.skeleton
	var fwd := sk.global_basis.z.normalized()
	var left := sk.global_basis.x.normalized()
	var dir := v.death_dir
	if v.last_hit_by != null and is_instance_valid(v.last_hit_by) and v.last_hit_by != v:
		dir = v.global_position - v.last_hit_by.global_position
	dir.y = 0.0
	dir = dir.normalized() if dir.length() > 0.01 else -fwd
	var speed := clampf(2.2 + v.death_force * 0.06, 2.2, 9.0)  # heavy blows throw harder
	var skip := {}
	var by_attach := {}
	for w: Node3D in [mdl._weapon, mdl._offhand]:
		if w != null and is_instance_valid(w) and w.is_visible_in_tree():
			_weapon_body(w, skip, v.death_vel + dir * speed * 0.6 + Vector3.UP * 2.0)
	for key in mdl.parts.keys():
		var n := str(key)
		var mi: MeshInstance3D = mdl.parts[key]
		if not is_instance_valid(mi) or not (mi.mesh is CapsuleMesh):
			continue
		skip[mi] = true
		var cm := mi.mesh as CapsuleMesh
		var gt := mi.global_transform
		var s := gt.basis.get_scale()
		var r := cm.radius * s.x
		var h := maxf(cm.height * s.y, 2.0 * r + 0.002)
		var b := _new_body(Transform3D(gt.basis.orthonormalized(), gt.origin))
		var vol := PI * r * r * (h - 2.0 * r) + 4.0 / 3.0 * PI * r * r * r
		b.mass = clampf(vol * DENSITY, 0.6, 14.0)
		var sh := CapsuleShape3D.new()
		sh.radius = r
		sh.height = h
		var cs := CollisionShape3D.new()
		cs.shape = sh
		b.add_child(cs)
		var m := MeshInstance3D.new()
		m.mesh = cm
		m.scale = s
		b.add_child(m)
		_meshes.append(m)
		var share := float(SHARE.get(n.trim_suffix("_l").trim_suffix("_r"), 0.5))
		b.linear_velocity = v.death_vel + dir * speed * share + Vector3.UP * (1.0 + speed * 0.2) * share
		b.angular_velocity = dir.cross(Vector3.UP) * -speed * 0.6 * share  # tips over away from the blow
		pieces[n] = b
		by_attach[mi.get_parent()] = b
	root_body = pieces.get("pelvis")
	if root_body == null:
		return
	# joint spheres, the visor, armour shells: ride the piece on the same bone (else the nearest)
	for node in sk.find_children("*", "MeshInstance3D", true, false):
		var mi := node as MeshInstance3D
		if skip.has(mi) or mi.skin != null or not mi.is_visible_in_tree() or mi.mesh == null:
			continue
		var host: RigidBody3D = by_attach.get(mi.get_parent())
		if host == null:
			var best := INF
			for b: RigidBody3D in pieces.values():
				var d := b.global_position.distance_squared_to(mi.global_position)
				if d < best:
					best = d
					host = b
		var c := MeshInstance3D.new()
		c.mesh = mi.mesh
		c.material_override = mi.material_override
		host.add_child(c)
		c.transform = host.global_transform.affine_inverse() * mi.global_transform
		_meshes.append(c)
	for key in PARENT.keys():
		var n := str(key)
		var pn := str(PARENT[key])
		if pieces.has(n) and pieces.has(pn):
			_joint(sk, n, pn, left)
	for b in bodies:
		b.reset_physics_interpolation()


func _new_body(xf: Transform3D) -> RigidBody3D:
	var b := RigidBody3D.new()
	b.collision_layer = Layers.DEBRIS
	b.collision_mask = Layers.WORLD | Layers.PROPS  # the world only: never fighters or projectiles
	b.linear_damp = 0.15
	b.angular_damp = 1.2
	b.continuous_cd = true
	var pm := PhysicsMaterial.new()
	pm.friction = 0.9
	pm.bounce = 0.05
	b.physics_material_override = pm
	add_child(b)
	b.global_transform = xf
	bodies.append(b)
	return b


## A dropped weapon: its model copied onto a body boxed to its bounds.
func _weapon_body(w: Node3D, skip: Dictionary, vel: Vector3) -> void:
	var gt := w.global_transform
	var inv := gt.affine_inverse()
	var bb := AABB()
	var first := true
	var vis: Array = w.find_children("*", "VisualInstance3D", true, false)
	if w is VisualInstance3D:
		vis.append(w)
	for node in vis:
		var vi := node as VisualInstance3D
		skip[vi] = true
		if not vi.is_visible_in_tree():
			continue
		var box := (inv * vi.global_transform) * vi.get_aabb()
		bb = box if first else bb.merge(box)
		first = false
	var s := gt.basis.get_scale()
	if first or (bb.size * s).length() < 0.08:
		return
	var b := _new_body(Transform3D(gt.basis.orthonormalized(), gt.origin))
	b.mass = 1.2
	var copy := w.duplicate() as Node3D
	b.add_child(copy)
	copy.transform = b.global_transform.affine_inverse() * gt
	for p in copy.find_children("*", "GPUParticles3D", true, false) + copy.find_children("*", "CPUParticles3D", true, false):
		p.queue_free()
	for node in copy.find_children("*", "MeshInstance3D", true, false):
		_meshes.append(node as MeshInstance3D)
	if copy is MeshInstance3D:
		_meshes.append(copy as MeshInstance3D)
	var sh := BoxShape3D.new()
	sh.size = (bb.size * s).max(Vector3.ONE * 0.03)
	var cs := CollisionShape3D.new()
	cs.shape = sh
	cs.position = bb.get_center() * s
	b.add_child(cs)
	b.linear_velocity = vel
	b.angular_velocity = Vector3(randf_range(-6, 6), randf_range(-3, 3), randf_range(-6, 6))


## Join segment `n` to its parent `pn` at the joint between them, as posed now.
func _joint(sk: Skeleton3D, n: String, pn: String, left: Vector3) -> void:
	var e := BodyParts.endpoints(sk, n)
	var pe := BodyParts.endpoints(sk, pn)
	if e.size() < 2 or pe.size() < 2:
		return
	var a: Vector3 = e[0]
	var axis: Vector3 = (e[1] as Vector3) - a
	if n == "chest" or n == "head":  # the chest capsule lies across the body; the head sits on the neck
		a = pe[1]
		axis = (pe[1] as Vector3) - (pe[0] as Vector3)
	axis = axis.normalized() if axis.length() > 0.001 else Vector3.UP
	var stem := n.trim_suffix("_l").trim_suffix("_r")
	var j: Joint3D
	if stem == "shin" or stem == "forearm":
		# a hinge about the bend, 0 = the pose now; positive bends further (knees back, elbows in)
		var up_d := ((pe[1] as Vector3) - (pe[0] as Vector3)).normalized()
		var hinge := up_d.cross(axis)
		var bend := up_d.angle_to(axis)
		if hinge.length() < 0.08:
			hinge = left if stem == "shin" else -left
			bend = 0.0
		var hj := HingeJoint3D.new()
		add_child(hj)
		hj.global_transform = Transform3D(_frame_z(hinge, axis), a)
		hj.set_flag(HingeJoint3D.FLAG_USE_LIMIT, true)
		hj.set_param(HingeJoint3D.PARAM_LIMIT_LOWER, -bend + 0.03)
		hj.set_param(HingeJoint3D.PARAM_LIMIT_UPPER, maxf(0.2, 2.4 - bend))
		j = hj
	else:
		var lim: Array = CONE.get(stem, [0.6, 0.4])
		var cj := ConeTwistJoint3D.new()
		add_child(cj)
		cj.global_transform = Transform3D(_frame_x(axis, left), a)
		cj.set_param(ConeTwistJoint3D.PARAM_SWING_SPAN, float(lim[0]))
		cj.set_param(ConeTwistJoint3D.PARAM_TWIST_SPAN, float(lim[1]))
		j = cj
	j.node_a = j.get_path_to(pieces[pn])
	j.node_b = j.get_path_to(pieces[n])


static func _frame_x(x: Vector3, hint: Vector3) -> Basis:
	x = x.normalized()
	var y := hint - x * hint.dot(x)
	if y.length() < 0.01:
		y = Vector3.UP - x * x.y if absf(x.y) < 0.9 else Vector3.FORWARD - x * x.z
	y = y.normalized()
	return Basis(x, y, x.cross(y))


static func _frame_z(z: Vector3, hint: Vector3) -> Basis:
	z = z.normalized()
	var x := hint - z * hint.dot(z)
	if x.length() < 0.01:
		x = Vector3.UP - z * z.y if absf(z.y) < 0.9 else Vector3.RIGHT - z * z.x
	x = x.normalized()
	return Basis(x, z.cross(x), z)


func _physics_process(delta: float) -> void:
	t += delta
	if not dissolving and (t >= HOLD or (t > 1.0 and _settled())):
		_begin_dissolve()
	if not dissolving or _done:
		return
	_dt += delta
	var k := clampf(_dt / DISSOLVE, 0.0, 1.0)
	for g in _meshes:
		if is_instance_valid(g):
			g.set_instance_shader_parameter("dissolve", k)
	if k >= 1.0:
		_finish()


func _settled() -> bool:
	for b in bodies:
		if is_instance_valid(b) and b.linear_velocity.length() > 0.12:
			return false
	return true


func _begin_dissolve() -> void:
	if dissolving:
		return
	dissolving = true
	for g in _meshes:
		if not is_instance_valid(g):
			continue
		var col := Color(0.6, 0.6, 0.6)
		var mat: Material = g.material_override
		if mat == null and g is MeshInstance3D and (g as MeshInstance3D).mesh != null:
			mat = (g as MeshInstance3D).get_active_material(0)
		if mat is BaseMaterial3D:
			col = (mat as BaseMaterial3D).albedo_color
		g.material_override = dissolve_material()
		g.set_instance_shader_parameter("base_col", col)
		g.set_instance_shader_parameter("dissolve", 0.0)
	for b in bodies:
		if is_instance_valid(b):
			_fx.append(_embers(b))
	if root_body != null and is_instance_valid(root_body):
		Vfx.spark(self, root_body.global_position, Vector3.UP, Color(0.55, 0.5, 0.45), 18)  # a puff of dust


## Embers and ash rising off a piece while it burns (world-space: they linger where they left it).
func _embers(b: RigidBody3D) -> CPUParticles3D:
	var p := CPUParticles3D.new()
	var r := 0.08
	for c in b.get_children():
		if c is CollisionShape3D and (c as CollisionShape3D).shape is CapsuleShape3D:
			r = ((c as CollisionShape3D).shape as CapsuleShape3D).radius
	p.amount = clampi(int(r * 120.0), 6, 18)
	p.lifetime = 0.9
	p.local_coords = false
	p.emission_shape = CPUParticles3D.EMISSION_SHAPE_SPHERE
	p.emission_sphere_radius = r * 1.1
	p.direction = Vector3.UP
	p.spread = 50.0
	p.initial_velocity_min = 0.2
	p.initial_velocity_max = 1.0
	p.gravity = Vector3(0, 1.4, 0)
	p.damping_min = 0.5
	p.damping_max = 1.5
	p.scale_amount_min = 0.5
	p.scale_amount_max = 1.3
	var q := QuadMesh.new()
	q.size = Vector2(0.04, 0.04)
	q.material = Vfx._mat(EMBER, 3.0)
	p.mesh = q
	var g := Gradient.new()
	g.set_color(0, Color(1.0, 0.85, 0.4))
	g.set_color(1, Color(0.35, 0.3, 0.28, 0.0))  # embers cool to ash
	p.color_ramp = g
	b.add_child(p)
	p.emitting = true
	return p


func _finish() -> void:
	_done = true
	var at := root_body.global_position if root_body != null and is_instance_valid(root_body) else global_position
	for fx in _fx:  # the embers in the air drift on a moment after the body is gone
		if is_instance_valid(fx):
			fx.emitting = false
			fx.reparent(self)
	for c in get_children():
		if c is Joint3D:
			c.free()
	for b in bodies:
		if is_instance_valid(b):
			b.free()
	bodies.clear()
	pieces.clear()
	root_body = null
	_meshes.clear()
	live.erase(self)
	finished.emit(at)
	get_tree().create_timer(1.0, false).timeout.connect(queue_free)


## One material for every burning piece: per-instance colour and progress (noise threshold, hot edge).
static func dissolve_material() -> ShaderMaterial:
	if _dmat != null:
		return _dmat
	var sh := Shader.new()
	sh.code = """shader_type spatial;
render_mode cull_disabled;
instance uniform vec4 base_col : source_color = vec4(0.6, 0.6, 0.6, 1.0);
instance uniform float dissolve = 0.0;
uniform vec4 edge_col : source_color = vec4(1.0, 0.5, 0.12, 1.0);
varying vec3 lp;
float hash(vec3 p) {
	p = fract(p * 0.3183099 + vec3(0.1, 0.2, 0.3));
	p *= 17.0;
	return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}
float vnoise(vec3 x) {
	vec3 i = floor(x);
	vec3 f = fract(x);
	f = f * f * (3.0 - 2.0 * f);
	float a = mix(hash(i), hash(i + vec3(1.0, 0.0, 0.0)), f.x);
	float b = mix(hash(i + vec3(0.0, 1.0, 0.0)), hash(i + vec3(1.0, 1.0, 0.0)), f.x);
	float c = mix(hash(i + vec3(0.0, 0.0, 1.0)), hash(i + vec3(1.0, 0.0, 1.0)), f.x);
	float d = mix(hash(i + vec3(0.0, 1.0, 1.0)), hash(i + vec3(1.0, 1.0, 1.0)), f.x);
	return mix(mix(a, b, f.y), mix(c, d, f.y), f.z);
}
void vertex() {
	lp = VERTEX;
}
void fragment() {
	float n = vnoise(lp * 14.0) * 0.65 + vnoise(lp * 33.0) * 0.35;
	float cut = dissolve * 1.15 - 0.075;
	if (n < cut) {
		discard;
	}
	float edge = (1.0 - smoothstep(0.0, 0.09, n - cut)) * step(0.001, dissolve);
	ALBEDO = mix(base_col.rgb * (1.0 - dissolve * 0.5), edge_col.rgb, edge);
	EMISSION = edge_col.rgb * edge * 5.0;
	ROUGHNESS = 0.7;
}
"""
	_dmat = ShaderMaterial.new()
	_dmat.shader = sh
	return _dmat


## A slime's death: the blob squashes flat, bursts into goo and shrinks away; then `done(at)` (its loot).
static func burst_slime(host: Node, at: Vector3, mesh: Mesh, col: Color, radius: float, done: Callable) -> void:
	if host == null or not host.is_inside_tree():
		done.call(at)
		return
	var goo := MeshInstance3D.new()
	goo.mesh = mesh
	var mat := StandardMaterial3D.new()
	mat.albedo_color = col
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.roughness = 0.15
	mat.emission_enabled = true
	mat.emission = Color(col.r, col.g, col.b) * 0.3
	goo.material_override = mat
	host.add_child(goo)
	goo.global_position = at + Vector3.UP * radius * 0.9
	var p := CPUParticles3D.new()
	host.add_child(p)
	p.global_position = at + Vector3.UP * radius * 0.5
	p.one_shot = true
	p.explosiveness = 0.85
	p.amount = 28
	p.lifetime = 0.8
	p.emission_shape = CPUParticles3D.EMISSION_SHAPE_SPHERE
	p.emission_sphere_radius = radius * 0.6
	p.direction = Vector3.UP
	p.spread = 80.0
	p.initial_velocity_min = 1.5
	p.initial_velocity_max = 4.5
	p.gravity = Vector3(0, -12.0, 0)
	p.scale_amount_min = 0.6
	p.scale_amount_max = 1.6
	var sm := SphereMesh.new()
	sm.radius = 0.05
	sm.height = 0.1
	sm.radial_segments = 6
	sm.rings = 3
	var dm := StandardMaterial3D.new()
	dm.albedo_color = Color(col.r, col.g, col.b, 0.9)
	dm.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	dm.vertex_color_use_as_albedo = true
	sm.material = dm
	p.mesh = sm
	var g := Gradient.new()
	g.set_color(0, Color.WHITE)
	g.set_color(1, Color(1, 1, 1, 0))
	p.color_ramp = g
	p.emitting = false
	Vfx._free_after(p, 1.6)
	var tw := goo.create_tween()
	tw.tween_property(goo, "scale", Vector3(1.45, 0.4, 1.45), 0.14).set_trans(Tween.TRANS_BACK).set_ease(Tween.EASE_OUT)
	tw.tween_callback(func() -> void: p.emitting = true)
	tw.tween_property(goo, "position:y", goo.position.y - radius * 0.6, 0.5)
	tw.parallel().tween_property(goo, "scale", Vector3(0.4, 0.05, 0.4), 0.5).set_ease(Tween.EASE_IN)
	tw.parallel().tween_property(mat, "albedo_color:a", 0.0, 0.5)
	tw.tween_callback(func() -> void:
		done.call(at)
		goo.queue_free())
