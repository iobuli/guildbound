extends SceneTree
## Prints what an imported character looks like to the engine (headless):
##   Godot --headless --path <guildbound> --script res://tools/model_probe.gd -- res://path/model.vrm
## Node tree (2 levels), skeleton name and bones (humanoid profile ones first), meshes, height.


func _initialize() -> void:
	for path in OS.get_cmdline_user_args():
		var ps: PackedScene = load(path)
		var root := ps.instantiate()
		get_root().add_child(root)
		print("== ", path)
		for c in root.get_children():
			print("  ", c.name, " (", c.get_class(), ")")
			for cc in c.get_children():
				print("    ", cc.name, " (", cc.get_class(), ")")
		for sk: Skeleton3D in root.find_children("*", "Skeleton3D", true, false):
			var prof := SkeletonProfileHumanoid.new()
			var hum: Array[String] = []
			var other: Array[String] = []
			for i in sk.get_bone_count():
				var n := sk.get_bone_name(i)
				if prof.find_bone(n) >= 0:
					hum.append(n)
				else:
					other.append(n)
			print("  skeleton %s: %d bones, %d of the %d humanoid-profile bones" % [sk.name, sk.get_bone_count(), hum.size(), prof.bone_size])
			print("  humanoid: ", ", ".join(hum))
			print("  extra (%d): %s" % [other.size(), ", ".join(other.slice(0, 40))])
			var hips := sk.find_bone("Hips")
			var head := sk.find_bone("Head")
			if hips >= 0 and head >= 0:
				print("  hips at %.3f m, head at %.3f m" % [sk.get_bone_global_rest(hips).origin.y, sk.get_bone_global_rest(head).origin.y])
		var aabb := AABB()
		for mi: MeshInstance3D in root.find_children("*", "MeshInstance3D", true, false):
			print("  mesh %s: %d surfaces, %d blend shapes" % [mi.name, mi.mesh.get_surface_count(), mi.mesh.get_blend_shape_count() if mi.mesh is ArrayMesh else 0])
			aabb = mi.get_aabb() if aabb.size == Vector3.ZERO else aabb.merge(mi.get_aabb())
		print("  bounds: ", aabb)
		root.free()
	quit()
