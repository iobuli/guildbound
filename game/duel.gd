class_name Duel
extends Node
## A refereed 1v1 in an arena (data/arena.json). Entry needs a minimum rank. When the ranks differ,
## the referee (an NPC mage) evens the field for the lower rank, without handing them the win:
##   - their health is raised to at least `hp_floor` (80%) of the opponent's (health scales with rank)
##   - they get random status effects from the referee's pool (tougher skin, faster swings, lifesteal,
##     crits...): more of them the wider the gap (1 + one per 2 steps, up to 4), stronger per letter
## Their damage stays their own. Everything the referee gave is taken back when the duel ends; the
## winner, if it is the player, gets the XP the loser's rank is worth.

signal finished(winner: Player, loser: Player)

var arena_id := "yard"
var data := {}
var a: Player
var b: Player
var active := false
var granted: Array = []       # buff names the referee gave this duel
var rng := RandomNumberGenerator.new()


func setup(id: String) -> void:
	arena_id = id
	data = DataStore.load_json("arena.json")[id]
	rng.randomize()


func can_enter(f: Player) -> bool:
	return f.sheet.rank_step() >= Rank.parse(str(data.get("min_rank", "F-")))


## Start the duel: [ok, what the referee said].
func start(p1: Player, p2: Player) -> Array:
	if active:
		return [false, "A duel is already on"]
	for f in [p1, p2]:
		if not can_enter(f):
			return [false, "%s is below this arena's minimum rank (%s)" % [f.sheet.display_name, data.get("min_rank", "F-")]]
	a = p1
	b = p2
	for f in [a, b]:
		_clear(f)
		if not f.died.is_connected(_on_died):
			f.died.connect(_on_died)
	granted = []
	var ref := str(data.get("referee", "The referee"))
	var msg := "%s: %s [%s] vs %s [%s]" % [ref, a.sheet.display_name, a.sheet.rank_name(), b.sheet.display_name, b.sheet.rank_name()]
	var sa := a.sheet.rank_step()
	var sb := b.sheet.rank_step()
	if sa != sb:
		var lo: Player = a if sa < sb else b
		var hi: Player = b if sa < sb else a
		_handicap(lo, hi, absi(sa - sb), absi(Rank.letter_of(sa) - Rank.letter_of(sb)))
		msg += ".  %s grants %s: %s" % [ref, lo.sheet.display_name, ", ".join(granted)]
		Vfx.cast_flare(lo, "light", "might")
	for f in [a, b]:
		f.sheet.restore()
	active = true
	return [true, msg]


func _handicap(lo: Player, hi: Player, steps: int, letters: int) -> void:
	var s := lo.sheet
	var floor_hp := float(data.get("hp_floor", 0.8)) * hi.sheet.v("max_hp")
	if s.v("max_hp") < floor_hp:
		_effect(s, "Referee's Vigour", "S_Holy01", [Stats.make_mod("Referee's Vigour", "max_hp", "more", floor_hp / s.v("max_hp"))])
		granted.append("Vigour (health to %d)" % int(floor_hp))
	var pool: Array = (data["buffs"] as Array).duplicate()
	var n := mini(int(data.get("buffs_max", 4)), int(data.get("buffs_base", 1)) + floori(float(steps) / int(data.get("buffs_per_steps", 2))))
	var scale := 1.0 + float(data.get("magnitude_per_letter", 0.5)) * letters
	for i in mini(n, pool.size()):
		var bf: Dictionary = pool.pop_at(rng.randi_range(0, pool.size() - 1))
		var mods: Array = []
		for m in bf["mods"]:
			mods.append(Stats.make_mod("Referee: " + str(bf["name"]), m["stat"], m["op"], float(m["value"]) * scale))
		_effect(s, str(bf["name"]), str(bf.get("icon", "S_Buff01")), mods)
		granted.append(str(bf["name"]))


func _effect(s: CharModel, nm: String, icon: String, mods: Array) -> void:
	s.effects.append({"key": "referee:" + nm, "name": nm, "kind": "referee", "icon": icon, "expires_at": null,
		"slots": 0, "upkeep": 0.0, "hp_drain": 0.0, "mods": mods, "backlash": null, "aspect": "", "family": "referee"})
	s.invalidate()


## Take back everything the referee gave.
func _clear(f: Player) -> void:
	var s := f.sheet
	s.effects = s.effects.filter(func(e: Dictionary) -> bool: return e["kind"] != "referee")
	s.invalidate()
	s.hp = minf(s.hp, s.v("max_hp"))


func referee_effects(f: Player) -> Array:
	return f.sheet.effects.filter(func(e: Dictionary) -> bool: return e["kind"] == "referee")


func _on_died(victim: Player, _killer: Player) -> void:
	if active and (victim == a or victim == b):
		end(b if victim == a else a)


## End the duel (`winner` null: called off). The referee's gifts are taken back.
func end(winner: Player) -> void:
	if not active:
		return
	active = false
	var loser: Player = null
	if winner != null:
		loser = b if winner == a else a
	for f in [a, b]:
		if is_instance_valid(f):
			_clear(f)
	if winner != null and not winner.is_ai:
		winner.gain_xp(Rank.kill_xp(loser.sheet.rank_step(), winner.sheet.rank_step()))
	if winner != null:
		winner.toast.emit("%s wins the duel" % winner.sheet.display_name)
	finished.emit(winner, loser)
