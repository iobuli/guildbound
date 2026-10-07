class_name GridBag
extends RefCounted
## A Tarkov-style backpack: items take a w x h footprint on a grid and can be turned 90 degrees
## (w and h swap). Stackable items (food, potions, bundles) share a cell up to their max_stack.
## Sizes and stack caps come from data/items.json ("size": [w, h], "max_stack"); anything without
## a size is 1x1. Pure logic, no UI: SidePanel draws it, CharModel keeps it in step with its bag.

var w := 6
var h := 8
var entries: Array = []   # {"id": String, "x": int, "y": int, "rot": bool, "n": int}


func _init(width: int = 6, height: int = 8) -> void:
	w = width
	h = height


static func _data(id: String) -> Dictionary:
	var d: Dictionary = DataStore.items().get(id, {})
	return d if not d.is_empty() else DataStore.consumables().get(id, {})


static func size_of(id: String) -> Vector2i:
	var s: Array = _data(id).get("size", [1, 1])
	return Vector2i(int(s[0]), int(s[1]))


static func max_stack(id: String) -> int:
	return int(_data(id).get("max_stack", 1))


## The footprint of an item turned or not.
static func footprint(id: String, rot: bool) -> Vector2i:
	var s := size_of(id)
	return Vector2i(s.y, s.x) if rot else s


func _rect(e: Dictionary) -> Rect2i:
	return Rect2i(int(e["x"]), int(e["y"]), footprint(str(e["id"]), bool(e["rot"])).x, footprint(str(e["id"]), bool(e["rot"])).y)


## Would `id` fit with its top-left cell at (x, y)? `skip` is an entry to ignore (the one being moved).
func fits(id: String, x: int, y: int, rot: bool, skip: Variant = null) -> bool:
	var f := footprint(id, rot)
	if x < 0 or y < 0 or x + f.x > w or y + f.y > h:
		return false
	var r := Rect2i(x, y, f.x, f.y)
	for e in entries:
		if e != skip and _rect(e).intersects(r):
			return false
	return true


## The first free spot, scanning rows top to bottom; tries the item upright, then turned.
func find_spot(id: String) -> Dictionary:
	for rot in [false, true]:
		if rot and size_of(id).x == size_of(id).y:
			continue
		for y in h:
			for x in w:
				if fits(id, x, y, rot):
					return {"x": x, "y": y, "rot": rot}
	return {}


## The entry covering cell (x, y), or null.
func at(x: int, y: int) -> Variant:
	for e in entries:
		if _rect(e).has_point(Vector2i(x, y)):
			return e
	return null


func count(id: String) -> int:
	var n := 0
	for e in entries:
		if e["id"] == id:
			n += int(e["n"])
	return n


## Put n of `id` in: top up existing stacks, then new stacks in free spots. All or nothing.
func add(id: String, n: int = 1) -> bool:
	var trial := duplicate_bag()
	if not trial._add(id, n):
		return false
	entries = trial.entries
	return true


func _add(id: String, n: int) -> bool:
	var cap := max_stack(id)
	for e in entries:
		if n <= 0:
			break
		if e["id"] == id and int(e["n"]) < cap:
			var put := mini(cap - int(e["n"]), n)
			e["n"] = int(e["n"]) + put
			n -= put
	while n > 0:
		var spot := find_spot(id)
		if spot.is_empty():
			return false
		var put := mini(cap, n)
		entries.append({"id": id, "x": spot["x"], "y": spot["y"], "rot": spot["rot"], "n": put})
		n -= put
	return true


## Take n of `id` out (the last-placed stacks first). False (and nothing taken) if there are fewer.
func remove(id: String, n: int = 1) -> bool:
	if count(id) < n:
		return false
	for i in range(entries.size() - 1, -1, -1):
		if n <= 0:
			break
		var e: Dictionary = entries[i]
		if e["id"] != id:
			continue
		var take := mini(int(e["n"]), n)
		e["n"] = int(e["n"]) - take
		n -= take
		if int(e["n"]) <= 0:
			entries.remove_at(i)
	return true


## Move an entry (drag and drop). Dropping a stack onto the same item merges what fits.
func move(e: Dictionary, x: int, y: int, rot: bool) -> bool:
	var other: Variant = at(x, y)
	if other != null and other != e and other["id"] == e["id"] and max_stack(str(e["id"])) > 1:
		var room := max_stack(str(e["id"])) - int(other["n"])
		if room <= 0:
			return false
		var put := mini(room, int(e["n"]))
		other["n"] = int(other["n"]) + put
		e["n"] = int(e["n"]) - put
		if int(e["n"]) <= 0:
			entries.erase(e)
		return true
	if not fits(str(e["id"]), x, y, rot, e):
		return false
	e["x"] = x
	e["y"] = y
	e["rot"] = rot
	return true


func duplicate_bag() -> GridBag:
	var g := GridBag.new(w, h)
	g.entries = entries.duplicate(true)
	return g


func cells_used() -> int:
	var n := 0
	for e in entries:
		n += _rect(e).get_area()
	return n


func to_dict() -> Dictionary:
	return {"w": w, "h": h, "entries": entries.duplicate(true)}


static func from_dict(d: Dictionary) -> GridBag:
	var g := GridBag.new(int(d.get("w", 6)), int(d.get("h", 8)))
	g.entries = (d.get("entries", []) as Array).duplicate(true)
	return g
