extends Node3D
## Test arena entry point.  Run:  Godot --path <guildbound>      (add -- --selftest for the automated check)

var arena: Arena
var player: Player
var hud: Hud
var status: StatusWindow
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
	layer.add_child(status)
	player.sheet.add_item("hearty_stew")
	player.sheet.add_item("swift_tonic")
	player.sheet.gold = 120

	# sparring ring: a training dummy and a sparring partner (same bodies and code as the player)
	dummy = spawn_fighter(CharModel.new("Training Dummy", "human", 10), DUMMY_POS, FighterAI.new("dummy"))
	spar = spawn_fighter(CharModel.new("Sparring Partner", "human", 8), SPAR_POS, FighterAI.new("spar"))
	(spar.get_node("ai") as FighterAI).home = SPAR_POS
	dummy.facing = Vector3(0.0, 0.0, -1.0)
	hud.add_toast("Test arena: climb the cliff, jump the pit, swim the pond, lift the crates")

	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED

	if "--preview" in OS.get_cmdline_user_args():
		_preview()
		return
	if "--combatshots" in OS.get_cmdline_user_args():
		var cs := AnimShots.new()
		add_child(cs)
		cs.run_combat(self)
		return
	if "--getupshots" in OS.get_cmdline_user_args():
		var gs := AnimShots.new()
		add_child(gs)
		gs.run_getup(self)
		return
	if "--animshots" in OS.get_cmdline_user_args():
		var shots := AnimShots.new()
		add_child(shots)
		shots.run_shots(self)
		return
	if "--selftest" in OS.get_cmdline_user_args():
		var st := SelfTest.new()
		add_child(st)
		st.run(self)


## Another fighter in the arena: same physical body, its own stats, no camera. `brain` (optional) is
## added as the `ai` child that supplies its intent each physics frame via think(player, delta).
func spawn_fighter(sheet: CharModel, pos: Vector3, brain: Node = null) -> Player:
	var f := Player.new()
	f.is_ai = true
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
	elif event is InputEventMouseButton and event.pressed and Input.mouse_mode == Input.MOUSE_MODE_VISIBLE:
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


## Close-up screenshots of the character for checking the model: --preview [--female]
func _preview() -> void:
	var args := OS.get_cmdline_user_args()
	for i in 20:
		await get_tree().physics_frame
	player.cam.zoom = 3.2
	player.cam.pitch = -0.08
	player.cam.height = 1.25
	var shots := {"front": PI + 0.4, "back": -0.4, "side": PI * 0.5}
	for name in shots:
		player.cam.yaw = shots[name]
		for i in 6:
			await get_tree().process_frame
		var img := get_viewport().get_texture().get_image()
		img.save_png(ProjectSettings.globalize_path("res://docs/character_%s.png" % name))
	print("preview saved")
	get_tree().quit()
