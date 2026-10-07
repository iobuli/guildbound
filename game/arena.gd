class_name Arena
extends Node3D
## Greybox test arena. Everything is built in code so there is nothing to hand-edit.
## Layout (spawn at origin, facing -Z):
##   x -30..-12  ramps 30 / 45 / 55 / 75 deg      x -8  stairs    x -4  vault block + 1.6 m wall
##   x 4..8      props (20 / 60 / 80 kg crates, 200 kg boulder)
##   x 10..34    20 m cliff (rock, climbable) + ice wall (not climbable), z -34..-30
##   x 44..52    8 m pit (needs a buff to jump)             x -60..-40  pond (3 m deep)

var water_volumes: Array[AABB] = []
var props := {}


func _mat(c: Color, rough: float = 0.9, alpha: float = 1.0) -> StandardMaterial3D:
	var m := StandardMaterial3D.new()
	m.albedo_color = Color(c.r, c.g, c.b, alpha)
	m.roughness = rough
	if alpha < 1.0:
		m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	return m


func box(pos: Vector3, size: Vector3, color: Color, tag: String = "rock", rot: Vector3 = Vector3.ZERO) -> StaticBody3D:
	var b := StaticBody3D.new()
	b.position = pos
	b.rotation = rot
	b.set_meta("tag", tag)
	var mi := MeshInstance3D.new()
	var bm := BoxMesh.new()
	bm.size = size
	mi.mesh = bm
	mi.material_override = _mat(color)
	var cs := CollisionShape3D.new()
	var bs := BoxShape3D.new()
	bs.size = size
	cs.shape = bs
	b.add_child(mi)
	b.add_child(cs)
	add_child(b)
	return b


func ramp(x: float, z0: float, length: float, width: float, deg: float, color: Color) -> void:
	var th := deg_to_rad(deg)
	var t := 0.6
	var rise := length * sin(th)
	var run := length * cos(th)
	var normal := Vector3(0.0, cos(th), sin(th))
	var center := Vector3(x, rise * 0.5, z0 - run * 0.5) - normal * (t * 0.5)
	box(center, Vector3(width, t, length), color, "rock", Vector3(th, 0.0, 0.0))
	label("%d deg" % int(deg), Vector3(x, 1.6, z0 + 1.5))


func label(text: String, pos: Vector3) -> void:
	var l := Label3D.new()
	l.text = text
	l.position = pos
	l.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	l.pixel_size = 0.005
	l.visibility_range_end = 45.0
	l.modulate = Color(1, 1, 1, 0.9)
	l.font_size = 48
	l.outline_size = 12
	add_child(l)


func prop(label_text: String, pos: Vector3, size: Vector3, mass: float, color: Color, round: bool = false) -> RigidBody3D:
	var rb := RigidBody3D.new()
	rb.position = pos
	rb.mass = mass
	rb.add_to_group("prop")
	rb.set_meta("label", label_text)
	rb.collision_layer = Layers.PROPS
	rb.collision_mask = Layers.WORLD | Layers.PLAYER | Layers.HITBOX | Layers.PROPS
	var pm := PhysicsMaterial.new()
	pm.friction = 0.8
	rb.physics_material_override = pm
	var mi := MeshInstance3D.new()
	var cs := CollisionShape3D.new()
	if round:
		var sm := SphereMesh.new()
		sm.radius = size.x * 0.5
		sm.height = size.x
		mi.mesh = sm
		var ss := SphereShape3D.new()
		ss.radius = size.x * 0.5
		cs.shape = ss
	else:
		var bm := BoxMesh.new()
		bm.size = size
		mi.mesh = bm
		var bs := BoxShape3D.new()
		bs.size = size
		cs.shape = bs
	mi.material_override = _mat(color)
	rb.add_child(mi)
	rb.add_child(cs)
	add_child(rb)
	props[label_text] = rb
	return rb


