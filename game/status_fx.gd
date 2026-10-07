class_name StatusFx
extends Node
## Element status effects on anything a blow can land on (fighters and slimes):
##   burn   damage over time (credited to whoever set it alight)
##   chill  slowed movement and swings
##   hex    armour torn open (fighters)
## Fighters carry chill and hex as timed effects on their sheet (shown in the HUD buff row); burns
## and slime statuses are ticked here.

static var instance: StatusFx

const ICONS := {"burn": "S_Fire02", "chill": "S_Wind01", "hex": "S_Magic04"}

var burns: Array = []   # {"target", "dps", "left", "source", "acc"}


func _ready() -> void:
	instance = self


func _exit_tree() -> void:
	if instance == self:
		instance = null


static func apply(target: Node3D, kind: String, power: float, dur: float, source: Player) -> void:
	if instance == null or not is_instance_valid(target) or not Combat.alive(target):
		return
	instance._apply(target, kind, power, dur, source)


func _apply(target: Node3D, kind: String, power: float, dur: float, source: Player) -> void:
	match kind:
		"burn":
			for b in burns:
				if b["target"] == target:  # a fresh burn replaces the old one if stronger
					b["dps"] = maxf(float(b["dps"]), power)
					b["left"] = maxf(float(b["left"]), dur)
					b["source"] = source
					return
			burns.append({"target": target, "dps": power, "left": dur, "source": source, "acc": 0.0})
		"chill", "hex":
			if target is Slime:
				(target as Slime).slow = maxf((target as Slime).slow, power if kind == "chill" else 0.0)
				(target as Slime).slow_t = maxf((target as Slime).slow_t, dur)
				return
			var s := (target as Player).sheet
			s.effects = s.effects.filter(func(e: Dictionary) -> bool: return e["key"] != "status:" + kind)
			var mods: Array = []
			if kind == "chill":
				mods.append(Stats.make_mod("Chilled", "move_speed", "slow", power))
				mods.append(Stats.make_mod("Chilled", "attack_speed", "add_pct", -power * 0.5))
			else:
				mods.append(Stats.make_mod("Hexed", "armor", "add_pct", -power))
			s.effects.append({"key": "status:" + kind, "name": kind.capitalize(), "kind": "status", "icon": ICONS[kind],
				"expires_at": s.clock + dur, "slots": 0, "upkeep": 0.0, "hp_drain": 0.0, "mods": mods, "backlash": null,
				"aspect": "", "family": "status"})
			s.invalidate()
	Vfx.spark(self, target.global_position + Vector3.UP * 0.9, Vector3.UP, Color(1.0, 0.5, 0.2) if kind == "burn" else Color(0.6, 0.85, 1.0), 10)


func burning(target: Node3D) -> bool:
	return burns.any(func(b: Dictionary) -> bool: return b["target"] == target)


func _physics_process(delta: float) -> void:
	for b in burns.duplicate():
		var tgt: Node3D = b["target"]
		b["left"] = float(b["left"]) - delta
		if not is_instance_valid(tgt) or not Combat.alive(tgt) or float(b["left"]) <= 0.0:
			burns.erase(b)
			continue
		var d := float(b["dps"]) * delta
		b["acc"] = float(b["acc"]) + d
		if tgt is Slime:
			(tgt as Slime).take_damage(d, b["source"])
		else:
			var f := tgt as Player
			f.sheet.hp -= d
			if f.sheet.hp <= 0.0:
				f.last_hit_by = b["source"]
				f.combat.die(Vector3(0, 0, 1))
		if float(b["acc"]) >= 8.0:  # show the burn in readable chunks
			Vfx.number(self, tgt.global_position + Vector3.UP * 1.6, "%d" % roundi(float(b["acc"])), Color(1.0, 0.55, 0.2))
			b["acc"] = 0.0
