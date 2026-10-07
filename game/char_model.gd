class_name CharModel
extends RefCounted
## GDScript port of sim/character.py (stat sheet + buffs). Base values are the only truth;
## every number is recomputed through Stats.resolve and cached until something changes.

const DERIVED := {
	"max_hp": {"c": 200.0, "pl": 10.0, "a": [["str", "flat", 22.0]]},
	"hp_regen": {"c": 1.0, "pl": 0.0, "a": [["str", "flat", 0.10]]},
	"armor": {"c": 5.0, "pl": 0.0, "a": [["agi", "flat", 0.15]]},
	"magic_resist": {"c": 5.0, "pl": 0.0, "a": []},
	"attack_damage": {"c": 10.0, "pl": 0.0, "a": []},
	"attack_speed": {"c": 1.0, "pl": 0.0, "a": [["agi", "add_pct", 0.01]]},
	"crit_chance": {"c": 0.05, "pl": 0.0, "a": []},
	"crit_damage": {"c": 1.75, "pl": 0.0, "a": []},
	"armor_pen": {"c": 0.0, "pl": 0.0, "a": []},
	"magic_pen": {"c": 0.0, "pl": 0.0, "a": []},
	"lifesteal": {"c": 0.0, "pl": 0.0, "a": []},
	"spell_power": {"c": 0.0, "pl": 0.0, "a": [["int", "flat", 1.0]]},
	"max_mana": {"c": 50.0, "pl": 0.0, "a": [["int", "flat", 12.0]]},
	"mana_regen": {"c": 1.0, "pl": 0.0, "a": [["int", "flat", 0.05]]},
	"cdr": {"c": 0.0, "pl": 0.0, "a": []},
	"tenacity": {"c": 0.0, "pl": 0.0, "a": []},
	"power": {"c": 1.0, "pl": 0.0, "a": [["str", "add_pct", 0.008]]},  # impact of blows: stagger, knockdown, shove
	"move_speed": {"c": 6.0, "pl": 0.0, "a": []},
	"climb_speed": {"c": 1.5, "pl": 0.0, "a": []},
	"jump_height": {"c": 1.2, "pl": 0.0, "a": []},
	"stamina": {"c": 100.0, "pl": 0.0, "a": []},
	"carry": {"c": 30.0, "pl": 0.0, "a": [["str", "flat", 2.0]]},
	"stealth": {"c": 10.0, "pl": 0.0, "a": []},
	"perception": {"c": 10.0, "pl": 0.0, "a": [["int", "flat", 0.2]]},
	"attunement_slots": {"c": 3.0, "pl": 0.0, "a": []},
}
const HP_FLOOR_FRACTION := 0.10
const STOMACH_LIMIT := 3
const SLOTS := ["weapon", "offhand", "head", "body", "hands", "feet", "accessory1", "accessory2"]
## Status-window groups (same as the Python sim)
const GROUPS := [
	["Offence", ["attack_damage", "attack_speed", "power", "crit_chance", "crit_damage", "armor_pen", "lifesteal", "spell_power"]],
	["Defence", ["max_hp", "hp_regen", "armor", "magic_resist", "tenacity"]],
	["Magic", ["max_mana", "mana_regen", "cdr", "attunement_slots"]],
	["Movement", ["move_speed", "climb_speed", "jump_height", "stamina", "carry"]],
]
const STAT_NAMES := {"attack_damage": "Attack Damage", "attack_speed": "Attack Speed", "power": "Power", "crit_chance": "Crit Chance",
	"crit_damage": "Crit Damage", "armor_pen": "Armor Pen", "lifesteal": "Lifesteal", "spell_power": "Spell Power",
	"max_hp": "Max HP", "hp_regen": "HP Regen", "armor": "Armor", "magic_resist": "Magic Resist", "tenacity": "Tenacity",
	"max_mana": "Max Mana", "mana_regen": "Mana Regen", "cdr": "Cooldown Red.", "attunement_slots": "Attunement Slots",
	"move_speed": "Move Speed", "climb_speed": "Climb Speed", "jump_height": "Jump Height", "stamina": "Stamina", "carry": "Carry",
	"perception": "Perception", "stealth": "Stealth", "magic_pen": "Magic Pen"}
const PCT_STATS := ["crit_chance", "tenacity", "cdr", "lifesteal", "armor_pen", "magic_pen"]

var display_name := ""
var race := ""
var level := 1
var equipment := {}
var effects: Array = []
var cooldowns := {}
var corruption := 0.0
var clock := 0.0
var hp := 0.0
var mana := 0.0
var bag := {}        # item id -> count (gear and consumables you carry but are not wearing)
var grid := GridBag.new()   # where each of those sits in the backpack (Tarkov-style; bag stays the count)
var gold := 0

