class_name SelfTest
extends Node
## Automated check of the controller, camera-free combat and UI, driving the real player through a
## puppet brain (the same Intent the keyboard fills).
##   Godot --headless --path <guildbound> -- --selftest [--only=move,dodge,melee,guard,ranged,ai,ui]

var m: Node
var p: Player
var ai: FighterAI
var results: Array[bool] = []


func check(name: String, ok: bool, detail: String = "") -> void:
	results.append(ok)
	print("[%s] %s%s" % ["PASS" if ok else "FAIL", name, "  -  " + detail if detail != "" else ""])


func frames(n: int) -> void:
	for i in n:
		await get_tree().physics_frame


func run(main: Node) -> void:
	m = main
	p = main.player
	ai = FighterAI.new("puppet")
	ai.name = "ai"
	p.add_child(ai)
	(m.spar.get_node("ai") as FighterAI).mode = "dummy"
	Sfx.enabled = false
	Combat.log_events = "--combatlog" in OS.get_cmdline_user_args()
	await frames(30)
	print("\n=== SELFTEST: %s ===" % p.sheet.display_name)
	var only := ""
	for a in OS.get_cmdline_user_args():
		if a.begins_with("--only="):
			only = a.substr(7)
	var tests := [["move", t_move], ["dodge", t_dodge], ["melee", t_melee], ["guard", t_guard], ["ranged", t_ranged], ["ai", t_ai], ["ui", t_ui]]
	for t in tests:
		if only == "" or t[0] in only.split(","):
			await (t[1] as Callable).call()
	var bad := results.count(false)
	print("\n=== %d checks, %d passed, %d failed ===" % [results.size(), results.size() - bad, bad])
	get_tree().quit(1 if bad > 0 else 0)


func put(pos: Vector3, face: Vector3 = Vector3(0, 0, -1)) -> void:
	p.teleport(pos)
	p.facing = face
	ai.puppet = Intent.new()
	ai.puppet.view_yaw = atan2(-face.x, -face.z)
	await frames(25)


func press(field: String, n: int = 1) -> void:
	ai.puppet.set(field, true)
	await frames(n)
	ai.puppet.set(field, false)


func t_move() -> void:
	print("-- movement")
	await put(Vector3(0, 0.2, 10))
	var a := p.global_position
	ai.puppet.move = Vector2(0, 1)
	await frames(60)
	var run := p.global_position.distance_to(a)
	var bp: Vector2 = p.model.tree.get("parameters/loco/blend_position")
	var anim_ok := bp.length() > 1.2
	ai.puppet.sprint = true
	a = p.global_position
	await frames(60)
	var sprint := p.global_position.distance_to(a)
	ai.puppet.sprint = false
	ai.puppet.move = Vector2.ZERO
	await frames(30)
	check("running moves forward at run pace and plays the run blend; sprint is faster", run > 3.0 and sprint > run * 1.2 and anim_ok,
		"run %.2f m/s, sprint %.2f m/s, blend %s" % [run, sprint, str(p.model.tree.get("parameters/loco/blend_position"))])
	var y0 := p.global_position.y
	ai.puppet.just_jump = true
	await frames(1)
	ai.puppet.just_jump = false
	var top := y0
	for i in 60:
		await frames(1)
		top = maxf(top, p.global_position.y)
	check("jump rises and lands", top - y0 > 0.8 and p.is_on_floor(), "apex %.2f m" % (top - y0))


func t_dodge() -> void:
	print("-- dodge")
	await put(Vector3(0, 0.2, 10))
	var a := p.global_position
	ai.puppet.move = Vector2(0, 1)
	await frames(10)
	await press("just_dodge")
	var inv := p.invuln_t > 0.0 and p.state == Player.State.DODGE
	await frames(70)
	ai.puppet.move = Vector2.ZERO
	var d := p.global_position.distance_to(a)
	check("Alt rolls (invulnerable, carried by the roll's own motion) and comes back to the ground", inv and d > 2.0 and p.state == Player.State.GROUND,
		"moved %.2f m, invulnerable %s, state %d" % [d, inv, p.state])


func t_melee() -> void:
	print("-- melee: every weapon style lands its combo and heavy on the dummy")
	var dm: Player = m.dummy
	(dm.get_node("ai") as FighterAI).mode = "puppet"  # no healing between blows
	for style in ["sword_shield", "greatsword", "dagger", "unarmed"]:
		dm.teleport(m.DUMMY_POS)
		await put(m.DUMMY_POS + Vector3(0, 0.1, 1.25))
		p.combat.set_stance(style)
		p.lock.target = dm
		await frames(10)
		var landed0 := p.combat.hits_landed
		for k in 3:
			await press("just_attack")
			await frames(int(0.45 * 60))
		await frames(60)
		var combo_hits := p.combat.hits_landed - landed0
		dm.teleport(m.DUMMY_POS)  # a stagger knocks it back: set it up again for the heavy
		await put(m.DUMMY_POS + Vector3(0, 0.1, 1.25))
		p.lock.target = dm
		await frames(10)
		ai.puppet.attack = true
		ai.puppet.just_attack = true
		await frames(1)
		ai.puppet.just_attack = false
		await frames(30)
		ai.puppet.attack = false
		var heavy_landed := false
		for i in 120:
			await frames(1)
			if p.combat.move_kind == "heavy" and p.combat.hits_landed > landed0 + combo_hits:
				heavy_landed = true
		await frames(40)
		check("%s: combo lands (%d of 3), heavy lands" % [style, combo_hits], combo_hits >= 2 and heavy_landed, "heavy %s" % heavy_landed)
	p.combat.set_stance("sword_shield")
	(dm.get_node("ai") as FighterAI).mode = "dummy"


