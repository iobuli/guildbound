class_name Arena
extends Node3D
## The playable map, "Guild outskirts". Everything is built in code so there is nothing to hand-edit.
## Layout (spawn at the plaza centre, facing -Z = north):
##   plaza       x -14..14, z -12..14   guild hall (north side), quest board, merchant stall (east),
##                                      blacksmith corner (west, later); the lane x -3..3 stays clear
##   yard        x 18..44,  z 16..40    training dummy + sparring partner, low fence
##   meadow      around (0, -58)        slimes
##   goblin camp around (-58, 8)        palisade, tents, campfire
##   pond        around (-30, 42)       3 m deep (swimming)
## Named positions for gameplay and tests are in `spots`.

var water_volumes: Array[AABB] = []
var props := {}
var interactions: Array = []   # {"pos", "radius", "name", "action": Callable} - E uses the nearest in reach


## Something on the map E can use (the quest board, the merchant...).
func add_interaction(pos: Vector3, name: String, action: Callable, radius: float = 2.6) -> void:
	interactions.append({"pos": pos, "radius": radius, "name": name, "action": action})


func interaction_near(pos: Vector3) -> Dictionary:
	var best := {}
	var best_d := INF
	for it in interactions:
		var d := Vector2(pos.x - (it["pos"] as Vector3).x, pos.z - (it["pos"] as Vector3).z).length()
		if d <= float(it["radius"]) and d < best_d:
			best_d = d
			best = it
	return best


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


func deco(pos: Vector3, size: Vector3, color: Color, rot: Vector3 = Vector3.ZERO) -> MeshInstance3D:
	# looks only: paving, roads, flowers - nothing to collide with
	var mi := MeshInstance3D.new()
	var bm := BoxMesh.new()
	bm.size = size
	mi.mesh = bm
	mi.material_override = _mat(color)
	mi.position = pos
	mi.rotation = rot
	add_child(mi)
	return mi


func pillar(pos: Vector3, radius: float, height: float, color: Color, tag: String = "wood") -> StaticBody3D:
	var b := StaticBody3D.new()
	b.position = pos + Vector3.UP * height * 0.5
	b.set_meta("tag", tag)
	var mi := MeshInstance3D.new()
	var cm := CylinderMesh.new()
	cm.top_radius = radius
	cm.bottom_radius = radius
	cm.height = height
	mi.mesh = cm
	mi.material_override = _mat(color)
	var cs := CollisionShape3D.new()
	var sh := CylinderShape3D.new()
	sh.radius = radius
	sh.height = height
	cs.shape = sh
	b.add_child(mi)
	b.add_child(cs)
	add_child(b)
	return b


func tree(pos: Vector3, size: float) -> void:
	pillar(pos, 0.22 * size, 2.4 * size, Color(0.42, 0.30, 0.18))
	var crown := MeshInstance3D.new()
	var sm := SphereMesh.new()
	sm.radius = 1.4 * size
	sm.height = 2.6 * size
	crown.mesh = sm
	crown.material_override = _mat(Color(0.20, 0.45 + 0.08 * fposmod(pos.x, 1.0), 0.18))
	crown.position = pos + Vector3.UP * 3.0 * size
	add_child(crown)


func tent(pos: Vector3, yaw: float, color: Color) -> void:
	# an A-frame: two leaning panels (solid, so the camp has cover)
	var basis := Basis(Vector3.UP, yaw)
	for side in [-1.0, 1.0]:
		var off := basis * Vector3(side * 0.9, 1.0, 0.0)
		box(pos + off, Vector3(0.12, 2.3, 3.2), color, "wood", Vector3(0.0, yaw, side * -0.72))


## Named positions (gameplay, NPCs, spawners and the self-tests).
var spots := {
	"spawn": Vector3(0.0, 0.2, 4.0),
	"run_lane": Vector3(0.0, 0.2, 12.0),      # clear and flat for 18 m toward -Z
	"board": Vector3(-5.0, 0.1, -9.0),         # where you stand to read the quest board
	"merchant": Vector3(10.5, 0.1, -1.0),      # the merchant stands behind the counter
	"blacksmith": Vector3(-10.5, 0.1, -1.0),
	"yard_dummy": Vector3(24.0, 0.1, 25.0),
	"yard_spar": Vector3(36.0, 0.1, 30.0),
	"slime_field": Vector3(0.0, 0.1, -58.0),
	"goblin_camp": Vector3(-58.0, 0.1, 8.0),
	"pond": Vector3(-30.0, 0.0, 42.0),
}


