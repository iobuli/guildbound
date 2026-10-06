class_name AnimShots
extends SelfTest
## Drives the real player through each movement state and saves a contact sheet of the animation,
## so poses can be checked by eye.  Run (needs a window):  Godot --path <guildbound> -- --animshots

const CELL := Vector2i(360, 440)
const COLS := 5

var shots: Array = []


func side(deg: float = 90.0, zoom: float = 3.4) -> void:
	p.cam.view_offset = deg_to_rad(deg)
	p.cam.zoom = zoom
	p.cam.pitch = -0.1


func shot(label: String) -> void:
	await get_tree().process_frame
	await get_tree().process_frame
	if DisplayServer.get_name() != "headless":
		var img := get_viewport().get_texture().get_image()
		var sz := img.get_size()
		var c := Vector2i(sz.x / 2 - CELL.x / 2, sz.y / 2 - CELL.y / 2 - 30)
		shots.append([label, img.get_region(Rect2i(c, CELL))])
	else:
		shots.append([label, null])
	print("   state %d speed %.2f pushing %s carrying %s mode %s intent %.2f" % [p.state, p.last_h_speed, p.pushing, p.carried != null,
		p.model.tree.get("parameters/mode/current_state"), p.move_intent.length()])
	print("   held: ", ACTIONS.filter(func(ac): return Input.is_action_pressed(ac)), " vec ", Input.get_vector("move_left", "move_right", "move_forward", "move_back"))
	var cp := p.cam.camera.global_position - p.global_position
	print("shot %d: %s | cam.yaw %.0f off %.0f rig %.0f | cam rel (%.1f %.1f %.1f) | facing (%.2f %.2f) model.yaw %.0f" % [shots.size(), label,
		rad_to_deg(p.cam.yaw), rad_to_deg(p.cam.view_offset), rad_to_deg(p.cam.rotation.y), cp.x, cp.y, cp.z, p.facing.x, p.facing.z, rad_to_deg(p.model.rotation.y)])


func run_shots(main: Node) -> void:
	m = main
	p = main.player
	main.hud.visible = false
	p.cam.sens = 0.0
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	await frames(20)

	tp(Vector3(10, 0.1, 2.5), 0)
	side(80, 3.2)
	await frames(20)
	press("move_forward")
	await frames(70)
	print("   push: pushing=%s body=%s hands=%d" % [p.pushing, p.push_body, p.model.ik.push_targets.size()])
	await shot("push boulder")
	release_all()

	tp(Vector3(-8, 0.1, -3.0), 0)
	side(90, 3.4)
	p.cam.pitch = -0.42
	await frames(20)
	press("move_forward")
	await frames(32)
	await shot("stairs: walking up")
	release_all()
	await frames(40)
	await shot("stairs: standing")
	tp(Vector3(-8, 3.15, -13.2), 180)
	side(90, 3.4)
	p.cam.pitch = -0.42
	await frames(30)
	press("move_forward")
	await frames(30)
	await shot("stairs: walking down")
	release_all()

	tp(Vector3(0, 0.1, 60), 0)
	side(90)
	await frames(60)
	await shot("idle")
	press("move_forward")
	await frames(40)
	release("move_forward")
	await frames(4)
	await shot("walk (decelerating)")
	press("move_forward")
	await frames(60)
	await shot("run")
	press("sprint")
	await frames(40)
	await shot("sprint")
	p.sheet.cast("gale_step")
	p.model.on_cast()
	await frames(10)
	await shot("cast while sprinting")
	release_all()
	await frames(30)

	tp(Vector3(0, 0.1, 60), 0)
	side(90)
	await frames(10)
	press("crouch")
	await frames(2)
	release("crouch")
	press("move_forward")
	await frames(40)
	await shot("crouch walk")
	release_all()
	press("crouch")
	await frames(2)
	release("crouch")

	tp(Vector3(0, 0.1, 60), 0)
	side(90)
	await frames(20)
	press("jump")
	await frames(14)
	await shot("jump (air)")
	release_all()
	await frames(40)
	press("dodge")
	await frames(7)
	await shot("dodge roll")
	release_all()
	await frames(30)

	tp(Vector3(6, 0.1, -0.5), 0)
	side(70)
	await frames(30)
	press("interact")
	await frames(40)
	await shot("carry (standing)")
	press("move_left")
	await frames(30)
	await shot("carry (walking)")
	release_all()
	await frames(20)


	tp(Vector3(16, 0.1, -29.0), 0)
	side(65, 4.0)
	await frames(15)
	press("move_forward")
	press("jump")
	await frames(3)
	release("jump")
	await frames(70)
	await shot("climb A")
	p.cam.view_offset = deg_to_rad(20)
	p.cam.pitch = -0.15
	await frames(24)
	await shot("climb B (behind)")
	release_all()
	await frames(10)

	tp(Vector3(0, 0.1, 40), 0)
	side(90, 4.0)
	await frames(20)
	press("move_forward")
	press("sprint")
	await frames(40)
	press_event("ragdoll")
	await frames(14)
	await shot("ragdoll (G mid-sprint)")
	release_all()
	await wait_until(func(): return p.state == Player.State.RECOVER, 400)
	await frames(10)
	await shot("get-up: " + p.recover_kind)
	await frames(25)
	await shot("get-up later")
	await wait_until(func(): return p.state == Player.State.GROUND, 300)

	tp(Vector3(16, 20.2, -32.5), 180)
	side(90, 5.0)
	await frames(30)
	press("move_forward")
	await wait_until(func(): return p.state == Player.State.AIR and p.air_time > 0.8, 300)
	await shot("long fall")
	release_all()
	await wait_until(func(): return p.state == Player.State.RAGDOLL, 300)
	await frames(30)
	await shot("ragdoll after 20 m fall")
	await wait_until(func(): return p.state == Player.State.RECOVER, 500)
	await frames(8)
	await shot("get-up: " + p.recover_kind)
	await wait_until(func(): return p.state == Player.State.GROUND, 300)

	tp(Vector3(-38, 0.1, 0), 90)
	side(90, 4.0)
	p.cam.pitch = -0.55
	press("move_forward")
	await frames(150)
	await shot("swim")
	release_all()
	await frames(5)

	if DisplayServer.get_name() != "headless":
		_save_sheet()
	get_tree().quit()


