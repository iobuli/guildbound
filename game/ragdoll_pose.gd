class_name RagdollPose
extends SkeletonModifier3D
## Last modifier on the skeleton. Two jobs every frame:
##   1. record the pose the animation (AnimationTree + MotionIK) wants -> BodyRig.targets, in
##      skeleton space so the physics step can place it at the character's *current* position
##   2. draw the body from the physics parts, so the visible character is exactly the solid body
##      (blocked hands stay on the wall, feet stay on top of steps, ragdolls keep their momentum)

var rig: BodyRig
var drawn := {}   # bone -> world transform as last drawn (read by the self-test)


func _process_modification_with_delta(_delta: float) -> void:
	var sk := get_skeleton()
	if rig == null or sk == null:
		return
	var gt := sk.global_transform
	var fresh := {}
	for p in rig.parts:
		fresh[p["bone"]] = sk.get_bone_global_pose(p["bone"])
	rig.set_targets(fresh)
	influence = 1.0
	var inv := gt.affine_inverse()
	# pelvis: full transform. Every other part: its rotation from physics, its position from the
	# already-placed parent chain, so bones keep their real length and the mesh never stretches.
	for p in rig.parts:
		var t: Transform3D = inv * p["body"].get_global_transform_interpolated()
		if p["name"] != "pelvis":
			t.origin = sk.get_bone_global_pose(p["bone"]).origin
		sk.set_bone_global_pose(p["bone"], t)
		drawn[p["bone"]] = gt * sk.get_bone_global_pose(p["bone"])
