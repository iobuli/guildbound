extends SceneTree
## One-off repair of clips made before tools/retarget.gd wrote the hips in their parent's space.
## Those clips placed the pelvis as if the root bone were Y up; the base body's root bone is Z up, so the
## clip's height landed on the backward axis (the body stood 0.8-1.0 m behind its collider) and the height
## stayed at the rest value. Each pelvis key V becomes (V.x, hr.z + hr.y - V.z, V.y), hr = pelvis rest.
## Clips already repaired carry the meta "pelvis_space" and are skipped.
##   Godot --headless --path <guildbound> --script res://tools/fix_pelvis.gd

const ANIMS := "res://assets/anims/"


func _initialize() -> void:
	var model: String = (JSON.parse_string(FileAccess.get_file_as_string("res://data/anim_sources.json")) as Dictionary)["_target"]
	var ch: Node3D = (load(model) as PackedScene).instantiate()
	var sk: Skeleton3D = ch.find_child("Skeleton3D", true, false)
	var pelvis := sk.find_bone("pelvis")
	var hr := sk.get_bone_rest(pelvis).origin
	var parent_basis := sk.get_bone_global_rest(sk.get_bone_parent(pelvis)).basis
	# the repair below assumes exactly this parent frame (local Y -> skeleton -Z, local Z -> skeleton +Y)
	if not parent_basis.is_equal_approx(Basis(Vector3(1, 0, 0), Vector3(0, 0, -1), Vector3(0, 1, 0))):
		push_error("unexpected root bone rest %s - not repairing" % str(parent_basis))
		quit(1)
		return
	var fixed := 0
	var skipped := 0
	for f in DirAccess.get_files_at(ANIMS):
		if not f.ends_with(".res"):
			continue
		var path := ANIMS + f
		var a: Animation = load(path)
		if a.has_meta("pelvis_space"):
			skipped += 1
			continue
		for tr in a.get_track_count():
			if a.track_get_type(tr) != Animation.TYPE_POSITION_3D or a.track_get_path(tr).get_concatenated_subnames() != "pelvis":
				continue
			for k in a.track_get_key_count(tr):
				var v: Vector3 = a.track_get_key_value(tr, k)
				a.track_set_key_value(tr, k, Vector3(v.x, hr.z + hr.y - v.z, v.y))
		a.set_meta("pelvis_space", "parent")
		var err := ResourceSaver.save(a, path)
		if err != OK:
			push_error("could not save %s (%d)" % [path, err])
		else:
			fixed += 1
	ch.free()
	print("pelvis repaired in %d clips, %d already done" % [fixed, skipped])
	quit()
