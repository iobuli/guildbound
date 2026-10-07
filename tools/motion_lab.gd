extends SceneTree
## Motion lab: how smooth does the hero LOOK, per rendered frame? Runs the real game (game/main.tscn),
## drives the player through scripted moves with a puppet Intent and records, on every RENDERED frame,
## what the screen shows: the camera, the body's visible (interpolated) transform and the world positions
## of hand_r, foot_l, foot_r, Head and pelvis. Prints a table per scenario.
##
##   xvfb-run -a -s "-screen 0 1280x720x24" Godot --rendering-driver opengl3 --audio-driver Dummy \
##       --path <guildbound> --resolution 960x540 --fixed-fps 144 --script res://tools/motion_lab.gd \
##       [-- --only=run,startstop,turn,strafe,combo,dodge --csv=<file.csv> --label=<name>]
##
## --fixed-fps 144 makes every frame advance exactly 1/144 s of game time while physics ticks at 60 Hz (the
## 144 Hz monitor case: 2.4 frames per tick), so the numbers do not depend on how fast the machine renders.
## Headless works too (it then samples at the start of the next frame instead of just before drawing).
##
## Columns (lower is better unless noted):
##   cam-stall  % of frames where the body visibly moves but the camera does not (60 Hz camera stepping)
##   pose-stall % of frames where the animated pose (bones relative to the body) does not change while the
##              clip is moving it (the animation is evaluated less often than the screen is drawn)
##   scr-jit    p95 on-screen jerk of the head, px/frame^2 at 540 p (judder of the character on screen;
##              smooth motion ~0.1, a 60 Hz stair-step on a 144 Hz screen several px)
##   body-jit   p95 frame-to-frame change of the body's visible velocity, m/s (should be ~accel*dt)
##   hand-jit   p95 frame-to-frame change of the right hand's velocity relative to the body, m/s
##   pop        largest single-frame bone speed relative to the body, m/s (blend snaps show as spikes)
##   slide      net horizontal travel of a planted foot (ankle within 2 cm of its low, not rising) over
##              each contact / contact time, m/s (foot
##              sliding; ~0 when the stride matches the ground speed)
##   plant      mean per-frame horizontal speed of a planted foot, m/s (sliding plus judder: what the eye
##              sees; a planted foot that steps with the body between 60 Hz poses reads ~body speed)
##   dip        deepest an ankle goes below its contact height, cm (a foot through the floor mid-blend)
##   turn-jit   p95 frame-to-frame change of the body's yaw rate, deg/s
##   max-turn   fastest body yaw rate, deg/s
##
## --gait: no game, a clip report instead. For every locomotion loop the stances use, the travel its FEET
## imply (the planted foot's speed against the body) next to the root speed in clips.json, the number of
## left-foot contacts per loop and where they start. Feet and root disagreeing = sliding at any playback rate.

const DT := 1.0 / 144.0
const BONES := ["hand_r", "foot_l", "foot_r", "Head", "pelvis"]
var main: Node
var p: Player
var ai: FighterAI
var rec := false
var frames: Array = []     # one Dictionary per rendered frame while recording
var results: Array = []
var csv_rows: PackedStringArray = []
var label := ""
var headless := false
var trace := ""            # --trace=<scenario>: print the locomotion drive every 6th frame of it
var cur := ""
var verbose := false
var _drawn := {}           # bone -> skeleton-space position as last drawn (after the skeleton's modifiers)


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	headless = DisplayServer.get_name() == "headless"
	if "--gait" in OS.get_cmdline_user_args():
		_gait()
		quit()
		return
	var only := ""
	var csv := ""
	for a in OS.get_cmdline_user_args():
		if a.begins_with("--only="):
			only = a.substr(7)
		elif a.begins_with("--csv="):
			csv = a.substr(6)
		elif a.begins_with("--label="):
			label = a.substr(8)
		elif a == "--verbose":
			verbose = true
		elif a.begins_with("--trace="):
			trace = a.substr(8)
	main = (load("res://game/main.tscn") as PackedScene).instantiate()
	root.add_child(main)
	current_scene = main
	Sfx.enabled = false
	p = main.get("player")
	ai = FighterAI.new("puppet")
	ai.name = "ai"
	p.add_child(ai)
	var spar: Player = main.get("spar")
	(spar.get_node("ai") as FighterAI).mode = "dummy"
	var dm: Player = main.get("dummy")
	(dm.get_node("ai") as FighterAI).mode = "puppet"
	# the pose that is drawn includes the skeleton modifiers (lean, foot grounding), which the skeleton
	# undoes again before the next frame's animation: capture it when the skeleton has just updated
	p.model.skeleton.skeleton_updated.connect(_capture_pose)
	if headless:
		process_frame.connect(_sample)
	else:
		RenderingServer.frame_pre_draw.connect(_sample)
	await _ticks(40)
	var scen := [["run", s_run], ["startstop", s_startstop], ["turn", s_turn], ["strafe", s_strafe], ["combo", s_combo], ["dodge", s_dodge]]
	for s in scen:
		if only == "" or s[0] in only.split(","):
			var t0 := Time.get_ticks_msec()
			cur = str(s[0])
			await (s[1] as Callable).call()
			_analyse(str(s[0]))
			print("  %s: %d frames recorded (%.1f s wall)" % [s[0], frames.size(), (Time.get_ticks_msec() - t0) / 1000.0])
	_print_table()
	if csv != "":
		var f := FileAccess.open(csv, FileAccess.WRITE)
		if f != null:
			f.store_string("scenario,frame,body_x,body_y,body_z,yaw,cam_x,cam_y,cam_z,head_px_x,head_px_y,foot_l_x,foot_l_y,foot_l_z,foot_r_x,foot_r_y,foot_r_z\n" + "\n".join(csv_rows) + "\n")
	quit()


