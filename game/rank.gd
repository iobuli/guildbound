class_name Rank
extends RefCounted
## Guild rank from experience (data/ranks.json). Letters F E D C B A S, each split into four steps:
## X-, X, X+, X++ (28 in all). The XP to reach a letter grows ~8x per letter while monster XP grows
## 4x, so every letter takes about twice as long as the one before: the low ranks are quick to leave,
## the high ones are hard to reach, and most hunters end up in the middle.
## Rank is power: each letter doubles health and damage, each step inside a letter adds 15%
## (E- = 2x F-, D- = 4x, S++ ~ 93x). Rank comes only from XP, which only grows, so it cannot be
## hidden or held down (no smurfing); monsters below your letter pay a quarter of the XP per letter.
## Titles: "Adventurer", unless you are an outlier for your rank ("Rich", "Mighty", "Weak", "Clumsy").


static func data() -> Dictionary:
	return DataStore.load_json("ranks.json")


static func _subs() -> Array:
	return data()["subs"]


static func steps() -> int:
	return (data()["letters"] as Array).size() * _subs().size()


static func letter_of(step: int) -> int:
	return floori(float(step) / _subs().size())


static func sub_of(step: int) -> int:
	return step % _subs().size()


static func letter_index(letter: String) -> int:
	return maxi(0, (data()["letters"] as Array).find(letter))


static func name(step: int) -> String:
	return str(data()["letters"][letter_of(step)]) + str(_subs()[sub_of(step)])


## "E-" -> its step; a bare letter ("E") is its middle step X.
static func parse(r: String) -> int:
	if r == "":
		return 0
	var sub := _subs().find(r.substr(1))
	return letter_index(r.substr(0, 1)) * _subs().size() + maxi(0, sub)


## XP needed to reach a step.
static func threshold(step: int) -> int:
	var d := data()
	var lx: Array = d["letter_xp"]
	var li := letter_of(step)
	var lo := float(lx[li])
	var hi := float(lx[li + 1]) if li + 1 < lx.size() else lo * float(d.get("last_letter_span", 8.0))
	return int(lo + (hi - lo) * float(sub_of(step)) / float(_subs().size()))


static func step_for_xp(xp: int) -> int:
	var s := 0
	for i in steps():
		if xp >= threshold(i):
			s = i
	return s


## How much stronger a rank is than F- (health, damage, spell power).
static func power(step: int) -> float:
	var d := data()
	return pow(float(d["power_per_letter"]), letter_of(step)) * (1.0 + float(d["power_per_sub"]) * sub_of(step))


## XP for killing a monster of rank `monster` as a hunter of rank `hero`.
static func kill_xp(monster: int, hero: int) -> int:
	var d := data()
	var base := float(d["kill_xp"][letter_of(monster)]) * (1.0 + float(d["power_per_sub"]) * sub_of(monster))
	var below := maxi(0, letter_of(hero) - letter_of(monster))
	return maxi(1, Economy.round_half_up(base * pow(float(d["lower_rank_xp"]), below)))


## The next thing a rank opens up: [rank name, what], or [] when nothing is left.
static func next_unlock(step: int) -> Array:
	var u: Dictionary = data().get("unlocks", {})
	for r: String in u:
		if parse(r) > step:
			return [r, str(u[r])]
	return []


## "Adventurer", or an outlier for your rank: "Rich Adventurer", "Weak Adventurer"...
static func title(sheet: CharModel, combat: Combat = null) -> String:
	var t: Dictionary = data()["titles"]
	var step := sheet.rank_step()
	for a: Dictionary in t["adjectives"]:
		var v := _measure(str(a["measure"]), sheet, combat, step, a)
		if v < 0.0:
			continue
		if (a.has("above") and v > float(a["above"])) or (a.has("below") and v < float(a["below"])):
			return "%s %s" % [a["title"], t["noun"]]
	return str(t["noun"])


## How far from the norm for the rank something is (1.0 = the norm); -1 = cannot tell yet.
static func _measure(m: String, sheet: CharModel, combat: Combat, step: int, rule: Dictionary) -> float:
	var t: Dictionary = data()["titles"]
	match m:
		"gold":
			return float(sheet.gold) / (float(t["expected_gold"]) * pow(float(t["expected_gold_per_letter"]), letter_of(step)))
		"power":
			# attack damage against the norm for your level (20 + 3 per level) at your rank's power
			return sheet.v("attack_damage") / (power(step) * (20.0 + 3.0 * sheet.level))
		"hits_taken_per_landed":
			if combat == null or combat.hits_taken + combat.hits_landed < int(rule.get("min_events", 20)):
				return -1.0
			return float(combat.hits_taken) / maxf(1.0, float(combat.hits_landed))
	return -1.0
