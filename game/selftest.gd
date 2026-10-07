class_name SelfTest
extends Node
## Automated check of the controller, camera-free combat and UI, driving the real player through a
## puppet brain (the same Intent the keyboard fills).
##   Godot --headless --path <guildbound> -- --selftest [--only=move,dodge,melee,guard,ranged,ai,ui,grid,mobs,quests,shop]

var m: Node
var p: Player
var ai: FighterAI
var results: Array[bool] = []
## Item values (the Python sim checks the same numbers: tests/test_stats.py EconomyValues).
const EXPECTED_VALUES := {"ring_ogre": 495, "brawler_wraps": 580, "sprinter_greaves": 650, "hearty_stew": 67, "swift_tonic": 37, "keen_eye_tea": 18, "ironbark_draught": 28, "slime_gel": 6, "hide_bundle": 30, "core_common": 40, "leather_cap": 120, "iron_sword": 770}


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
	var tests := [["move", t_move], ["dodge", t_dodge], ["melee", t_melee], ["guard", t_guard], ["ranged", t_ranged], ["ai", t_ai], ["ui", t_ui],
		["grid", t_grid], ["mobs", t_mobs], ["quests", t_quests], ["shop", t_shop]]
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
	await put(m.arena.spots["run_lane"])
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
	await put(m.arena.spots["run_lane"])
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
	var view := sp._content.find_child("BagView", true, false) as BagView
	check("Tab panel opens with the backpack grid", sp.visible and view != null and view.bag == p.sheet.grid)
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


func t_grid() -> void:
	print("-- backpack grid (Tarkov-style)")
	var g := GridBag.new(4, 4)
	var four := true
	for i in 4:
		four = four and g.add("leather_cap")  # 2x2
	check("a 4x4 grid takes exactly four 2x2 items, the fifth is refused", four and not g.add("leather_cap") and g.cells_used() == 16)
	g = GridBag.new(4, 2)
	var turned := g.add("iron_sword")  # 1x4 only fits lying down in a 4x2 bag
	var e: Dictionary = g.entries[0] if g.entries.size() > 0 else {}
	check("a long item turns to fit (1x4 sword in a 4x2 bag)", turned and bool(e.get("rot", false)))
	g = GridBag.new(2, 1)
	var stacked := g.add("swift_tonic", 7)  # max_stack 5: two cells
	check("stackables share a cell up to their cap (7 tonics -> 5 + 2)", stacked and g.entries.size() == 2 and g.count("swift_tonic") == 7 and not g.add("swift_tonic", 4))
	g = GridBag.new(4, 4)
	g.add("leather_cap")
	g.add("oak_buckler")
	var a: Dictionary = g.entries[0]
	var b: Dictionary = g.entries[1]
	var blocked := not g.move(a, int(b["x"]), int(b["y"]), false)
	var moved := g.move(a, 2, 2, false) and int(a["x"]) == 2 and int(a["y"]) == 2
	check("moving onto another item is refused, onto free cells works", blocked and moved)
	# the player's backpack through the game: a pickup lands in the grid; a full bag refuses it
	await put(m.arena.spots["run_lane"])
	var sheet := p.sheet
	var before := sheet.grid.count("chain_vest")
	var pk := ItemPickup.make("chain_vest")
	m.add_child(pk)
	pk.global_position = p.global_position + p.facing * 0.6
	await frames(5)
	await press("just_interact")
	await frames(5)
	var picked := sheet.grid.count("chain_vest") == before + 1 and int(sheet.bag.get("chain_vest", 0)) == before + 1
	var saved := sheet.grid.duplicate_bag()
	var saved_bag := sheet.bag.duplicate()
	while sheet.add_item("chain_vest"):
		pass
	var pk2 := ItemPickup.make("chain_vest")
	m.add_child(pk2)
	pk2.global_position = p.global_position + p.facing * 0.6
	await frames(5)
	await press("just_interact")
	await frames(5)
	var refused := is_instance_valid(pk2) and not pk2.is_queued_for_deletion()
	check("E picks an item up into the grid; with the grid full it stays on the ground", picked and refused,
		"picked %s, refused %s, %d cells used" % [picked, refused, sheet.grid.cells_used()])
	if is_instance_valid(pk2):
		pk2.queue_free()
	sheet.grid = saved
	sheet.bag = saved_bag
	sheet.invalidate()