# ---- driving -----------------------------------------------------------------------------------

func _ticks(n: int) -> void:
	for i in n:
		await physics_frame


func _secs(s: float) -> void:
	await _ticks(int(round(s * 60.0)))


func _put(pos: Vector3, face: Vector3 = Vector3(0, 0, -1)) -> void:
	p.teleport(pos)
	p.facing = face
	p.model.rotation.y = atan2(-face.x, -face.z)
	p.combat.set_stance("sword_shield")
	ai.puppet = Intent.new()
	ai.puppet.view_yaw = atan2(-face.x, -face.z)
	if p.cam != null:
		p.cam.yaw = ai.puppet.view_yaw
		p.cam.pitch = -0.25
	await _ticks(45)


func _spot(n: String) -> Vector3:
	return (main.get("arena") as Arena).spots[n]


func _rec(on: bool) -> void:
	if on:
		frames.clear()
	rec = on


func s_run() -> void:
	await _put(_spot("run_lane"))
	ai.puppet.move = Vector2(0, 1)
	await _secs(0.6)
	_rec(true)
	await _secs(1.6)
	_rec(false)
	ai.puppet.move = Vector2.ZERO


func s_startstop() -> void:
	await _put(_spot("run_lane"))
	_rec(true)
	await _secs(0.2)
	ai.puppet.move = Vector2(0, 1)
	await _secs(1.1)
	ai.puppet.move = Vector2.ZERO
	await _secs(1.0)
	_rec(false)


func s_turn() -> void:
	await _put(_spot("run_lane"))
	ai.puppet.move = Vector2(0, 1)
	await _secs(1.0)
	_rec(true)
	await _secs(0.15)
	ai.puppet.move = Vector2(0, -1)   # a hard 180 on the stick
	await _secs(1.4)
	_rec(false)
	ai.puppet.move = Vector2.ZERO


func s_strafe() -> void:
	var dpos: Vector3 = main.get("DUMMY_POS")
	var dm: Player = main.get("dummy")
	dm.teleport(dpos)
	await _put(dpos + Vector3(0, 0.1, 3.0))
	p.lock.target = dm
	await _secs(0.4)
	ai.puppet.move = Vector2(1, 0)
	await _secs(0.4)
	_rec(true)
	await _secs(1.4)
	ai.puppet.move = Vector2(-1, 0)
	await _secs(1.0)
	_rec(false)
	ai.puppet.move = Vector2.ZERO
	p.lock.target = null


func s_combo() -> void:
	var dpos: Vector3 = main.get("DUMMY_POS")
	var dm: Player = main.get("dummy")
	dm.teleport(dpos)
	await _put(dpos + Vector3(0, 0.1, 1.6))
	p.lock.target = dm
	await _secs(0.3)
	_rec(true)
	for k in 3:
		ai.puppet.just_attack = true
		await _ticks(1)
		ai.puppet.just_attack = false
		await _secs(0.45)
	await _secs(1.0)
	_rec(false)
	p.lock.target = null


