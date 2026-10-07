class_name QuestLog
extends Node
## The player's guild record (data/quests.json): active quests and their progress, rank points and
## the F..S rank, quests finished, and a tally of every monster killed (shown on the Hunter's Card).
## Kill objectives count the kills the player lands (Mobs.killed); collect objectives are handed
## over from the backpack when the quest is turned in.

signal changed

var player: Player
var data := {}
var active := {}          # quest id -> Array of progress counts (one per objective)
var finished := {}        # quest id -> times completed
var kills := {}           # monster kind -> count
var clock := 0.0          # seconds of play (quest cooldowns)
var ready_at := {}        # quest id -> clock time it is back on the board


func setup(p: Player, mobs: Mobs) -> void:
	player = p
	data = DataStore.load_json("quests.json")
	if mobs != null:
		mobs.killed.connect(_on_killed)


func _process(delta: float) -> void:
	clock += delta


func quest(id: String) -> Dictionary:
	return data["quests"].get(id, {})


## The player's guild rank ("F-" .. "S++"), from their XP (Rank).
func rank() -> String:
	return player.sheet.rank_name()


## XP for the next rank step: [have, need, its name], or [] at the top.
func next_rank() -> Array:
	var st := player.sheet.rank_step()
	if st + 1 >= Rank.steps():
		return []
	return [player.sheet.xp, Rank.threshold(st + 1), Rank.name(st + 1)]


## Quests the board offers you now: your rank allows them, not already taken, repeatable or new.
func available() -> Array:
	var out: Array = []
	for id: String in data["quests"]:
		var q: Dictionary = data["quests"][id]
		if active.has(id) or (finished.has(id) and not bool(q.get("repeatable", false))):
			continue
		if clock < float(ready_at.get(id, 0.0)):
			continue
		if Rank.letter_index(str(q.get("rank", "F"))) <= Rank.letter_of(player.sheet.rank_step()):
			out.append(id)
	return out


func accept(id: String) -> Array:
	var q := quest(id)
	if q.is_empty() or not available().has(id):
		return [false, "That quest is not on offer"]
	if active.size() >= int(data.get("max_active", 3)):
		return [false, "You can carry %d quests at once" % int(data.get("max_active", 3))]
	var prog: Array = []
	for o in q["objectives"]:
		prog.append(0)
	active[id] = prog
	changed.emit()
	return [true, "Accepted: " + str(q["name"])]


## How far each objective is: [[text, have, need], ...]. Collect objectives count what you carry.
func progress(id: String) -> Array:
	var q := quest(id)
	var out: Array = []
	var prog: Array = active.get(id, [])
	for k in q["objectives"].size():
		var o: Dictionary = q["objectives"][k]
		var need := int(o["count"])
		if o["type"] == "kill":
			out.append(["Defeat %s" % _plural(str(o["target"])), mini(int(prog[k]) if k < prog.size() else 0, need), need])
		else:
			var item := str(o["item"])
			out.append(["Bring %s" % str(player.sheet.item_data(item).get("name", item)), mini(int(player.sheet.bag.get(item, 0)), need), need])
	return out


func complete(id: String) -> bool:
	if not active.has(id):
		return false
	for row in progress(id):
		if int(row[1]) < int(row[2]):
			return false
	return true


## Hand the quest in: collected items leave the backpack, rewards arrive. [ok, message]
func turn_in(id: String) -> Array:
	if not complete(id):
		return [false, "Not finished yet"]
	var q := quest(id)
	for o in q["objectives"]:
		if o["type"] == "collect":
			player.sheet.remove_item(str(o["item"]), int(o["count"]))
	var r: Dictionary = q.get("rewards", {})
	var before := rank()
	player.sheet.gold += int(r.get("gold", 0))
	for it in r.get("items", []):
		if not player.sheet.add_item(str(it[0]), int(it[1])):
			var pk := ItemPickup.make(str(it[0]), int(it[1]))
			player.get_parent().add_child(pk)
			pk.global_position = player.global_position + player.facing * 0.8
	active.erase(id)
	finished[id] = int(finished.get(id, 0)) + 1
	if bool(q.get("repeatable", false)):
		ready_at[id] = clock + float(q.get("cooldown", 0))
	player.gain_xp(int(r.get("xp", 0)), true)
	var msg := "%s complete: +%d gold, +%d XP" % [q["name"], int(r.get("gold", 0)), int(r.get("xp", 0))]
	if rank() != before:
		msg += "   RANK UP: %s" % rank()
	changed.emit()
	return [true, msg]


## Repeatable quests resting before they come back: [[id, seconds left], ...].
func cooling() -> Array:
	var out: Array = []
	for id: String in ready_at:
		var left := float(ready_at[id]) - clock
		if left > 0.0:
			out.append([id, left])
	return out


func abandon(id: String) -> void:
	active.erase(id)
	changed.emit()


func _on_killed(kind: String, killer: Player) -> void:
	if killer != player:
		return
	kills[kind] = int(kills.get(kind, 0)) + 1
	for id: String in active:
		var q := quest(id)
		var prog: Array = active[id]
		for k in q["objectives"].size():
			var o: Dictionary = q["objectives"][k]
			if o["type"] == "kill" and o["target"] == kind and int(prog[k]) < int(o["count"]):
				prog[k] = int(prog[k]) + 1
				if complete(id):
					player.toast.emit("%s: done - turn it in at the Quest Board" % q["name"])
	changed.emit()


func _plural(kind: String) -> String:
	var d: Dictionary = DataStore.load_json("monsters.json").get(kind, {})
	return str(d.get("name", kind.capitalize())) + "s"
