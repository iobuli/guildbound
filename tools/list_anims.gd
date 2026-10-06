extends SceneTree
func _initialize():
	for path in ["res://assets/animations/quaternius_ual1/UAL1_Standard.glb", "res://assets/animations/quaternius_ual2/UAL2_Standard.glb"]:
		var src: Node = (load(path) as PackedScene).instantiate()
		var ap: AnimationPlayer = src.find_child("AnimationPlayer", true, false)
		var out := []
		for n in ap.get_animation_list():
			out.append("%s(%.2f)" % [n, ap.get_animation(n).length])
		print(path.get_file(), ": ", ", ".join(out))
		src.free()
	quit()