func build() -> void:
	# --- ground: one CSG slab with the pond carved out --------------------------------------------
	var combiner := CSGCombiner3D.new()
	combiner.use_collision = true
	combiner.set_meta("tag", "grass")
	var g := CSGBox3D.new()
	g.size = Vector3(300.0, 4.0, 300.0)
	g.position = Vector3(0.0, -2.0, 0.0)
	g.material = _mat(Color(0.36, 0.58, 0.30))
	combiner.add_child(g)
	var pond_c: Vector3 = spots["pond"]
	var pond := CSGBox3D.new()
	pond.operation = CSGShape3D.OPERATION_SUBTRACTION
	pond.size = Vector3(16.0, 3.6, 16.0)
	pond.position = pond_c + Vector3(0.0, -1.2, 0.0)
	combiner.add_child(pond)
	add_child(combiner)
	var water := MeshInstance3D.new()
	var wm := BoxMesh.new()
	wm.size = Vector3(16.0, 0.05, 16.0)
	water.mesh = wm
	water.position = pond_c + Vector3(0.0, -0.35, 0.0)
	water.material_override = _mat(Color(0.2, 0.5, 0.85), 0.1, 0.6)
	add_child(water)
	water_volumes.append(AABB(pond_c + Vector3(-8.0, -3.0, -8.0), Vector3(16.0, 2.65, 16.0)))
	label("Pond", pond_c + Vector3(0.0, 2.0, -9.0))

	# --- roads (looks only) -------------------------------------------------------------------------
	var dirt := Color(0.55, 0.45, 0.32)
	deco(Vector3(0.0, 0.01, 1.0), Vector3(28.0, 0.02, 26.0), Color(0.48, 0.46, 0.43))      # plaza paving
	deco(Vector3(0.0, 0.01, -36.0), Vector3(4.0, 0.02, 46.0), dirt)                        # north to the meadow
	deco(Vector3(-36.0, 0.01, 4.0), Vector3(44.0, 0.02, 4.0), dirt)                        # west to the camp
	deco(Vector3(18.0, 0.01, 11.0), Vector3(4.0, 0.02, 13.0), dirt, Vector3(0.0, 0.675, 0.0)) # east to the yard gate
	deco(Vector3(0.0, 0.01, 30.0), Vector3(4.0, 0.02, 32.0), dirt)                         # south road

	# --- plaza: guild hall, quest board, merchant stall, blacksmith corner ---------------------------
	var stone := Color(0.70, 0.66, 0.58)
	var timber := Color(0.45, 0.32, 0.20)
	box(Vector3(0.0, 3.0, -17.0), Vector3(16.0, 6.0, 9.0), stone, "rock")                  # hall
	box(Vector3(0.0, 7.0, -17.0), Vector3(17.0, 2.0, 10.0), Color(0.55, 0.22, 0.18), "wood")  # roof
	box(Vector3(0.0, 1.5, -12.45), Vector3(2.4, 3.0, 0.1), timber, "wood")                 # door
	label("Adventurers' Guild", Vector3(0.0, 6.6, -12.2))
	var bp: Vector3 = spots["board"]
	pillar(bp + Vector3(-1.2, 0.0, -1.2), 0.1, 2.4, timber)
	pillar(bp + Vector3(1.2, 0.0, -1.2), 0.1, 2.4, timber)
	box(bp + Vector3(0.0, 1.6, -1.2), Vector3(2.6, 1.4, 0.12), Color(0.60, 0.45, 0.28), "wood")
	for i in 5:  # pinned notices
		deco(bp + Vector3(-0.9 + 0.45 * i, 1.6 + 0.2 * (i % 2), -1.13), Vector3(0.32, 0.4, 0.02), Color(0.93, 0.90, 0.80))
	label("Quest Board  (E)", bp + Vector3(0.0, 2.7, -1.2))
	var mp: Vector3 = spots["merchant"]
	box(mp + Vector3(-1.3, 0.55, 0.0), Vector3(0.8, 1.1, 3.6), timber, "wood")             # counter
	for z in [-1.7, 1.7]:
		pillar(mp + Vector3(-1.6, 0.0, z), 0.08, 2.8, timber)
		pillar(mp + Vector3(1.2, 0.0, z), 0.08, 2.8, timber)
	deco(mp + Vector3(-0.2, 2.9, 0.0), Vector3(3.4, 0.08, 4.2), Color(0.75, 0.25, 0.22), Vector3(0.0, 0.0, 0.12))  # awning
	label("Merchant  (E)", mp + Vector3(-1.3, 3.4, 0.0))
	var sp: Vector3 = spots["blacksmith"]
	box(sp + Vector3(1.0, 0.4, 0.0), Vector3(0.8, 0.8, 0.5), Color(0.25, 0.25, 0.27), "metal")   # anvil
	box(sp + Vector3(-0.8, 0.8, -1.4), Vector3(1.6, 1.6, 1.4), Color(0.40, 0.36, 0.33), "rock")  # forge
	label("Blacksmith (coming soon)", sp + Vector3(0.0, 2.6, 0.0))
	for i in 4:  # lamp posts at the plaza corners
		var c := Vector3(-13.0 if i % 2 == 0 else 13.0, 0.0, -10.0 if i < 2 else 13.0)
		pillar(c, 0.08, 3.2, Color(0.2, 0.2, 0.22), "metal")

	# --- training yard (the dummy and the sparring partner are spawned by main.gd) -------------------
	var fence := Color(0.50, 0.38, 0.24)
	for x in range(18, 45, 2):
		for z in [16.0, 40.0]:
			if absf(x - 22.0) > 2.0:  # the gate on the road side
				pillar(Vector3(x, 0.0, z), 0.07, 1.0, fence)
	for z in range(18, 40, 2):
		pillar(Vector3(18.0, 0.0, z), 0.07, 1.0, fence)
		pillar(Vector3(44.0, 0.0, z), 0.07, 1.0, fence)
	label("Training Yard (MMB lock on, Q switch weapon)", Vector3(31.0, 2.6, 17.0))

	# --- slime meadow -------------------------------------------------------------------------------
	var mc: Vector3 = spots["slime_field"]
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	for i in 40:  # flowers
		var a := rng.randf() * TAU
		var r := rng.randf_range(2.0, 18.0)
		deco(mc + Vector3(cos(a) * r, 0.08, sin(a) * r), Vector3(0.18, 0.16, 0.18),
			[Color(0.95, 0.85, 0.3), Color(0.9, 0.4, 0.6), Color(0.6, 0.5, 0.95)][i % 3])
	for i in 6:
		var a := TAU * i / 6.0 + 0.4
		box(mc + Vector3(cos(a) * 14.0, 0.5, sin(a) * 14.0), Vector3(1.6, 1.0, 1.3), Color(0.5, 0.5, 0.48), "rock", Vector3(0.0, a, 0.2))
	label("Slime Meadow", mc + Vector3(0.0, 3.0, 16.0))

	# --- goblin camp --------------------------------------------------------------------------------
	var gc: Vector3 = spots["goblin_camp"]
	var stake := Color(0.38, 0.27, 0.16)
	for i in 36:  # palisade ring, open toward the road (east)
		var a := TAU * i / 36.0
		if absf(wrapf(a, -PI, PI)) < 0.35:
			continue
		pillar(gc + Vector3(cos(a) * 14.0, 0.0, sin(a) * 14.0), 0.25, 2.6 + 0.4 * (i % 2), stake)
	tent(gc + Vector3(-5.0, 0.0, -5.0), 0.6, Color(0.45, 0.42, 0.30))
	tent(gc + Vector3(-6.0, 0.0, 4.0), -0.5, Color(0.40, 0.36, 0.26))
	tent(gc + Vector3(3.0, 0.0, -7.0), 1.4, Color(0.48, 0.40, 0.28))
	box(gc + Vector3(0.0, 0.15, 0.0), Vector3(1.4, 0.3, 1.4), Color(0.30, 0.28, 0.26), "rock")   # fire pit
	var fire := OmniLight3D.new()
	fire.light_color = Color(1.0, 0.6, 0.25)
	fire.light_energy = 2.0
	fire.omni_range = 9.0
	fire.position = gc + Vector3(0.0, 1.0, 0.0)
	add_child(fire)
	deco(gc + Vector3(0.0, 0.45, 0.0), Vector3(0.5, 0.5, 0.5), Color(1.0, 0.45, 0.1), Vector3(0.6, 0.6, 0.0))
	prop("Crate", gc + Vector3(4.0, 0.5, 4.0), Vector3(1, 1, 1), 20.0, Color(0.65, 0.48, 0.28))
	prop("Crate (heavy)", gc + Vector3(5.2, 0.5, 4.4), Vector3(1.1, 1.1, 1.1), 60.0, Color(0.58, 0.42, 0.25))
	label("Goblin Camp", gc + Vector3(14.0, 3.2, 0.0))

	# --- trees and rocks around the edges (kept out of every zone) ---------------------------------
	var zones := [[Vector3(0.0, 0.0, 1.0), 20.0], [Vector3(31.0, 0.0, 28.0), 18.0], [mc, 20.0], [gc, 17.0],
		[pond_c, 11.0], [Vector3(0.0, 0.0, -36.0), 4.0], [Vector3(0.0, 0.0, 30.0), 4.0], [Vector3(-36.0, 0.0, 4.0), 4.0]]
	var placed := 0
	var tries := 0
	while placed < 70 and tries < 2000:
		tries += 1
		var p := Vector3(rng.randf_range(-95.0, 95.0), 0.0, rng.randf_range(-95.0, 95.0))
		var free := true
		for zn in zones:
			if Vector2(p.x - (zn[0] as Vector3).x, p.z - (zn[0] as Vector3).z).length() < float(zn[1]):
				free = false
				break
		if absf(p.x) < 3.0 or absf(p.z - 4.0) < 3.0:  # keep the roads clear
			free = false
		if not free:
			continue
		if placed % 5 == 4:
			box(p + Vector3(0.0, 0.5, 0.0), Vector3(rng.randf_range(1.0, 2.4), 1.0, rng.randf_range(1.0, 2.0)), Color(0.48, 0.47, 0.45), "rock", Vector3(0.0, rng.randf() * TAU, 0.0))
		else:
			tree(p, rng.randf_range(0.8, 1.3))
		placed += 1
