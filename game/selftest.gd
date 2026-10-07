class_name SelfTest
extends Node
## Automated check of the controller, camera-free combat and UI, driving the real player through a
## puppet brain (the same Intent the keyboard fills).
##   Godot --headless --path <guildbound> -- --selftest [--only=move,dodge,melee,guard,ranged,ai,ui,grid,mobs,quests,shop,rank,duel,magic,flow]

var m: Node
var p: Player
var ai: FighterAI
var results: Array[bool] = []
## Item values (the Python sim checks the same numbers: tests/test_stats.py EconomyValues).
const EXPECTED_VALUES := {"ring_ogre": 495, "brawler_wraps": 580, "sprinter_greaves": 650, "hearty_stew": 67, "swift_tonic": 37, "keen_eye_tea": 18, "ironbark_draught": 28, "slime_gel": 6, "hide_bundle": 30, "core_common": 40, "leather_cap": 120, "iron_sword": 770}


func check(what: String, ok: bool, detail: String = "") -> void:
	results.append(ok)
	print("[%s] %s%s" % ["PASS" if ok else "FAIL", what, "  -  " + detail if detail != "" else ""])


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
		["grid", t_grid], ["mobs", t_mobs], ["quests", t_quests], ["shop", t_shop], ["rank", t_rank], ["duel", t_duel], ["magic", t_magic], ["flow", t_flow]]
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
	var run_m := p.global_position.distance_to(a)
	var bp: Vector2 = p.model.tree.get("parameters/loco/blend_position")
	var anim_ok := bp.length() > 1.2
	ai.puppet.sprint = true
	a = p.global_position
	await frames(60)
	var sprint := p.global_position.distance_to(a)
	ai.puppet.sprint = false
	ai.puppet.move = Vector2.ZERO
	await frames(30)
	check("running moves forward at run_m pace and plays the run_m blend; sprint is faster", run_m > 3.0 and sprint > run_m * 1.2 and anim_ok,
		"run_m %.2f m/s, sprint %.2f m/s, blend %s" % [run_m, sprint, str(p.model.tree.get("parameters/loco/blend_position"))])
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
	var tips_ok := true
	for id: String in DataStore.items().keys() + DataStore.consumables().keys():
		tips_ok = tips_ok and m.status.item_tip(id, true).length() > 0 and sp._item_tip(id).length() > 0
	check("every item's tooltip builds (gear, food, monster materials, cores)", tips_ok)
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
	p.sheet.xp = 0
	p.sheet.invalidate()
	q.ready_at.clear()
	var offer := q.available()
	check("a new hunter (rank F-) sees the F quests, not the E ones", q.rank() == "F-" and offer.has("slime_cull") and not offer.has("goblin_trouble"), str(offer))
	q.accept("slime_cull")
	var spot: Vector3 = m.arena.spots["run_lane"]
	await put(spot)
	for i in 4:
		mobs.reward("slime", spot + Vector3(0, 0, -30), p)  # kills credited to the player
	mobs.reward("slime", spot + Vector3(0, 0, -30), null)  # someone else's kill does not count
	var four: bool = int(q.progress("slime_cull")[0][1]) == 4 and not q.complete("slime_cull")
	mobs.reward("slime", spot + Vector3(0, 0, -30), p)
	var gold0 := p.sheet.gold
	var xp0 := p.sheet.xp
	var cores0 := int(p.sheet.bag.get("core_common", 0))
	var r := q.turn_in("slime_cull")
	check("Slime Cull: 5 slime kills by the player complete it; turning in pays gold, XP and a core",
		four and bool(r[0]) and p.sheet.gold == gold0 + 60 and p.sheet.xp == xp0 + 60 and int(p.sheet.bag.get("core_common", 0)) == cores0 + 1, str(r[1]))
	q.accept("sticky_business")
	var not_yet := not q.complete("sticky_business")
	var gel0 := int(p.sheet.bag.get("slime_gel", 0))
	var added := maxi(0, 6 - gel0)
	p.sheet.add_item("slime_gel", added)
	var r2 := q.turn_in("sticky_business")
	check("a collect quest takes the items from the backpack when turned in",
		not_yet and bool(r2[0]) and int(p.sheet.bag.get("slime_gel", 0)) == gel0 + added - 6, str(r2[1]))
	p.sheet.xp = 499
	p.sheet.invalidate()
	var still_f := q.rank() == "F++" and not q.available().has("goblin_trouble")
	p.sheet.xp = 500
	p.sheet.invalidate()
	check("500 XP makes rank E- and opens the goblin quests", still_f and q.rank() == "E-" and q.available().has("goblin_trouble"))
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
	p.sheet.xp = 0
	p.sheet.invalidate()
	p.sheet.restore()


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


