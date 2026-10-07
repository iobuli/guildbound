class_name Mobs
extends Node
## PvE monsters (data/monsters.json): spawns them, rolls their loot when they die and tells the rest
## of the game about the kill (`killed`, for quests). Slimes are Slime nodes; goblins are full
## fighters (Player + FighterAI "pack") on the "goblin" team, smaller and tinted green. They hunt as
## a pack: each wears the "Goblin frame" (light and quick: far less health, lighter blows, faster feet
## and hands) and the pack shares its attack tokens, so alone a goblin is easy, six of them are not.
## Loot is OSRS-style: the "always" drops, then weighted rolls out of "denom", then each Magic Core
## rolls 1 in N on its own. Items land on the ground at the body; gold goes straight to the killer.

signal killed(kind: String, killer: Player)

static var instance: Mobs

var main: Node
var data := {}
var rng := RandomNumberGenerator.new()


func setup(p_main: Node) -> void:
	main = p_main
	instance = self
	data = DataStore.load_json("monsters.json")
	rng.seed = 1234


func _exit_tree() -> void:
	if instance == self:
		instance = null


func spawn_slimes(center: Vector3, n: int = -1) -> Array[Slime]:
	var d: Dictionary = data["slime"]
	var out: Array[Slime] = []
	if n < 0:
		n = int(d.get("count", 6))
	for i in n:
		var a := TAU * i / maxf(1.0, n) + rng.randf_range(-0.3, 0.3)
		var r := rng.randf_range(3.0, 11.0)
		var s := Slime.make(d, center + Vector3(cos(a) * r, 0.3, sin(a) * r), 100 + i)
		main.add_child(s)
		out.append(s)
	return out


func spawn_goblins(center: Vector3, n: int = -1) -> Array[Player]:
	var d: Dictionary = data["goblin"]
	var out: Array[Player] = []
	if n < 0:
		n = int(d.get("count", 3))
	var pk: Dictionary = d.get("pack", {})
	for i in n:
		var a := TAU * i / maxf(1.0, n) + 0.5
		var r := 4.0 + 1.3 * (i % 2)  # two loose rings around the fire
		var pos := center + Vector3(cos(a) * r, 0.1, sin(a) * r)
		var sheet := CharModel.new(str(d["name"]), str(d.get("race", "human")), int(d.get("level", 3)))
		sheet.rank_override = str(d.get("rank", ""))
		apply_frame(sheet, d)
		sheet.restore()
		var ai := FighterAI.new("pack")
		ai.home = pos
		ai.pack = "goblin"
		ai.max_attackers = int(pk.get("attackers", 2))
		ai.ring = float(pk.get("ring", 3.6))
		ai.leash = float(pk.get("leash", 30.0))
		ai.rng.seed = 9001 + i * 31
		var g: Player = main.spawn_fighter(sheet, pos, ai, "Superhero_Male_FullBody", str(d.get("stance", "dagger")), str(d.get("hair", "")))
		g.team = "goblin"
		g.respawn_after = float(d.get("respawn", 20.0))
		g.set_meta("monster", "goblin")
		g.model.scale = Vector3.ONE * float(d.get("scale", 0.8))
		g.model.set_proportions(d.get("proportions", HeroAnim.GOBLIN_PROPORTIONS))  # big head and hands, wiry limbs
		var t: Array = d.get("tint", [0.55, 0.85, 0.45])
		g.model.tint(Color(t[0], t[1], t[2]))
		g.died.connect(_on_fighter_died)
		out.append(g)
	return out


## A monster's build on top of its level and rank: a permanent effect on its sheet ("frame" in
## data/monsters.json; for goblins: far less health, lighter blows, more move and attack speed).
static func apply_frame(sheet: CharModel, d: Dictionary) -> void:
	sheet.effects = sheet.effects.filter(func(e: Dictionary) -> bool: return e["kind"] != "frame")
	var fr: Dictionary = d.get("frame", {})
	if not fr.is_empty():
		var nm := str(fr.get("name", "Frame"))
		var mods: Array = []
		for m: Dictionary in fr.get("mods", []):
			mods.append(Stats.make_mod(nm, str(m["stat"]), str(m["op"]), float(m["value"])))
		sheet.effects.append({"key": "frame", "name": nm, "kind": "frame", "icon": str(fr.get("icon", "S_Buff01")), "expires_at": null,
			"slots": 0, "upkeep": 0.0, "hp_drain": 0.0, "mods": mods, "backlash": null, "aspect": "", "family": "frame"})
	sheet.invalidate()


func _on_fighter_died(victim: Player, killer: Player) -> void:
	var kind := str(victim.get_meta("monster", ""))
	if kind != "":
		reward(kind, victim.global_position, killer)


## What one kill drops: {"gold": int, "items": [[id, n], ...]}.
func roll(kind: String) -> Dictionary:
	var loot: Dictionary = data[kind].get("loot", {})
	var items: Array = []
	for a in loot.get("always", []):
		items.append([str(a[0]), int(a[1])])
	var table: Array = loot.get("table", [])
	var denom := float(loot.get("denom", 128))
	for k in int(loot.get("rolls", 0)):
		var pick := rng.randf() * denom
		for entry in table:
			pick -= float(entry[2])
			if pick < 0.0:
				if str(entry[0]) != "nothing":
					items.append([str(entry[0]), int(entry[1])])
				break
	for c in loot.get("cores", []):
		if rng.randi_range(1, int(c[1])) == 1:
			items.append([str(c[0]), 1])
	var g: Array = loot.get("gold", [0, 0])
	return {"gold": rng.randi_range(int(g[0]), int(g[1])), "items": items}


## A monster died at `at`: drop its loot there, pay the killer, tell everyone.
func reward(kind: String, at: Vector3, killer: Player) -> Dictionary:
	var r := roll(kind)
	var items: Array = r["items"]
	for i in items.size():
		var pk := ItemPickup.make(str(items[i][0]), int(items[i][1]))
		main.add_child(pk)
		var a := TAU * i / maxf(1.0, items.size())
		pk.global_position = Vector3(at.x + cos(a) * 0.5, maxf(at.y, 0.0), at.z + sin(a) * 0.5)
	r["xp"] = 0
	if killer != null and is_instance_valid(killer):
		killer.sheet.gold += int(r["gold"])
		if not killer.is_ai:
			r["xp"] = Rank.kill_xp(Rank.parse(str(data[kind].get("rank", "F-"))), killer.sheet.rank_step())
		killer.toast.emit("+%d gold   +%d XP" % [int(r["gold"]), int(r["xp"])])
		killer.gain_xp(int(r["xp"]), true)
	killed.emit(kind, killer)
	return r
