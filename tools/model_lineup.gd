extends SceneTree
## Renders characters side by side (front, three-quarter light) -> docs/model_lineup.png (windowed run).
##   Godot --path <guildbound> --script res://tools/model_lineup.gd -- res://a.vrm res://b.vrm ...

const CELL := Vector2i(300, 560)


func _initialize() -> void:
	_run()


func _run() -> void:
	var paths: Array = Array(OS.get_cmdline_user_args())
	var w := Node3D.new()
	root.add_child(w)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-40, 25, 0)
	sun.light_energy = 1.2
	w.add_child(sun)
	var env := WorldEnvironment.new()
	env.environment = Environment.new()
	env.environment.background_mode = Environment.BG_COLOR
	env.environment.background_color = Color(0.55, 0.62, 0.72)
	env.environment.ambient_light_color = Color(0.85, 0.85, 0.9)
	env.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.environment.ambient_light_energy = 0.8
	w.add_child(env)
	var cam := Camera3D.new()
	w.add_child(cam)
	cam.fov = 30.0
	var shots: Array[Image] = []
	for p in paths:
		var ch: Node3D = (load(p) as PackedScene).instantiate()
		w.add_child(ch)
		await process_frame
		cam.look_at_from_position(Vector3(0.0, 0.95, 4.2), Vector3(0, 0.85, 0))
		cam.make_current()
		for i in 6:
			await process_frame
		await RenderingServer.frame_post_draw
		var full := get_root().get_texture().get_image()
		var c := full.get_size() / 2
		shots.append(full.get_region(Rect2i(c - CELL / 2, CELL)))
		ch.free()
	var sheet := Image.create(CELL.x * shots.size(), CELL.y, false, Image.FORMAT_RGBA8)
	for i in shots.size():
		var img := shots[i]
		img.convert(Image.FORMAT_RGBA8)
		sheet.blit_rect(img, Rect2i(Vector2i.ZERO, CELL), Vector2i(i * CELL.x, 0))
	sheet.save_png(ProjectSettings.globalize_path("res://docs/model_lineup.png"))
	print("lineup saved: ", paths.size())
	quit()