func t_rank() -> void:
	print("-- guild rank: XP, power per letter, titles, kill XP")
	var names: Array = []
	for x in [0, 125, 250, 375, 500, 4000, 30000]:
		names.append(Rank.name(Rank.step_for_xp(x)))
	check("XP sets the rank: F- F F+ F++, then E- at 500, D- at 4000, C- at 30000", names == ["F-", "F", "F+", "F++", "E-", "D-", "C-"], str(names))
	check("each letter doubles power, each step inside a letter adds 15% (F++ x1.45, E- x2, D- x4, S++ ~x93)",
		is_equal_approx(Rank.power(Rank.parse("F++")), 1.45) and is_equal_approx(Rank.power(Rank.parse("E-")), 2.0)
		and is_equal_approx(Rank.power(Rank.parse("D-")), 4.0) and is_equal_approx(Rank.power(Rank.parse("S++")), 64.0 * 1.45))
	var s := CharModel.new("Recruit", "human", 5)
	var hp0 := s.v("max_hp")
	var ad0 := s.v("attack_damage")
	var r := s.gain_xp(500)
	check("reaching E- doubles health and damage; health keeps its share of the new maximum",
		Rank.name(r[1]) == "E-" and is_equal_approx(s.v("max_hp"), hp0 * 2.0) and is_equal_approx(s.v("attack_damage"), ad0 * 2.0) and is_equal_approx(s.hp, s.v("max_hp")),
		"hp %.0f -> %.0f, ad %.1f -> %.1f" % [hp0, s.v("max_hp"), ad0, s.v("attack_damage")])
	s.gain_xp(-400)
	check("XP is never lost, so the rank never drops (no smurfing)", s.xp == 500 and s.rank_name() == "E-")
	check("monsters pay XP by their rank; below your letter, a quarter per letter",
		Rank.kill_xp(Rank.parse("F-"), Rank.parse("F-")) == 10 and Rank.kill_xp(Rank.parse("E-"), Rank.parse("F-")) == 40
		and Rank.kill_xp(Rank.parse("F-"), Rank.parse("E-")) == 3 and Rank.kill_xp(Rank.parse("F-"), Rank.parse("D-")) == 1)
	var k: CharModel = m.build_kaelen()
	var t0 := Rank.title(k)
	k.gold = 100000
	var t1 := Rank.title(k)
	check("titles: a hunter near the norm for their rank is an Adventurer; far richer is a Rich Adventurer", t0 == "Adventurer" and t1 == "Rich Adventurer", "%s / %s" % [t0, t1])
	var xp0 := p.sheet.xp
	m.mobs.reward("slime", Vector3(0, 0, -80), p)
	check("a slime killed by the player gives XP", p.sheet.xp == xp0 + 10, "xp %d -> %d" % [xp0, p.sheet.xp])
	var gob: Player = _goblins()[0]
	check("goblins are rank E-: double the health and damage of the same fighter at F-",
		gob.sheet.rank_name() == "E-" and is_equal_approx(Rank.power(gob.sheet.rank_step()), 2.0))
	var nu := Rank.next_unlock(Rank.parse("E"))
	check("ranks open things up (D- opens a class specialisation)", nu.size() == 2 and nu[0] == "D-", str(nu))
	p.sheet.xp = 0
	p.sheet.invalidate()
	p.sheet.restore()


