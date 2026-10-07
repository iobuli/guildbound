class_name Sfx
extends RefCounted
## Sound effects. Recorded foley from Kenney "Impact Sounds" + "RPG Audio" (CC0), and the combat /
## magic / monster / ambience cues in assets/audio/synth that tools/make_sfx.py builds FROM those
## recordings (layered, varispeeded, filtered, transient-shaped, reversed, granulated, reverb tails;
## synthesis only as a sweetener). assets/audio/CREDITS.txt lists the sources of every cue.
## Every generated file is loudness-matched (about -16 LUFS, peaks under -1 dBFS: synth/levels.json),
## so the dB here IS the mix: blows ~-16, guard clangs ~-18, casts ~-22, swishes ~-23 (they support a
## hit, never compete with it), slime hops ~-28, footsteps ~-37, birds ~-30 on the Ambience bus.
## Each cue picks one of its variants at random with a small pitch and level spread, so a flurry of
## blows never sounds like one sample on repeat. 3D cues sit where it happened; UI cues are flat.
## Buses: SFX (a touch of room reverb), UI, Ambience; a limiter on Master keeps big fights clean.
## A cue never plays more than MAX_PER_CUE copies at once (MAX_VOICES in all).

const DIR := "res://assets/audio/"
const MAX_PER_CUE := 4
const MAX_VOICES := 28
const PITCH_SPREAD := 0.04      # +-4 %: variety without the chipmunk wobble of a wide random pitch
const GAIN_SPREAD := 1.0        # +-1 dB


static func _v(base: String, n: int) -> Array:
	var out: Array = []
	for i in n:
		out.append("synth/%s_%d" % [base, i])
	return out


