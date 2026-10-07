class_name Economy
extends RefCounted
## What things are worth. One rule set, mirrored by sim/character.py item_price:
##   gear        the gold value of its stats (data/items.json "gold_values": MOBA-style gold
##               efficiency, docs/MAGIC_AND_STATS.md 7.3) + weapon damage + premium; penalties count
##               against it, but it is never worth less than a quarter of its bonuses (or 10)
##   consumables 10 + premium + the gold value of their stats x duration / one hour
##   materials   their premium (monster parts, Magic Cores)
## Rounded half up, so the game and the Python sim agree to the gold piece.


static func round_half_up(x: float) -> int:
	return int(floor(x + 0.5))


static func is_consumable(id: String) -> bool:
	return DataStore.consumables().has(id)


## "gear", "consumable", "core" or "material": what a merchant files it under.
static func category(id: String) -> String:
	if is_consumable(id):
		return "consumable"
	var d: Dictionary = DataStore.items().get(id, {})
	if d.get("slot") != null:
		return "gear"
	return "core" if id.begins_with("core_") else "material"


static func value(id: String) -> int:
	var gv: Dictionary = DataStore.load_json("items.json")["gold_values"]
	if is_consumable(id):
		var c: Dictionary = DataStore.consumables()[id]
		var t := 10.0 + float(c.get("premium", 0))
		for m in c.get("mods", []):
			t += float((gv.get(m["stat"], {}) as Dictionary).get(m["op"], 0)) * float(m["value"]) * float(c.get("duration", 0)) / 3600.0
		return round_half_up(t)
	var d: Dictionary = DataStore.items().get(id, {})
	if d.is_empty():
		return 0
	var pos := float(d.get("premium", 0))
	var neg := 0.0
	for m in d.get("mods", []):
		if m["op"] == "more":
			continue  # unique multipliers are priced through the item's premium
		var v := float((gv.get(m["stat"], {}) as Dictionary).get(m["op"], 0)) * float(m["value"])
		if v >= 0.0:
			pos += v
		else:
			neg += v
	var w: Dictionary = d.get("weapon", {})
	if not w.is_empty():
		pos += float(w["base_damage"]) * float(gv["attack_damage"]["flat"]) + (float(w["aps"]) - 1.0) * float(gv["attack_speed"]["add_pct"])
	if category(id) == "gear":
		return round_half_up(maxf(pos + neg, maxf(0.25 * pos, 10.0)))
	return round_half_up(pos + neg)
