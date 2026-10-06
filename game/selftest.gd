class_name SelfTest
extends Node
## Drives the real player through scripted input and checks the physics numbers.
## Run:  Godot --headless --path <guildbound> -- --selftest        (add --shot for a screenshot, needs a window)

const ACTIONS := ["move_forward", "move_back", "move_left", "move_right", "sprint", "jump", "crouch", "dodge", "interact", "attack"]

var m: Node
var p: Player
var results: Array = []
var probe: AnimProbe
var _prop_home := {}


func frames(n: int) -> void:
	for i in n:
		await get_tree().physics_frame


func press(a: String, strength: float = 1.0) -> void:
	Input.action_press(a, strength)


func release(a: String) -> void:
	Input.action_release(a)


func release_all() -> void:
	for a in ACTIONS:
		Input.action_release(a)


func tp(pos: Vector3, yaw_deg: float) -> void:
	release_all()
	p.teleport(pos)
	p.cam.yaw = deg_to_rad(yaw_deg)
	p.facing = Basis(Vector3.UP, deg_to_rad(yaw_deg)) * Vector3(0.0, 0.0, -1.0)
	p.wet = false
	p.seen_states.clear()
	p.sheet.effects.clear()
	p.sheet.cooldowns.clear()
	p.sheet.invalidate()


func check(name: String, ok: bool, detail: String = "") -> void:
	results.append(ok)
	print("[%s] %s%s" % ["PASS" if ok else "FAIL", name, ("  -  " + detail) if detail != "" else ""])


func run(main: Node) -> void:
	m = main
	p = main.player
	p.cam.sens = 0.0  # real mouse movement must not steer a scripted run
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	await frames(20)
	print("\n=== SELFTEST: Kaelen, Beastkin Hunter lvl 10 ===")
	for pr in m.arena.props.values():
		_prop_home[pr] = pr.global_transform
	var only := ""
	for a in OS.get_cmdline_user_args():
		if a.begins_with("--only="):
			only = a.substr(7)
	var tests := [["parity", t_parity], ["run", t_run_sprint_jump], ["slopes", t_slopes_stairs_vault], ["cliff", t_cliff],
		["gap", t_gap], ["swim", t_swim], ["carry", t_carry], ["fall", t_fall_ragdoll], ["getup", t_getup], ["gdbg", t_gdbg], ["hotbar", t_exhaust_and_hotbar],
		["slide", t_anim_slide], ["pops", t_anim_pops], ["roll", t_anim_roll], ["climbanim", t_anim_climb], ["stairsanim", t_anim_stairs], ["selfcol", t_selfcol], ["combat", t_combat], ["ui", t_ui], ["debug", t_debug]]
	for t in tests:
		if (only == "" and t[0] != "debug" and t[0] != "gdbg") or t[0] in only.split(","):
			await (t[1] as Callable).call()
	var bad := results.count(false)
	print("\n=== %d checks, %d passed, %d failed ===" % [results.size(), results.size() - bad, bad])
	if "--shot" in OS.get_cmdline_user_args() and DisplayServer.get_name() != "headless":
		await screenshot()
	get_tree().quit(1 if bad > 0 else 0)


# ---- tests -----------------------------------------------------------------------------------

func t_parity() -> void:
	print("-- GDScript stat engine matches the Python reference")
	var s := p.sheet
	check("max_hp 782", absf(s.v("max_hp") - 782.4) < 0.5, "%.1f" % s.v("max_hp"))
	check("move_speed 7.80", absf(s.v("move_speed") - 7.8) < 0.01, "%.2f" % s.v("move_speed"))
	check("attack_speed 1.725 (x1.15 ring)", absf(s.v("attack_speed") - 1.725) < 0.005, "%.3f" % s.v("attack_speed"))
	check("armor 39.8", absf(s.v("armor") - 39.8) < 0.3, "%.1f" % s.v("armor"))
	check("carry 68.4", absf(s.v("carry") - 68.4) < 0.1, "%.1f" % s.v("carry"))
	check("stamina max 120", absf(s.v("stamina") - 120.0) < 0.1, "%.0f" % s.v("stamina"))
	s.cast("gale_step")
	check("Gale Step: move_speed hits +60% soft cap = 9.6", absf(s.v("move_speed") - 9.6) < 0.01, "%.2f" % s.v("move_speed"))
	check("Gale Step: attack_speed 2.07", absf(s.v("attack_speed") - 2.07) < 0.01, "%.3f" % s.v("attack_speed"))
	s.dismiss("gale_step")
	s.cooldowns.clear()
	s.cast("quick_step")
	s.cast("gale_step")
	var lines: Array = s.stat("move_speed")["lines"]
	var suppressed := lines.filter(func(l): return "suppressed" in l).size()
	check("Quick Step suppressed by Gale Step (same group)", suppressed == 1, str(suppressed))
	s.effects.clear()
	s.invalidate()


func t_run_sprint_jump() -> void:
	print("-- running, sprinting, jumping (open ground, z=60)")
	tp(Vector3(0, 0.1, 60), 0)
	await frames(20)
	press("move_forward")
	await frames(70)
	var ms := p.sheet.v("move_speed")
	check("run speed == Move Speed stat", absf(p.last_h_speed - ms) < ms * 0.03, "%.2f vs %.2f" % [p.last_h_speed, ms])
	press("sprint")
	await frames(50)
	var want := minf(ms * Player.SPRINT_MULT, Player.SPRINT_CLAMP)
	check("sprint speed == 1.35x (clamped 12)", absf(p.last_h_speed - want) < want * 0.03, "%.2f vs %.2f" % [p.last_h_speed, want])
	var st0 := p.stamina
	await frames(5)
	p.model.rig.tracking_error = 0.0
	p.model.rig.tracking_by_part.clear()
	await frames(60)
	var err := 0.0
	var worst := ""
	var feet := 0.0
	for k in p.model.rig.tracking_by_part:
		var e: float = p.model.rig.tracking_by_part[k]
		if k.begins_with("foot") or k.begins_with("calf"):
			feet = maxf(feet, e)
		elif e > err:
			err = e
			worst = k
	check("sprinting drains stamina (~10/s)", st0 - p.stamina > 6.0, "%.1f -> %.1f" % [st0, p.stamina])
	check("torso, arms and thighs reach their animated targets while sprinting (< 8 cm)", err < 0.08,
		"max %.3f m (%s); lower legs held up by the ground up to %.3f m" % [err, worst, feet])
	release_all()
	await frames(30)

	tp(Vector3(0, 0.1, 60), 0)
	await frames(20)
	var y0 := p.global_position.y
	var top := y0
	press("jump")
	for i in 45:
		await get_tree().physics_frame
		top = maxf(top, p.global_position.y)
	release_all()
	var h := p.sheet.v("jump_height")
	check("jump apex == Jump Height stat", absf((top - y0) - h) < 0.15, "%.2f vs %.2f m" % [top - y0, h])
	await frames(40)

	tp(Vector3(0, 0.1, 60), 0)
	await frames(20)
	press("jump")
	await frames(2)
	release("jump")
	await frames(60)
	check("tapping jump gives a short hop (variable jump)", p.global_position.y < 0.5 and p.is_on_floor(), "y=%.2f" % p.global_position.y)


func max_y_while(frames_n: int) -> float:
	var top := p.global_position.y
	for i in frames_n:
		await get_tree().physics_frame
		top = maxf(top, p.global_position.y)
	return top


func t_slopes_stairs_vault() -> void:
	print("-- slopes, stairs, vault")
	tp(Vector3(-30, 0.1, -2), 0)
	await frames(15)
	press("move_forward")
	var top := await max_y_while(420)
	check("30 deg ramp: walks up (6 m top)", top > 5.0, "max y %.1f" % top)

	tp(Vector3(-25, 0.1, -2), 0)
	await frames(15)
	press("move_forward")
	top = await max_y_while(420)
	check("45 deg ramp: walks up (8.5 m top)", top > 7.5, "max y %.1f" % top)

	tp(Vector3(-20, 0.1, -2), 0)
	await frames(15)
	press("move_forward")
	top = await max_y_while(300)
	check("55 deg ramp: too steep to walk", top < 2.5 and not p.seen_states.has(Player.State.CLIMB), "max y %.1f" % top)

	tp(Vector3(-15, 0.1, -2.5), 0)
	await frames(15)
	press("move_forward")
	press("jump")
	await frames(3)
	release("jump")
	top = await max_y_while(700)
	check("75 deg ramp: jump into it to climb", p.seen_states.has(Player.State.CLIMB), "seen CLIMB=%s" % p.seen_states.has(Player.State.CLIMB))
	check("75 deg ramp: climbs to the top (11.6 m)", top > 10.0, "max y %.1f" % top)

	tp(Vector3(-8, 0.1, -2), 0)
	await frames(15)
	press("move_forward")
	top = await max_y_while(300)
	check("stairs: 10 x 0.3 m steps auto-stepped", top > 2.8 and top < 3.4, "max y %.2f" % top)
	tp(Vector3(-8, 0.1, -3.0), 0)
	await frames(15)
	press("move_forward")
	var sink := 0.0  # how far a foot went below the top of the step it is over
	for i in 70:
		await get_tree().physics_frame
		for nm in ["foot_l", "foot_r"]:
			var fp: Vector3 = p.model.rig.by_name[nm]["body"].global_position
			var hit := p._ray(fp + Vector3.UP * 1.0, fp + Vector3.DOWN * 0.5)
			if not hit.is_empty() and fp.z < -5.6 and fp.z > -13.6:
				sink = minf(sink, fp.y - hit["position"].y)
	release_all()
	check("stairs: feet never sink into a step", sink > -0.06, "deepest %.3f m" % sink)
	# the drawn skeleton keeps real bone lengths (no stretched arms or legs)
	var sk: Skeleton3D = p.model.skeleton
	var worst_stretch := 0.0
	for pair in [["upperarm_l", "lowerarm_l"], ["lowerarm_l", "hand_l"], ["upperarm_r", "lowerarm_r"], ["lowerarm_r", "hand_r"], ["thigh_l", "calf_l"], ["calf_l", "foot_l"], ["thigh_r", "calf_r"], ["calf_r", "foot_r"]]:
		var a := sk.find_bone(pair[0])
		var b := sk.find_bone(pair[1])
		var rest := sk.get_bone_global_rest(a).origin.distance_to(sk.get_bone_global_rest(b).origin)
		var now := sk.get_bone_global_pose(a).origin.distance_to(sk.get_bone_global_pose(b).origin)
		worst_stretch = maxf(worst_stretch, absf(now - rest))
	check("bones keep their length (no stretched limbs)", worst_stretch < 0.01, "worst %.3f m" % worst_stretch)

	tp(Vector3(-4, 0.1, 0), 0)
	await frames(15)
	press("move_forward")
	press("sprint")
	top = await max_y_while(120)
	check("sprint into a 1.0 m block vaults it", top > 0.95 and p.global_position.z < -10.4, "max y %.2f, z %.1f" % [top, p.global_position.z])