func s_dodge() -> void:
	await _put(_spot("run_lane"))
	ai.puppet.move = Vector2(0, 1)
	await _secs(0.6)
	_rec(true)
	ai.puppet.just_dodge = true
	await _ticks(1)
	ai.puppet.just_dodge = false
	await _secs(1.4)
	_rec(false)
	ai.puppet.move = Vector2.ZERO


# ---- sampling ----------------------------------------------------------------------------------

func _sample() -> void:
	if not rec or p == null:
		return
	var body: Transform3D = p.model.get_global_transform_interpolated()
	var sk: Skeleton3D = p.model.skeleton
	var skx: Transform3D = sk.get_global_transform_interpolated()
	var f := {"body": body.origin, "yaw": body.basis.get_euler().y}
	for b: String in BONES:
		var w: Vector3 = skx * (_drawn.get(b, sk.get_bone_global_pose(sk.find_bone(b)).origin) as Vector3)
		f[b] = w
		f[b + "_l"] = body.affine_inverse() * w  # relative to the body: the animated pose itself
	if p.cam != null:
		var cx: Transform3D = p.cam.camera.get_global_transform_interpolated()
		f["cam"] = cx.origin
		var fov := deg_to_rad(p.cam.camera.fov)
		var k := 270.0 / tan(fov * 0.5)  # px at 540 p
		var h: Vector3 = cx.affine_inverse() * (f["Head"] as Vector3)
		f["head_px"] = Vector2(h.x / -h.z, h.y / -h.z) * k
	else:
		f["cam"] = Vector3.ZERO
		f["head_px"] = Vector2.ZERO
	frames.append(f)
	if cur == trace and frames.size() % 6 == 0:
		var m := p.model
		print("  f%3d v=%s bp=%s move=%.2f rate=%.2f act=%.2f lean=(%.3f, %.3f) state=%d" % [frames.size(), str(Vector2(p.velocity.x, p.velocity.z).length()).pad_decimals(2),
			str(m.get("_bp")), float(m.get("_move")), float(m.get("_rate")), float(m.get("_act_w")), float(m._lean.roll) if m.get("_lean") != null else 0.0,
			float(m._lean.pitch) if m.get("_lean") != null else 0.0, p.state])


func _capture_pose() -> void:
	var sk: Skeleton3D = p.model.skeleton
	for b: String in BONES:
		_drawn[b] = sk.get_bone_global_pose(sk.find_bone(b)).origin


# ---- analysis ----------------------------------------------------------------------------------

static func _p95(a: Array) -> float:
	if a.is_empty():
		return 0.0
	var s := a.duplicate()
	s.sort()
	return float(s[mini(s.size() - 1, int(floor(0.95 * s.size())))])