func t_mobs() -> void:
	print("-- monsters: slimes and goblins, loot")
	var mobs: Mobs = m.mobs
	check("the meadow has slimes and the camp has goblins", Slime.all.size() >= 3 and _goblins().size() >= 2,
		"%d slimes, %d goblins" % [Slime.all.size(), _goblins().size()])
	# a slime in front of the player: swing until it dies, its loot lands, the kill is announced
	var sl: Slime = Slime.all[0]
	var spot: Vector3 = m.arena.spots["run_lane"]
	await put(spot)
	sl.home = spot + Vector3(0, 0.3, -1.3)
	sl.global_position = sl.home
	sl.velocity = Vector3.ZERO
	var kills := []
	var on_kill := func(kind: String, _who: Player) -> void: kills.append(kind)
	mobs.killed.connect(on_kill)
	var gold0 := p.sheet.gold
	var picks0 := ItemPickup.all.size()
	p.combat.set_stance("greatsword")
	for k in 12:
		if sl.dead:
			break
		p.facing = (sl.global_position - p.global_position) * Vector3(1, 0, 1)
		p.facing = p.facing.normalized() if p.facing.length() > 0.1 else Vector3(0, 0, -1)
		await press("just_attack")
		await frames(int(0.6 * 60))
	check("swords hit a slime until it dies; it drops loot and pays gold", sl.dead and kills.has("slime") and ItemPickup.all.size() > picks0 and p.sheet.gold > gold0,
		"dead %s, kills %s, pickups +%d, gold +%d" % [sl.dead, str(kills), ItemPickup.all.size() - picks0, p.sheet.gold - gold0])
	# a live slime lunges at a player standing still: it hurts
	var s2: Slime = Slime.all[1]
	await put(spot)
	p.combat.set_stance("sword_shield")
	s2.home = spot + Vector3(0, 0.3, -3.0)
	s2.global_position = s2.home
	s2._cool = 0.0
	var hp0 := p.sheet.hp
	for i in 240:
		await frames(1)
		if p.sheet.hp < hp0:
			break
	check("a slime hops over and its lunge hurts", p.sheet.hp < hp0, "hp %.0f -> %.0f" % [hp0, p.sheet.hp])
	s2.home = m.arena.spots["slime_field"]
	s2.global_position = s2.home + Vector3(0, 0.3, 0)
	# a goblin: killed by the player, it drops its loot (always a goblin ear) and comes back later
	var gob: Player = _goblins()[0]
	var ears0 := _ground_count("goblin_ear")
	gob.sheet.hp = 1.0
	gob.teleport(spot + Vector3(0, 0, -1.4))
	gob.sheet.hp = 1.0
	await put(spot)
	p.lock.target = gob
	for k in 6:
		if gob.knocked_out:
			break
		await press("just_attack")
		await frames(int(0.6 * 60))
	check("a goblin killed by the player drops its ear and counts as a kill", gob.knocked_out and kills.has("goblin") and _ground_count("goblin_ear") > ears0,
		"down %s, kills %s" % [gob.knocked_out, str(kills)])
	# loot rolls follow the table: always-drops always, cores rarely
	var n_core := 0
	for i in 400:
		var r := mobs.roll("slime")
		var has_gel := false
		for it in r["items"]:
			has_gel = has_gel or it[0] == "slime_gel"
			n_core += 1 if str(it[0]).begins_with("core_") else 0
		if not has_gel:
			n_core = -9999
	check("loot table: slime gel every time, Magic Cores now and then (about 1 in 12)", n_core > 10 and n_core < 70, "%d cores in 400 kills" % n_core)
	mobs.killed.disconnect(on_kill)
	p.lock.target = null
	gob.teleport(gob.respawn_pos)


func _goblins() -> Array:
	return Player.fighters.filter(func(f: Player) -> bool: return f.team == "goblin")


func _ground_count(id: String) -> int:
	var n := 0
	for pk in ItemPickup.all:
		if is_instance_valid(pk) and pk.item_id == id:
			n += pk.count
	return n


func t_quests() -> void:
	print("-- guild quests: board, kill and collect objectives, rewards, rank")
	var q := p.quests
	var mobs: Mobs = m.mobs
	q.active.clear()
	q.finished.clear()
	q.rank_points = 0
	q.ready_at.clear()
	var offer := q.available()
	check("a new hunter (rank F) sees the F quests, not the E ones", q.rank() == "F" and offer.has("slime_cull") and not offer.has("goblin_trouble"), str(offer))
	q.accept("slime_cull")
	var spot: Vector3 = m.arena.spots["run_lane"]
	await put(spot)
	for i in 4:
		mobs.reward("slime", spot + Vector3(0, 0, -30), p)  # kills credited to the player
	mobs.reward("slime", spot + Vector3(0, 0, -30), null)  # someone else's kill does not count
	var four: bool = int(q.progress("slime_cull")[0][1]) == 4 and not q.complete("slime_cull")
	mobs.reward("slime", spot + Vector3(0, 0, -30), p)
	var gold0 := p.sheet.gold
	var cores0 := int(p.sheet.bag.get("core_common", 0))
	var r := q.turn_in("slime_cull")
	check("Slime Cull: 5 slime kills by the player complete it; turning in pays gold, rank points and a core",
		four and bool(r[0]) and p.sheet.gold == gold0 + 60 and q.rank_points == 20 and int(p.sheet.bag.get("core_common", 0)) == cores0 + 1, str(r[1]))
	q.accept("sticky_business")
	var not_yet := not q.complete("sticky_business")
	var gel0 := int(p.sheet.bag.get("slime_gel", 0))
	var added := maxi(0, 6 - gel0)
	p.sheet.add_item("slime_gel", added)
	var r2 := q.turn_in("sticky_business")
	check("a collect quest takes the items from the backpack when turned in",
		not_yet and bool(r2[0]) and int(p.sheet.bag.get("slime_gel", 0)) == gel0 + added - 6, str(r2[1]))
	q.rank_points = 39
	var still_f := q.rank() == "F"
	q.rank_points = 40
	check("40 rank points make rank E and open the goblin quests", still_f and q.rank() == "E" and q.available().has("goblin_trouble"))
	# the board on the plaza: E opens it (the game pauses), it lists your quests and the offers
	q.accept("goblin_trouble")
	await put(m.arena.spots["board"])
	p.facing = Vector3(0, 0, -1)
	await press("just_interact")
	await get_tree().process_frame
	await get_tree().process_frame
	var board: QuestBoard = m.board
	var shown := board.visible and get_tree().paused and board._list.get_child_count() >= 3
	board.close_board()
	await get_tree().process_frame
	check("E at the Quest Board opens it (paused) with your quests and the offers", shown)
	q.active.clear()