func t_cliff() -> void:
	print("-- 20 m cliff: stamina is the limiter")
	tp(Vector3(16, 0.1, -29.0), 0)
	await frames(15)
	press("move_forward")
	press("jump")
	await frames(3)
	release("jump")
	var top := p.global_position.y
	var deep_part := ""
	var palm_gap := 0.0   # how far the palms sit off the wall while climbing (worst)
	var spread := 99.0    # hand-to-hand distance while climbing (smallest)
	var deepest := 0.0  # how far any hand/head/chest part pushed into the wall (wall face is z = -30)
	for i in 1500:
		await get_tree().physics_frame
		top = maxf(top, p.global_position.y)
		if p.state == Player.State.CLIMB and p.global_position.y > 2.0 and p.global_position.y < 17.0:
			var hl: Vector3 = p.model.rig.by_name["hand_l"]["body"].global_position
			var hr: Vector3 = p.model.rig.by_name["hand_r"]["body"].global_position
			spread = minf(spread, hl.distance_to(hr))
			palm_gap = maxf(palm_gap, maxf(hl.z, hr.z) - (-30.0))
		if p.state == Player.State.CLIMB:
			for nm in ["hand_l", "hand_r", "head", "chest", "lowerarm_l", "lowerarm_r"]:
				var bp: Vector3 = p.model.rig.by_name[nm]["body"].global_position
				var bz: float = bp.z
				if bp.y < 19.7 and bz - (-30.0) < deepest:  # above 20 m the hands are over the lip, not in the wall
					deepest = bz - (-30.0)
					deep_part = nm
		if p.global_position.y > 19.8 and p.is_on_floor():
			break
	release_all()
	await frames(10)
	check("climbing: arms spread wider than the shoulders", spread > 0.6, "closest hands %.2f m apart" % spread)
	check("climbing: palms rest on the wall", palm_gap < 0.2, "wrists at most %.2f m off the wall" % palm_gap)
	check("climbing: no body part passes into the wall", deepest > -0.05, "deepest %.3f m (%s)" % [deepest, deep_part])
	check("dry: climbs the full 20 m and mantles onto the top", top > 19.5 and p.global_position.y > 19.5 and p.is_on_floor(), "max y %.1f, y %.1f, stamina left %.0f" % [top, p.global_position.y, p.stamina])

	tp(Vector3(16, 0.1, -29.0), 0)
	p.wet = true
	await frames(15)
	press("move_forward")
	press("jump")
	await frames(3)
	release("jump")
	top = await max_y_while(1500)
	check("rain: climb costs x2, runs out of stamina before the top", top > 8.0 and top < 18.0, "max y %.1f" % top)

	tp(Vector3(28, 0.1, -29.0), 0)
	await frames(15)
	press("move_forward")
	press("jump")
	await frames(3)
	release("jump")
	top = await max_y_while(300)
	check("ice wall cannot be climbed", not p.seen_states.has(Player.State.CLIMB) and top < 3.0, "max y %.1f" % top)


func t_gap() -> void:
	print("-- 8 m pit: needs a speed buff (naked Beastkin has no gear)")
	var kit := p.sheet
	p.sheet = CharModel.new("Naked", "beastkin", 10)
	var landed_x: Array = []
	for buffed in [false, true]:
		tp(Vector3(30, 0.1, 0), -90)
		if buffed:
			p.sheet.cast("gale_step")
		await frames(15)
		press("move_forward")
		press("sprint")
		var jumped := false
		for i in 400:
			await get_tree().physics_frame
			if not jumped and p.global_position.x >= 43.9:
				press("jump")
				jumped = true
				await frames(100)
				break
		release_all()
		await frames(20)
		landed_x.append([p.global_position.x, p.global_position.y])
	var a: Array = landed_x[0]
	var b: Array = landed_x[1]
	check("without a buff the pit swallows you", a[1] < -1.0, "x %.1f y %.1f" % [a[0], a[1]])
	check("with Gale Step you clear it", b[0] > 52.0 and b[1] > -0.5, "x %.1f y %.1f" % [b[0], b[1]])
	p.sheet = kit


func t_swim() -> void:
	print("-- pond")
	tp(Vector3(-38, 0.1, 0), 90)
	await frames(15)
	press("move_forward")
	await frames(120)
	check("walking off the edge enters SWIM", p.seen_states.has(Player.State.SWIM) and p.state == Player.State.SWIM, "state %d y %.2f" % [p.state, p.global_position.y])
	check("no fall damage into deep water", p.sheet.hp >= p.sheet.v("max_hp") - 1.0, "hp %.0f" % p.sheet.hp)
	var st := p.stamina
	await frames(120)
	check("swimming costs stamina", p.stamina < st or p.stamina < p.sheet.v("stamina") - 1.0, "%.0f" % p.stamina)
	var guard := 0
	while p.global_position.x > -58.8 and guard < 800:
		await get_tree().physics_frame
		guard += 1
	press("jump")
	await frames(3)
	release("jump")
	await frames(150)
	release_all()
	check("hop out of the pond onto the bank", p.global_position.y > -0.2 and p.global_position.x < -59.9, "x %.1f y %.2f state %d" % [p.global_position.x, p.global_position.y, p.state])


func t_carry() -> void:
	print("-- carry and push (kg are the same units as the stat sheet)")
	tp(Vector3(6, 0.1, -0.5), 0)
	await frames(30)
	press("interact")
	await frames(25)
	check("60 kg crate is liftable (carry 68.4)", p.carried != null and p.carried.mass == 60.0, "carried=%s" % str(p.carried))
	release("interact")
	await frames(10)

	tp(Vector3(8, 0.1, -0.5), 0)
	await frames(30)
	press("interact")
	await frames(25)
	check("80 kg crate is too heavy without a buff", p.carried == null)
	release("interact")
	await frames(5)
	p.sheet.cast("bright_valor")
	check("Bright Valor raises Carry to 83.4", absf(p.sheet.v("carry") - 83.4) < 0.1, "%.1f" % p.sheet.v("carry"))
	press("interact")
	await frames(25)
	check("same 80 kg crate liftable with Might buff", p.carried != null and p.carried.mass == 80.0)
	press("attack")
	await frames(2)
	release("attack")
	await frames(5)
	release_all()
	var crate: RigidBody3D = m.arena.props["Crate 80 kg"]
	check("LMB throws the carried crate", p.carried == null and crate.linear_velocity.length() > 3.0, "v=%.1f" % crate.linear_velocity.length())
	await frames(90)

	tp(Vector3(10, 0.1, 2.5), 0)
	await frames(30)
	var boulder: RigidBody3D = m.arena.props["Boulder 200 kg"]
	var z0 := boulder.global_position.z
	press("move_forward")
	await frames(120)
	release_all()
	check("200 kg boulder can be pushed but not lifted", p.carried == null and absf(boulder.global_position.z - z0) > 0.5, "moved %.2f m" % absf(boulder.global_position.z - z0))


## A one-shot key press: held for a few physics frames (the Player reads it as just_pressed), then let go.
func press_event(action: String) -> void:
	Input.action_press(action)
	get_tree().create_timer(0.05, true, true).timeout.connect(func() -> void: Input.action_release(action))


func wait_until(cond: Callable, max_frames: int) -> int:
	var n := 0
	while n < max_frames and not cond.call():
		await get_tree().physics_frame
		n += 1
	return n


func t_fall_ragdoll() -> void:
	print("-- no glider; hard falls ragdoll, then you get up")
	tp(Vector3(16, 20.2, -32.5), 180)
	await frames(30)
	press("move_forward")
	await wait_until(func(): return p.state == Player.State.AIR and p.air_time > 0.3, 200)
	press("jump")
	await frames(6)
	release_all()
	check("Space in the air does nothing (no glider)", p.state == Player.State.AIR and p.velocity.y < -3.0, "vy %.1f" % p.velocity.y)
	await wait_until(func(): return p.state == Player.State.RAGDOLL, 300)
	check("a 20 m fall knocks you into a ragdoll", p.state == Player.State.RAGDOLL)
	check("the hard landing cost HP", p.sheet.hp < p.sheet.v("max_hp") - 50.0, "hp %.0f" % p.sheet.hp)
	check("the capsule is off while ragdolled", p.shape.disabled)
	var n := await wait_until(func(): return p.state == Player.State.GROUND, 600)
	check("the ragdoll settles and you get back up", p.state == Player.State.GROUND, "after %.1fs, get-up: %s" % [n / 60.0, p.recover_kind])
	check("capsule back on, standing near where you fell", not p.shape.disabled and p.global_position.y < 1.0 and p.global_position.z > -31.0,
		"pos %s" % str(p.global_position.snappedf(0.1)))

	tp(Vector3(0, 0.1, 60), 0)
	await frames(20)
	press("move_forward")
	press("sprint")
	await frames(40)
	press_event("ragdoll")
	await frames(3)
	check("G throws you into a ragdoll mid-sprint", p.state == Player.State.RAGDOLL)
	release_all()
	n = await wait_until(func(): return p.state == Player.State.GROUND, 600)
	check("...and you recover", p.state == Player.State.GROUND, "after %.1fs, get-up: %s" % [n / 60.0, p.recover_kind])

	tp(Vector3(100, 12.0, 100), 0)
	await wait_until(func(): return p.air_time > 0.3, 100)
	await wait_until(func(): return p.is_on_floor() or p.state == Player.State.RAGDOLL, 300)
	await frames(5)
	var lost := p.sheet.v("max_hp") - p.sheet.hp
	var expect := p.sheet.v("max_hp") * 0.06 * (12.0 - Player.SAFE_FALL)
	check("12 m fall: 8 m safe, then 6% max HP per metre", absf(lost - expect) < 40.0, "lost %.0f, expected ~%.0f" % [lost, expect])
	await wait_until(func(): return p.state == Player.State.GROUND, 600)


