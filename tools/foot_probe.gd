extends SceneTree
## Prints the foot bone frames in the Idle clip (flat-footed stance on the floor at y = 0), for sizing the
## physical foot so it lies flat exactly when the animated foot does.
const LIB := "res://assets/animations/quaternius_ual1/UAL1_Standard.glb"

func _initialize() -> void:
	_run()

func _run() -> void:
	var ch: Node3D = (load("res://assets/characters/quaternius_ubc/Superhero_Male_FullBody.gltf") as PackedScene).instantiate()
	root.add_child(ch)
	var sk: Skeleton3D = ch.find_child("Skeleton3D", true, false)
	var src: Node = (load(LIB) as PackedScene).instantiate()
	var sap: AnimationPlayer = src.find_child("AnimationPlayer", true, false)
	var ap := AnimationPlayer.new()
	ch.add_child(ap)
	var lib := AnimationLibrary.new()
	lib.add_animation("Idle", sap.get_animation("Idle").duplicate())
	ap.add_animation_library("", lib)
	ap.play("Idle")
	await process_frame
	ap.seek(0.0, true)
	for side in ["l", "r"]:
		var f := sk.get_bone_global_pose(sk.find_bone("foot_" + side))
		var b := sk.get_bone_global_pose(sk.find_bone("ball_" + side))
		print(side, " foot origin ", f.origin, " basis x ", f.basis.x, " y ", f.basis.y, " z ", f.basis.z)
		print(side, " ball origin ", b.origin, " ball local ", f.affine_inverse() * b.origin)
		# the floor (y = 0) and world up, expressed in the foot's own frame
		var inv := f.affine_inverse()
		print(side, " up in foot frame ", (inv.basis * Vector3.UP).normalized(), "  floor point below ankle in foot frame ", inv * Vector3(f.origin.x, 0.0, f.origin.z))
		var toe := sk.find_bone("ball_" + side)
		for c in sk.get_bone_children(toe):
			print(side, " toe child ", sk.get_bone_name(c), " ", sk.get_bone_global_pose(c).origin)
	for n in ["Superhero_Male_FullBody"]:
		var mn := 9.0
		for mi in ch.find_children("*", "MeshInstance3D", true, false):
			var a: AABB = (mi as MeshInstance3D).get_aabb()
			mn = minf(mn, a.position.y)
		print("mesh min y (rest) ", mn)
	quit()
