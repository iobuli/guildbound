extends SceneTree
## Renders library clips (assets/anims) on the VRoid base at 8 moments each -> docs/clip_sheet.png (windowed run).
##   Godot --path <guildbound> --script res://tools/clip_sheet.gd [-- Clip1 Clip2 ...]
## Clip names are the library's (data/anim_sources.json).

const DEFAULT_CLIPS := ["Idle", "Walk", "Run", "FightStance", "Jab_L", "Roundhouse_R"]
const STEPS := [0.0, 0.12, 0.25, 0.37, 0.5, 0.62, 0.75, 0.98]
const CELL := Vector2i(260, 320)


func _initialize() -> void:
	_run()


func _find(name: String) -> Animation:
	if ResourceLoader.exists("res://assets/anims/%s.res" % name):
		return load("res://assets/anims/%s.res" % name)
	push_error("clip not found: " + name)
	return null


func _run() -> void:
	var clips: Array = Array(OS.get_cmdline_user_args())
	if clips.is_empty():
		clips = DEFAULT_CLIPS
	var w := Node3D.new()
	root.add_child(w)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-45, -30, 0)
	w.add_child(sun)
	var env := WorldEnvironment.new()
	env.environment = Environment.new()
	env.environment.background_mode = Environment.BG_COLOR
	env.environment.background_color = Color(0.55, 0.6, 0.68)
	env.environment.ambient_light_color = Color(0.7, 0.7, 0.75)
	env.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	w.add_child(env)
	var floor := MeshInstance3D.new()
	var pm := PlaneMesh.new()
	pm.size = Vector2(10, 10)
	floor.mesh = pm
	w.add_child(floor)
	var model: String = (JSON.parse_string(FileAccess.get_file_as_string("res://data/anim_sources.json")) as Dictionary)["_target"]
	var ch: Node3D = (load(model) as PackedScene).instantiate()
	w.add_child(ch)
	var ap := AnimationPlayer.new()
	ch.add_child(ap)
	var lib := AnimationLibrary.new()
	for c in clips:
		var a := _find(c)
		if a != null:
			lib.add_animation(c, a)
	ap.add_animation_library("", lib)
	var cam := Camera3D.new()
	w.add_child(cam)
	await process_frame
	cam.look_at_from_position(Vector3(5.5, 2.2, 0.0), Vector3(0, 0.5, 0))
	cam.make_current()
	var shots: Array = []
	for c in clips:
		var a: Animation = lib.get_animation(c)
		print("%s length %.2f s" % [c, a.length])
		for f in STEPS:
			ap.play(c)
			ap.seek(a.length * f, true)
			ap.pause()
			for i in 3:
				await process_frame
			var img := root.get_texture().get_image()
			var sz := img.get_size()
			shots.append(img.get_region(Rect2i(sz.x / 2 - CELL.x / 2, sz.y / 2 - CELL.y / 2, CELL.x, CELL.y)))
	var sheet := Image.create(CELL.x * STEPS.size(), CELL.y * clips.size(), false, Image.FORMAT_RGBA8)
	for i in shots.size():
		var im: Image = shots[i]
		im.convert(Image.FORMAT_RGBA8)
		sheet.blit_rect(im, Rect2i(Vector2i.ZERO, CELL), Vector2i((i % STEPS.size()) * CELL.x, (i / STEPS.size()) * CELL.y))
	sheet.save_png(ProjectSettings.globalize_path("res://docs/clip_sheet.png"))
	print("clip sheet saved; rows: ", clips)
	quit()
