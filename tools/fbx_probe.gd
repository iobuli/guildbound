extends SceneTree
## Loads an FBX at runtime (FBXDocument) and prints its skeleton bones and animations.
##   Godot --headless --path <guildbound> --script res://tools/fbx_probe.gd -- D:/path/file.fbx


func _initialize() -> void:
	for path in OS.get_cmdline_user_args():
		var doc := FBXDocument.new()
		var st := FBXState.new()
		var err := doc.append_from_file(path, st)
		print("== ", path, " err ", err)
		var sc := doc.generate_scene(st)
		for sk: Skeleton3D in sc.find_children("*", "Skeleton3D", true, false):
			var names: Array[String] = []
			for i in sk.get_bone_count():
				names.append(sk.get_bone_name(i))
			print("  skeleton ", sk.name, " ", sk.get_bone_count(), ": ", ", ".join(names))
			print("  hips rest ", sk.get_bone_global_rest(0).origin)
		var ap: AnimationPlayer = sc.find_child("AnimationPlayer", true, false)
		if ap:
			for a in ap.get_animation_list():
				var an := ap.get_animation(a)
				print("  anim ", a, " ", an.length, "s tracks ", an.get_track_count(), " first ", an.track_get_path(0) if an.get_track_count() > 0 else "")
		sc.free()
	quit()
