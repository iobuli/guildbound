class_name Icons
extends RefCounted
## Item, spell and stat icons: "496 pixel art icons for medieval/fantasy RPG" by Henrique Lazarini
## (CC0, OpenGameArt; assets/icons/rpg/CREDITS.txt). Items and spells name their icon in data/*.json.

const DIR := "res://assets/icons/rpg/"
const STAT := {
	"attack_damage": "W_Fist003", "attack_speed": "S_Wind01", "power": "S_Buff02", "crit_chance": "S_Light01",
	"crit_damage": "S_Fire02", "armor_pen": "W_Dagger005", "lifesteal": "S_Shadow04", "spell_power": "S_Magic01",
	"max_hp": "S_Holy01", "hp_regen": "S_Holy03", "armor": "A_Armor04", "magic_resist": "S_Holy09",
	"tenacity": "S_Earth05", "max_mana": "P_Blue01", "mana_regen": "P_Blue05", "cdr": "I_Clock",
	"attunement_slots": "S_Magic04", "move_speed": "A_Shoes03", "climb_speed": "I_Rock01", "jump_height": "S_Wind05",
	"stamina": "S_Physic01", "carry": "I_Chest01", "perception": "I_Eye",
	"str": "W_Fist005", "agi": "S_Wind03", "int": "W_Book05",
}
## Empty equipment slots show a faded silhouette of what goes there
const SLOT := {"weapon": "W_Sword017", "offhand": "E_Wood01", "head": "C_Elm04", "body": "A_Clothing01",
	"hands": "W_Fist001", "feet": "A_Shoes01", "accessory1": "Ac_Necklace01", "accessory2": "Ac_Ring01"}
const UI := {"combat": "W_Fist003", "stats": "S_Buff01", "inventory": "I_Chest01", "worn": "A_Armor04",
	"magic": "W_Book02", "coins": "I_GoldCoin", "help": "I_Scroll"}

static var _cache := {}


static func tex(name: String) -> Texture2D:
	if name == "":
		return null
	if not _cache.has(name):
		var path := DIR + name + ".png"
		_cache[name] = load(path) if ResourceLoader.exists(path) else null
	return _cache[name]


static func item(id: String) -> Texture2D:
	var d: Dictionary = DataStore.items().get(id, DataStore.consumables().get(id, {}))
	var t := tex(str(d.get("icon", "")))
	if t == null:
		t = tex(SLOT.get(str(d.get("slot", "")), "I_Scroll"))
	return t


static func spell(id: String) -> Texture2D:
	var d: Dictionary = DataStore.spells().get(id, {})
	return tex(str(d.get("icon", "S_Magic01")))


static func stat(st: String) -> Texture2D:
	return tex(STAT.get(st, "I_Scroll"))