func t_getup() -> void:
	print("-- get-up: from wherever the ragdoll lies; contacts hold, nothing sinks, no snaps")
	await ensure_probe()
	var sk: Skeleton3D = p.model.skeleton
	var gu: GetUpPose = p.model.getup
	for a in OS.get_cmdline_user_args():
		if a == "--nosettle":
			gu.settle = false
	var kinds := {}
	await fresh(Vector3(0, 0.1, 60), 0)
	var base := 0.0
	for i in 30:
		await get_tree().physics_frame
		for part in p.model.rig.parts:
			base = minf(base, part_lowest(part))
	print("   standing still, the lowest body part is %.3f m into the floor (contact slop)" % base)
	var flat := Vector3(0, 0.1, 60)
	for c in [["thrown forward", Vector3(0, 0, -4), flat], ["thrown backward", Vector3(0, 0, 4), flat],
			["sprint + G", Vector3.ZERO, flat], ["thrown left", Vector3(-4, 0, 0), flat], ["thrown right", Vector3(4, 0, 0), flat],
			["spun forward", Vector3(1.5, 0, -6), flat], ["sprint + G at z=40", Vector3.ZERO, Vector3(0, 0.1, 40)],
			["down the 30 deg slope", Vector3(0, 0, 3), Vector3(-30, 4.0, -10)], ["across the 30 deg slope", Vector3(3, 0, 0), Vector3(-30, 4.0, -10)],
			["on the stairs", Vector3(0, 0, 3), Vector3(-8, 2.2, -9.5)], ["into the cliff", Vector3(0, 0, -4), Vector3(16, 0.1, -28.6)]]:
		await fresh(c[2], 0)
		if c[1] == Vector3.ZERO:
			press("move_forward")
			press("sprint")
			await frames(40)
			press_event("ragdoll")
			await frames(2)
			release_all()
		else:
			p.start_ragdoll(Vector3(c[1].x, 1.5, c[1].z))
		await wait_until(func() -> bool: return p.state == Player.State.RECOVER, 600)
		kinds[p.recover_kind] = true
		probe.reset()
		var sink := 0.0
		var sink_part := ""
		var slide := 0.0
		var slide_limb := ""
		var held := {}
		var n := 0
		var when := {}
		var trace := "--getuptrace" in OS.get_cmdline_user_args()
		var frames_log := {}
		var hist := {}         # part -> [[body pos, target pos, contacts], ...] last 3 frames
		var plan_jerk := 0.0
		var plan_at := ""
		var body_jerk := 0.0
		var body_at := ""
		var touch := {}
		for part in p.model.rig.parts:
			(part["body"] as RigidBody3D).contact_monitor = true
			(part["body"] as RigidBody3D).max_contacts_reported = 8
		while p.state == Player.State.RECOVER and n < 400:
			await get_tree().physics_frame
			n += 1
			probe.sample()
			when[probe._n] = gu.t
			for part in p.model.rig.parts:
				var bb: RigidBody3D = part["body"]
				var nm: String = part["name"]
				var tg: Vector3 = p.model.rig._last_target.get(nm, bb.global_position)
				var cs: Array = bb.get_colliding_bodies().map(func(o: Node) -> String: return str(o.name))
				var h: Array = hist.get(nm, [])
				h.append([bb.global_position, tg, cs])
				if h.size() > 3:
					h.pop_front()
				hist[nm] = h
				if h.size() == 3 and gu.t > 0.1:
					var tj: float = ((h[2][1] as Vector3) - 2.0 * (h[1][1] as Vector3) + (h[0][1] as Vector3)).length()
					if tj > plan_jerk:
						plan_jerk = tj
						plan_at = "%s at %.2fs" % [nm, gu.t]
					var bj: float = ((h[2][0] as Vector3) - 2.0 * (h[1][0] as Vector3) + (h[0][0] as Vector3)).length()
					var new_contact := false
					for o in (h[2][2] as Array) + (h[1][2] as Array):
						if not (h[0][2] as Array).has(o):
							new_contact = true
					if bj > body_jerk and not new_contact:
						body_jerk = bj
						body_at = "%s at %.2fs" % [nm, gu.t]
			if trace:
				var fr := {}
				for part in p.model.rig.parts:
					var bb: RigidBody3D = part["body"]
					var tg: Vector3 = p.model.rig._last_target.get(part["name"], Vector3.ZERO)
					fr[part["name"]] = [bb.global_position, tg, bb.linear_velocity.length(),
						bb.get_colliding_bodies().map(func(o: Node) -> String: return str(o.name).replace("Part_", ""))]
				frames_log[probe._n] = [gu.t, fr]
			for part in p.model.rig.parts:
				for o in (part["body"] as RigidBody3D).get_colliding_bodies():
					if str(o.name).begins_with("Part_") and str(part["name"]) < str(o.name).substr(5):
						var key := "%s-%s" % [part["name"], str(o.name).substr(5)]
						if not touch.has(key):
							touch[key] = [gu.t, gu.t, 0]
						touch[key][1] = gu.t
						touch[key][2] += 1
			for part in p.model.rig.parts:
				var bp: Vector3 = (part["body"] as RigidBody3D).global_position
				var gh := p._ray(bp + Vector3.UP * 0.5, bp + Vector3.DOWN * 2.0)
				var lo := part_lowest(part) - ((gh["position"] as Vector3).y if not gh.is_empty() else 0.0)
				if lo < sink:
					sink = lo
					sink_part = "%s at %.2fs" % [part["name"], gu.t]
			var d: Dictionary = p.model.ragdoll_pose.drawn
			for limb in GetUpPose.LIMBS:
				var end: String = GetUpPose.LIMBS[limb][2]
				if gu.is_planted(limb) and d.has(sk.find_bone(end)):
					var at: Vector3 = (d[sk.find_bone(end)] as Transform3D).origin
					if not held.has(limb):
						held[limb] = at
					var drift: float = (held[limb] as Vector3).distance_to(at)
					if drift > slide:
						slide = drift
						slide_limb = "%s at %.2fs, out of reach by %.3f m, body off its target by %.3f m" % [limb, gu.t,
							gu.short.get(limb, 0.0), (p.model.rig.by_name[end.replace("foot", "foot").replace("hand", "hand")]["body"] as RigidBody3D).global_position.distance_to(sk.global_transform * (p.model.rig.targets[sk.find_bone(end)] as Transform3D).origin)]
				else:
					held.erase(limb)
		if trace and frames_log.has(probe.pop_at):
			print("   trace %s: worst %s at frame %d" % [c[0], probe.pop_part, probe.pop_at])
			for fnum in range(probe.pop_at - 3, probe.pop_at + 2):
				if frames_log.has(fnum):
					var e: Array = frames_log[fnum][1][probe.pop_part]
					print("     f%d t=%.2f pos %s target %s off %.3f v %.2f touching %s" % [fnum, frames_log[fnum][0],
						str((e[0] as Vector3).snappedf(0.01)), str((e[1] as Vector3).snappedf(0.01)), (e[0] as Vector3).distance_to(e[1]), e[2], str(e[3])])
		var longest := ""
		for k in touch:
			if longest == "" or touch[k][2] > touch[longest][2]:
				longest = k
		print("   get-up %s: %d body-part pairs touched%s" % [p.recover_kind, touch.size(),
			"" if longest == "" else ", longest %s for %d frames" % [longest, touch[longest][2]]])
		for part in p.model.rig.parts:
			(part["body"] as RigidBody3D).contact_monitor = false
		await frames(20)
		var feet := 0.0
		for s in ["l", "r"]:
			var fp: Vector3 = (p.model.ragdoll_pose.drawn[sk.find_bone("foot_" + s)] as Transform3D).origin
			var fh := p._ray(fp + Vector3.UP * 0.5, fp + Vector3.DOWN * 2.0)
			feet = maxf(feet, absf(fp.y - ((fh["position"] as Vector3).y if not fh.is_empty() else 0.0) - probe.ankle_h))
		var tag := "%s, %s" % [c[0], p.recover_kind]
		var on_flat: bool = c[2] == flat or c[2].y < 0.5
		check("get-up (%s): nothing sinks into the floor (< 2 cm)" % tag, sink > -0.02 or not on_flat, "deepest %.3f m (%s)" % [sink, sink_part])
		check("get-up (%s): planted palms / knees / feet hold still (< 2 cm)" % tag, slide < 0.02, "worst %.3f m (%s)" % [slide, slide_limb])
		check("get-up (%s): the plan never jumps (target < 6 cm/frame change)" % tag, plan_jerk < 0.06, "%.3f m (%s)" % [plan_jerk, plan_at])
		check("get-up (%s): no snaps except on impact (< 6 cm/frame change)" % tag, body_jerk < 0.06,
			"%.3f m (%s); raw incl. impacts %.3f m (%s at %.2fs)" % [body_jerk, body_at, probe.pop, probe.pop_part, when.get(probe.pop_at, -1.0)])
		check("get-up (%s): ends standing, feet on the floor" % tag, p.state == Player.State.GROUND and (feet < 0.03 or not on_flat),
			"state %d, feet %.3f m off" % [p.state, feet])
	check("get-up: both face-down and face-up get-ups were exercised", kinds.has("front") and kinds.has("back"), str(kinds.keys()))


