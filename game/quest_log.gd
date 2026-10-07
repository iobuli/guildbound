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
var rank_points := 0
var kills := {}           # monster kind -> count


func setup(p: Player, mobs: Mobs) -> void:
	player = p
	data = DataStore.load_json("quests.json")
	if mobs != null:
		mobs.killed.connect(_on_killed)


func quest(id: String) -> Dictionary:
	return data["quests"].get(id, {})


func rank() -> String:
	var r := "F"
	for step in data["ranks"]:
		if rank_points >= int(step[1]):
			r = str(step[0])
	return r


func rank_index(letter: String) -> int:
	for i in data["ranks"].size():
		if data["ranks"][i][0] == letter:
			return i
	return 0


## Points needed for the next rank ([have, need]), or [] at the top.
func next_rank() -> Array:
	var i := rank_index(rank())
	if i + 1 >= data["ranks"].size():
		return []
	return [rank_points, int(data["ranks"][i + 1][1]), str(data["ranks"][i + 1][0])]


## Quests the board offers you now: your rank allows them, not already taken, repeatable or new.
func available() -> Array:
	var out: Array = []
	for id: String in data["quests"]:
		var q: Dictionary = data["quests"][id]
		if active.has(id) or (finished.has(id) and not bool(q.get("repeatable", false))):
			continue
		if rank_index(str(q.get("rank", "F"))) <= rank_index(rank()):
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
	rank_points += int(r.get("rank_points", 0))
	for it in r.get("items", []):
		if not player.sheet.add_item(str(it[0]), int(it[1])):
			var pk := ItemPickup.make(str(it[0]), int(it[1]))
			player.get_parent().add_child(pk)
			pk.global_position = player.global_position + player.facing * 0.8
	active.erase(id)
	finished[id] = int(finished.get(id, 0)) + 1
	var msg := "%s complete: +%d gold, +%d rank points" % [q["name"], int(r.get("gold", 0)), int(r.get("rank_points", 0))]
	if rank() != before:
		msg += "   RANK UP: %s" % rank()
	changed.emit()
	return [true, msg]


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