func t_guard() -> void:
	print("-- guard and parry against the sparring partner")
	var sp: Player = m.spar
	var sb := sp.get_node("ai") as FighterAI
	sp.teleport(m.SPAR_POS)
	await put(m.SPAR_POS + Vector3(0, 0.1, 1.8))
	p.lock.target = sp
	sb.mode = "puppet"
	sp.lock.target = p
	ai.puppet.guard = true
	await frames(20)
	var hp0 := p.sheet.hp
	sb.puppet.just_attack = true
	await frames(1)
	sb.puppet.just_attack = false
	for i in 90:
		await frames(1)
	var blocked := p.combat.last_result == "guard"
	ai.puppet.guard = false
	await frames(40)
	check("holding RMB blocks a frontal blow (most damage stopped)", blocked and hp0 - p.sheet.hp < sp.sheet.v("attack_damage") * 0.5,
		"result %s, took %.0f" % [p.combat.last_result, hp0 - p.sheet.hp])
	# parry: guard pressed just before the blow lands
	sb.puppet.just_attack = true
	await frames(1)
	sb.puppet.just_attack = false
	for i in 120:
		await frames(1)
		if sp.combat.attacking() and sp.combat.t >= sp.combat.window.x - 0.05:
			break
	ai.puppet.just_guard = true
	ai.puppet.guard = true
	await frames(1)
	ai.puppet.just_guard = false
	await frames(40)
	ai.puppet.guard = false
	check("guard tapped as the blow lands parries it: the attacker staggers", p.combat.last_result == "parry" and sp.combat.state == Combat.S.STAGGER,
		"result %s, attacker state %d" % [p.combat.last_result, sp.combat.state])
	sb.mode = "dummy"
	await frames(60)


func t_ranged() -> void:
	print("-- bow and staff")
	var dm: Player = m.dummy
	dm.teleport(m.DUMMY_POS)
	await put(m.DUMMY_POS + Vector3(0, 0.1, 8.0))
	p.combat.set_stance("bow")
	p.lock.target = dm
	await frames(10)
	var landed := p.combat.hits_landed
	ai.puppet.guard = true
	await frames(50)
	await press("just_attack")
	await frames(60)
	ai.puppet.guard = false
	check("bow: aim (RMB) and loose (LMB) - the arrow flies and hits", p.combat.hits_landed > landed, "hits %d" % (p.combat.hits_landed - landed))
	p.combat.set_stance("magic")
	await frames(10)
	landed = p.combat.hits_landed
	await press("just_attack")
	await frames(120)
	check("staff: LMB casts a bolt that homes in and hits", p.combat.hits_landed > landed, "hits %d" % (p.combat.hits_landed - landed))
	p.combat.set_stance("sword_shield")


func t_ai() -> void:
	print("-- the sparring partner fights")
	var sp: Player = m.spar
	var sb := sp.get_node("ai") as FighterAI
	sp.teleport(m.SPAR_POS)
	sb.mode = "spar"
	await put(m.SPAR_POS + Vector3(0, 0.1, 6.0))
	var taken := p.combat.hits_taken
	var closest := 99.0
	for i in 600:
		await frames(1)
		closest = minf(closest, sp.global_position.distance_to(p.global_position))
		if p.state == Player.State.DEAD:
			break
	check("the sparring partner closes in and lands hits (>= 2 in 10 s)", p.combat.hits_taken - taken >= 2 and closest < 3.0,
		"hits %d, closest %.2f m" % [p.combat.hits_taken - taken, closest])
	sb.mode = "dummy"


func t_ui() -> void:
	print("-- side panel and character sheet")
	process_mode = Node.PROCESS_MODE_ALWAYS
	var sp: SidePanel = m.panel
	sp.open(SidePanel.TABS.find("inventory"))
	await get_tree().process_frame
	var grid: GridContainer = sp._content.get_child(0)
	check("Tab panel opens with the 28-slot inventory", sp.visible and grid.get_child_count() == SidePanel.INV_SIZE)
	var ok := true
	for k in SidePanel.TABS.size():
		sp.show_tab(k)
		await get_tree().process_frame
		ok = ok and sp._content.get_child_count() > 0
	check("every side-panel tab fills (combat shows the weapon style)", ok)
	sp.close()
	var sw: StatusWindow = m.status
	sw.open()
	await get_tree().process_frame
	check("P character sheet opens and pauses", sw.visible and get_tree().paused)
	sw.close()
	await get_tree().process_frame
	process_mode = Node.PROCESS_MODE_INHERIT