func t_gdbg() -> void:
	await ensure_probe()
	var gu: GetUpPose = p.model.getup
	var rig: BodyRig = p.model.rig
	var sk: Skeleton3D = p.model.skeleton
	await fresh(Vector3(0, 0.1, 60), 0)
	p.start_ragdoll(Vector3(0, 1.5, -4))
	await wait_until(func() -> bool: return p.state == Player.State.RECOVER, 600)
	for k in gu._keys:
		print("key t %.2f o %s limbs %s" % [k["t"], (k["o"] as Vector3).snappedf(0.01), (k["limbs"] as Dictionary).keys().map(func(l): return "%s:%s" % [l, k["limbs"][l]["mode"]])])
	for part in rig.parts:
		(part["body"] as RigidBody3D).contact_monitor = true
		(part["body"] as RigidBody3D).max_contacts_reported = 8
	var hist := {}
	while p.state == Player.State.RECOVER:
		await get_tree().physics_frame
		for part in rig.parts:
			var nm: String = part["name"]
			var b: RigidBody3D = rig.by_name[nm]["body"]
			var tg: Vector3 = sk.global_transform * (rig.targets[rig.by_name[nm]["bone"]] as Transform3D).origin
			var h: Array = hist.get(nm, [])
			h.append(b.global_position)
			hist[nm] = h
			if h.size() >= 3:
				var a: float = ((h[-1] as Vector3) - 2.0 * (h[-2] as Vector3) + (h[-3] as Vector3)).length()
				if a > 0.06 and gu.t > 1.7 and gu.t < 2.2:
					var hits := (b.get_colliding_bodies()).map(func(o): return o.name)
					print("t %.2f %s pop %.3f  pos %s target %s off %.3f  v %.2f  touching %s" % [gu.t, nm, a, b.global_position.snappedf(0.01),
						tg.snappedf(0.01), b.global_position.distance_to(tg), b.linear_velocity.length(), hits])


func t_exhaust_and_hotbar() -> void:
	print("-- buffs through the hotbar path")
	tp(Vector3(0, 0.1, 60), 0)
	await frames(20)
	var ms0 := p.sheet.v("move_speed")
	var r := p.sheet.cast("shadowrush")
	check("Shadowrush casts", r[0])
	press("move_forward")
	press("sprint")
	await frames(90)
	var cap := 6.0 * 1.6 + 0.0
	check("Dark speed obeys the Move Speed soft/hard caps (sprint <= 12)", p.last_h_speed <= Player.SPRINT_CLAMP + 0.05, "%.2f m/s" % p.last_h_speed)
	var hp0 := p.sheet.hp
	await frames(120)
	check("Dark upkeep drains HP", p.sheet.hp < hp0 + 4.0 or p.sheet.mana < p.sheet.v("max_mana"), "hp %.0f" % p.sheet.hp)
	p.sheet.dismiss("shadowrush")
	var ms1 := p.sheet.v("move_speed")
	check("dismissing leaves Exhaustion backlash (-25%)", ms1 < ms0, "%.2f < %.2f" % [ms1, ms0])
	release_all()
	await frames(10)


# ---- animation quality (measured on the physical body) ----------------------------------------

func reset_props() -> void:
	for pr: RigidBody3D in _prop_home:
		pr.global_transform = _prop_home[pr]
		pr.linear_velocity = Vector3.ZERO
		pr.angular_velocity = Vector3.ZERO
		pr.freeze = false


func ensure_probe() -> void:
	if probe == null:
		probe = AnimProbe.new(p)
		tp(Vector3(0, 0.1, 60), 0)
		await frames(60)
		probe.calibrate()
		print("   probe: planted ankle %.3f m, ball %.3f m above ground" % [probe.ankle_h, probe.ball_h])


## Sample the probe for n physics frames (only frames where cond holds count for sliding).
func probe_frames(n: int, cond: Callable = Callable()) -> void:
	for i in n:
		await get_tree().physics_frame
		probe.sample(1.0 / 60.0, cond.is_null() or cond.call())


func slide_case(label: String, pos: Vector3, yaw: float, held: Dictionary, warm: int, n: int, cond: Callable = Callable(), crouch: bool = false) -> float:
	reset_props()
	tp(pos, yaw)
	await frames(20)
	if crouch:
		p._set_crouch(true)
	for a in held:
		press(a, held[a])
	await frames(warm)
	probe.reset()
	var spd := 0.0
	var cnt := 0
	for i in n:
		await get_tree().physics_frame
		var c: bool = cond.is_null() or cond.call()
		probe.sample(1.0 / 60.0, c)
		if c:
			spd += p.last_h_speed
			cnt += 1
	release_all()
	p._set_crouch(false)
	var s := probe.slide_speed()
	print("   slide %-16s planted foot %.3f m/s (peak %.2f, stance %.2f s) at body speed %.2f m/s" % [label, s, probe.slide_peak, probe.stance_t, spd / maxf(1, cnt)])
	return s


## Analog stick deflection that gives `f` of full speed (get_vector applies the 0.2 action deadzone).
static func stick(f: float) -> float:
	return 0.2 + 0.8 * f


func t_debug() -> void:
	await ensure_probe()
	var skk: Skeleton3D = p.model.skeleton
	print("ball rest local: ", skk.get_bone_rest(skk.find_bone("ball_l")).origin, " foot len ", probe.foot_len)
	await fresh(Vector3(0, 0.1, 60), 0)
	var mode: String = "run"
	for a in OS.get_cmdline_user_args():
		if a.begins_with("--dbg="):
			mode = a.substr(6)
	match mode:
		"run":
			press("move_forward")
		"sprint":
			press("move_forward")
			press("sprint")
		"crouch":
			p._set_crouch(true)
			press("move_forward")
		"push":
			reset_props()
			tp(Vector3(10, 0.1, 2.5), 0)
			press("move_forward")
		"runnocol":
			for nm in ["foot_l", "foot_r"]:
				(p.model.rig.by_name[nm]["body"] as RigidBody3D).collision_mask = 0
			press("move_forward")
		"walk":
			press("move_forward", stick(0.1923))
		"slow":
			press("move_forward", stick(0.0962))
		"stairs":
			tp(Vector3(-8, 0.1, -3.0), 0)
			await frames(20)
			press("move_forward")
		"stairswalk":
			tp(Vector3(-8, 0.1, -3.0), 0)
			await frames(20)
			press("move_forward", stick(0.1923))
		"stairsdown":
			tp(Vector3(-8, 3.15, -13.6), 180)
			await frames(20)
			press("move_forward")
	await frames(40 if not mode.begins_with("stairs") else 5)
	var inv: Transform3D
	var prevp := {}
	for i in 60:
		if mode == "turn":
			if i == 2:
				press("move_back")
			if i == 4:
				release("move_back")
		await get_tree().physics_frame
		inv = p.model.global_transform.affine_inverse()
		var line := "%3d z%.2f y%.2f so%.2f st%d v%.1f sc %.2f g %.2f yaw %.0f " % [i, p.global_position.z, p.global_position.y, p.step_offset, p.state, p.last_h_speed, p.model.tree.get("parameters/loco_speed/scale"), p.model.tree.get("parameters/ground/blend_position"), rad_to_deg(p.model.rotation.y)]
		for side in ["l", "r"]:
			var pts := probe._points(side)
			var ha: float = pts[0].y - probe._ground(pts[0]) - probe.ankle_h
			var hb: float = pts[1].y - probe._ground(pts[1]) - probe.ball_h
			var rel: Vector3 = inv * (pts[0] as Vector3)
			var tgt: Vector3 = inv * (p.model.rig._last_target.get("foot_" + side, Vector3.ZERO) as Vector3)
			var ftst: Dictionary = p.model.ik._ft[side]
			var cp: Vector3 = pts[0] if ha <= hb else pts[1]
			var spd := 0.0
			if prevp.has(side):
				spd = Vector2(cp.x - prevp[side].x, cp.z - prevp[side].z).length() * 60.0
			prevp[side] = cp
			line += " v%.2f%s" % [spd, "b" if ha > hb else "h"]
			var pl: Vector3 = inv * (ftst["plant"] as Vector3)
			var fbd: RigidBody3D = p.model.rig.by_name["foot_" + side]["body"]
			var bone: int = p.model.rig.by_name["foot_" + side]["bone"]
			var tb: Basis = (p.model.skeleton.global_transform * (p.model.rig.targets[bone] as Transform3D)).basis
			var pitch := rad_to_deg(asin(clampf(fbd.global_basis.y.normalized().y, -1, 1)))
			var tpitch := rad_to_deg(asin(clampf(tb.y.normalized().y, -1, 1)))
			line += "| %s ha %.3f hb %.3f y %.2f ty %.2f pit %.0f/%.0f %s%s%s " % [side, ha, hb, rel.y, tgt.y, pitch, tpitch,
				"P" if ftst["planted"] else "-", "S" if ftst["step"] >= 0.0 else " ", "W" if ftst["wait_up"] else " "]
		print(line)
	release_all()