func t_duel() -> void:
	print("-- refereed duel: minimum rank, the referee's handicap for the lower rank")
	var duel: Duel = m.duel
	var sp: Player = m.spar
	var sb := sp.get_node("ai") as FighterAI
	sb.mode = "dummy"
	p.sheet.xp = 0
	p.sheet.invalidate()
	var hp_alone := p.sheet.v("max_hp")
	var saved_min: String = duel.data["min_rank"]
	duel.data["min_rank"] = "E-"
	var r0 := duel.start(p, sp)
	duel.data["min_rank"] = saved_min
	check("an arena turns away hunters below its minimum rank", not bool(r0[0]) and not duel.active, str(r0[1]))
	await put(m.SPAR_POS + Vector3(0, 0.1, 5.0))
	m.start_yard_duel()
	var mine := duel.referee_effects(p)
	var buffs := mine.filter(func(e: Dictionary) -> bool: return e["name"] != "Referee's Vigour")
	var floor_ok := p.sheet.v("max_hp") >= 0.8 * sp.sheet.v("max_hp") - 0.5
	check("F- vs an E- opponent: the referee lifts the F-'s health to 80% of the opponent's and grants 3 random buffs (1 + 4 steps / 2)",
		duel.active and floor_ok and buffs.size() == 3 and duel.referee_effects(sp).is_empty() and is_equal_approx(p.sheet.hp, p.sheet.v("max_hp")),
		"%s; hp %.0f vs %.0f" % [str(duel.granted), p.sheet.v("max_hp"), sp.sheet.v("max_hp")])
	var bf: Dictionary = buffs[0] if buffs.size() > 0 else {}
	var base := {}
	for b in duel.data["buffs"]:
		base[b["name"]] = float(b["mods"][0]["value"])
	check("a letter apart, each buff is 1.5x as strong", not bf.is_empty() and is_equal_approx(float(bf["mods"][0]["value"]), base[bf["name"]] * 1.5))
	check("the lower rank still hits with its own damage (no free win)", is_equal_approx(p.sheet.v("attack_damage"), m.build_kaelen().v("attack_damage")))
	var xp0 := p.sheet.xp
	sp.last_hit_by = p
	sp.combat.die(Vector3(0, 0, -1))
	await frames(2)
	check("the duel ends when one goes down: the referee takes the buffs back, the winner gets the loser's XP",
		not duel.active and duel.referee_effects(p).is_empty() and is_equal_approx(p.sheet.v("max_hp"), hp_alone) and p.sheet.xp == xp0 + 40 and sp.sheet.rank_override == "",
		"xp +%d, max hp %.0f" % [p.sheet.xp - xp0, p.sheet.v("max_hp")])
	await frames(int(Player.RESPAWN_AFTER * 60) + 10)
	var r2 := duel.start(p, sp)
	check("equal ranks: no handicap", bool(r2[0]) and duel.referee_effects(p).is_empty() and duel.referee_effects(sp).is_empty(), str(r2[1]))
	duel.end(null)
	p.sheet.xp = 0
	p.sheet.invalidate()
	p.sheet.restore()


