class_name Hud
extends Control
## Immediate-mode HUD: framed vitals (with trailing damage), icon buff bar, icon hotbar with cooldown
## sweeps, lock-on target frame, combo counter, stamina wheel on the character, toasts, F3 help.

var player: Player
var toasts: Array = []
var show_debug := false
var rain := false

const STATE_NAMES := ["GROUND", "AIR", "SWIM", "DODGE", "DOWN"]
const STONE := Color(0.13, 0.115, 0.09, 0.82)
const EDGE := Color(0.47, 0.40, 0.29, 0.95)
const GOLD := Color(1.0, 0.82, 0.35)
const HP_COL := Color(0.82, 0.16, 0.14)
const MP_COL := Color(0.22, 0.45, 0.95)
const ST_COL := Color(0.30, 0.78, 0.35)

var _font: Font
var _bold: Font
var _ghost_hp := -1.0       # trailing "damage taken" bar (drains after a beat)
var _ghost_hold := 0.0
var _ghost_tgt := -1.0
var _ghost_tgt_hold := 0.0
var _ghost_who: Player = null
var _combo := 0
var _combo_t := 0.0
var _combo_pop := 0.0
var _hits_seen := 0
var _combo_names: Array = []
var _t := 0.0


func _ready() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	var f := SystemFont.new()
	f.font_names = PackedStringArray(["Segoe UI", "Noto Sans", "DejaVu Sans", "sans-serif"])
	_font = f
	var b := SystemFont.new()
	b.font_names = f.font_names
	b.font_weight = 700
	_bold = b


func add_toast(text: String) -> void:
	toasts.append([text, 4.0])
	if toasts.size() > 5:
		toasts.pop_front()


func _process(delta: float) -> void:
	_t += delta
	for t in toasts:
		t[1] -= delta
	toasts = toasts.filter(func(t): return t[1] > 0.0)
	if player != null and player.sheet != null:
		_track(delta)
	queue_redraw()


## Trailing bars and the combo counter (state the drawing needs between frames).
func _track(delta: float) -> void:
	var s := player.sheet
	var f := s.hp / maxf(1.0, s.v("max_hp"))
	if _ghost_hp < 0.0 or f > _ghost_hp:
		_ghost_hp = f
	elif f < _ghost_hp:
		_ghost_hold -= delta
		if _ghost_hold <= 0.0:
			_ghost_hp = move_toward(_ghost_hp, f, delta * 0.5)
	if absf(_ghost_hp - f) < 0.001:
		_ghost_hold = 0.5
	var tgt := player.lock.target
	if tgt != null and is_instance_valid(tgt):
		var tf := tgt.sheet.hp / maxf(1.0, tgt.sheet.v("max_hp"))
		if tgt != _ghost_who or tf > _ghost_tgt:
			_ghost_tgt = tf
			_ghost_who = tgt
		elif tf < _ghost_tgt:
			_ghost_tgt_hold -= delta
			if _ghost_tgt_hold <= 0.0:
				_ghost_tgt = move_toward(_ghost_tgt, tf, delta * 0.6)
		if absf(_ghost_tgt - tf) < 0.001:
			_ghost_tgt_hold = 0.45
	# combo: landed hits less than 1.4 s apart
	var cb := player.combat
	if cb.hits_landed != _hits_seen:
		_hits_seen = cb.hits_landed
		if cb.last_given in ["hit", "stagger", "dead"]:
			_combo = _combo + 1 if _combo_t > 0.0 else 1
			if _combo == 1:
				_combo_names.clear()
			_combo_names.append(str(cb.move.get("name", "")))
			if _combo_names.size() > 4:
				_combo_names.pop_front()
			_combo_t = 1.4
			_combo_pop = 1.0
	_combo_t -= delta
	_combo_pop = maxf(0.0, _combo_pop - delta * 4.0)


# ---- drawing helpers -------------------------------------------------------------------------