func t_anim_slide() -> void:
	print("-- foot sliding: a planted foot should stay put")
	await ensure_probe()
	var r := {}
	var ground := func() -> bool: return p.state == Player.State.GROUND
	r["walk"] = await slide_case("walk", Vector3(0, 0.1, 60), 0, {"move_forward": stick(0.1923)}, 40, 120, ground)
	r["slow walk"] = await slide_case("slow walk", Vector3(0, 0.1, 60), 0, {"move_forward": stick(0.0962)}, 40, 120, ground)
	r["run"] = await slide_case("run", Vector3(0, 0.1, 60), 0, {"move_forward": 1.0}, 40, 90, ground)
	r["sprint"] = await slide_case("sprint", Vector3(0, 0.1, 60), 0, {"move_forward": 1.0, "sprint": 1.0}, 50, 90, ground)
	r["crouch"] = await slide_case("crouch walk", Vector3(0, 0.1, 60), 0, {"move_forward": 1.0}, 40, 90, ground, true)
	r["carry"] = await slide_case("carry walk", Vector3(4, 0.1, -0.5), 0, {"interact": 1.0}, 40, 1)
	press("interact")
	press("move_back")
	await frames(40)
	probe.reset()
	await probe_frames(90, ground)
	release_all()
	r["carry"] = probe.slide_speed()
	print("   slide %-16s planted foot %.3f m/s (peak %.2f) at body speed %.2f" % ["carry walk", r["carry"], probe.slide_peak, p.last_h_speed])
	r["push"] = await slide_case("push", Vector3(10, 0.1, 2.5), 0, {"move_forward": 1.0}, 50, 90, func() -> bool: return p.pushing)
	var on_stairs := func() -> bool: return p.state == Player.State.GROUND and p.global_position.z < -5.8 and p.global_position.z > -13.2
	r["stairs up"] = await slide_case("stairs up", Vector3(-8, 0.1, -3.0), 0, {"move_forward": 1.0}, 5, 80, on_stairs)
	r["stairs down"] = await slide_case("stairs down", Vector3(-8, 3.15, -13.6), 180, {"move_forward": 1.0}, 5, 80, on_stairs)
	r["walk stairs up"] = await slide_case("walk stairs up", Vector3(-8, 0.1, -3.0), 0, {"move_forward": stick(0.1923)}, 5, 200, on_stairs)
	r["walk stairs down"] = await slide_case("walk stairs down", Vector3(-8, 3.15, -13.6), 180, {"move_forward": stick(0.1923)}, 5, 200, on_stairs)
	# turning on the spot: tap a direction, let the body swing round
	tp(Vector3(0, 0.1, 60), 0)
	await frames(40)
	probe.reset()
	for dir in ["move_back", "move_left", "move_right", "move_forward"]:
		press(dir)
		await probe_frames(2)
		release(dir)
		await probe_frames(45)
	r["turn"] = probe.slide_dist
	print("   slide %-16s planted feet slid %.3f m in total over 4 turns (%.3f m/s)" % ["turn in place", probe.slide_dist, probe.slide_speed()])
	slide_results = r


var slide_results := {}
var pop_results := {}


func pop_seg(label: String, n: int, until: Callable = Callable(), extra: int = 0) -> void:
	probe.reset()
	var i := 0
	while i < n and (until.is_null() or not until.call()):
		await get_tree().physics_frame
		probe.sample()
		i += 1
	for j in extra:
		await get_tree().physics_frame
		probe.sample()
	pop_results[label] = probe.pop
	print("   pop %-24s %.3f m (%s, state %d)" % [label, probe.pop, probe.pop_part, probe.pop_state])


func fresh(pos: Vector3, yaw: float) -> void:
	reset_props()
	tp(pos, yaw)
	await frames(30)
	probe.reset()
	probe._hist.clear()


func t_anim_pops() -> void:
	print("-- pops: no body part may jump off its smooth path in one frame")
	await ensure_probe()
	pop_results.clear()
	var grounded := func() -> bool: return p.state == Player.State.GROUND
	await fresh(Vector3(0, 0.1, 60), 0)
	await pop_seg("idle", 30)
	press("move_forward")
	await pop_seg("idle -> run", 40)
	press("sprint")
	await pop_seg("run -> sprint", 40)
	release("sprint")
	await pop_seg("sprint -> run", 40)
	release("move_forward")
	await pop_seg("run -> stop", 40)
	press("jump")
	await frames(1)
	await pop_seg("jump takeoff", 4)
	release("jump")
	await pop_seg("jump air + land", 120, grounded, 30)
	press("dodge")
	await pop_seg("dodge (standing)", 3)
	release("dodge")
	await pop_seg("dodge end", 50)
	press("move_forward")
	await pop_seg("run", 30)
	press("dodge")
	await pop_seg("dodge (running)", 3)
	release("dodge")
	await pop_seg("dodge end (running)", 50)
	release_all()
	await pop_seg("stop", 40)

	await fresh(Vector3(0, 6.0, 60), 0)
	await pop_seg("6 m drop + landing", 200, grounded, 40)

	await fresh(Vector3(0, 0.1, -8.0), 0)
	press("move_forward")
	press("jump")
	await pop_seg("jump onto wall", 60, func() -> bool: return p.state == Player.State.CLIMB, 20)
	release("jump")
	await pop_seg("climb -> mantle", 200, func() -> bool: return p.state == Player.State.GROUND, 40)
	release_all()

	await fresh(Vector3(-38, 0.1, 0), 90)
	press("move_forward")
	await pop_seg("swim enter", 200, func() -> bool: return p.state == Player.State.SWIM, 40)
	release_all()
	press("move_back")
	await pop_seg("swim back", 200, func() -> bool: return p.global_position.x > -41.6)
	press("jump")
	await pop_seg("swim exit", 200, grounded, 40)
	release_all()

	await fresh(Vector3(4, 0.1, -0.5), 0)
	press("interact")
	await pop_seg("carry pickup", 50)
	press("move_back")
	await pop_seg("carry walk", 50)
	release("move_back")
	await pop_seg("carry stop", 30)
	release("interact")
	await pop_seg("carry drop", 40)
	press("interact")
	await pop_seg("carry pickup 2", 50)
	press("attack")
	await pop_seg("carry throw", 3)
	release_all()
	await pop_seg("after throw", 40)

	await fresh(Vector3(10, 0.1, 2.5), 0)
	press("move_forward")
	await pop_seg("push start", 80)
	release_all()
	await pop_seg("push stop", 40)

	await fresh(Vector3(0, 0.1, 40), 0)
	press("move_forward")
	press("sprint")
	await frames(40)
	press_event("ragdoll")
	await pop_seg("ragdoll start", 20)
	release_all()
	await pop_seg("ragdoll -> get-up", 600, func() -> bool: return p.state == Player.State.RECOVER, 20)
	await pop_seg("get-up end", 600, grounded, 60)

	await fresh(Vector3(0, 0.1, 60), 0)
	for dir in ["move_back", "move_left"]:
		press(dir)
		await pop_seg("turn tap " + dir, 2)
		release(dir)
		await pop_seg("turn " + dir, 45)


func part_lowest(part: Dictionary) -> float:
	var b: RigidBody3D = part["body"]
	var cs: CollisionShape3D = b.get_child(0)
	var t := b.global_transform * cs.transform
	var sh := cs.shape
	if sh is SphereShape3D:
		return t.origin.y - sh.radius
	if sh is CapsuleShape3D:
		var hh: float = sh.height * 0.5 - sh.radius
		return minf((t * Vector3(0, hh, 0)).y, (t * Vector3(0, -hh, 0)).y) - sh.radius
	var e: Vector3 = (sh as BoxShape3D).size * 0.5
	var lo := INF
	for sx in [-1.0, 1.0]:
		for sy in [-1.0, 1.0]:
			for sz in [-1.0, 1.0]:
				lo = minf(lo, (t * Vector3(e.x * sx, e.y * sy, e.z * sz)).y)
	return lo


func t_anim_roll() -> void:
	print("-- dodge roll: the body must actually roll over, without sinking into the floor")
	await ensure_probe()
	for running in [false, true]:
		await fresh(Vector3(0, 0.1, 60), 0)
		if running:
			press("move_forward")
			await frames(30)
		press("dodge")
		var tilt := 0.0
		var pel_lo := 9.0
		var sink := 0.0
		var sink_part := ""
		var head_lo := 9.0
		for i in 45:
			await get_tree().physics_frame
			if i == 2:
				release("dodge")
			var chest: RigidBody3D = p.model.rig.by_name["chest"]["body"]
			tilt = maxf(tilt, rad_to_deg(chest.global_basis.y.normalized().angle_to(Vector3.UP)))
			pel_lo = minf(pel_lo, p.model.rig.pelvis().global_position.y)
			head_lo = minf(head_lo, p.model.rig.by_name["head"]["body"].global_position.y)
			for part in p.model.rig.parts:
				var bp: Vector3 = (part["body"] as RigidBody3D).global_position
				var gh := p._ray(bp + Vector3.UP * 0.5, bp + Vector3.DOWN * 2.0)
				var lo := part_lowest(part) - ((gh["position"] as Vector3).y if not gh.is_empty() else 0.0)
				if lo < sink:
					sink = lo
					sink_part = part["name"]
		release_all()
		print("   roll%s: chest tilts up to %.0f deg, pelvis down to %.2f m, head down to %.2f m, deepest part %.3f m (%s)" % [
			" (running)" if running else "", tilt, pel_lo, head_lo, sink, sink_part])
		roll_results[running] = [tilt, pel_lo, sink]
		await frames(30)


var roll_results := {}
var climb_results := {}


