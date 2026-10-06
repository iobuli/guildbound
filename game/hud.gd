class_name Hud
extends Control
## Immediate-mode HUD: bars, stamina wheel on the character, MOBA-style buff bar, hotbar, debug panel.

var player: Player
var toasts: Array = []
var show_debug := true
var rain := false

const STATE_NAMES := ["GROUND", "AIR", "CLIMB", "SWIM", "RAGDOLL", "GET UP"]


func _ready() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE


func add_toast(text: String) -> void:
	toasts.append([text, 4.0])
	if toasts.size() > 6:
		toasts.pop_front()


func _process(delta: float) -> void:
	for t in toasts:
		t[1] -= delta
	toasts = toasts.filter(func(t): return t[1] > 0.0)
	queue_redraw()


func _text(pos: Vector2, s: String, size: int = 14, col: Color = Color.WHITE) -> void:
	var f := ThemeDB.fallback_font
	draw_string(f, pos + Vector2(1, 1), s, HORIZONTAL_ALIGNMENT_LEFT, -1, size, Color(0, 0, 0, 0.8))
	draw_string(f, pos, s, HORIZONTAL_ALIGNMENT_LEFT, -1, size, col)


func _bar(pos: Vector2, size: Vector2, frac: float, col: Color, caption: String) -> void:
	draw_rect(Rect2(pos, size), Color(0, 0, 0, 0.55))
	draw_rect(Rect2(pos, Vector2(size.x * clampf(frac, 0.0, 1.0), size.y)), col)
	_text(pos + Vector2(6, size.y - 4), caption, 12)