func _text(pos: Vector2, s: String, font_size: int = 14, col: Color = Color.WHITE, bold := false) -> void:
	var f := _bold if bold else _font
	draw_string_outline(f, pos, s, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size, 4 if font_size >= 16 else 2, Color(0, 0, 0, 0.75 * col.a))
	draw_string(f, pos, s, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size, col)


func _text_c(center_x: float, y: float, s: String, font_size: int, col: Color, bold := false) -> void:
	var f := _bold if bold else _font
	var w := f.get_string_size(s, HORIZONTAL_ALIGNMENT_LEFT, -1, font_size).x
	_text(Vector2(center_x - w * 0.5, y), s, font_size, col, bold)


func _panel(r: Rect2, bg: Color = STONE, edge: Color = EDGE, radius := 6) -> void:
	var sb := StyleBoxFlat.new()
	sb.bg_color = bg
	sb.border_color = edge
	sb.set_border_width_all(2)
	sb.set_corner_radius_all(radius)
	sb.shadow_color = Color(0, 0, 0, 0.35)
	sb.shadow_size = 4
	draw_style_box(sb, r)


## A bar with a dark track, a trailing ghost (damage just taken), a sheen and centred numbers.
func _bar(r: Rect2, frac: float, col: Color, caption: String, ghost: float = -1.0) -> void:
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.05, 0.04, 0.03, 0.85)
	sb.set_corner_radius_all(3)
	draw_style_box(sb, r)
	var inner := r.grow(-2.0)
	frac = clampf(frac, 0.0, 1.0)
	if ghost > frac:
		draw_rect(Rect2(inner.position, Vector2(inner.size.x * clampf(ghost, 0.0, 1.0), inner.size.y)), Color(1.0, 0.92, 0.75, 0.75))
	if frac > 0.0:
		var fill := Rect2(inner.position, Vector2(inner.size.x * frac, inner.size.y))
		draw_rect(fill, col)
		draw_rect(Rect2(fill.position, Vector2(fill.size.x, fill.size.y * 0.42)), Color(1, 1, 1, 0.16))
		draw_rect(Rect2(fill.position + Vector2(0, fill.size.y - 2), Vector2(fill.size.x, 2)), col.darkened(0.4))
	if caption != "":
		var fs := int(clampf(r.size.y - 5.0, 10.0, 15.0))
		_text_c(r.position.x + r.size.x * 0.5, r.position.y + r.size.y * 0.5 + fs * 0.36, caption, fs, Color(1, 1, 1, 0.95), true)


func _icon(tex: Texture2D, r: Rect2, tint := Color.WHITE) -> void:
	if tex != null:
		draw_texture_rect(tex, r, false, tint)


## Clockwise dark sweep over a square (cooldown), `frac` of it still covered.
func _sweep(r: Rect2, frac: float) -> void:
	if frac <= 0.0:
		return
	var c := r.get_center()
	var rad := r.size.length() * 0.5
	var pts := PackedVector2Array([c])
	var n := 24
	for k in n + 1:
		var a := -PI / 2.0 + TAU * (1.0 - frac) + TAU * frac * float(k) / n
		var p := c + Vector2(cos(a), sin(a)) * rad
		pts.append(Vector2(clampf(p.x, r.position.x, r.end.x), clampf(p.y, r.position.y, r.end.y)))
	draw_colored_polygon(pts, Color(0, 0, 0, 0.62))


# ---- frame -----------------------------------------------------------------------------------