func t_anim_climb() -> void:
	print("-- climbing: limbs move with the distance climbed, palms hold on the wall")
	await ensure_probe()
	await fresh(Vector3(16, 0.1, -29.0), 0)
	press("move_forward")
	press("jump")
	await frames(3)
	release("jump")
	await wait_until(func() -> bool: return p.state == Player.State.CLIMB and p.global_position.y > 2.5, 400)
	var y0 := p.global_position.y
	var t := 0.0
	var scale_max := 0.0
	var slip := 0.0       # how fast the slower palm moves along the wall (one hand should always hold)
	var both := 0.0
	var prev := {}
	var lifts := 0
	var holding := {}
	var n := 150
	for i in n:
		await get_tree().physics_frame
		probe.sample()
		scale_max = maxf(scale_max, p.model.climb_rate())
		var v := {}
		for s in ["l", "r"]:
			var hp: Vector3 = p.model.rig.by_name["hand_" + s]["body"].global_position
			if prev.has(s):
				v[s] = (hp - prev[s]).length() * 60.0
				var hold: bool = v[s] < 0.25
				if holding.get(s, true) and not hold:
					lifts += 1
				holding[s] = hold
			prev[s] = hp
		if v.size() == 2:
			slip += minf(v["l"], v["r"])
			both += 1
	var dy := p.global_position.y - y0
	release_all()
	var cps := scale_max  # clip cycles per second at the worst moment
	climb_results = {"speed": dy / (n / 60.0), "slip": slip / maxf(1, both), "reach": dy / maxf(1, lifts), "rate": cps}
	print("   climb: %.2f m/s up, slower palm slips %.2f m/s, %d hand reaches over %.2f m (%.2f m per reach), clip at most %.2f cycles/s" % [
		dy / (n / 60.0), slip / maxf(1, both), lifts, dy, dy / maxf(1, lifts), cps])
	press("crouch")
	await frames(3)
	release_all()
	await wait_until(func() -> bool: return p.state == Player.State.GROUND, 400)


## Height of the stair surface at z (the 10 x 0.3 m steps at x -9.5..-6.5), else 0.
static func stair_top(x: float, z: float) -> float:
	if x < -9.5 or x > -6.5 or z > -5.6 or z < -13.6:
		return 0.0
	return 0.3 * (floor((-5.6 - z) / 0.8) + 1.0)


func t_anim_stairs() -> void:
	print("-- stairs: shins, knees and feet stay out of the steps (drawn skeleton)")
	await ensure_probe()
	var sk: Skeleton3D = p.model.skeleton
	var segs := [["thigh_l", "calf_l", 0.07], ["calf_l", "foot_l", 0.055], ["thigh_r", "calf_r", 0.07], ["calf_r", "foot_r", 0.055]]
	for c in [["up, running", Vector3(-8, 0.1, -3.0), 0.0, 1.0], ["down, running", Vector3(-8, 3.15, -13.6), 180.0, 1.0],
			["up, walking", Vector3(-8, 0.1, -3.0), 0.0, stick(0.1923)], ["down, walking", Vector3(-8, 3.15, -13.6), 180.0, stick(0.1923)]]:
		await fresh(c[1], c[2])
		press("move_forward", c[3])
		var worst := 0.0
		var worst_part := ""
		var on_step := 0
		var steps := 0
		for i in 400:
			await get_tree().physics_frame
			var d: Dictionary = p.model.ragdoll_pose.drawn
			if d.is_empty():
				continue
			var pts: Array = []
			for sg in segs:
				var a: Vector3 = (d[sk.find_bone(sg[0])] as Transform3D).origin
				var b: Vector3 = (d[sk.find_bone(sg[1])] as Transform3D).origin
				for k in 6:
					pts.append([a.lerp(b, k / 5.0), sg[2], sg[0]])
			for side in ["l", "r"]:
				var ft: Transform3D = d[sk.find_bone("foot_" + side)]
				pts.append([ft.origin, probe.ankle_h - 0.01, "heel_" + side])
				pts.append([ft * Vector3(0, probe.foot_len, 0), probe.ball_h - 0.01, "ball_" + side])
			for q in pts:
				var pos: Vector3 = q[0]
				var depth: float = stair_top(pos.x, pos.z) - (pos.y - q[1])
				if depth > worst:
					worst = depth
					worst_part = q[2]
			var z := p.global_position.z
			if (c[2] == 0.0 and z < -14.5) or (c[2] == 180.0 and z > -4.0):
				break
		release_all()
		print("   stairs %s: deepest limb point %.3f m inside a step (%s)" % [c[0], worst, worst_part])
		stairs_results[c[0]] = worst


var stairs_results := {}


var other_contacts := 0


## Own parts that touched each other this frame (contact monitor on), as "a-b" names.
func _own_contacts(own: Dictionary, hits: Dictionary) -> void:
	for b in own:
		for o in (b as RigidBody3D).get_colliding_bodies():
			if own.has(o):
				hits["%s-%s" % [own[b], own[o]]] = true
			else:
				other_contacts += 1  # proves the monitor is reporting at all


func _all_excepted(own: Dictionary) -> bool:
	for b in own:
		var ex: Array = (b as RigidBody3D).get_collision_exceptions()
		for o in own:
			if o != b and not ex.has(o):
				return false
	return true


func t_selfcol() -> void:
	print("-- own body parts never collide while driven; fighters collide with each other")
	var rig := p.model.rig
	var own := {}
	for part in rig.parts:
		own[part["body"]] = part["name"]
		(part["body"] as RigidBody3D).contact_monitor = true
		(part["body"] as RigidBody3D).max_contacts_reported = 8
	check("driven: all 105 own part pairs excepted", _all_excepted(own) and not rig.self_colliding())
	var hits := {}
	other_contacts = 0
	tp(Vector3(0, 0.1, 60), 0)
	await frames(20)
	press("move_forward")
	press("sprint")
	for i in 90:
		await frames(1)
		_own_contacts(own, hits)
	tp(Vector3(6, 0.1, -0.5), 0)
	await frames(30)
	press("interact")
	await frames(25)
	var carrying := p.carried != null
	press("move_back")
	for i in 60:
		await frames(1)
		_own_contacts(own, hits)
	release_all()
	await frames(10)
	tp(Vector3(16, 0.1, -29.0), 0)
	await frames(15)
	press("move_forward")
	press("jump")
	await frames(3)
	release("jump")
	var climbed := 0
	for i in 200:
		await frames(1)
		if p.state == Player.State.CLIMB:
			climbed += 1
			_own_contacts(own, hits)
	release_all()
	check("walk / carry / climb: no own part touches another", hits.is_empty() and carrying and climbed > 60 and other_contacts > 0,
		"own contacts %s, %d other contacts, carried %s, %d climb frames" % [str(hits.keys()), other_contacts, carrying, climbed])
	tp(Vector3(0, 0.1, 60), 0)
	await frames(20)
	press_event("ragdoll")
	await frames(10)
	var loose := rig.self_colliding() and not _all_excepted(own)
	tp(Vector3(0, 0.1, 60), 0)
	await frames(5)
	check("ragdoll lets own limbs collide, reset restores the exceptions", loose and _all_excepted(own) and not rig.self_colliding())
	for b in own:
		(b as RigidBody3D).contact_monitor = false

	var n0 := Player.fighters.size()
	var dummy: Player = m.spawn_fighter(m.build_kaelen(), Vector3(0, 0.1, 50))
	await frames(2)
	var n_live := Player.fighters.size() - n0 + 1
	tp(Vector3(0, 0.1, 53), 0)
	await frames(20)
	press("move_forward")
	var closest := 99.0
	for i in 90:
		await frames(1)
		closest = minf(closest, p.global_position.distance_to(dummy.global_position))
	release_all()
	check("fighters: a second (camera-less, AI) Player spawns and blocks the first", n_live == 2 and dummy.cam == null and closest > 0.6,
		"%d fighters, closest %.2f m, dummy state %d" % [n_live, closest, dummy.state])
	dummy.queue_free()
	await frames(2)
	check("fighters registry drops a freed fighter", Player.fighters.size() == n0 and Player.fighters.has(p),
		str(Player.fighters.size()))


func screenshot() -> void:
	tp(Vector3(2, 0.1, 6), 25)
	p.sheet.cast("gale_step")
	p.sheet.cooldowns.clear()
	p.sheet.cast("bright_valor")
	p.cam.pitch = -0.25
	await frames(40)
	var img := get_viewport().get_texture().get_image()
	var path := ProjectSettings.globalize_path("res://docs/arena.png")
	img.save_png(path)
	print("screenshot -> ", path)


# ---- combat ----------------------------------------------------------------------------------

func tap(action: String, hold: int = 2) -> void:
	press(action)
	await frames(hold)
	release(action)


func brain(f: Player) -> FighterAI:
	return f.get_node("ai")


## Player `dist` m in front of fighter `f` (on its +Z side), facing it; optionally locked on.
func face_off(f: Player, dist: float, lock: bool = true) -> void:
	var fp := f.global_position
	tp(Vector3(fp.x, 0.1, fp.z + dist), 0)
	await frames(12)
	if lock:
		await tap("lock")
		await frames(2)


## Deepest overlap (m) of the attacker's fists / forearms with any of the victim's body parts.
func fist_depth(att: Player, vic: Player) -> float:
	var space := p.get_world_3d().direct_space_state
	var ex: Array[RID] = []
	for f in Player.fighters:
		if f != vic:
			ex.append_array(f.model.rig.rids())
	var worst := 0.0
	for n in ["hand_l", "hand_r", "lowerarm_l", "lowerarm_r"]:
		var b: RigidBody3D = att.model.rig.by_name[n]["body"]
		var cs: CollisionShape3D = b.get_child(0)
		var q := PhysicsShapeQueryParameters3D.new()
		q.shape = cs.shape
		q.transform = b.global_transform * cs.transform
		q.collision_mask = BodyRig.L_BODY
		q.exclude = ex
		var pts := space.collide_shape(q, 8)
		for i in range(0, pts.size() - 1, 2):
			var d := (pts[i] as Vector3).distance_to(pts[i + 1])
			if d > worst:
				worst = d
				_pen_part = n
	return worst


var _pen_part := ""


## Run frames while measuring fist penetration and how close the two capsules get.
func fight_frames(n: int, att: Player, vic: Player, acc: Dictionary) -> void:
	for i in n:
		await get_tree().physics_frame
		pen_note(acc, fist_depth(att, vic), "fight %s %s t=%.2f" % [att.combat.move_id, att.combat.state, att.combat.t])
		acc["close"] = minf(acc.get("close", 9.0), att.global_position.distance_to(vic.global_position))