var _cache := {}
var _races: Dictionary
var _items: Dictionary
var _spells: Dictionary


func _init(p_name: String, p_race: String, p_level: int) -> void:
	display_name = p_name
	race = p_race
	level = p_level
	_races = DataStore.races()
	_items = DataStore.items()
	_spells = DataStore.spells()
	restore()


func invalidate() -> void:
	_cache.clear()


func restore() -> void:
	hp = stat("max_hp")["value"]
	mana = stat("max_mana")["value"]


func equip(item_id: String) -> void:
	var item: Dictionary = _items[item_id]
	var slot = item["slot"]
	assert(slot != null)
	if slot == "accessory":
		slot = "accessory1" if not equipment.has("accessory1") else "accessory2"
	equipment[slot] = item_id
	invalidate()


# ---- bag / equipment (the status window drives these) ---------------------------------------

static func is_consumable(id: String) -> bool:
	return DataStore.consumables().has(id)


func item_data(id: String) -> Dictionary:
	if _items.has(id):
		return _items[id]
	return DataStore.consumables().get(id, {})


## Put items in the backpack. False (and nothing added) when they do not fit.
func add_item(id: String, n: int = 1) -> bool:
	if not grid.add(id, n):
		return false
	bag[id] = int(bag.get(id, 0)) + n
	return true


func remove_item(id: String, n: int = 1) -> bool:
	if int(bag.get(id, 0)) < n or not grid.remove(id, n):
		return false
	bag[id] = int(bag[id]) - n
	if int(bag[id]) <= 0:
		bag.erase(id)
	return true


## Would n of `id` fit in the backpack right now?
func can_carry(id: String, n: int = 1) -> bool:
	return grid.duplicate_bag().add(id, n)


## Which slot an item would go into (the second ring slot if the first is taken).
func slot_for(id: String) -> String:
	var slot = _items[id].get("slot")
	if slot == null:
		return ""
	if slot == "accessory":
		return "accessory1" if not equipment.has("accessory1") or equipment.has("accessory2") else "accessory2"
	return slot


## Put on an item from the bag; whatever was in that slot goes back in the bag.
func equip_from_bag(id: String) -> bool:
	if not _items.has(id) or slot_for(id) == "" or not remove_item(id):
		return false
	var slot := slot_for(id)
	if equipment.has(slot) and not add_item(equipment[slot]):
		add_item(id)  # the worn one does not fit where this came from: keep things as they were
		return false
	equipment[slot] = id
	invalidate()
	hp = minf(hp, v("max_hp"))
	mana = minf(mana, v("max_mana"))
	return true


func unequip(slot: String) -> bool:
	if not equipment.has(slot) or not add_item(equipment[slot]):
		return false
	equipment.erase(slot)
	invalidate()
	hp = minf(hp, v("max_hp"))
	mana = minf(mana, v("max_mana"))
	return true


## Eat / drink a consumable from the bag (at most three active, same as the Python sim).
func consume(id: String) -> Array:
	var c: Dictionary = DataStore.consumables().get(id, {})
	if c.is_empty() or int(bag.get(id, 0)) <= 0:
		return [false, "nothing to use"]
	var active := 0
	for e in effects:
		if e["kind"] == "consumable":
			active += 1
	if active >= STOMACH_LIMIT:
		return [false, "stomach is full (3 active consumables)"]
	if find_effect(id) != null:
		return [false, "%s is already active" % c["name"]]
	remove_item(id)
	var expires: float = clock + float(c["duration"])
	var mods: Array = []
	for m in c["mods"]:
		mods.append(Stats.make_mod(c["name"], m["stat"], m["op"], m["value"], m.get("group"), expires))
	effects.append({"key": id, "name": c["name"], "kind": "consumable", "expires_at": expires, "slots": 0,
		"upkeep": 0.0, "hp_drain": 0.0, "mods": mods, "backlash": null, "aspect": "", "family": "food"})
	invalidate()
	return [true, "Used " + c["name"]]


## Appraisal: what equipping this item would do to every stat that changes (name -> [now, after]).
func appraise(id: String) -> Dictionary:
	var out := {}
	if not _items.has(id) or slot_for(id) == "":
		return out
	var before := {}
	for g in GROUPS:
		for st in g[1]:
			before[st] = v(st)
	var saved := equipment.duplicate()
	equipment[slot_for(id)] = id
	invalidate()
	for g in GROUPS:
		for st in g[1]:
			var after := v(st)
			if absf(after - float(before[st])) > 0.0001:
				out[st] = [before[st], after]
	equipment = saved
	invalidate()
	return out