func _draw() -> void:
	if player == null or player.sheet == null:
		return
	var s := player.sheet
	var sz := get_viewport_rect().size
	_vitals(s)
	_buffs(s)
	var cam := player.cam.camera
	_stamina_wheel(s, cam)
	_fight(s, cam, sz)
	_hotbar(s, sz)
	_combo_counter(sz)
	_toasts(sz)
	_element_bar(sz)
	var near: Dictionary = player.arena.interaction_near(player.global_position) if player.arena != null else {}
	if not near.is_empty():
		_text_c(sz.x * 0.5, sz.y * 0.62, "E: " + str(near["name"]), 18, Color(1.0, 0.92, 0.6), true)
	_text(Vector2(16, sz.y - 14.0), "Tab inventory   P character   F3 controls", 12, Color(0.95, 0.88, 0.7, 0.7))
	if show_debug:
		_debug(s, sz)


func _vitals(s: CharModel) -> void:
	var r := Rect2(14, 14, 318, 92)
	_panel(r)
	# level badge
	var bc := Vector2(52, 60)
	draw_circle(bc, 30.0, Color(0.08, 0.07, 0.05, 0.95))
	draw_arc(bc, 30.0, 0.0, TAU, 40, GOLD, 2.5, true)
	_text_c(bc.x, bc.y - 6.0, "LV", 10, GOLD, true)
	_text_c(bc.x, bc.y + 14.0, str(s.level), 20, Color.WHITE, true)
	_text(Vector2(92, 34), "%s  [%s]" % [s.display_name, s.rank_name()], 14, GOLD, true)
	_text(Vector2(248, 34), "%d gold" % s.gold, 12, GOLD)
	var max_hp := s.v("max_hp")
	var max_mp := s.v("max_mana")
	var smax := s.v("stamina")
	_bar(Rect2(92, 42, 228, 20), s.hp / max_hp, HP_COL, "%d / %d" % [s.hp, max_hp], _ghost_hp)
	_bar(Rect2(92, 65, 228, 14), s.mana / max_mp, MP_COL, "%d / %d" % [s.mana, max_mp])
	var st_col := ST_COL if not player.exhausted else Color(0.9, 0.3, 0.2).lerp(Color(1.0, 0.6, 0.2), 0.5 + 0.5 * sin(_t * 10.0))
	_bar(Rect2(92, 82, 228, 10), player.stamina / smax, st_col, "")


func _buffs(s: CharModel) -> void:
	var x := 16.0
	var y := 114.0
	for e in s.effects:
		var col := Color(0.62, 0.62, 0.66)
		match e["aspect"]:
			"light":
				col = Color(1.0, 0.85, 0.3)
			"dark":
				col = Color(0.7, 0.35, 1.0)
		if e["kind"] == "backlash":
			col = Color(0.95, 0.25, 0.2)
		elif e["kind"] == "consumable":
			col = Color(0.45, 0.85, 0.45)
		elif e["kind"] == "combat":
			col = Color(0.5, 1.0, 0.8)
		elif e["kind"] == "referee":
			col = Color(0.55, 0.75, 1.0)
		var key: String = e["key"]
		var tex: Texture2D = Icons.tex(str(e["icon"])) if e.has("icon") else Icons.item(key) if e["kind"] == "consumable" else (Icons.spell(key) if DataStore.spells().has(key) else Icons.tex("S_Wind01" if key == "flurry" else "S_Shadow14"))
		var box := Rect2(x, y, 40, 40)
		_panel(box, Color(0.07, 0.06, 0.05, 0.85), col, 4)
		_icon(tex, box.grow(-4.0))
		var left := -1.0 if e["expires_at"] == null else maxf(0.0, float(e["expires_at"]) - s.clock)
		if left >= 0.0 and left < 5.0 and fmod(_t, 0.5) < 0.25:
			draw_rect(box.grow(-2.0), Color(1, 1, 1, 0.18))  # about to run out: blinks
		_text_c(box.get_center().x, y + 54.0, "on" if left < 0.0 else ("%ds" % ceili(left) if left < 60.0 else "%dm" % int(left / 60.0)), 11, Color.WHITE, true)
		var m0 := ""
		if e["mods"].size() > 0:
			m0 = CharModel.mod_text(e["mods"][0])
		_text(Vector2(x - 2.0, y + 68.0), m0.substr(0, 14), 9, Color(0.85, 0.92, 1.0, 0.9))
		x += 74.0