func pen_note(acc: Dictionary, d: float, where: String) -> void:
	if d > acc.get("pen", 0.0):
		acc["pen"] = d
		acc["pen_at"] = where + " " + _pen_part


func puppet_strike(f: Player) -> void:
	var b := brain(f)
	b.puppet.just_attack = true
	b.puppet.attack = true
	await frames(1)
	b.puppet.attack = false


## Player 1.2 m in front of the sparring partner (a puppet facing the player), both locked on.
func duel(sp: Player) -> void:
	var sb := brain(sp)
	sp.teleport(m.DUMMY_POS + Vector3(-6.0, 0, 0))
	await frames(5)
	tp(sp.global_position + Vector3(0, 0, 1.2), 0)
	p.sheet.restore()
	p.stamina = p.sheet.v("stamina")
	p.exhausted = false
	sb.puppet = Intent.new()
	sb.puppet.view_yaw = PI  # facing +Z, toward the player
	sb.puppet.just_lock = true
	await frames(10)
	await tap("lock")
	await frames(2)


## Three taps 0.1 s apart from 1.2 m: the moves seen and when the hook's active window starts.
func combo_time(dm: Player, acc: Dictionary, buff: String = "") -> Array:
	await face_off(dm, 1.2)
	if buff != "":
		p.sheet.restore()
		p.sheet.cast(buff)
	p.combat.combo_i = 0
	var jab_at := -1.0
	var seen: Array = []
	var hook_at := -1.0
	var clock := 0.0
	var last := ""
	var pressed_for := ""
	press("attack")
	for n in 150:
		await get_tree().physics_frame
		clock += get_physics_process_delta_time() * Engine.time_scale
		release("attack")
		pen_note(acc, fist_depth(p, dm), "%s %d t=%.2f" % [p.combat.move_id, p.combat.state, p.combat.t])
		acc["close"] = minf(acc.get("close", 9.0), p.global_position.distance_to(dm.global_position))
		var c := p.combat
		if c.state == Combat.S.WINDUP and c.move_id != last:
			seen.append(c.move_id)
			last = c.move_id
			if c.move_id == "jab" and jab_at < 0.0:
				jab_at = clock - c.t
			if Combat.log_events:
				print("   combo: %s windup at test clock %.3f (sheet clock %.3f) dt %.4f ts %.2f" % [c.move_id, clock, p.sheet.clock,
					get_physics_process_delta_time(), Engine.time_scale])
		if c.state == Combat.S.ACTIVE and c.t >= c.active * 0.5 and pressed_for != c.move_id and seen.size() < 3:
			pressed_for = c.move_id
			press("attack")  # pressed during the strike: buffered, the next one follows as soon as it recovers
		if c.move_id == "hook" and c.state == Combat.S.ACTIVE and hook_at < 0.0:
			hook_at = clock - c.t - jab_at  # from the jab's first frame to the hook's active window
	release("attack")
	return [seen, hook_at]


func t_combat() -> void:
	print("-- hand-to-hand combat and lock-on")
	Combat.log_events = "--combatlog" in OS.get_cmdline_user_args()
	var dm: Player = m.dummy
	var sp: Player = m.spar
	var sb := brain(sp)
	sb.mode = "dummy"
	p.combat.crits = false
	sp.combat.crits = false
	dm.teleport(m.DUMMY_POS)
	sp.teleport(m.SPAR_POS)
	await frames(10)
	var acc := {}

	# 1. lock-on: acquire, cone, range, drop
	await face_off(dm, 2.0)
	var front := p.lock.target == dm
	await tap("lock")
	var unlocked := p.lock.target == null
	tp(dm.global_position + Vector3(0, 0, 2.0), 180)
	await frames(8)
	await tap("lock")
	var behind := p.lock.target
	tp(dm.global_position + Vector3(0, 0, 16.0), 0)
	await frames(8)
	await tap("lock")
	var far := p.lock.target
	tp(dm.global_position + Vector3(0, 0, 10.0), 0)
	await frames(8)
	await tap("lock")
	var at10 := p.lock.target == dm
	dm.teleport(m.DUMMY_POS + Vector3(0, 0, -11.0))
	await frames(5)
	var dropped := p.lock.target == null
	dm.teleport(m.DUMMY_POS)
	await frames(5)
	check("lock-on: MMB locks the fighter in front, toggles off; nothing behind or beyond 15 m; drops beyond 20 m",
		front and unlocked and behind == null and far == null and at10 and dropped,
		"front %s off %s behind %s far %s at10 %s dropped %s" % [front, unlocked, behind != null, far != null, at10, dropped])

	# 2. cycle with the wheel
	sp.teleport(m.DUMMY_POS + Vector3(2.0, 0, 0))
	await face_off(dm, 3.0)
	var c1 := p.lock.target
	p.cam.wheel_steps = 1
	await frames(2)
	var c2 := p.lock.target
	p.cam.wheel_steps = -1
	await frames(2)
	var c3 := p.lock.target
	check("lock-on: the wheel cycles between targets", c1 == dm and c2 == sp and c3 == dm,
		"%s -> %s -> %s" % [c1.sheet.display_name if c1 else "-", c2.sheet.display_name if c2 else "-", c3.sheet.display_name if c3 else "-"])
	sp.teleport(m.SPAR_POS)

	# 3. a jab: damage through the stat engine, hit-stop releases
	await face_off(dm, 1.2)
	dm.sheet.restore()
	var landed := p.combat.hits_landed
	await tap("attack")
	var stop_ms := -1
	for i in 60:
		await get_tree().physics_frame
		pen_note(acc, fist_depth(p, dm), "%s %d t=%.2f" % [p.combat.move_id, p.combat.state, p.combat.t])
		if p.combat.hits_landed > landed and stop_ms < 0:
			var t0 := Time.get_ticks_msec()
			while Engine.time_scale < 1.0 and Time.get_ticks_msec() - t0 < 500:
				await get_tree().process_frame
			stop_ms = Time.get_ticks_msec() - t0
	var zone := p.combat.last_zone
	var expect := p.sheet.v("attack_damage") * 0.8 * float(Combat.ZONE.get(zone, 0.85)) * Stats.damage_taken_fraction(dm.sheet.v("armor"))
	check("jab lands: damage = Attack Damage x 0.8 x zone x armor (+-1)", p.combat.hits_landed > landed and absf(dm.combat.last_damage - expect) <= 1.0,
		"dealt %.1f, expected %.1f (%s), result %s" % [dm.combat.last_damage, expect, zone, p.combat.last_result])
	check("hit-stop: time scale back to 1 within 120 ms", stop_ms >= 0 and stop_ms <= 120 and Engine.time_scale == 1.0, "%d ms" % stop_ms)

	# 4. jab -> cross -> hook, timed by Attack Speed (with and without Shadowrush)
	var k := clampf(Combat.REF_ATTACK_SPEED / p.sheet.v("attack_speed"), 0.5, 1.5)
	var want := k * (0.10 + 0.14 + 0.18) + 0.24
	var r1: Array = await combo_time(dm, acc)
	var r2: Array = await combo_time(dm, acc, "gale_step")
	var cast := [p.sheet.find_effect("gale_step") != null]
	var k2 := clampf(Combat.REF_ATTACK_SPEED / p.sheet.v("attack_speed"), 0.5, 1.5)
	var want2 := k2 * (0.10 + 0.14 + 0.18) + 0.24
	p.sheet.effects.clear()
	p.sheet.invalidate()
	var seq_ok: bool = (r1[0] as Array).slice(0, 3) == ["jab", "cross", "hook"]
	check("LMB x3 = jab, cross, hook; hook lands on time (+-10%), faster with an Attack Speed buff by the expected ratio",
		seq_ok and absf(r1[1] - want) <= want * 0.1 and absf(r2[1] - want2) <= want2 * 0.1,
		"%s hook at %.3f s (want %.3f); with Gale Step (AS %.2f) %.3f s (want %.3f) %s" % [str(r1[0]), r1[1], want, 1.5 / k2, r2[1], want2, str(cast)])

	# 5-7. guard, guard break, parry, dodge: the sparring partner (as a puppet) strikes the player
	sb.mode = "puppet"
	await duel(sp)
	press("guard")
	await frames(8)
	var st0 := p.stamina
	await puppet_strike(sp)
	await wait_until(func() -> bool: return sp.combat.state == Combat.S.RECOVER, 60)
	await frames(2)
	var frac := Stats.damage_taken_fraction(p.sheet.v("armor"))
	var raw := sp.combat.last_raw
	var share := p.combat.last_damage / maxf(0.001, raw * frac)
	var st_lost := st0 - p.stamina
	var guarded := p.combat.last_result == "guard"
	await frames(30)
	p.stamina = 1.0
	await puppet_strike(sp)
	await wait_until(func() -> bool: return p.combat.state == Combat.S.STAGGER, 60)
	var broke := p.combat.state == Combat.S.STAGGER
	release("guard")
	check("guard: 20% of the damage (+-2%), costs stamina; an empty stamina bar breaks the guard",
		guarded and absf(share - 0.2) <= 0.02 and absf(st_lost - raw * Combat.GUARD_STAMINA) <= 1.5 and broke,
		"result %s, share %.3f, stamina -%.1f (want %.1f), broke %s" % [p.combat.last_result, share, st_lost, raw * Combat.GUARD_STAMINA, broke])

	await frames(60)
	await duel(sp)
	release("guard")
	await frames(3)
	await puppet_strike(sp)
	var chest: RigidBody3D = p.model.rig.by_name["chest"]["body"]
	var guard_hand: RigidBody3D = p.model.rig.by_name["hand_r"]["body"]
	await wait_until(func() -> bool:
		var fist: Vector3 = (sp.model.rig.by_name["hand_l"]["body"] as RigidBody3D).global_position
		return sp.combat.striking() and (fist.distance_to(chest.global_position) < 0.65 or fist.distance_to(guard_hand.global_position) < 0.3), 60)
	await tap("guard", 6)
	await wait_until(func() -> bool: return sp.combat.state == Combat.S.STAGGER or sp.combat.state == Combat.S.RECOVER, 60)
	var parried := sp.combat.state == Combat.S.STAGGER and p.combat.last_result == "parry"
	await frames(70)
	await duel(sp)
	press("guard")
	await frames(18)
	await puppet_strike(sp)
	await wait_until(func() -> bool: return sp.combat.state == Combat.S.RECOVER or sp.combat.state == Combat.S.STAGGER, 60)
	var late := sp.combat.state != Combat.S.STAGGER and p.combat.last_result == "guard"
	release("guard")
	check("parry: guard pressed as the strike lands staggers the attacker; pressed 300 ms early it only guards", parried and late,
		"parry %s, early press gave %s" % [parried, p.combat.last_result])

	await frames(40)
	await duel(sp)
	var hp0 := p.sheet.hp
	await puppet_strike(sp)
	await wait_until(func() -> bool: return sp.combat.state == Combat.S.WINDUP and sp.combat.startup - sp.combat.t <= 0.08, 60)
	p.invuln_t = 0.35
	await wait_until(func() -> bool: return sp.combat.state == Combat.S.RECOVER, 60)
	var iframe_ok := p.sheet.hp >= hp0 - 0.01 and p.combat.last_result == "dodge"
	await frames(40)
	await duel(sp)
	await puppet_strike(sp)
	await wait_until(func() -> bool: return sp.combat.state == Combat.S.WINDUP and sp.combat.startup - sp.combat.t <= 0.1, 60)
	press("move_back")
	await tap("dodge")
	await frames(20)
	release_all()
	var flurry := p.sheet.find_effect("flurry") != null and "Flurry" in p.sheet.explain("attack_speed")
	check("dodge: no damage inside the i-frames; a perfect dodge grants Flurry (shown in the attack speed breakdown)", iframe_ok and flurry,
		"i-frames %s (%s), flurry %s" % [iframe_ok, p.combat.last_result, flurry])
	p.sheet.effects.clear()
	p.sheet.invalidate()
	sb.mode = "dummy"
	sp.teleport(m.SPAR_POS)

	# 8. charged strike staggers; on a staggered target it knocks down, and it gets back up
	dm.teleport(m.DUMMY_POS)
	await face_off(dm, 1.2)
	press("attack")
	await fight_frames(72, p, dm, acc)
	release("attack")
	await wait_until(func() -> bool: return dm.combat.state == Combat.S.STAGGER or p.combat.state == Combat.S.IDLE, 90)
	var stag := dm.combat.state == Combat.S.STAGGER
	await frames(40)
	dm.combat.stagger(3.0, Vector3.ZERO)
	press("attack")
	await fight_frames(40, p, dm, acc)
	release("attack")
	await wait_until(func() -> bool: return dm.state == Player.State.RAGDOLL, 90)
	var down := dm.state == Player.State.RAGDOLL
	await wait_until(func() -> bool: return dm.state == Player.State.GROUND, 900)
	check("charged strike staggers; on a staggered target it knocks down (ragdoll) and the target gets back up",
		stag and down and dm.state == Player.State.GROUND, "stagger %s, knockdown %s, state now %d" % [stag, down, dm.state])

	# 9. physical: fists stop on the body, capsules never overlap
	check("fists never sink into a body (< 2 cm); fighters' capsules never overlap",
		acc.get("pen", 0.0) < 0.02 and acc.get("close", 9.0) >= 0.69, "deepest %.3f m (%s), closest %.2f m" % [acc.get("pen", 0.0), acc.get("pen_at", ""), acc.get("close", 9.0)])

	# 10. strafing round a locked target keeps the distance
	dm.teleport(m.DUMMY_POS)
	await face_off(dm, 2.0)
	press("move_right")
	await frames(20)
	var d0 := p.global_position.distance_to(dm.global_position)
	if Combat.log_events:
		print("   strafe start: d %.2f, dummy state %d, lock %s" % [d0, dm.state, p.lock.target.sheet.display_name if p.lock.target else "-"])
	var dmin := d0
	var dmax := d0
	for i in 180:
		await get_tree().physics_frame
		var d := p.global_position.distance_to(dm.global_position)
		dmin = minf(dmin, d)
		dmax = maxf(dmax, d)
		if Combat.log_events and i % 30 == 0:
			print("   strafe %d: d %.2f lock %s cam.yaw %.2f facing %s dummy %s state %d" % [i, d, p.lock.target.sheet.display_name if p.lock.target else "-",
				p.cam.yaw, str(p.facing.snappedf(0.01)), str(dm.global_position.snappedf(0.01)), dm.state])
	release_all()
	check("locked strafe circles the target at a steady distance (+-0.3 m over 3 s)", dmax - d0 <= 0.3 and d0 - dmin <= 0.3 and p.lock.target == dm,
		"%.2f m, range %.2f .. %.2f" % [d0, dmin, dmax])
	await tap("lock")

	# 11. the sparring partner fights
	sb.mode = "spar"
	sp.teleport(m.SPAR_POS)
	tp(m.SPAR_POS + Vector3(0, 0, 3.0), 0)
	p.sheet.restore()
	var taken := p.combat.hits_taken
	await frames(360)
	var got := p.combat.hits_taken - taken
	tp(m.SPAR_POS + Vector3(0, 0, 1.5), 0)
	p.sheet.restore()
	await frames(10)
	await tap("lock")
	var guards := 0
	var was_guard := false
	for n in 10:
		await tap("attack")
		for f in 30:
			await get_tree().physics_frame
			var g := sp.combat.state == Combat.S.GUARD
			if g and not was_guard:
				guards += 1
			was_guard = g
	check("sparring partner lands hits on an idle player (>= 2 in 6 s) and guards some of 10 jabs", got >= 2 and guards >= 1,
		"%d hits in 6 s, guarded %d times" % [got, guards])

	release_all()
	Engine.time_scale = 1.0
	p.combat.crits = true
	sp.combat.crits = true
	sb.mode = "spar"
	dm.teleport(m.DUMMY_POS)
	sp.teleport(m.SPAR_POS)
	p.sheet.restore()