func t_shop() -> void:
	print("-- economy: item values, the merchant, quest cooldowns")
	var got := {}
	var same := true
	for id: String in EXPECTED_VALUES:
		got[id] = Economy.value(id)
		same = same and int(got[id]) == int(EXPECTED_VALUES[id])
	check("item values follow the gold-value table (same numbers as the Python sim)", same, str(got))
	var worthless: Array = []
	for id: String in DataStore.items():
		if Economy.value(id) <= 0:
			worthless.append(id)
	check("every item is worth something (trade-off gear is never negative)", worthless.is_empty(), str(worthless))
	var shop: Merchant = m.merchant
	var s := p.sheet
	var saved_gold := s.gold
	var saved_grid := s.grid.duplicate_bag()
	var saved_bag := s.bag.duplicate()
	var saved_stock := shop.stock.duplicate()
	s.gold = 0
	var broke := shop.buy(s, "chain_vest")
	s.gold = 5000
	var left0 := int(shop.stock["chain_vest"])
	var cost := shop.price("chain_vest")
	var bought := shop.buy(s, "chain_vest")
	check("buying: refused without the gold; with it the price is paid, the item is in the backpack, one fewer on the shelf",
		not bool(broke[0]) and bool(bought[0]) and s.gold == 5000 - cost and int(s.bag.get("chain_vest", 0)) >= 1 and int(shop.stock["chain_vest"]) == left0 - 1,
		"%s / %s" % [broke[1], bought[1]])
	s.add_item("slime_gel", 5)
	var g0 := s.gold
	var sold := shop.sell(s, "slime_gel", 5)
	var each := shop.offer("slime_gel")
	check("selling: monster materials fetch 60%% of their value (5 slime gel = 5 x %d g)" % each,
		bool(sold[0]) and s.gold == g0 + 5 * each and each == Economy.round_half_up(Economy.value("slime_gel") * 0.6), str(sold[1]))
	while s.add_item("ring_ogre"):
		pass
	var g1 := s.gold
	var full := shop.buy(s, "iron_helm")
	check("a full backpack refuses a purchase and keeps your gold", not bool(full[0]) and s.gold == g1, str(full[1]))
	shop.stock["iron_helm"] = 0
	shop._restock_t = 9999.0
	shop._process(0.0)
	check("sold-out goods come back on the shelf over time", int(shop.stock["iron_helm"]) == 1)
	s.grid = saved_grid
	s.bag = saved_bag
	s.gold = saved_gold
	shop.stock = saved_stock
	s.invalidate()
	# the stall: E at the counter opens the shop
	await put(m.arena.spots["merchant"] + Vector3(-2.9, 0.1, 0.0), Vector3(1, 0, 0))
	await press("just_interact")
	await get_tree().process_frame
	await get_tree().process_frame
	var win: ShopWindow = m.shop
	var shown := win.visible and get_tree().paused and win._list.get_child_count() >= 4
	win.close_window()
	await get_tree().process_frame
	check("E at the merchant's counter opens the shop (paused) with the wares and your backpack", shown)
	# repeatable quests rest after you turn them in, then come back
	var q := p.quests
	q.active.clear()
	q.ready_at.clear()
	q.accept("slime_cull")
	for i in 5:
		m.mobs.reward("slime", Vector3(0, 0, -80), p)
	var r := q.turn_in("slime_cull")
	var resting := bool(r[0]) and not q.available().has("slime_cull") and q.cooling().size() == 1
	q.clock += float(q.quest("slime_cull").get("cooldown", 0)) + 1.0
	check("a repeatable quest rests after you turn it in, then comes back", resting and q.available().has("slime_cull"))
	q.active.clear()
