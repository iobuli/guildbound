extends SceneTree
## For each clip: how far each bone track turns over the clip (max angle from its first key), to spot
## bones that do not move.   Godot --headless --path <guildbound> --script res://tools/track_range.gd -- Clip ...


func _initialize() -> void:
	for c in OS.get_cmdline_user_args():
		var a: Animation = load("res://assets/anims/%s.res" % c)
		var out: Array[String] = []
		for tr in a.get_track_count():
			if a.track_get_type(tr) != Animation.TYPE_ROTATION_3D:
				continue
			var bn := str(a.track_get_path(tr).get_concatenated_subnames())
			if not (bn in ["pelvis", "spine_01", "spine_03", "neck_01", "Head", "thigh_l", "calf_l", "foot_l", "thigh_r", "calf_r", "upperarm_r", "lowerarm_r"]):
				continue
			var q0: Quaternion = a.rotation_track_interpolate(tr, 0.0)
			var mx := 0.0
			var t := 0.0
			while t <= a.length:
				mx = maxf(mx, rad_to_deg(q0.angle_to(a.rotation_track_interpolate(tr, t))))
				t += 1.0 / 30.0
			out.append("%s %d" % [bn, roundi(mx)])
		print("%-14s %.2fs  %s" % [c, a.length, ", ".join(out)])
	quit()
