class_name Sfx
extends RefCounted
## Sound effects: Kenney "Impact Sounds" + "RPG Audio" (CC0, assets/audio/CREDITS.txt).
## Each cue picks one of its variants at random with a little pitch spread, so a flurry of punches
## never sounds like one sample on repeat. 3D cues sit where it happened; UI cues are flat.

const DIR := "res://assets/audio/"
const CUES := {
	"hit": [["impactPunch_medium_000", "impactPunch_medium_001", "impactPunch_medium_002", "impactPunch_medium_003", "impactPunch_medium_004"], -2.0],
	"hit_heavy": [["impactPunch_heavy_000", "impactPunch_heavy_001", "impactPunch_heavy_002", "impactPunch_heavy_003", "impactPunch_heavy_004"], 0.0],
	"block": [["impactSoft_heavy_000", "impactSoft_heavy_001", "impactSoft_heavy_002", "impactSoft_heavy_003"], -3.0],
	"parry": [["impactMetal_light_000", "impactMetal_light_001", "impactPlate_light_000", "impactPlate_light_001"], -1.0],
	"thud": [["impactSoft_heavy_004", "impactSoft_medium_000", "impactSoft_medium_001"], 0.0],
	"swing": [["cloth1", "cloth2", "cloth3", "cloth4"], -6.0],
	"swing_kick": [["clothBelt", "clothBelt2"], -4.0],
	"step": [["footstep_grass_000", "footstep_grass_001", "footstep_grass_002", "footstep_grass_003", "footstep_grass_004"], -16.0],
	"open": [["bookOpen"], -8.0],
	"close": [["bookClose"], -8.0],
	"page": [["bookFlip1", "bookFlip2", "bookFlip3"], -8.0],
	"equip": [["beltHandle1", "beltHandle2", "metalClick"], -6.0],
	"consume": [["handleSmallLeather", "handleSmallLeather2"], -6.0],
	"coins": [["handleCoins", "handleCoins2"], -6.0],
	"drop": [["dropLeather"], -6.0],
	"pickup": [["handleSmallLeather", "metalLatch"], -6.0],
}

static var _cache := {}
static var enabled := true    # the self-test turns it off


static func _stream(cue: String) -> AudioStream:
	var c: Array = CUES[cue]
	var names: Array = c[0]
	var n: String = names[randi() % names.size()]
	if not _cache.has(n):
		var path := DIR + n + ".ogg"
		_cache[n] = load(path) if ResourceLoader.exists(path) else null
	return _cache[n]


## A cue at a point in the world (heard from the camera).
static func at(from: Node, pos: Vector3, cue: String, pitch: float = 1.0, gain_db: float = 0.0) -> void:
	if not enabled or from == null or not from.is_inside_tree() or DisplayServer.get_name() == "headless":
		return
	var st := _stream(cue)
	if st == null:
		return
	var a := AudioStreamPlayer3D.new()
	a.stream = st
	a.volume_db = float(CUES[cue][1]) + gain_db
	a.pitch_scale = pitch * randf_range(0.92, 1.08)
	a.unit_size = 6.0
	a.max_distance = 60.0
	from.get_tree().current_scene.add_child(a)
	a.global_position = pos
	a.finished.connect(a.queue_free)
	a.play()


## A flat (2D) cue: menus, inventory.
static func ui(from: Node, cue: String) -> void:
	if not enabled or from == null or not from.is_inside_tree() or DisplayServer.get_name() == "headless":
		return
	var st := _stream(cue)
	if st == null:
		return
	var a := AudioStreamPlayer.new()
	a.stream = st
	a.volume_db = float(CUES[cue][1])
	a.pitch_scale = randf_range(0.95, 1.05)
	a.process_mode = Node.PROCESS_MODE_ALWAYS  # menus pause the world
	from.get_tree().root.add_child(a)
	a.finished.connect(a.queue_free)
	a.play()