func t_magic() -> void:
	print("-- elements: spells, statuses, Awakening; fists and kicks")
	var dm: Player = m.dummy
	var cb := p.combat
	var elems: Array = HeroAnim.data["_elements"].keys()
	var resolved := true
	for e: String in elems:
		cb.element = e
		resolved = resolved and HeroAnim.data["_spells"].has(cb.spell_id("@light")) and HeroAnim.data["_spells"].has(cb.spell_id("@heavy"))
	cb.element = "fire"
	check("six elements, each with its own light and heavy spell (staff: LMB / hold)", elems.size() == 6 and resolved, str(elems))
	var order := []
	for k in 6:
		cb.next_element()
		order.append(cb.element)
	check("X cycles the element and comes back round", order[5] == "fire" and order[0] == "ice", str(order))
	# statuses on the dummy
	(dm.get_node("ai") as FighterAI).mode = "puppet"
	dm.teleport(m.DUMMY_POS)
	await frames(5)
	var hp0 := dm.sheet.hp
	StatusFx.apply(dm, "burn", 30.0, 1.0, p)
	await frames(70)
	check("burn: damage over time (30 a second for a second)", hp0 - dm.sheet.hp > 24.0 and hp0 - dm.sheet.hp < 36.0, "took %.1f" % (hp0 - dm.sheet.hp))
	var ms0 := dm.sheet.v("move_speed")
	StatusFx.apply(dm, "chill", 0.4, 2.0, p)
	check("chill: slower movement", dm.sheet.v("move_speed") < ms0 * 0.7, "%.2f -> %.2f" % [ms0, dm.sheet.v("move_speed")])
	# a lightning bolt chains from the dummy to the sparring partner next to it
	var sp: Player = m.spar
	(sp.get_node("ai") as FighterAI).mode = "dummy"
	sp.teleport(m.DUMMY_POS + Vector3(2.0, 0, 0))
	await put(m.DUMMY_POS + Vector3(0, 0.1, 6.0))
	await frames(5)
	var sp0 := sp.sheet.hp
	var d0 := dm.sheet.hp
	var tb: Dictionary = HeroAnim.data["_spells"]["thunderbolt"]
	var from: Vector3 = m.DUMMY_POS + Vector3(0, 1.0, 1.5)
	var pr := Projectile.make(p, "thunderbolt", from, Vector3(0, 0, -30), 0.0, 1.0, 10.0, float(tb["radius"]))
	pr.configure(tb)
	m.add_child(pr)
	await frames(20)
	check("lightning chains: the bolt hits the dummy, then jumps to the fighter beside it", dm.sheet.hp < d0 and sp.sheet.hp < sp0,
		"dummy -%.0f, partner -%.0f" % [d0 - dm.sheet.hp, sp0 - sp.sheet.hp])
	sp.teleport(m.SPAR_POS)
	# Awakening: full meter + R -> damage up, and a sword blow sets the target alight (fire)
	cb.element = "fire"
	cb.awaken = 100.0
	var ad0 := p.sheet.v("attack_damage")
	await press("just_awaken")
	await frames(2)
	check("R with a full meter Awakens: +30% damage", cb.awakened() and is_equal_approx(p.sheet.v("attack_damage"), ad0 * 1.3), "%.1f -> %.1f" % [ad0, p.sheet.v("attack_damage")])
	dm.teleport(m.DUMMY_POS)
	await put(m.DUMMY_POS + Vector3(0, 0.1, 1.25))
	p.lock.target = dm
	p.combat.set_stance("sword_shield")
	for k in 3:
		if StatusFx.instance.burning(dm):
			break
		await press("just_attack")
		await frames(30)
	check("an Awakened (fire) sword hit sets the target burning", StatusFx.instance.burning(dm))
	cb.awaken_t = 0.0
	p.sheet.effects = p.sheet.effects.filter(func(e: Dictionary) -> bool: return e["kind"] != "awakening")
	p.sheet.invalidate()
	var gain0 := cb.awaken
	cb.gain_awaken(8.0)
	check("landing hits fills the Awakening meter", cb.awaken > gain0 or cb.awaken >= 100.0)
	# fists: jab, cross, hook, then a kick
	p.combat.set_stance("unarmed")
	var lights: Array = p.combat.stance["light"]
	check("fists: jab, cross, hook and a front-kick finisher; heavy is a roundhouse kick",
		lights.size() == 4 and str(lights[3]["clip"]) == "FIST_front_kick" and str(lights[3]["limb"]) == "foot_r" and str(p.combat.stance["heavy"]["limb"]) == "foot_r")
	p.combat.set_stance("sword_shield")
	p.lock.target = null
	(dm.get_node("ai") as FighterAI).mode = "dummy"
	dm.sheet.effects.clear()
	dm.sheet.invalidate()
	dm.sheet.restore()


## Combos flow: a chained hit starts past its wind-up, and nothing in the body jumps. Measured as speed
## over the real time between pose updates (headless runs can take two physics steps per drawn pose),
## against the library's fastest swings and kicks (~15 m/s x attack speed).
func t_flow() -> void:
	print("-- animation flow: chained hits, no pops")
	var dm: Player = m.dummy
	dm.teleport(m.DUMMY_POS)
	await put(m.DUMMY_POS + Vector3(0, 0.1, 1.25))
	p.lock.target = dm
	p.combat.set_stance("sword_shield")
	var worst := 0.0
	var worst_at := ""
	var prev := {}
	var acc := {}
	var starts: Array = []
	var last_clip := ""
	for i in 150:
		ai.puppet.just_attack = (i % 25) == 0
		await frames(1)
		var inv := p.model.inst.global_transform.affine_inverse()
		for b in ["hand_r", "foot_l", "Head"]:
			var lp: Vector3 = inv * p.model.bone_pos(b)
			acc[b] = float(acc.get(b, 0.0)) + 1.0 / 60.0
			if prev.has(b) and not lp.is_equal_approx(prev[b]):
				var sp: float = lp.distance_to(prev[b]) / float(acc[b])
				if sp > worst:
					worst = sp
					worst_at = "%s, %s t %.2f" % [b, str(p.combat.move.get("clip", "-")), p.combat.t]
				acc[b] = 0.0
			prev[b] = lp
		if p.combat.attacking() and str(p.combat.move.get("clip", "")) != last_clip:
			last_clip = str(p.combat.move.get("clip", ""))
			starts.append(snappedf(p.combat.t, 0.01))
	ai.puppet.just_attack = false
	check("chained hits start past their wind-up (the first from 0)", starts.size() >= 2 and starts[0] == 0.0 and float(starts[1]) > 0.0, str(starts))
	check("no pops: no hand, foot or head moves faster than 28 m/s through a combo", worst < 28.0, "fastest %.1f m/s (%s)" % [worst, worst_at])
	p.lock.target = null
	await frames(60)