static var CUES := {
	"hit": [["impactPunch_medium_000", "impactPunch_medium_001", "impactPunch_medium_002", "impactPunch_medium_003", "impactPunch_medium_004"], -3.0],
	"hit_heavy": [["impactPunch_heavy_000", "impactPunch_heavy_001", "impactPunch_heavy_002", "impactPunch_heavy_003", "impactPunch_heavy_004"], 0.0],
	"hit_blade": [_v("hit_blade", 3), 0.0],
	"block": [_v("block", 3), -2.0],
	"clang": [_v("clang", 3), -2.0],
	"parry": [_v("parry", 3), 0.0],
	"deflect": [_v("parry", 3), -1.0],   # a parried projectile flying back
	"thud": [["impactSoft_heavy_004", "impactSoft_medium_000", "impactSoft_medium_001"], 0.0],
	"swing": [_v("swish_light", 3), -9.0],
	"swish_light": [_v("swish_light", 3), -8.0],
	"swish_blade": [_v("swish_blade", 3), -7.0],
	"swish_heavy": [_v("swish_heavy", 3), -5.0],
	"swing_kick": [_v("swish_kick", 3), -7.0],
	"cloth": [["cloth1", "cloth2", "cloth3", "cloth4"], -10.0],
	"fire_cast": [_v("fire_cast", 3), -6.0], "fire_hit": [_v("fire_hit", 3), -1.0],
	"ice_cast": [_v("ice_cast", 3), -7.0], "ice_hit": [_v("ice_hit", 3), -2.0],
	"lightning_cast": [_v("lightning_cast", 3), -6.0], "lightning_hit": [_v("lightning_hit", 3), 0.0],
	"wind_cast": [_v("wind_cast", 3), -7.0], "wind_hit": [_v("wind_hit", 3), -3.0],
	"arcane_cast": [_v("arcane_cast", 3), -7.0], "arcane_hit": [_v("arcane_hit", 3), -2.0],
	"shadow_cast": [_v("shadow_cast", 3), -6.0], "shadow_hit": [_v("shadow_hit", 3), -2.0],
	"awaken": [_v("awaken", 1), 0.0],
	"rank_up": [_v("rank_up", 1), -2.0],
	"slime_hop": [_v("slime_hop", 3), -12.0], "slime_hit": [_v("slime_hit", 3), -3.0], "slime_die": [_v("slime_die", 3), -3.0],
	"step": [["footstep_grass_000", "footstep_grass_001", "footstep_grass_002", "footstep_grass_003", "footstep_grass_004"], -18.0],
	"step_stone": [_v("step_stone", 4), -21.0],
	"bird": [_v("bird", 4), -14.0],
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
static var _playing := {}       # cue -> copies playing now
static var _voices := 0
static var enabled := true      # the self-test turns it off


## Buses and the Master limiter (once, from main).
static func setup_buses() -> void:
	for b in ["SFX", "UI", "Ambience"]:
		if AudioServer.get_bus_index(b) < 0:
			AudioServer.add_bus()
			var i := AudioServer.bus_count - 1
			AudioServer.set_bus_name(i, b)
			AudioServer.set_bus_send(i, "Master")
	var sfx := AudioServer.get_bus_index("SFX")
	if AudioServer.get_bus_effect_count(sfx) == 0:
		var rv := AudioEffectReverb.new()
		rv.room_size = 0.35
		rv.damping = 0.6
		rv.wet = 0.08
		rv.dry = 1.0
		AudioServer.add_bus_effect(sfx, rv)
	AudioServer.set_bus_volume_db(AudioServer.get_bus_index("Ambience"), -6.0)
	if AudioServer.get_bus_effect_count(0) == 0:
		var lim: AudioEffect = ClassDB.instantiate("AudioEffectHardLimiter") if ClassDB.class_exists("AudioEffectHardLimiter") else AudioEffectLimiter.new()
		AudioServer.add_bus_effect(0, lim)


static func _stream(cue: String) -> AudioStream:
	var c: Array = CUES[cue]
	var names: Array = c[0]
	var n: String = names[randi() % names.size()]
	if not _cache.has(n):
		var st: AudioStream = null
		for ext in [".ogg", ".wav"]:
			if ResourceLoader.exists(DIR + n + ext):
				st = load(DIR + n + ext)
				break
		_cache[n] = st
	return _cache[n]


static func _can_play(from: Node, cue: String) -> bool:
	if not enabled or from == null or not from.is_inside_tree() or DisplayServer.get_name() == "headless":
		return false
	return int(_playing.get(cue, 0)) < MAX_PER_CUE and _voices < MAX_VOICES


static func _track(a: Node, cue: String) -> void:
	_playing[cue] = int(_playing.get(cue, 0)) + 1
	_voices += 1
	a.tree_exited.connect(func() -> void:
		_playing[cue] = maxi(0, int(_playing.get(cue, 1)) - 1)
		_voices = maxi(0, _voices - 1))


## A cue at a point in the world (heard from the camera).
static func at(from: Node, pos: Vector3, cue: String, pitch: float = 1.0, gain_db: float = 0.0) -> void:
	if not _can_play(from, cue):
		return
	var st := _stream(cue)
	if st == null:
		return
	var a := AudioStreamPlayer3D.new()
	a.stream = st
	a.bus = "SFX"
	a.volume_db = float(CUES[cue][1]) + gain_db + randf_range(-GAIN_SPREAD, GAIN_SPREAD)
	a.pitch_scale = pitch * randf_range(1.0 - PITCH_SPREAD, 1.0 + PITCH_SPREAD)
	a.unit_size = 6.0
	a.max_distance = 60.0
	var tree := from.get_tree()
	(tree.current_scene if tree.current_scene != null else tree.root).add_child(a)
	a.global_position = pos
	a.finished.connect(a.queue_free)
	_track(a, cue)
	a.play()


## A flat (2D) cue: menus, inventory, fanfares. `bus` "Ambience" for the background.
static func ui(from: Node, cue: String, gain_db: float = 0.0, bus: String = "UI") -> void:
	if not _can_play(from, cue):
		return
	var st := _stream(cue)
	if st == null:
		return
	var a := AudioStreamPlayer.new()
	a.stream = st
	a.bus = bus
	a.volume_db = float(CUES[cue][1]) + gain_db + randf_range(-GAIN_SPREAD, GAIN_SPREAD) * 0.5
	a.pitch_scale = randf_range(1.0 - PITCH_SPREAD * 0.5, 1.0 + PITCH_SPREAD * 0.5)
	a.process_mode = Node.PROCESS_MODE_ALWAYS  # menus pause the world
	from.get_tree().root.add_child(a)
	a.finished.connect(a.queue_free)
	_track(a, cue)
	a.play()
