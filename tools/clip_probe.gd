extends SceneTree
## Prints bone heights / tilt over a clip, for tuning (headless):
##   Godot --headless --path <guildbound> --script res://tools/clip_probe.gd -- Clip [step]

const LIBS := ["res://assets/animations/quaternius_ual1/UAL1_Standard.glb", "res://assets/animations/quaternius_ual2/UAL2_Standard.glb"]


func _find(name: String) -> Animation:
	if name.begins_with("CMU_"):
		return load("res://assets/animations/cmu/%s.res" % name)
	for path in LIBS:
		var src: Node = (load(path) as PackedScene).instantiate()
		var ap: AnimationPlayer = src.find_child("AnimationPlayer", true, false)
		if ap.has_animation(name):
			var a: Animation = ap.get_animation(name).duplicate()
			src.free()
			return a
		src.free()
	return null


func _initialize() -> void:
	_run()


func _run() -> void:
	var args := OS.get_cmdline_user_args()
	var clip: String = args[0]
	var step := float(args[1]) if args.size() > 1 else 0.05
	var ch: Node3D = (load("res://assets/characters/quaternius_ubc/Superhero_Male_FullBody.gltf") as PackedScene).instantiate()
	root.add_child(ch)
	var sk: Skeleton3D = ch.find_child("Skeleton3D", true, false)
	var ap := AnimationPlayer.new()
	ch.add_child(ap)
	var lib := AnimationLibrary.new()
	var a := _find(clip)
	lib.add_animation(clip, a)
	ap.add_animation_library("", lib)
	ap.play(clip)
	await process_frame
	print("%s length %.3f  skel scale %s" % [clip, a.length, str(sk.global_transform.basis.get_scale())])
	print("  t     pelvis(x,y,z)        chestTilt  footL(y,z) footR(y,z) ballL.y ballR.y  handL(y,z) handR(y,z)")
	var t := 0.0
	while t <= a.length + 0.0001:
		ap.seek(t, true)
		var g := func(n: String) -> Vector3: return sk.global_transform * sk.get_bone_global_pose(sk.find_bone(n)).origin
		var cb: Basis = sk.global_transform.basis * sk.get_bone_global_pose(sk.find_bone("spine_02")).basis
		var tilt := rad_to_deg(cb.y.normalized().angle_to(Vector3.UP))
		var pv: Vector3 = g.call("pelvis")
		var fl: Vector3 = g.call("foot_l")
		var fr: Vector3 = g.call("foot_r")
		var hl: Vector3 = g.call("hand_l")
		var hr: Vector3 = g.call("hand_r")
		print("%5.2f  (%5.2f %5.2f %5.2f)  %6.1f   (%5.2f %5.2f) (%5.2f %5.2f) %5.2f %5.2f  (%5.2f %5.2f) (%5.2f %5.2f)" % [t, pv.x, pv.y, pv.z, tilt,
			fl.y, fl.z, fr.y, fr.z, (g.call("ball_l") as Vector3).y, (g.call("ball_r") as Vector3).y, hl.y, hl.z, hr.y, hr.z])
		t += step
	print(stance_speed(sk, ap, a))
	quit()


## Average speed of the planted (lowest) foot over the clip = the ground speed the clip "walks" at.
static func stance_speed(sk: Skeleton3D, ap: AnimationPlayer, a: Animation) -> String:
	var dt := 1.0 / 120.0
	var t := 0.0
	var sum := 0.0
	var n := 0
	var vmin := 99.0
	var vmax := 0.0
	var fl := sk.find_bone("foot_l")
	var fr := sk.find_bone("foot_r")
	var bl := sk.find_bone("ball_l")
	var br := sk.find_bone("ball_r")
	var hmin := 99.0
	while t <= a.length:
		ap.seek(t, true)
		hmin = minf(hmin, minf(minf(sk.get_bone_global_pose(fl).origin.y - 0.05, sk.get_bone_global_pose(bl).origin.y + 0.03),
			minf(sk.get_bone_global_pose(fr).origin.y - 0.05, sk.get_bone_global_pose(br).origin.y + 0.03)))
		t += dt
	t = 0.0
	while t + dt <= a.length:
		ap.seek(t, true)
		var p0 := [sk.get_bone_global_pose(fl).origin, sk.get_bone_global_pose(bl).origin, sk.get_bone_global_pose(fr).origin, sk.get_bone_global_pose(br).origin]
		ap.seek(t + dt, true)
		var p1 := [sk.get_bone_global_pose(fl).origin, sk.get_bone_global_pose(bl).origin, sk.get_bone_global_pose(fr).origin, sk.get_bone_global_pose(br).origin]
		# sole height of each contact point (ankle rests at 0.05, ball at -0.03)
		var best := -1
		var bh := 99.0
		for i in 4:
			var h: float = p0[i].y - (0.05 if i % 2 == 0 else -0.03)
			if h < bh:
				bh = h
				best = i
		var v: Vector3 = (p1[best] - p0[best]) / dt
		var hv := Vector2(v.x, v.z).length()
		if bh > hmin + 0.03:
			t += dt
			continue
		sum += hv
		n += 1
		vmin = minf(vmin, hv)
		vmax = maxf(vmax, hv)
		t += dt
	return "stance foot speed: mean %.2f m/s (min %.2f max %.2f) sole min %.3f contact %.0f%%" % [sum / n, vmin, vmax, hmin, 100.0 * n * dt / a.length]