## Filmstrips of get-ups: the ragdoll at rest, then a frame every ~0.3 s until standing.
## Run (needs a window):  Godot --path <guildbound> -- --getupshots   ->  docs/getup_sheet.png
const GCELL := Vector2i(360, 380)
const GCOLS := 10


func grab() -> Image:
	await get_tree().process_frame
	var img := get_viewport().get_texture().get_image()
	var sz := img.get_size()
	var c := Vector2i(sz.x / 2 - GCELL.x / 2, sz.y / 2 - GCELL.y / 2 + 150)
	var r := img.get_region(Rect2i(c, GCELL))
	r.convert(Image.FORMAT_RGBA8)
	return r


func run_getup(main: Node) -> void:
	m = main
	p = main.player
	main.hud.visible = false
	p.cam.sens = 0.0
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	await frames(20)
	var rows: Array = []
	for c in [[Vector3(0, 0, -4), Vector3(0, 0.1, 60)], [Vector3(0, 0, 4), Vector3(0, 0.1, 60)],
			[Vector3(0, 0, 3), Vector3(-30, 4.0, -10)], [Vector3(0, 0, 3), Vector3(-8, 2.2, -9.5)], [Vector3(0, 0, -4), Vector3(16, 0.1, -28.6)]]:
		tp(c[1], 0)
		await frames(30)
		p.start_ragdoll(Vector3(c[0].x, 1.5, c[0].z))
		await wait_until(func() -> bool: return p.state == Player.State.RECOVER, 600)
		var row: Array = []
		var kind := p.recover_kind
		while row.size() < GCOLS:
			# side-on to the way the character will stand up
			p.cam.yaw = atan2(-p.facing.x, -p.facing.z) + deg_to_rad(90)
			p.cam.view_offset = 0.0
			p.cam.zoom = 3.6
			p.cam.pitch = -0.2
			row.append(await grab())
			if p.state != Player.State.RECOVER:
				break
			await frames(24)
		print("getup row %d: %s, %d frames" % [rows.size(), kind, row.size()])
		rows.append(row)
		await wait_until(func() -> bool: return p.state == Player.State.GROUND, 300)
	var sheet := Image.create(GCELL.x * GCOLS, GCELL.y * rows.size(), false, Image.FORMAT_RGBA8)
	sheet.fill(Color(0.1, 0.1, 0.12))
	for y in rows.size():
		for x in (rows[y] as Array).size():
			sheet.blit_rect(rows[y][x], Rect2i(Vector2i.ZERO, GCELL), Vector2i(x * GCELL.x, y * GCELL.y))
	var path := ProjectSettings.globalize_path("res://docs/getup_sheet.png")
	sheet.save_png(path)
	print("sheet -> ", path)
	get_tree().quit()


func _save_sheet() -> void:
	var rows := int(ceil(shots.size() / float(COLS)))
	var sheet := Image.create(CELL.x * COLS, CELL.y * rows, false, Image.FORMAT_RGBA8)
	sheet.fill(Color(0.1, 0.1, 0.12))
	for i in shots.size():
		var img: Image = shots[i][1]
		img.convert(Image.FORMAT_RGBA8)
		sheet.blit_rect(img, Rect2i(Vector2i.ZERO, CELL), Vector2i((i % COLS) * CELL.x, (i / COLS) * CELL.y))
	var path := ProjectSettings.globalize_path("res://docs/anim_sheet.png")
	sheet.save_png(path)
	print("sheet -> ", path, "  order: ", ", ".join(shots.map(func(s): return s[0])))


## Contact sheet of the fighting: stance, each strike at impact, reactions, the lock-on view.
## Run (needs a window):  Godot --path <guildbound> -- --combatshots   ->  docs/combat_sheet.png
const CCELL := Vector2i(420, 420)