func _analyse(name: String) -> void:
	var n := frames.size()
	if n < 4:
		results.append({"name": name, "n": n})
		return
	var cam_stall := 0
	var moving := 0
	var pose_stall := 0
	var pose_moving := 0
	var scr: Array = []
	var bodyj: Array = []
	var handj: Array = []
	var turnj: Array = []
	var pop := 0.0
	var pop_at := ""
	var max_turn := 0.0
	var plant_sum := 0.0      # per frame: what the eye sees (includes judder)
	var plant_n := 0
	var slide_sum := 0.0      # per contact: net travel of a planted foot over the contact
	var slide_t := 0.0
	# a foot is planted when its ankle is near its contact height and not moving up or down (the heel-down
	# part of a step). Contact height: a low percentile of the heights it holds still at (standing or in
	# a step), so a foot dipping through the floor mid-blend does not set it.
	var ground := {}
	var dips := 0.0
	for ft: String in ["foot_l", "foot_r"]:
		var ys: Array = []
		for i in range(1, n):
			var y1 := (frames[i][ft] as Vector3).y
			if absf(y1 - (frames[i - 1][ft] as Vector3).y) / DT < 0.15:
				ys.append(y1)
		ys.sort()
		var c := float(ys[int(0.3 * ys.size())]) if not ys.is_empty() else 0.0
		ground[ft] = c + 0.025
		for f: Dictionary in frames:
			dips = maxf(dips, c - (f[ft] as Vector3).y)
	var prev_w := 0.0
	for i in range(1, n):
		var a: Dictionary = frames[i - 1]
		var b: Dictionary = frames[i]
		var db := (b["body"] as Vector3) - (a["body"] as Vector3)
		var dc := (b["cam"] as Vector3) - (a["cam"] as Vector3)
		if db.length() > 0.0005:
			moving += 1
			if dc.length() < 0.00002:
				cam_stall += 1
		# the pose relative to the body: did the animation change it this frame?
		var dpose := 0.0
		for bn: String in BONES:
			var d := ((b[bn + "_l"] as Vector3) - (a[bn + "_l"] as Vector3)).length()
			dpose = maxf(dpose, d)
			if d / DT > pop:
				pop = d / DT
				pop_at = "%s@%d" % [bn, i]
		if i >= 2:
			var c: Dictionary = frames[i - 2]
			var dpose_prev := 0.0
			for bn: String in BONES:
				dpose_prev = maxf(dpose_prev, ((a[bn + "_l"] as Vector3) - (c[bn + "_l"] as Vector3)).length())
			if dpose_prev > 0.0015 or dpose > 0.0015:
				pose_moving += 1
				if dpose < 0.00002:
					pose_stall += 1
			var v1 := ((b["body"] as Vector3) - (a["body"] as Vector3)) / DT
			var v0 := ((a["body"] as Vector3) - (c["body"] as Vector3)) / DT
			bodyj.append((v1 - v0).length())
			var h1 := ((b["hand_r_l"] as Vector3) - (a["hand_r_l"] as Vector3)) / DT
			var h0 := ((a["hand_r_l"] as Vector3) - (c["hand_r_l"] as Vector3)) / DT
			handj.append((h1 - h0).length())
			var s2: Vector2 = (b["head_px"] as Vector2) - 2.0 * (a["head_px"] as Vector2) + (c["head_px"] as Vector2)
			scr.append(s2.length())
		var w := wrapf(float(b["yaw"]) - float(a["yaw"]), -PI, PI) / DT
		max_turn = maxf(max_turn, absf(rad_to_deg(w)))
		if i >= 2:
			turnj.append(absf(rad_to_deg(w - prev_w)))
		prev_w = w
		for ft: String in ["foot_l", "foot_r"]:
			var fa: Vector3 = a[ft]
			var fb: Vector3 = b[ft]
			if fb.y < float(ground[ft]) and absf(fb.y - fa.y) / DT < 0.15:
				plant_sum += Vector2(fb.x - fa.x, fb.z - fa.z).length() / DT
				plant_n += 1
		csv_rows.append("%s,%d,%.4f,%.4f,%.4f,%.4f,%.4f,%.4f,%.4f,%.2f,%.2f,%.4f,%.4f,%.4f,%.4f,%.4f,%.4f" % [name, i, (b["body"] as Vector3).x, (b["body"] as Vector3).y, (b["body"] as Vector3).z,
			float(b["yaw"]), (b["cam"] as Vector3).x, (b["cam"] as Vector3).y, (b["cam"] as Vector3).z, (b["head_px"] as Vector2).x, (b["head_px"] as Vector2).y,
			(b["foot_l"] as Vector3).x, (b["foot_l"] as Vector3).y, (b["foot_l"] as Vector3).z, (b["foot_r"] as Vector3).x, (b["foot_r"] as Vector3).y, (b["foot_r"] as Vector3).z])
	for ft: String in ["foot_l", "foot_r"]:
		var i0 := -1
		for i in n + 1:
			var low := i < n and (frames[i][ft] as Vector3).y < float(ground[ft]) and (i == 0 or absf((frames[i][ft] as Vector3).y - (frames[i - 1][ft] as Vector3).y) / DT < 0.15)
			if low and i0 < 0:
				i0 = i
			elif not low and i0 >= 0:
				if i - 1 - i0 >= 4:  # a contact of at least 4 frames
					var fa: Vector3 = frames[i0][ft]
					var fb: Vector3 = frames[i - 1][ft]
					slide_sum += Vector2(fb.x - fa.x, fb.z - fa.z).length()
					slide_t += (i - 1 - i0) * DT
				i0 = -1
	var dist := ((frames[n - 1]["body"] as Vector3) - (frames[0]["body"] as Vector3))
	results.append({"name": name, "n": n, "cam_stall": 100.0 * cam_stall / maxf(1.0, moving), "pose_stall": 100.0 * pose_stall / maxf(1.0, pose_moving),
		"scr": _p95(scr), "bodyj": _p95(bodyj), "handj": _p95(handj), "pop": pop, "slide": slide_sum / maxf(0.001, slide_t), "dip": dips * 100.0, "plant": plant_sum / maxf(1.0, plant_n),
		"turnj": _p95(turnj), "pop_at": pop_at, "max_turn": max_turn, "dist": Vector2(dist.x, dist.z).length()})


