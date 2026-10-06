class_name InputSetup
extends RefCounted
## Everything reads named actions, never raw keys, so rebinding and gamepads slot in later.

const KEYS := {
	"move_forward": [KEY_W],
	"move_back": [KEY_S],
	"move_left": [KEY_A],
	"move_right": [KEY_D],
	"sprint": [KEY_SHIFT],
	"jump": [KEY_SPACE],
	"crouch": [KEY_C],
	"dodge": [KEY_ALT],
	"interact": [KEY_E],
	"cast_1": [KEY_1],
	"cast_2": [KEY_2],
	"cast_3": [KEY_3],
	"cast_4": [KEY_4],
	"toggle_debug": [KEY_F3],
	"toggle_rain": [KEY_F1],
	"restore": [KEY_F2],
	"respawn": [KEY_F5],
	"ragdoll": [KEY_G],
	"free_mouse": [KEY_ESCAPE],
	"menu": [KEY_TAB, KEY_I],
}


static func apply() -> void:
	for action in KEYS:
		if not InputMap.has_action(action):
			InputMap.add_action(action)
		for key in KEYS[action]:
			var ev := InputEventKey.new()
			ev.physical_keycode = key
			InputMap.action_add_event(action, ev)
	var mouse := {"attack": MOUSE_BUTTON_LEFT, "guard": MOUSE_BUTTON_RIGHT, "lock": MOUSE_BUTTON_MIDDLE,
		"lock_next": MOUSE_BUTTON_WHEEL_UP, "lock_prev": MOUSE_BUTTON_WHEEL_DOWN}
	for action in mouse:
		if not InputMap.has_action(action):
			InputMap.add_action(action)
			var mb := InputEventMouseButton.new()
			mb.button_index = mouse[action]
			InputMap.action_add_event(action, mb)