func _stamina_wheel(s: CharModel, cam: Camera3D) -> void:
	if cam == null or cam.is_position_behind(player.global_position):
		return
	var smax := s.v("stamina")
	var frac := player.stamina / smax
	if frac >= 0.999 and not player.exhausted:
		return
	var c := cam.unproject_position(player.global_position + Vector3.UP * 1.0) + Vector2(46, -10)
	draw_arc(c, 16.0, 0.0, TAU, 32, Color(0, 0, 0, 0.5), 6.0, true)
	var col := Color(0.35, 0.9, 0.4).lerp(Color(1.0, 0.8, 0.2), 1.0 - frac)
	if player.exhausted:
		col = Color(0.95, 0.25, 0.2)
	draw_arc(c, 16.0, -PI / 2.0, -PI / 2.0 + TAU * frac, 32, col, 5.0, true)


func _fight(_s: CharModel, cam: Camera3D, sz: Vector2) -> void:
	if cam == null:
		return
	var tgt := player.lock.current()
	if tgt != null and is_instance_valid(tgt):
		var chest: Vector3 = tgt.model.chest_position()
		if not cam.is_position_behind(chest):
			var rc := cam.unproject_position(chest)
			var locked := tgt == player.lock.target
			var rcol := Color(1.0, 0.35, 0.3) if locked else Color(1.0, 1.0, 1.0, 0.6)
			if locked:
				# four corner brackets that slowly turn
				var a0 := _t * 0.8
				for k in 4:
					var a := a0 + k * PI / 2.0
					draw_arc(rc, 18.0, a - 0.35, a + 0.35, 8, rcol, 2.5, true)
				draw_circle(rc, 2.5, rcol)
			else:
				draw_arc(rc, 8.0, 0.0, TAU, 24, rcol, 1.5, true)
		if tgt == player.lock.target:
			var ts := tgt.sheet
			var fr := Rect2(sz.x * 0.5 - 190.0, 14.0, 380.0, 50.0)
			_panel(fr)
			var tag := ""
			var tag_col := Color(0.8, 0.8, 0.8)
			match tgt.combat.state:
				Combat.S.GUARD:
					tag = "GUARDING"
					tag_col = Color(0.55, 0.8, 1.0)
				Combat.S.STAGGER:
					tag = "STAGGERED"
					tag_col = Color(1.0, 0.75, 0.3)
			if tgt.state == Player.State.DEAD:
				tag = "DOWN"
				tag_col = Color(1.0, 0.45, 0.35)
			_text(fr.position + Vector2(12, 19), "%s   Lv %d" % [ts.display_name, ts.level], 14, GOLD, true)
			if tag != "":
				var f := _bold
				var w := f.get_string_size(tag, HORIZONTAL_ALIGNMENT_LEFT, -1, 12).x
				_text(Vector2(fr.end.x - 12.0 - w, fr.position.y + 19.0), tag, 12, tag_col, true)
			_bar(Rect2(fr.position + Vector2(10, 26), Vector2(360, 16)), ts.hp / ts.v("max_hp"), HP_COL,
				"%d / %d" % [ts.hp, ts.v("max_hp")], _ghost_tgt)
	var cb := player.combat
	# aiming (bow / staff): a crosshair, and for the bow how far the string is drawn
	if cb.aiming():
		var c := sz * 0.5
		draw_circle(c, 2.5, Color(1, 1, 1, 0.9))
		for k in 4:
			var a := k * PI / 2.0
			draw_line(c + Vector2(cos(a), sin(a)) * 7.0, c + Vector2(cos(a), sin(a)) * 14.0, Color(1, 1, 1, 0.8), 2.0)
		if cb.stance_id == "bow":
			var full := float((cb.stance["arrow"] as Dictionary)["full_draw"])
			var dr := clampf(cb.aim_t / full, 0.0, 1.0)
			draw_arc(c, 22.0, -PI / 2.0, -PI / 2.0 + TAU * dr, 40, GOLD if dr >= 1.0 else Color(1, 1, 1, 0.7), 3.0, true)
	# the weapon in hand
	_text(Vector2(16, sz.y - 34.0), "%s   (Q to switch)" % str(cb.stance.get("name", "")), 14, Color(0.95, 0.88, 0.7, 0.9), true)