func cgrab() -> Image:
	await get_tree().process_frame
	await get_tree().process_frame
	var img := get_viewport().get_texture().get_image()
	var sz := img.get_size()
	var r := img.get_region(Rect2i(Vector2i(sz.x / 2 - CCELL.x / 2, sz.y / 2 - CCELL.y / 2 + 60), CCELL))
	r.convert(Image.FORMAT_RGBA8)
	return r


## Side-on camera between the two fighters.
func side_cam(a: Player, b: Player) -> void:
	var mid := (a.global_position + b.global_position) * 0.5
	var to := b.global_position - a.global_position
	to.y = 0.0
	p.cam.lock_target = null
	p.cam.yaw = atan2(-to.x, -to.z) - PI / 2.0
	p.cam.view_offset = 0.0
	p.cam.pitch = -0.12
	p.cam.zoom = 3.0


func run_combat(main: Node) -> void:
	m = main
	p = main.player
	main.hud.visible = false
	p.cam.sens = 0.0
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	var dm: Player = main.dummy
	var sp: Player = main.spar
	(sp.get_node("ai") as FighterAI).mode = "dummy"
	p.combat.crits = false
	await frames(20)
	var cells: Array = []
	var labels: Array = []
	var snap := func(label: String) -> void:
		labels.append(label)
		cells.append(await cgrab())
	dm.teleport(main.DUMMY_POS)
	sp.teleport(main.SPAR_POS)
	await frames(10)
	# stance, locked on
	tp(dm.global_position + Vector3(0, 0, 1.3), 0)
	await frames(15)
	press("lock")
	await frames(2)
	release("lock")
	await frames(30)
	await snap.call("fighting stance (locked on)")
	p.cam.yaw = 0.0
	press("guard")
	await frames(20)
	side_cam(p, dm)
	await snap.call("guard (RMB)")
	release("guard")
	await frames(20)
	# each strike, caught at the moment it lands (and a moment later)
	for id in ["jab", "cross", "hook"]:
		dm.teleport(main.DUMMY_POS)
		tp(dm.global_position + Vector3(0, 0, 1.1), 0)
		await frames(10)
		press("lock")
		await frames(2)
		release("lock")
		await frames(10)
		p.combat.combo_i = ["jab", "cross", "hook"].find(id)
		var landed := p.combat.hits_landed
		press("attack")
		await frames(1)
		release("attack")
		for i in 40:
			await get_tree().physics_frame
			side_cam(p, dm)
			if p.combat.hits_landed > landed or (p.combat.state == Combat.S.ACTIVE and p.combat.t > 0.08):
				break
		await snap.call("%s at impact" % id)
		await frames(5)
		side_cam(p, dm)
		await snap.call("%s + 80 ms (target reacts)" % id)
		await frames(40)
	# charged strike, the stagger, then a knockdown and the get-up
	dm.teleport(main.DUMMY_POS)
	tp(dm.global_position + Vector3(0, 0, 1.2), 0)
	await frames(10)
	press("lock")
	await frames(2)
	release("lock")
	press("attack")
	await frames(60)
	side_cam(p, dm)
	await snap.call("charging (hold LMB)")
	release("attack")
	await wait_until(func() -> bool: return dm.combat.state == Combat.S.STAGGER, 60)
	await frames(4)
	side_cam(p, dm)
	await snap.call("charged hit: stagger")
	await frames(40)
	dm.combat.stagger(3.0, Vector3.ZERO)
	press("attack")
	await frames(40)
	release("attack")
	await wait_until(func() -> bool: return dm.state == Player.State.RAGDOLL, 60)
	await frames(12)
	side_cam(p, dm)
	await snap.call("knockdown (ragdoll)")
	await wait_until(func() -> bool: return dm.state == Player.State.RECOVER, 400)
	await frames(50)
	side_cam(p, dm)
	await snap.call("getting up")
	# sparring partner in action, seen through the lock-on camera
	(sp.get_node("ai") as FighterAI).mode = "spar"
	dm.teleport(main.DUMMY_POS)
	tp(main.SPAR_POS + Vector3(0, 0, 2.5), 0)
	await frames(10)
	press("lock")
	await frames(2)
	release("lock")
	for k in 3:
		await frames(35)
		await snap.call("sparring, lock-on camera %d" % (k + 1))
	var cols := 4
	var rows := int(ceil(cells.size() / float(cols)))
	var sheet := Image.create(CCELL.x * cols, CCELL.y * rows, false, Image.FORMAT_RGBA8)
	sheet.fill(Color(0.1, 0.1, 0.12))
	for i in cells.size():
		sheet.blit_rect(cells[i], Rect2i(Vector2i.ZERO, CCELL), Vector2i((i % cols) * CCELL.x, (i / cols) * CCELL.y))
	var path := ProjectSettings.globalize_path("res://docs/combat_sheet.png")
	sheet.save_png(path)
	print("sheet -> ", path, "  order: ", ", ".join(labels))
	get_tree().quit()
