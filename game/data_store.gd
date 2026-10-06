class_name DataStore
extends RefCounted
## Loads the same JSON content the Python reference sim uses (res://data).

static var _cache := {}


static func load_json(rel: String) -> Dictionary:
	if _cache.has(rel):
		return _cache[rel]
	var f := FileAccess.open("res://data/" + rel, FileAccess.READ)
	assert(f != null, "cannot open data/" + rel)
	var parsed = JSON.parse_string(f.get_as_text())
	_cache[rel] = parsed
	return parsed


static func races() -> Dictionary:
	return load_json("races.json")


static func items() -> Dictionary:
	return load_json("items.json")["items"]


static func consumables() -> Dictionary:
	return load_json("items.json").get("consumables", {})


static func spells() -> Dictionary:
	return load_json("spells.json")["spells"]