# ---- status window, bag, pickups -------------------------------------------------------------

func t_ui() -> void:
	print("-- status window, bag, gear and pickups")
	process_mode = Node.PROCESS_MODE_ALWAYS
	var sw: StatusWindow = m.status
	tp(Vector3(0, 0.1, 4.5), 180)  # just short of the loot row (z = 7), facing it
	await frames(20)
	# Tab opens it: on screen, world paused
	var ev := InputEventAction.new()
	ev.action = "menu"
	ev.pressed = true
	Input.parse_input_event(ev)
	await get_tree().process_frame
	await get_tree().process_frame
	var vp := sw.get_viewport_rect()
	var panel: Control = sw.get_child(1)
	var r := panel.get_global_rect()
	# pinned to the window edges (the headless window is tiny, so check the anchoring, not pixels)
	var on_screen := panel.anchor_left == 0.0 and panel.anchor_right == 1.0 and panel.anchor_top == 0.0 and panel.anchor_bottom == 1.0 		and panel.offset_left > 0.0 and panel.offset_right < 0.0 and panel.offset_top > 0.0 and panel.offset_bottom < 0.0
	check("Tab opens the status window, centred on screen, world paused", sw.visible and get_tree().paused and on_screen,
		"visible %s paused %s panel %s in %s" % [sw.visible, get_tree().paused, str(r), str(vp.size)])
	var tabs_ok := true
	for i in StatusWindow.TABS.size():
		sw.show_tab(i)
		await get_tree().process_frame
		tabs_ok = tabs_ok and sw._body.get_child_count() > 0
	check("every tab fills (status, gear, skills, combat, controls)", tabs_ok)
	# equip from the bag: the numbers change by exactly what the appraisal said
	p.sheet.add_item("sprinter_greaves")
	var diff := p.sheet.appraise("sprinter_greaves")
	var ms0 := p.sheet.v("move_speed")
	sw.use_item("sprinter_greaves")
	var ms1 := p.sheet.v("move_speed")
	var old_boots_back := p.sheet.bag.has("swift_boots")
	check("equipping from the bag changes the stats exactly as appraised; the old item goes back in the bag",
		diff.has("move_speed") and absf(float(diff["move_speed"][1]) - ms1) < 0.001 and absf(ms0 - float(diff["move_speed"][0])) < 0.001 and old_boots_back,
		"move speed %.2f -> %.2f (appraised %s), swift boots in bag %s" % [ms0, ms1, str(diff.get("move_speed", [])), old_boots_back])
	sw.use_item("swift_boots")
	p.sheet.add_item("swift_tonic")
	var ms2 := p.sheet.v("move_speed")
	sw.use_item("swift_tonic")
	check("using a consumable applies its effect", p.sheet.find_effect("swift_tonic") != null and p.sheet.v("move_speed") > ms2 + 0.1,
		"%.2f -> %.2f" % [ms2, p.sheet.v("move_speed")])
	var ev2 := InputEventAction.new()
	ev2.action = "menu"
	ev2.pressed = true
	Input.parse_input_event(ev2)
	await get_tree().process_frame
	await get_tree().process_frame
	check("Tab closes it and the world runs again", not sw.visible and not get_tree().paused)
	# pick an item up with E
	tp(Vector3(-9.0, 0.1, 5.8), 180)
	await frames(20)
	var bag0: int = p.sheet.bag.get("chain_vest", 0)
	press("interact")
	await frames(3)
	release("interact")
	await frames(5)
	check("E next to an item puts it in the bag", int(p.sheet.bag.get("chain_vest", 0)) == bag0 + 1, str(p.sheet.bag))
	p.sheet.effects.clear()
	p.sheet.invalidate()
	process_mode = Node.PROCESS_MODE_INHERIT