func build() -> void:
	# --- ground: one CSG slab with a pond and a pit carved out -------------------------------
	var combiner := CSGCombiner3D.new()
	combiner.use_collision = true
	combiner.set_meta("tag", "grass")
	var g := CSGBox3D.new()
	g.size = Vector3(300.0, 4.0, 300.0)
	g.position = Vector3(0.0, -2.0, 0.0)
	g.material = _mat(Color(0.36, 0.58, 0.30))
	combiner.add_child(g)
	var pond := CSGBox3D.new()
	pond.operation = CSGShape3D.OPERATION_SUBTRACTION
	pond.size = Vector3(20.0, 3.6, 20.0)
	pond.position = Vector3(-50.0, -1.2, 0.0)
	combiner.add_child(pond)
	var pit := CSGBox3D.new()
	pit.operation = CSGShape3D.OPERATION_SUBTRACTION
	pit.size = Vector3(8.0, 3.6, 20.0)
	pit.position = Vector3(48.0, -1.2, 0.0)
	combiner.add_child(pit)
	add_child(combiner)

	# pond water (volume is a plain AABB; the player checks it in code)
	var water := MeshInstance3D.new()
	var wm := BoxMesh.new()
	wm.size = Vector3(20.0, 0.05, 20.0)
	water.mesh = wm
	water.position = Vector3(-50.0, -0.35, 0.0)
	water.material_override = _mat(Color(0.2, 0.5, 0.85), 0.1, 0.6)
	add_child(water)
	water_volumes.append(AABB(Vector3(-60.0, -3.0, -10.0), Vector3(20.0, 2.65, 20.0)))
	label("Pond (3 m deep)", Vector3(-41.0, 2.0, 0.0))

	var lava := MeshInstance3D.new()
	var lm := BoxMesh.new()
	lm.size = Vector3(8.0, 0.05, 20.0)
	lava.mesh = lm
	lava.position = Vector3(48.0, -2.85, 0.0)
	lava.material_override = _mat(Color(0.9, 0.25, 0.1))
	add_child(lava)
	label("8 m gap: needs a speed buff", Vector3(43.0, 2.5, 0.0))

	# --- slopes -------------------------------------------------------------------------------
	ramp(-30.0, -5.0, 12.0, 4.0, 30.0, Color(0.55, 0.50, 0.42))
	ramp(-25.0, -5.0, 12.0, 4.0, 45.0, Color(0.58, 0.48, 0.36))
	ramp(-20.0, -5.0, 12.0, 4.0, 55.0, Color(0.62, 0.42, 0.32))
	ramp(-15.0, -5.0, 12.0, 4.0, 75.0, Color(0.66, 0.36, 0.30))

	# --- stairs (0.3 m risers, below the 0.4 m step height) ---------------------------------------
	for i in 10:
		box(Vector3(-8.0, 0.15 * (i + 1), -6.0 - 0.8 * i), Vector3(3.0, 0.3 * (i + 1), 0.8), Color(0.7, 0.68, 0.62))
	label("Stairs (0.3 m steps)", Vector3(-8.0, 1.8, -2.0))

	# --- vaulting -----------------------------------------------------------------------------
	box(Vector3(-4.0, 0.5, -10.0), Vector3(3.0, 1.0, 1.0), Color(0.6, 0.45, 0.3), "wood")
	box(Vector3(0.0, 0.8, -10.0), Vector3(3.0, 1.6, 1.0), Color(0.5, 0.4, 0.3), "wood")
	label("1.0 m block (sprint = vault)", Vector3(-4.0, 2.2, -8.5))

	# --- cliff and ice wall ---------------------------------------------------------------------
	box(Vector3(16.0, 10.0, -32.0), Vector3(12.0, 20.0, 4.0), Color(0.45, 0.42, 0.40), "rock")
	box(Vector3(28.0, 10.0, -32.0), Vector3(12.0, 20.0, 4.0), Color(0.65, 0.82, 0.92), "ice")
	label("20 m cliff (climb it)", Vector3(16.0, 4.0, -27.0))
	label("ICE (cannot climb)", Vector3(28.0, 4.0, -27.0))

	# --- props ------------------------------------------------------------------------------------
	prop("Crate 20 kg", Vector3(4.0, 0.5, -3.0), Vector3(1, 1, 1), 20.0, Color(0.72, 0.55, 0.32))
	prop("Crate 60 kg", Vector3(6.0, 0.5, -3.0), Vector3(1, 1, 1), 60.0, Color(0.65, 0.48, 0.28))
	prop("Crate 80 kg", Vector3(8.0, 0.5, -3.0), Vector3(1.1, 1.1, 1.1), 80.0, Color(0.58, 0.42, 0.25))
	prop("Boulder 200 kg", Vector3(10.0, 0.8, -3.0), Vector3(1.6, 1.6, 1.6), 200.0, Color(0.5, 0.5, 0.5), true)
	label("Crates 20/60/80 kg + boulder 200 kg", Vector3(7.0, 2.2, -1.0))
	label("SPAWN", Vector3(0.0, 2.4, 3.0))
	# loot row: gear with trade-offs and food / potions (E picks up, Tab opens the status window)
	var loot := [["chain_vest", 1], ["iron_helm", 1], ["brawler_wraps", 1], ["sprinter_greaves", 1], ["ring_ogre", 1],
		["steel_longsword", 1], ["windfang_daggers", 1], ["swift_tonic", 2], ["hearty_stew", 1], ["ironbark_draught", 1]]
	for i in loot.size():
		var pk := ItemPickup.make(loot[i][0], loot[i][1])
		pk.position = Vector3(-9.0 + 2.0 * i, 0.0, 7.0)
		add_child(pk)
	label("Loot: walk up, press E. Tab = status window", Vector3(0.0, 2.2, 8.5))
	label("Sparring ring: training dummy + sparring partner (MMB lock on)", Vector3(30.0, 2.6, 22.0))