func _draw() -> void:
	if player == null or player.sheet == null:
		return
	var s := player.sheet
	var sz := get_viewport_rect().size

	# vitals
	var max_hp := s.v("max_hp")
	var max_mp := s.v("max_mana")
	_bar(Vector2(20, 20), Vector2(280, 20), s.hp / max_hp, Color(0.8, 0.2, 0.2), "HP %d / %d" % [s.hp, max_hp])
	_bar(Vector2(20, 44), Vector2(280, 16), s.mana / max_mp, Color(0.25, 0.45, 0.95), "Mana %d / %d" % [s.mana, max_mp])
	var smax := s.v("stamina")
	_bar(Vector2(20, 64), Vector2(280, 12), player.stamina / smax, Color(0.3, 0.8, 0.35) if not player.exhausted else Color(0.9, 0.3, 0.2), "Stamina %d" % player.stamina)

	# stamina wheel on the character
	var cam := player.cam.camera
	if cam != null and not cam.is_position_behind(player.global_position):
		var c := cam.unproject_position(player.global_position + Vector3.UP * 1.0)
		var frac := player.stamina / smax
		if frac < 0.999 or player.exhausted:
			draw_arc(c, 36.0, 0.0, TAU, 48, Color(0, 0, 0, 0.45), 7.0)
			var col := Color(0.35, 0.9, 0.4).lerp(Color(1.0, 0.8, 0.2), 1.0 - frac)
			if player.exhausted:
				col = Color(0.95, 0.25, 0.2)
			draw_arc(c, 36.0, -PI / 2.0, -PI / 2.0 + TAU * frac, 48, col, 7.0)

	# fighting: lock / soft target reticle, target health, charge ring
	if cam != null:
		var tgt := player.lock.current()
		if tgt != null and is_instance_valid(tgt):
			var chest: Vector3 = (tgt.model.rig.by_name["chest"]["body"] as RigidBody3D).global_position
			if not cam.is_position_behind(chest):
				var rc := cam.unproject_position(chest)
				var locked := tgt == player.lock.target
				var rcol := Color(1.0, 0.35, 0.3) if locked else Color(1.0, 1.0, 1.0, 0.6)
				draw_arc(rc, 14.0 if locked else 8.0, 0.0, TAU, 24, rcol, 2.5 if locked else 1.5)
				if locked:
					draw_circle(rc, 3.0, rcol)
			if tgt == player.lock.target:
				var ts := tgt.sheet
				_bar(Vector2(sz.x * 0.5 - 160.0, 20.0), Vector2(320, 16), ts.hp / ts.v("max_hp"), Color(0.85, 0.25, 0.2),
					"%s   %d / %d" % [ts.display_name, ts.hp, ts.v("max_hp")])
		var cb := player.combat
		# the beat: a gold ring closing on the target - strike as it meets the reticle for PERFECT
		if cb.beat_at > 0.0 and tgt != null and is_instance_valid(tgt):
			var chest2: Vector3 = (tgt.model.rig.by_name["chest"]["body"] as RigidBody3D).global_position
			if not cam.is_position_behind(chest2):
				var bc := cam.unproject_position(chest2)
				var span := maxf(0.01, cb.beat_at - cb.beat_from)
				var u := clampf((s.clock - cb.beat_from) / span, 0.0, 1.2)
				var on_beat := absf(s.clock - cb.beat_at) <= Combat.PERFECT_WINDOW
				draw_arc(bc, lerpf(54.0, 14.0, minf(u, 1.0)), 0.0, TAU, 40, Color(1.0, 0.85, 0.3) if not on_beat else Color(1.0, 1.0, 1.0), 3.0 if not on_beat else 5.0)
		if cb.perfect_chain > 0:
			_text(Vector2(sz.x * 0.5 + 60.0, sz.y * 0.5 - 40.0), "PERFECT x%d%s" % [cb.perfect_chain, "  -  E: FINISHER" if cb.perfect_chain >= Combat.FINISHER_CHAIN else ""], 18, Color(1.0, 0.85, 0.3))
		if cb.state == Combat.S.CHARGING and not cam.is_position_behind(player.global_position):
			var cc := cam.unproject_position(player.global_position + Vector3.UP * 1.0)
			var ch := clampf(cb.t / Combat.CHARGE_MAX, 0.0, 1.0)
			draw_arc(cc, 46.0, -PI / 2.0, -PI / 2.0 + TAU * ch, 48, Color(1.0, 0.75, 0.2), 4.0)

	# buff bar
	var x := 20.0
	for e in s.effects:
		var col := Color(0.5, 0.5, 0.55)
		match e["aspect"]:
			"light":
				col = Color(1.0, 0.85, 0.3)
			"dark":
				col = Color(0.65, 0.3, 0.95)
		if e["kind"] == "backlash":
			col = Color(0.9, 0.25, 0.2)
		elif e["kind"] == "consumable":
			col = Color(0.4, 0.8, 0.4)
		# what it does, not just its name: the first two modifiers under the timer
		var ms: Array = []
		for m in e["mods"]:
			ms.append(CharModel.mod_text(m))
		var w := 132.0
		draw_rect(Rect2(x, 86, w, 52), Color(0, 0, 0, 0.55))
		draw_rect(Rect2(x, 86, w, 52), col, false, 2.0)
		var t := "on" if e["expires_at"] == null else "%ds" % maxf(0.0, float(e["expires_at"]) - s.clock)
		_text(Vector2(x + 5, 100), "%s  %s" % [String(e["name"]).substr(0, 14), t], 11, col)
		for k in mini(2, ms.size()):
			_text(Vector2(x + 5, 115 + 13 * k), String(ms[k]).substr(0, 22), 10, Color(0.85, 0.92, 1.0))
		x += w + 6.0

	# hotbar
	var spells := DataStore.spells()
	var page: Array = Player.PAGE1
	var hx := sz.x * 0.5 - 4 * 38.0
	for i in 4:
		var sid: String = page[i]
		var sp: Dictionary = spells[sid]
		var r := Rect2(hx + i * 76.0, sz.y - 86.0, 70.0, 66.0)
		draw_rect(r, Color(0, 0, 0, 0.55))
		var active := s.find_effect(sid) != null
		var asp: String = sp["aspect"]
		var col := Color(1.0, 0.85, 0.3) if asp == "light" else (Color(0.65, 0.3, 0.95) if asp == "dark" else Color(0.7, 0.7, 0.7))
		draw_rect(r, col if active else col.darkened(0.5), false, 3.0 if active else 1.5)
		_text(r.position + Vector2(4, 14), str(i + 1), 12, Color(1, 1, 0.6))
		_text(r.position + Vector2(4, 40), String(sp["name"]).substr(0, 10), 11)
		_text(r.position + Vector2(4, 58), "%d mana" % sp["cost"], 10, Color(0.6, 0.75, 1.0))
		var cd: float = s.cooldowns.get(sid, 0.0) - s.clock
		if cd > 0.0:
			var cdfrac := cd / float(sp["cooldown"])
			draw_rect(Rect2(r.position, Vector2(r.size.x, r.size.y * clampf(cdfrac, 0.0, 1.0))), Color(0, 0, 0, 0.6))
	_text(Vector2(hx, sz.y - 94.0), "attunement %d / %d    (Ctrl+1-4 = page 2)" % [s.slots_used(), s.v("attunement_slots")], 12, Color(0.8, 0.8, 0.8))

	# toasts
	var ty := sz.y - 130.0
	for t in toasts:
		var a := clampf(t[1], 0.0, 1.0)
		_text(Vector2(sz.x * 0.5 - 150.0, ty), t[0], 15, Color(1, 1, 1, a))
		ty -= 20.0

	if show_debug:
		var lines: Array = []
		lines.append("STATE  %s%s" % [STATE_NAMES[player.state], "   RAIN" if rain else ""])
		lines.append("speed  %.2f m/s   vy %.2f" % [player.last_h_speed, player.velocity.y])
		lines.append("pos    %.1f  %.1f  %.1f" % [player.global_position.x, player.global_position.y, player.global_position.z])
		lines.append("move_speed   %.2f" % s.v("move_speed"))
		lines.append("attack_speed %.2f" % s.v("attack_speed"))
		lines.append("armor        %.1f" % s.v("armor"))
		lines.append("jump_height  %.2f m" % s.v("jump_height"))
		lines.append("climb_speed  %.2f" % s.v("climb_speed"))
		lines.append("carry        %.1f kg" % s.v("carry"))
		lines.append("stamina max  %.0f" % s.v("stamina"))
		lines.append("")
		lines.append("WASD move   Shift sprint   Space jump")
		lines.append("C crouch  Alt dodge  E (hold) carry  LMB throw")
		lines.append("1-4 buffs (Ctrl = page 2)   mouse look/wheel zoom")
		lines.append("jump into steep rock to climb (C drops)")
		lines.append("LMB punch (+mouse flick: up/down/left/right)  hold = charge")
		lines.append("E kick when fighting (+flick)  RMB guard/parry  MMB lock")
		lines.append("Tab / I: status window (stats, gear, skills)")
		lines.append("G ragdoll  F1 rain  F2 restore  F3 hud  F5 respawn")
		var y := 24.0
		for ln in lines:
			_text(Vector2(sz.x - 340.0, y), ln, 13, Color(0.9, 0.95, 1.0))
			y += 17.0