## One modifier as text: "+20% Move Speed", "+8 Armor", "x1.15 Attack Speed".
static func mod_text(m: Dictionary) -> String:
	var st: String = m["stat"]
	var nm: String = STAT_NAMES.get(st, st)
	var x: float = float(m["value"])
	match m["op"]:
		"flat":
			if st in PCT_STATS:
				return "%+d%% %s" % [roundi(x * 100.0), nm]
			return "%s%s %s" % ["+" if x >= 0.0 else "", Stats._g(x), nm]
		"add_pct":
			return "%+d%% %s" % [roundi(x * 100.0), nm]
		"more":
			return "x%s %s" % [Stats._g(x), nm]
	return "-%d%% %s (slow)" % [roundi(x * 100.0), nm]


static func fmt_stat(st: String, x: float) -> String:
	if st in PCT_STATS:
		return "%d%%" % roundi(x * 100.0)
	if st == "power" or st == "crit_damage":
		return "x%.2f" % x
	return Stats._g(snappedf(x, 0.01))


# ---- modifiers -------------------------------------------------------------------------------

func _weapon() -> Dictionary:
	if not equipment.has("weapon"):
		return {}
	return _items[equipment["weapon"]].get("weapon", {})


func carried_weight() -> float:
	var w := 0.0
	for slot in equipment:
		w += float(_items[equipment[slot]]["weight"])
	return w


func _modifiers() -> Array:
	if _cache.has("_mods"):
		return _cache["_mods"]
	var mods: Array = []
	var r: Dictionary = _races[race]
	for m in r["passive"]["mods"]:
		mods.append(Stats.make_mod("Race: %s / %s" % [r["name"], r["passive"]["name"]], m["stat"], m["op"], m["value"], m.get("group")))
	for slot in equipment:
		var item: Dictionary = _items[equipment[slot]]
		for m in item["mods"]:
			mods.append(Stats.make_mod("Item: " + item["name"], m["stat"], m["op"], m["value"], m.get("group")))
	for e in effects:
		mods.append_array(e["mods"])
	_cache["_mods"] = mods
	return mods


func _all_modifiers() -> Array:
	if _cache.has("_allmods"):
		return _cache["_allmods"]
	var mods: Array = _modifiers().duplicate()
	var carry_cap: float = stat("carry")["value"]
	var w := carried_weight()
	if w > carry_cap:
		var over := minf(0.6, (w - carry_cap) / carry_cap)
		mods.append(Stats.make_mod("Overburdened (%.1f/%.1f kg)" % [w, carry_cap], "move_speed", "slow", over))
	_cache["_allmods"] = mods
	return mods


func attribute(attr: String) -> Dictionary:
	var r: Dictionary = _races[race]
	var base: float = float(r["base"][attr]) + float(r["growth"][attr]) * (level - 1)
	var label := "%s %s + %s/lvl x %d" % [r["name"], r["base"][attr], r["growth"][attr], level - 1]
	return Stats.resolve(attr, base, label, _modifiers(), clock)


func _attrs() -> Dictionary:
	if _cache.has("_attrs"):
		return _cache["_attrs"]
	var a := {}
	for k in ["str", "agi", "int"]:
		a[k] = attribute(k)["value"]
	_cache["_attrs"] = a
	return a


func stat(stat_name: String) -> Dictionary:
	if _cache.has(stat_name):
		return _cache[stat_name]
	var attrs := _attrs()
	var d: Dictionary = DERIVED[stat_name]
	var base: float = d["c"] + d["pl"] * level
	var label := "%s" % d["c"]
	var w := _weapon()
	if stat_name == "attack_damage":
		var wd: float = w.get("base_damage", 0.0)
		base = d["c"] + wd
		label = "10 unarmed + %s weapon" % wd
	elif stat_name == "attack_speed":
		base = w.get("aps", 1.0)
		label = "weapon %s/s" % base
	elif stat_name == "attunement_slots":
		var extra := floori(level / 10.0) + floori(attrs["int"] / 50.0)
		base += extra
		label = "3 + %d (lvl/10) + %d (INT/50)" % [floori(level / 10.0), floori(attrs["int"] / 50.0)]
	var mods: Array = (_modifiers() if stat_name == "carry" else _all_modifiers()).duplicate()
	for a in d["a"]:
		mods.append(Stats.make_mod("%s %.1f" % [String(a[0]).to_upper(), attrs[a[0]]], stat_name, a[1], float(a[2]) * attrs[a[0]]))
	if stat_name == "attack_damage":
		var scale: String = w.get("scales", "str")
		mods.append(Stats.make_mod("%s %.1f (weapon scaling)" % [scale.to_upper(), attrs[scale]], stat_name, "flat", attrs[scale]))
	var res := Stats.resolve(stat_name, base, label, mods, clock)
	_cache[stat_name] = res
	return res


