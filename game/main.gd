extends Node3D
## Test arena entry point.  Run:  Godot --path <guildbound>      (add -- --selftest for the automated check)

var arena: Arena
var player: Player
var hud: Hud
var status: StatusWindow
var panel: SidePanel
var dummy: Player
var spar: Player
const DUMMY_POS := Vector3(24.0, 0.1, 25.0)
const SPAR_POS := Vector3(36.0, 0.1, 30.0)


static func build_kaelen() -> CharModel:
	var c := CharModel.new("Kaelen", "beastkin", 10)
	for item in ["iron_sword", "oak_buckler", "leather_cap", "hunter_jerkin", "wolfhide_gloves", "swift_boots", "ring_tempest", "amulet_vigil"]:
		c.equip(item)
	c.restore()
	return c


func _ready() -> void:
	InputSetup.apply()
	_build_environment()
	arena = Arena.new()
	add_child(arena)
	arena.build()

	player = Player.new()
	add_child(player)
	player.setup(arena, build_kaelen())
	player.global_position = player.respawn_pos

	var layer := CanvasLayer.new()
	add_child(layer)
	hud = Hud.new()
	hud.player = player
	layer.add_child(hud)
	player.toast.connect(hud.add_toast)
	status = StatusWindow.new()
	status.player = player
	panel = SidePanel.new()
	panel.player = player
	panel.status = status
	layer.add_child(panel)
	layer.add_child(status)
	player.sheet.add_item("hearty_stew")
	player.sheet.add_item("swift_tonic")
	player.sheet.gold = 120

	# sparring ring: a training dummy and a sparring partner (same bodies and code as the player)
	dummy = spawn_fighter(CharModel.new("Training Dummy", "human", 10), DUMMY_POS, FighterAI.new("dummy"), "Superhero_Female_FullBody", "unarmed", "Hair_Long")
	spar = spawn_fighter(CharModel.new("Sparring Partner", "human", 8), SPAR_POS, FighterAI.new("spar"), "Superhero_Male_FullBody", "greatsword", "Hair_Buzzed")
	(spar.get_node("ai") as FighterAI).home = SPAR_POS
	dummy.facing = Vector3(0.0, 0.0, -1.0)
	hud.add_toast("Q: switch weapon (sword & shield, great sword, daggers, bow, staff, fists)")

	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED

	if "--uishots" in OS.get_cmdline_user_args():
		_ui_shots()
		return
	if "--shots" in OS.get_cmdline_user_args():
		_shots()
		return
	if "--selftest" in OS.get_cmdline_user_args():
		var st := SelfTest.new()
		add_child(st)
		st.run(self)


## Another fighter in the arena: same physical body, its own stats, no camera. `brain` (optional) is
## added as the `ai` child that supplies its intent each physics frame via think(player, delta).
func spawn_fighter(sheet: CharModel, pos: Vector3, brain: Node = null, body: String = "Superhero_Male_FullBody",
		stance: String = "sword_shield", hair: String = "Hair_SimpleParted") -> Player:
	var f := Player.new()
	f.is_ai = true
	f.body_id = body
	f.hair_id = hair
	f.stance_id = stance
	if brain != null:
		brain.name = "ai"
		f.add_child(brain)
	add_child(f)
	f.setup(arena, sheet)
	f.respawn_pos = pos
	f.teleport(pos)
	return f


func _build_environment() -> void:
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50.0, -35.0, 0.0)
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 120.0
	add_child(sun)
	var we := WorldEnvironment.new()
	var env := Environment.new()
	env.background_mode = Environment.BG_SKY
	var sky := Sky.new()
	sky.sky_material = ProceduralSkyMaterial.new()
	env.sky = sky
	env.ambient_light_source = Environment.AMBIENT_SOURCE_SKY
	env.tonemap_mode = Environment.TONE_MAPPER_FILMIC
	we.environment = env
	add_child(we)


func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("free_mouse"):
		Input.mouse_mode = Input.MOUSE_MODE_VISIBLE if Input.mouse_mode == Input.MOUSE_MODE_CAPTURED else Input.MOUSE_MODE_CAPTURED
	elif event is InputEventMouseButton and event.pressed and Input.mouse_mode == Input.MOUSE_MODE_VISIBLE 			and not panel.visible and not status.visible:
		Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
	elif event.is_action_pressed("toggle_debug"):
		hud.show_debug = not hud.show_debug
	elif event.is_action_pressed("toggle_rain"):
		player.wet = not player.wet
		hud.rain = player.wet
		hud.add_toast("Rain " + ("ON: rock is slippery (climb costs x2)" if player.wet else "OFF"))
	elif event.is_action_pressed("restore"):
		for f in [dummy, spar]:
			if is_instance_valid(f):
				f.teleport(f.respawn_pos)
		player.sheet.restore()
		player.stamina = player.sheet.v("stamina")
		player.exhausted = false
		hud.add_toast("Restored")
	elif event.is_action_pressed("respawn"):
		player.teleport(player.respawn_pos)