func _hotbar(s: CharModel, sz: Vector2) -> void:
	var spells := DataStore.spells()
	var alt := Input.is_key_pressed(KEY_CTRL)
	var page: Array = Player.PAGE2 if alt else Player.PAGE1
	var slot := 58.0
	var gap := 8.0
	var w := 4.0 * slot + 3.0 * gap + 24.0
	var fr := Rect2(sz.x * 0.5 - w * 0.5, sz.y - slot - 44.0, w, slot + 30.0)
	_panel(fr)
	for i in 4:
		var sid: String = page[i]
		var sp: Dictionary = spells[sid]
		var r := Rect2(fr.position.x + 12.0 + i * (slot + gap), fr.position.y + 8.0, slot, slot)
		var active := s.find_effect(sid) != null
		var asp: String = sp["aspect"]
		var col := Color(1.0, 0.85, 0.3) if asp == "light" else (Color(0.7, 0.35, 1.0) if asp == "dark" else Color(0.75, 0.75, 0.78))
		_panel(r, Color(0.05, 0.045, 0.035, 0.95), col if active else col.darkened(0.55), 4)
		if active:
			draw_rect(r.grow(2.0 + sin(_t * 5.0)), Color(col.r, col.g, col.b, 0.35), false, 2.0)
		var short := s.mana < float(sp["cost"]) and not active
		_icon(Icons.spell(sid), r.grow(-6.0), Color(0.45, 0.45, 0.55) if short else Color.WHITE)
		var cd: float = float(s.cooldowns.get(sid, 0.0)) - s.clock
		if cd > 0.0:
			_sweep(r.grow(-2.0), clampf(cd / float(sp["cooldown"]), 0.0, 1.0))
			_text_c(r.get_center().x, r.get_center().y + 6.0, "%.1f" % cd if cd < 10.0 else str(int(cd)), 15, Color.WHITE, true)
		_text(r.position + Vector2(4, 14), ("^" if alt else "") + str(i + 1), 12, Color(1, 1, 0.55), true)
		_text_c(r.get_center().x, r.end.y + 13.0, String(sp["name"]).substr(0, 11), 10, Color(0.95, 0.9, 0.78, 0.9))
		var cost := "%d" % sp["cost"]
		_text(Vector2(r.end.x - 6.0 - _font.get_string_size(cost, HORIZONTAL_ALIGNMENT_LEFT, -1, 10).x, r.end.y - 4.0), cost, 10, Color(0.6, 0.78, 1.0), true)
	_text_c(fr.get_center().x, fr.position.y - 6.0, "attunement %d / %d    (Ctrl: page 2)" % [s.slots_used(), s.v("attunement_slots")], 11, Color(0.95, 0.88, 0.7, 0.75))


func _combo_counter(sz: Vector2) -> void:
	if _combo < 2 or _combo_t <= 0.0:
		return
	var a := clampf(_combo_t / 0.4, 0.0, 1.0)
	var font_size := int(34.0 + 14.0 * _combo_pop)
	var x := sz.x - 200.0
	var y := sz.y * 0.42
	_text(Vector2(x, y), "%d" % _combo, font_size, Color(1.0, 0.85, 0.3, a), true)
	_text(Vector2(x + 14.0 + font_size * 0.6 * str(_combo).length(), y), "HITS", 18, Color(1, 1, 1, a), true)
	_text(Vector2(x, y + 22.0), " > ".join(_combo_names), 12, Color(0.95, 0.9, 0.78, a * 0.9))