func v(stat_name: String) -> float:
	return stat(stat_name)["value"]


func explain(stat_name: String) -> String:
	var r := stat(stat_name)
	var head := "%s = %s" % [stat_name, Stats._g(r["value"])]
	if r["note"] != "":
		head += "   [" + r["note"] + "]"
	return head + "\n" + "\n".join(r["lines"])


# ---- damage ----------------------------------------------------------------------------------

func take_physical(raw: float, pen: float = 0.0) -> float:
	var dmg := raw * Stats.damage_taken_fraction(v("armor") * (1.0 - pen))
	hp = maxf(0.0, hp - dmg)
	return dmg


func damage_percent_of_max(pct: float) -> float:
	var dmg: float = v("max_hp") * pct
	hp = maxf(0.0, hp - dmg)
	return dmg


# ---- magic -----------------------------------------------------------------------------------

func slots_used() -> int:
	var n := 0
	for e in effects:
		n += int(e["slots"])
	return n


func find_effect(key: String) -> Variant:
	for e in effects:
		if e["key"] == key:
			return e
	return null


func cast(spell_id: String) -> Array:
	var sp: Dictionary = _spells[spell_id]
	if find_effect(spell_id) != null:
		return [false, "%s is already active" % sp["name"]]
	if cooldowns.get(spell_id, 0.0) > clock:
		return [false, "%s on cooldown (%.1fs)" % [sp["name"], cooldowns[spell_id] - clock]]
	if mana < sp["cost"]:
		return [false, "not enough mana for %s" % sp["name"]]
	var slots: int = int(sp["slots"])
	if slots > 0 and slots_used() + slots > v("attunement_slots"):
		return [false, "no free attunement slot for %s" % sp["name"]]
	mana -= sp["cost"]
	cooldowns[spell_id] = clock + float(sp["cooldown"]) * (1.0 - v("cdr"))
	var expires = null
	if sp["mode"] == "timed":
		expires = clock + float(sp["duration"])
	var mods: Array = []
	for m in sp["mods"]:
		mods.append(Stats.make_mod(sp["name"], m["stat"], m["op"], m["value"], m.get("group"), expires))
	effects.append({
		"key": spell_id, "name": sp["name"], "kind": "spell", "expires_at": expires,
		"slots": slots, "upkeep": sp.get("upkeep", 0.0), "hp_drain": sp.get("hp_drain", 0.0),
		"mods": mods, "backlash": sp.get("backlash"), "aspect": sp["aspect"], "family": sp["family"],
	})
	corruption = minf(100.0, corruption + float(sp.get("corruption", 0.0)))
	invalidate()
	return [true, "cast " + sp["name"]]


func dismiss(spell_id: String) -> void:
	var e = find_effect(spell_id)
	if e != null:
		_end(e)


func _end(e: Dictionary) -> void:
	effects.erase(e)
	var b = e.get("backlash")
	if b != null:
		var expires: float = clock + float(b["duration"])
		var mods: Array = []
		for m in b["mods"]:
			mods.append(Stats.make_mod("Backlash: %s (from %s)" % [b["name"], e["name"]], m["stat"], m["op"], m["value"], m.get("group"), expires))
		effects.append({
			"key": "backlash:" + e["key"], "name": "%s (from %s)" % [b["name"], e["name"]], "kind": "backlash",
			"expires_at": expires, "slots": 0, "upkeep": 0.0, "hp_drain": 0.0, "mods": mods,
			"backlash": null, "aspect": "", "family": "",
		})
	invalidate()
	hp = minf(hp, v("max_hp"))
	mana = minf(mana, v("max_mana"))


func tick(dt: float) -> Array:
	var events: Array = []
	clock += dt
	for e in effects.duplicate():
		if e["expires_at"] != null:
			if clock >= float(e["expires_at"]):
				_end(e)
				events.append("%s expired" % e["name"])
			continue
		var cost: float = float(e["upkeep"]) * dt
		if mana < cost:
			_end(e)
			events.append("%s collapsed: out of mana" % e["name"])
			continue
		mana -= cost
		if float(e["hp_drain"]) > 0.0:
			if hp - float(e["hp_drain"]) * dt < v("max_hp") * HP_FLOOR_FRACTION:
				_end(e)
				events.append("%s collapsed: too weak to sustain" % e["name"])
				continue
			hp -= float(e["hp_drain"]) * dt
	hp = minf(v("max_hp"), hp + v("hp_regen") * dt)
	mana = minf(v("max_mana"), mana + v("mana_regen") * dt)
	return events