func _print_table() -> void:
	print("\n=== MOTION LAB %s (%s, %.0f Hz frames, %d Hz physics) ===" % [label, "headless" if headless else "windowed", 1.0 / DT, Engine.physics_ticks_per_second])
	print("%-10s %5s %9s %10s %8s %8s %8s %7s %6s %6s %5s %9s %9s %6s" % ["scenario", "frames", "cam-stall", "pose-stall", "scr-jit", "body-jit", "hand-jit", "pop", "slide", "plant", "dip", "turn-jit", "max-turn", "dist"])
	for r: Dictionary in results:
		if int(r["n"]) < 4:
			print("%-10s (no frames recorded)" % r["name"])
			continue
		print("%-10s %5d %8.1f%% %9.1f%% %8.2f %8.2f %8.2f %7.1f %6.2f %6.2f %5.1f %9.0f %9.0f %6.2f" % [r["name"], r["n"], r["cam_stall"], r["pose_stall"], r["scr"], r["bodyj"], r["handj"],
			r["pop"], r["slide"], r["plant"], r["dip"], r["turnj"], r["max_turn"], r["dist"]])
		if verbose:
			print("           pop at %s" % r["pop_at"])


# ---- gait report -------------------------------------------------------------------------------

func _gait() -> void:
	var h := HeroAnim.new()
	root.add_child(h)
	h.build()
	var sk := h.skeleton
	var names: Array = []
	for pre: String in ["LOCO_", "SS_", "GS_", "BOW_", "MAG_"]:
		for k: String in HeroAnim.loco_set(pre).values():
			if not names.has(k):
				names.append(k)
	names.append(str(HeroAnim.data["_sprint"]))
	var bl := sk.find_bone("foot_l")
	var br := sk.find_bone("foot_r")
	var rt := sk.find_bone("root")
	print("\n=== GAIT: clip, root speed (json) vs feet, contacts per loop ===")
	print("%-28s %5s %6s %6s %6s %7s %5s %6s %s" % ["clip", "len", "json", "feet", "ratio", "dir-dot", "lcont", "ankle", "lstrike json / found"])
	for n: String in names:
		var a := h.tree.get_animation(n)
		var m: Dictionary = HeroAnim.meta[n]
		var dt := 1.0 / 120.0
		var steps := int(a.length / dt)
		var ys := {bl: [], br: []}
		var ps := {bl: [], br: []}
		for k in steps:
			var pose := h._pose_at(a, k * dt)
			var r: Vector3 = (pose[rt] as Transform3D).origin
			for b: int in [bl, br]:
				var q: Vector3 = (pose[b] as Transform3D).origin - Vector3(r.x, 0.0, r.z)
				(ps[b] as Array).append(q)
				(ys[b] as Array).append(q.y)
		var v_sum := Vector2.ZERO
		var v_n := 0
		var lcont := 0
		var first := -1.0
		for b: int in [bl, br]:
			var y: Array = ys[b]
			var ymin := INF
			for yy: float in y:
				ymin = minf(ymin, yy)
			for k in steps:
				var k0 := (k - 1 + steps) % steps
				var k1 := (k + 1) % steps
				var low: bool = float(y[k]) < ymin + 0.03
				if low and absf(float(y[k1]) - float(y[k0])) / (2.0 * dt) < 0.3 and k > 0 and k < steps - 1:
					var dp: Vector3 = (ps[b][k1] as Vector3) - (ps[b][k0] as Vector3)
					v_sum += Vector2(dp.x, dp.z) / (2.0 * dt)
					v_n += 1
				if b == bl and low and float(y[k0]) >= ymin + 0.03:
					lcont += 1
					if first < 0.0:
						first = k * dt
		var low_y := minf((ys[bl] as Array).min(), (ys[br] as Array).min())
		var feet := -v_sum / maxf(1.0, v_n)   # the body travels against the planted foot
		var js := float(m.get("speed", 0.0))
		var d: Array = m.get("dir", [0.0, 1.0])
		var dd := feet.normalized().dot(Vector2(float(d[0]), float(d[1])))
		print("%-28s %5.2f %6.2f %6.2f %6.2f %7.2f %5d %6.3f %s / %.3f" % [n, a.length, js, feet.length(), feet.length() / maxf(0.01, js), dd, lcont, low_y, str(m.get("lstrike", "-")), first])
