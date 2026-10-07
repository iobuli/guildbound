class_name Ambience
extends Node
## The background: a looping wind bed and birds now and then (Ambience bus). Silent when headless.

var _wind: AudioStreamPlayer
var _bird_in := 3.0


func _ready() -> void:
	if DisplayServer.get_name() == "headless" or not ResourceLoader.exists("res://assets/audio/synth/ambience_wind.wav"):
		set_process(false)
		return
	var st: AudioStreamWAV = (load("res://assets/audio/synth/ambience_wind.wav") as AudioStreamWAV).duplicate()
	st.loop_mode = AudioStreamWAV.LOOP_FORWARD
	st.loop_begin = 0
	st.loop_end = int(st.get_length() * st.mix_rate)
	_wind = AudioStreamPlayer.new()
	_wind.stream = st
	_wind.bus = "Ambience"
	_wind.volume_db = -14.0
	add_child(_wind)
	_wind.play()


func _process(delta: float) -> void:
	_bird_in -= delta
	if _bird_in <= 0.0:
		_bird_in = randf_range(4.0, 11.0)
		Sfx.ui(self, "bird", randf_range(-6.0, 0.0), "Ambience")
