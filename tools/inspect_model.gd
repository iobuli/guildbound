extends SceneTree
func _init() -> void:
	for n in ["Superhero_Male_FullBody", "Superhero_Female_FullBody"]:
		var ps: PackedScene = load("res://assets/characters/quaternius_ubc/%s.gltf" % n)
		var inst := ps.instantiate()
		var sk: Skeleton3D = inst.find_child("Skeleton3D", true, false)
		print(n, " root=", inst.get_class(), " skeleton bones=", sk.get_bone_count() if sk else -1)
		var mn := Vector3(1e9, 1e9, 1e9)
		var mx := Vector3(-1e9, -1e9, -1e9)
		for c in inst.find_children("*", "MeshInstance3D", true, false):
			var a: AABB = c.get_aabb()
			var t: Transform3D = c.global_transform if c.is_inside_tree() else Transform3D.IDENTITY
			print("  mesh ", c.name, " aabb pos=", a.position, " size=", a.size)
			mn = mn.min(a.position); mx = mx.max(a.end)
		print("  height approx ", mx.y - mn.y, " minY ", mn.y)
		if sk:
			for b in ["root","pelvis","upperarm_l","upperarm_r","lowerarm_l","hand_l","Head"]:
				var i := sk.find_bone(b)
				print("  bone ", b, " idx ", i, " rest basis x=", sk.get_bone_rest(i).basis.x if i >= 0 else "-", " origin=", sk.get_bone_rest(i).origin if i>=0 else "-")
		inst.free()
	quit()