## Screenshots of the HUD (mid-fight, a buff up, hurt) and every side-panel tab: docs/ui_*.png
func _ui_shots() -> void:
	for id in ["iron_helm", "chain_vest", "brawler_wraps", "sprinter_greaves", "ring_ogre", "swift_tonic", "keen_eye_tea"]:
		player.sheet.add_item(id)
	player.sheet.equip_from_bag("iron_helm")
	player.teleport(SPAR_POS + Vector3(0.0, 0.0, 3.0))
	for i in 30:
		await get_tree().physics_frame
	player.cast_spell("bright_valor")
	player.sheet.hp *= 0.7
	player.lock.target = spar
	for i in 90:
		await get_tree().physics_frame
	spar.sheet.hp *= 0.6
	for i in 20:
		await get_tree().process_frame
	await RenderingServer.frame_post_draw
	get_viewport().get_texture().get_image().save_png("res://docs/ui_hud.png")
	for k in SidePanel.TABS.size():
		panel.open(k)
		for i in 6:
			await get_tree().process_frame
		await RenderingServer.frame_post_draw
		print("panel visible %s frame %s children %d" % [panel.visible, str(panel._frame.get_global_rect()), panel._content.get_child_count()])
		get_viewport().get_texture().get_image().save_png("res://docs/ui_panel_%s.png" % SidePanel.TABS[k])
	print("ui shots saved")
	get_tree().quit()


## Gameplay screenshots into one sheet (docs/shots.png): stance, run, a swing on the dummy, great sword,
## bow aim, staff cast. One image to check the whole look.
func _shots() -> void:
	var cells: Array[Image] = []
	var grab := func() -> void:
		for k in 3:
			await get_tree().process_frame
		await RenderingServer.frame_post_draw
		var img := get_viewport().get_texture().get_image()
		img.resize(640, 360)
		cells.append(img)
	(spar.get_node("ai") as FighterAI).mode = "dummy"
	player.teleport(DUMMY_POS + Vector3(0.0, 0.0, 2.2))
	for k in 40:
		await get_tree().physics_frame
	player.cam.yaw = 0.0
	await grab.call()
	var ai := FighterAI.new("puppet")
	ai.name = "ai"
	player.add_child(ai)
	ai.puppet.move = Vector2(0, 1)
	ai.puppet.view_yaw = PI
	for k in 50:
		await get_tree().physics_frame
	await grab.call()
	ai.puppet.move = Vector2.ZERO
	player.teleport(DUMMY_POS + Vector3(0.0, 0.0, 1.4))
	player.facing = Vector3(0, 0, -1)
	for k in 20:
		await get_tree().physics_frame
	player.lock.target = dummy
	for style in ["sword_shield", "greatsword"]:
		player.combat.set_stance(style)
		for k in 10:
			await get_tree().physics_frame
		ai.puppet.just_attack = true
		await get_tree().physics_frame
		ai.puppet.just_attack = false
		for k in 60:
			await get_tree().physics_frame
			if player.combat.window.x >= 0.0 and player.combat.t >= player.combat.window.x + 0.05:
				break
		await grab.call()
		for k in 60:
			await get_tree().physics_frame
	player.lock.target = null
	player.combat.set_stance("bow")
	ai.puppet.guard = true
	for k in 40:
		await get_tree().physics_frame
	await grab.call()
	ai.puppet.guard = false
	player.combat.set_stance("magic")
	player.lock.target = dummy
	for k in 10:
		await get_tree().physics_frame
	ai.puppet.just_attack = true
	await get_tree().physics_frame
	ai.puppet.just_attack = false
	for k in 30:
		await get_tree().physics_frame
	await grab.call()
	var sheet := Image.create(1920, 720, false, Image.FORMAT_RGBA8)
	for k in cells.size():
		var c: Image = cells[k]
		c.convert(Image.FORMAT_RGBA8)
		sheet.blit_rect(c, Rect2i(0, 0, 640, 360), Vector2i((k % 3) * 640, (k / 3) * 360))
	sheet.save_png(ProjectSettings.globalize_path("res://docs/shots.png"))
	print("shots saved: ", cells.size())
	get_tree().quit()