func _toasts(sz: Vector2) -> void:
	var ty := sz.y - 150.0
	for k in range(toasts.size() - 1, -1, -1):
		var t: Array = toasts[k]
		var a := clampf(t[1], 0.0, 1.0)
		var txt: String = t[0]
		var w := _font.get_string_size(txt, HORIZONTAL_ALIGNMENT_LEFT, -1, 15).x + 24.0
		var rise := (1.0 - clampf((4.0 - t[1]) / 0.2, 0.0, 1.0)) * 8.0
		var r := Rect2(sz.x * 0.5 - w * 0.5, ty - 18.0 + rise, w, 24.0)
		var sb := StyleBoxFlat.new()
		sb.bg_color = Color(0.06, 0.05, 0.04, 0.6 * a)
		sb.set_corner_radius_all(10)
		draw_style_box(sb, r)
		_text_c(sz.x * 0.5, ty - 1.0 + rise, txt, 15, Color(1, 0.97, 0.9, a))
		ty -= 28.0


func _debug(s: CharModel, sz: Vector2) -> void:
	var lines: Array = []
	lines.append("STATE  %s%s" % [STATE_NAMES[player.state], "   RAIN" if rain else ""])
	lines.append("speed  %.2f m/s   vy %.2f" % [player.last_h_speed, player.velocity.y])
	lines.append("pos    %.1f  %.1f  %.1f" % [player.global_position.x, player.global_position.y, player.global_position.z])
	lines.append("move %.2f  atk spd %.2f  armor %.1f  power %.2f" % [s.v("move_speed"), s.v("attack_speed"), s.v("armor"), s.v("power")])
	lines.append("")
	lines.append("WASD move   Shift sprint   Space jump   C crouch")
	lines.append("Alt dodge (roll; a step while locked / guarding / aiming)")
	lines.append("LMB attack, press again to chain the combo; hold LMB = heavy")
	lines.append("E special attack (shield kick, leap...) or pick up an item")
	lines.append("RMB guard (tap as a blow lands = parry); bow / staff: aim, LMB fires")
	lines.append("Q switch weapon   MMB lock on, wheel switches target")
	lines.append("1-4 Arts (Ctrl = page 2)   Tab inventory   P character")
	lines.append("F1 rain  F2 restore  F3 this help  F5 respawn")
	var h := lines.size() * 17.0 + 16.0
	var r := Rect2(sz.x - 470.0, 80.0, 456.0, h)
	_panel(r, Color(0.08, 0.07, 0.05, 0.75))
	var y := r.position.y + 20.0
	for ln in lines:
		_text(Vector2(r.position.x + 12.0, y), ln, 12, Color(0.92, 0.95, 1.0))
		y += 17.0


## The magic element (X) and the Awakening meter (R when full), bottom-left.
func _element_bar(sz: Vector2) -> void:
	var cb := player.combat
	var e: Dictionary = HeroAnim.data["_elements"][cb.element]
	var col := Color(e["color"][0], e["color"][1], e["color"][2])
	var r := Rect2(16, sz.y - 74.0, 228, 12)
	var frac := cb.awaken_t / float(HeroAnim.data["_awakening"]["duration"]) if cb.awakened() else cb.awaken / 100.0
	var cap := ("%s %ds" % [e["awaken"], ceili(cb.awaken_t)]) if cb.awakened() else ("Awakening %d%%" % int(cb.awaken))
	_bar(r, frac, col.lerp(Color.WHITE, 0.3 if cb.awaken >= 100.0 and fmod(_t, 0.6) < 0.3 else 0.0), cap)
	_text(Vector2(16, sz.y - 80.0), "Element: %s   (X switch, R awaken)" % e["name"], 12, col)
