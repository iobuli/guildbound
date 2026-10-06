class_name Stats
extends RefCounted
## GDScript port of sim/stats.py. Modifier = Dictionary {source, stat, op, value, group, expires_at}.
## Order: base -> +flat -> x(1+sum add_pct) -> x(product of more) -> soft/hard caps -> strongest slow.

const CAPS := {
	"attack_speed": {"mode": "absolute", "soft": 2.5, "hard": 3.5},
	"move_speed": {"mode": "bonus", "soft": 0.6, "hard": 1.0},
	"cdr": {"hard": 0.40},
	"crit_chance": {"hard": 1.0},
	"tenacity": {"hard": 0.75},
	"lifesteal": {"hard": 0.50},
	"attunement_slots": {"hard": 8.0},
}


static func make_mod(source: String, stat: String, op: String, value: float, group = null, expires_at = null) -> Dictionary:
	return {"source": source, "stat": stat, "op": op, "value": value, "group": group, "expires_at": expires_at}


static func _g(v: float) -> String:
	var s := "%.3f" % v
	if "." in s:
		s = s.rstrip("0").rstrip(".")
	return s


static func _fmt(op: String, v: float) -> String:
	match op:
		"flat":
			return ("+" if v >= 0.0 else "") + _g(v)
		"add_pct":
			return ("+" if v >= 0.0 else "") + _g(v * 100.0) + "%"
		"more":
			return "x" + _g(v)
	return "-" + _g(v * 100.0) + "% slow"


static func _strength(m: Dictionary) -> float:
	return absf(float(m["value"]) - 1.0) if m["op"] == "more" else absf(float(m["value"]))


static func resolve(stat: String, base: float, label: String, mods: Array, now: float) -> Dictionary:
	var live: Array = []
	for m in mods:
		if m["stat"] != stat:
			continue
		var ex = m["expires_at"]
		if ex != null and float(ex) <= now:
			continue
		live.append(m)

	var lines: Array = ["base %s  (%s)" % [_g(base), label]]
	var best := {}
	var kept: Array = []
	for m in live:
		if m["group"] == null or m["op"] == "slow":
			kept.append(m)
			continue
		var gkey: String = str(m["group"]) + "|" + str(m["op"])
		if not best.has(gkey) or _strength(m) > _strength(best[gkey]):
			best[gkey] = m
	for gk in best:
		kept.append(best[gk])
	for m in live:
		if m["group"] != null and m["op"] != "slow":
			var wk: String = str(m["group"]) + "|" + str(m["op"])
			if not is_same(best[wk], m):
				lines.append("  (suppressed) %s %s  -- same group as %s" % [_fmt(m["op"], m["value"]), m["source"], best[wk]["source"]])

	var flat := 0.0
	var pct := 0.0
	var mores: Array = []
	var slow := 0.0
	for m in kept:
		var v: float = m["value"]
		match m["op"]:
			"flat":
				flat += v
			"add_pct":
				pct += v
			"more":
				mores.append(v)
			"slow":
				slow = maxf(slow, v)
		lines.append("  %10s  %s" % [_fmt(m["op"], v), m["source"]])

	var value := (base + flat) * (1.0 + pct)
	for mv in mores:
		value *= mv

	var note := ""
	if CAPS.has(stat):
		var cap: Dictionary = CAPS[stat]
		var mode: String = cap.get("mode", "")
		if mode == "absolute":
			if cap.has("soft") and value > cap["soft"]:
				value = cap["soft"] + (value - cap["soft"]) * 0.5
				note = "soft cap %s (excess halved)" % _g(cap["soft"])
			if value > cap["hard"]:
				value = cap["hard"]
				note = "hard cap %s" % _g(cap["hard"])
		elif mode == "bonus" and base > 0.0:
			var bonus: float = value / base - 1.0
			if bonus > cap["soft"]:
				bonus = cap["soft"] + (bonus - cap["soft"]) * 0.5
				note = "soft cap +%s%% (excess halved)" % _g(cap["soft"] * 100.0)
			if bonus > cap["hard"]:
				bonus = cap["hard"]
				note = "hard cap +%s%%" % _g(cap["hard"] * 100.0)
			value = base * (1.0 + bonus)
		elif cap.has("hard") and value > cap["hard"]:
			value = cap["hard"]
			note = "hard cap %s" % _g(cap["hard"])

	if slow > 0.0:
		value *= 1.0 - slow
		note = (note + "; " if note != "" else "") + "strongest slow -%s%%" % _g(slow * 100.0)

	value = maxf(0.0, value)
	return {"stat": stat, "value": value, "base": base, "lines": lines, "note": note}


static func damage_taken_fraction(resist: float) -> float:
	return 100.0 / (100.0 + maxf(0.0, resist))
