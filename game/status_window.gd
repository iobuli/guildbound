class_name StatusWindow
extends Control
## The "status window" (isekai style): one screen with everything the stat engine knows.
##   STATUS     attributes and every derived stat; hover = the full breakdown (base, +flat, x%, caps)
##   GEAR       equipment slots and the bag; click to equip / unequip / use, hover = appraisal (what
##              equipping it would change, in your numbers)
##   SKILLS     every Boon on the hotbar: cost, cooldown, upkeep, each modifier, backlash
##   COMBAT     the moves at YOUR attack speed (startup / active / recovery), the timing rules
##   CONTROLS
## Opens with Tab (or I); the world pauses while it is open.

const BG := Color(0.03, 0.08, 0.17, 0.92)
const EDGE := Color(0.35, 0.75, 1.0, 0.9)
const TEXT := Color(0.86, 0.95, 1.0)
const DIM := Color(0.55, 0.7, 0.85)
const GOOD := Color(0.45, 1.0, 0.55)
const BAD := Color(1.0, 0.45, 0.4)
const GOLD := Color(1.0, 0.85, 0.35)
const TABS := ["STATUS", "GEAR", "SKILLS", "COMBAT", "CONTROLS"]
const STAT_HELP := {
	"attack_damage": "Damage of every blow before the target's armor. Weapons add their base; STR (or AGI for light weapons) adds on top.",
	"attack_speed": "Strikes per second at 1.0. Startup and recovery of every move shrink as it rises (soft cap 2.5, hard cap 3.5).",
	"power": "How hard your blows land: the target's flinch, stagger, knockdown and how far it is shoved all scale with Power. STR raises it.",
	"crit_chance": "Chance for a blow to crit.", "crit_damage": "Damage multiplier of a crit.",
	"armor_pen": "Share of the target's armor ignored.", "lifesteal": "Share of the damage you deal that heals you.",
	"spell_power": "Strength of spells that scale with it.",
	"max_hp": "Health. STR and level raise it.", "hp_regen": "Health per second.",
	"armor": "Physical damage taken = 100 / (100 + armor).", "magic_resist": "Same curve for magic.",
	"tenacity": "Shrugs off impact: less stagger and knockdown (CC resist).",
	"max_mana": "Mana for casting and upkeep. INT raises it.", "mana_regen": "Mana per second.",
	"cdr": "Cooldown reduction (cap 40%).", "attunement_slots": "How many sustained Boons you can hold at once.",
	"move_speed": "Run speed in m/s (sprint x1.35, capped at 12).", "climb_speed": "Climbing speed.",
	"jump_height": "Jump apex in metres.", "stamina": "Sprint, climb, dodge, guard and strikes spend it.",
	"carry": "Kilograms you can lift; heavier props can only be pushed.",
}

var player: Player
var tab := 0
var _body: VBoxContainer
var _tip: PanelContainer
var _tip_text: RichTextLabel
var _tab_btns: Array[Button] = []
var _header: RichTextLabel


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_STOP
	visible = false
	var dim := ColorRect.new()
	dim.color = Color(0, 0, 0, 0.35)
	dim.set_anchors_preset(Control.PRESET_FULL_RECT)
	add_child(dim)
	var panel := PanelContainer.new()
	panel.add_theme_stylebox_override("panel", _box(BG, EDGE, 2))
	# fills the screen less a margin, whatever the window size; the content scrolls inside
	panel.set_anchors_preset(Control.PRESET_FULL_RECT)
	panel.offset_left = 60.0
	panel.offset_top = 40.0
	panel.offset_right = -60.0
	panel.offset_bottom = -40.0
	panel.clip_contents = true
	add_child(panel)
	var v := VBoxContainer.new()
	v.add_theme_constant_override("separation", 8)
	panel.add_child(v)
	_header = _rich(16)
	_header.custom_minimum_size = Vector2(0, 56)
	_header.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	v.add_child(_header)
	var tabs := HBoxContainer.new()
	v.add_child(tabs)
	for i in TABS.size():
		var b := Button.new()
		b.text = TABS[i]
		b.toggle_mode = true
		b.custom_minimum_size = Vector2(130, 30)
		_style_button(b)
		b.pressed.connect(func() -> void: show_tab(i))
		tabs.add_child(b)
		_tab_btns.append(b)
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	scroll.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	v.add_child(scroll)
	_body = VBoxContainer.new()
	_body.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_body.add_theme_constant_override("separation", 4)
	scroll.add_child(_body)
	var foot := _label("Tab / I / Esc: close    hover anything for details", 12, DIM)
	v.add_child(foot)
	_tip = PanelContainer.new()
	_tip.add_theme_stylebox_override("panel", _box(Color(0.02, 0.05, 0.12, 0.97), GOLD, 1))
	_tip.visible = false
	_tip.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_tip.z_index = 10
	add_child(_tip)
	_tip_text = _rich(13)
	_tip_text.custom_minimum_size = Vector2(380, 10)
	_tip_text.fit_content = true
	_tip_text.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_tip.add_child(_tip_text)


func _input(event: InputEvent) -> void:  # before the GUI: Tab is also its focus-next key
	if event.is_action_pressed("menu") or (visible and event.is_action_pressed("free_mouse")):
		toggle()
		get_viewport().set_input_as_handled()


func is_open() -> bool:
	return visible


func toggle() -> void:
	if visible:
		close()
	else:
		open()


func open() -> void:
	visible = true
	get_tree().paused = true
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	show_tab(tab)


func close() -> void:
	visible = false
	_tip.visible = false
	get_tree().paused = false
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED


func _process(_delta: float) -> void:
	if _tip.visible:
		var m := get_viewport().get_mouse_position() + Vector2(18, 18)
		var vs := get_viewport_rect().size
		_tip.position = Vector2(minf(m.x, vs.x - _tip.size.x - 8), minf(m.y, vs.y - _tip.size.y - 8))


# ---- tabs ------------------------------------------------------------------------------------

func show_tab(i: int) -> void:
	tab = i
	for k in _tab_btns.size():
		_tab_btns[k].button_pressed = k == i
	for c in _body.get_children():
		c.queue_free()
	_refresh_header()
	match i:
		0:
			_status()
		1:
			_gear()
		2:
			_skills()
		3:
			_combat()
		4:
			_controls()


func _refresh_header() -> void:
	var s := player.sheet
	var r: Dictionary = DataStore.races()[s.race]
	_header.text = "[font_size=22][color=#9fd8ff]STATUS[/color][/font_size]   [b]%s[/b]   %s   Lv %d   [color=#ffd95a]%d gold[/color]\n" % [
		s.display_name, r["name"], s.level, s.gold] + \
		"HP [color=#ff8a80]%d / %d[/color]    Mana [color=#82b1ff]%d / %d[/color]    Stamina [color=#a5f2a5]%d / %d[/color]    Racial: %s" % [
		s.hp, s.v("max_hp"), s.mana, s.v("max_mana"), player.stamina, s.v("stamina"), r["passive"]["name"]]


func _status() -> void:
	var s := player.sheet
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 18)
	_body.add_child(row)
	var attrs := VBoxContainer.new()
	attrs.custom_minimum_size = Vector2(230, 0)
	row.add_child(attrs)
	attrs.add_child(_label("ATTRIBUTES", 15, EDGE))
	var help := {"str": "Strength: Max HP, carry, weapon damage (STR weapons), Power.",
		"agi": "Agility: armor, attack speed, weapon damage (AGI weapons).",
		"int": "Intellect: mana, mana regen, spell power, attunement slots."}
	for a in ["str", "agi", "int"]:
		var r := s.attribute(a)
		var b := _stat_button("%s   %s" % [a.to_upper(), Stats._g(snappedf(r["value"], 0.1))])
		var lines: Array = r["lines"]
		b.mouse_entered.connect(func() -> void: _show_tip("[b]%s[/b]\n%s\n\n[color=#8fb8d8]%s[/color]" % [a.to_upper(), help[a], "\n".join(lines)]))
		b.mouse_exited.connect(_hide_tip)
		attrs.add_child(b)
	for g in CharModel.GROUPS:
		var col := VBoxContainer.new()
		col.custom_minimum_size = Vector2(190, 0)
		row.add_child(col)
		col.add_child(_label(String(g[0]).to_upper(), 15, EDGE))
		for st in g[1]:
			var r := s.stat(st)
			var b := _stat_button("%-14s %s" % [CharModel.STAT_NAMES.get(st, st), CharModel.fmt_stat(st, r["value"])])
			var stn: String = st
			b.mouse_entered.connect(func() -> void: _show_tip("[b]%s[/b]  %s\n%s\n\n[color=#8fb8d8]%s[/color]" % [
				CharModel.STAT_NAMES.get(stn, stn), CharModel.fmt_stat(stn, s.v(stn)), STAT_HELP.get(stn, ""), s.explain(stn)]))
			b.mouse_exited.connect(_hide_tip)
			col.add_child(b)
	_body.add_child(_label("Active effects", 15, EDGE))
	if s.effects.is_empty():
		_body.add_child(_label("  none", 13, DIM))
	for e in s.effects:
		var t := "  %s  (%s)  " % [e["name"], "sustained" if e["expires_at"] == null else "%ds left" % maxf(0.0, float(e["expires_at"]) - s.clock)]
		var ms: Array = []
		for m in e["mods"]:
			ms.append(CharModel.mod_text(m))
		_body.add_child(_label(t + ", ".join(ms), 13, TEXT))


func _gear() -> void:
	var s := player.sheet
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 24)
	_body.add_child(row)
	var slots := VBoxContainer.new()
	slots.custom_minimum_size = Vector2(430, 0)
	row.add_child(slots)
	slots.add_child(_label("EQUIPPED   (click to take off)", 15, EDGE))
	for slot in CharModel.SLOTS:
		var id: String = s.equipment.get(slot, "")
		var b := _stat_button("%-11s %s" % [slot.capitalize(), s.item_data(id).get("name", "-") if id != "" else "-"])
		if id != "":
			var sl: String = slot
			b.pressed.connect(func() -> void:
				s.unequip(sl)
				player.toast.emit("Took off " + s.item_data(id).get("name", id))
				show_tab(1))
			b.mouse_entered.connect(func() -> void: _show_tip(item_tip(id, false)))
			b.mouse_exited.connect(_hide_tip)
		slots.add_child(b)
	slots.add_child(_label("Weight worn: %.1f kg   Carry: %.0f kg" % [s.carried_weight(), s.v("carry")], 13, DIM))
	var bagc := VBoxContainer.new()
	row.add_child(bagc)
	bagc.add_child(_label("BAG   (click to equip / use)", 15, EDGE))
	var grid := GridContainer.new()
	grid.columns = 3
	bagc.add_child(grid)
	if s.bag.is_empty():
		bagc.add_child(_label("  empty - items lying in the world: walk up and press E", 13, DIM))
	for id in s.bag.keys():
		var n: int = s.bag[id]
		var b := Button.new()
		b.text = "%s%s" % [s.item_data(id).get("name", id), "  x%d" % n if n > 1 else ""]
		b.custom_minimum_size = Vector2(185, 34)
		_style_button(b)
		var iid: String = id
		b.pressed.connect(func() -> void: use_item(iid))
		b.mouse_entered.connect(func() -> void: _show_tip(item_tip(iid, true)))
		b.mouse_exited.connect(_hide_tip)
		grid.add_child(b)


## Equip gear / use a consumable from the bag (also used by the self-test).
func use_item(id: String) -> void:
	var s := player.sheet
	if CharModel.is_consumable(id):
		var r := s.consume(id)
		player.toast.emit(r[1])
	elif s.equip_from_bag(id):
		player.toast.emit("Equipped " + s.item_data(id).get("name", id))
	_hide_tip()
	if visible:
		show_tab(tab)


## Item tooltip; with `appraise`, the exact change equipping it makes to your stats.
func item_tip(id: String, appraise: bool) -> String:
	var s := player.sheet
	var d := s.item_data(id)
	var t := "[b][color=#ffd95a]%s[/color][/b]" % d.get("name", id)
	if CharModel.is_consumable(id):
		t += "   consumable, %d s\n" % int(d.get("duration", 0))
	else:
		t += "   %s, tier %d, %.1f kg\n" % [String(d.get("slot", "")).capitalize(), int(d.get("tier", 1)), float(d.get("weight", 0.0))]
	var w: Dictionary = d.get("weapon", {})
	if not w.is_empty():
		t += "Weapon: %d damage, %.2f attacks/s, scales with %s\n" % [int(w["base_damage"]), float(w["aps"]), String(w["scales"]).to_upper()]
	for m in d.get("mods", []):
		t += "  %s\n" % CharModel.mod_text(m)
	if appraise and not CharModel.is_consumable(id):
		var diff := s.appraise(id)
		var slot := s.slot_for(id)
		if s.equipment.has(slot):
			t += "\n[color=#8fb8d8]replaces %s[/color]\n" % s.item_data(s.equipment[slot]).get("name", "")
		t += "[b]Appraisal[/b] (your numbers if equipped):\n"
		if diff.is_empty():
			t += "  no change\n"
		for st in diff:
			var a: float = diff[st][0]
			var b: float = diff[st][1]
			var good := b > a
			t += "  [color=#%s]%s  %s -> %s[/color]\n" % ["73ff8c" if good else "ff7366", CharModel.STAT_NAMES.get(st, st),
				CharModel.fmt_stat(st, a), CharModel.fmt_stat(st, b)]
	return t


func _skills() -> void:
	var s := player.sheet
	var sp := DataStore.spells()
	_body.add_child(_label("BOONS ON YOUR HOTBAR   (1-4, Ctrl+1-4 for page 2; press again to dismiss)", 15, EDGE))
	var pages := [Player.PAGE1, Player.PAGE2]
	for pi in 2:
		var grid := GridContainer.new()
		grid.columns = 2
		_body.add_child(grid)
		for k in 4:
			var sid: String = pages[pi][k]
			var d: Dictionary = sp[sid]
			var active := s.find_effect(sid) != null
			var b := _stat_button("%s%d  %s  [%s %s]%s" % ["Ctrl+" if pi == 1 else "", k + 1, d["name"], d["aspect"], d["family"], "  ACTIVE" if active else ""])
			b.custom_minimum_size = Vector2(510, 30)
			var sd := d
			b.mouse_entered.connect(func() -> void: _show_tip(spell_tip(sd)))
			b.mouse_exited.connect(_hide_tip)
			grid.add_child(b)
	_body.add_child(_label("Same family = the strongest one wins (no stacking a family with itself). Light: reliable. Dark: ~1.4x the peak, paid for with HP drain, a stat trade or backlash.", 13, DIM))


static func spell_tip(d: Dictionary) -> String:
	var col: String = {"light": "ffe9a8", "dark": "c792ea", "neutral": "9fd8ff"}.get(d["aspect"], "ffffff")
	var t := "[b][color=#%s]%s[/color][/b]   %s %s, %s\n" % [col, d["name"], d["aspect"], d["family"], d["mode"]]
	t += "Cost %s mana   cooldown %s s" % [Stats._g(float(d["cost"])), Stats._g(float(d["cooldown"]))]
	if d.has("duration") and d["duration"] != null:
		t += "   lasts %s s" % Stats._g(float(d["duration"]))
	t += "\n"
	if float(d.get("upkeep", 0.0)) > 0.0:
		t += "Upkeep %s mana/s" % Stats._g(float(d["upkeep"]))
	if float(d.get("hp_drain", 0.0)) > 0.0:
		t += "   drains %s HP/s" % Stats._g(float(d["hp_drain"]))
	if int(d.get("slots", 0)) > 0:
		t += "   uses %d attunement slot" % int(d["slots"])
	t += "\n"
	for m in d["mods"]:
		t += "  %s\n" % CharModel.mod_text(m)
	var bl = d.get("backlash")
	if bl != null:
		var ms: Array = []
		for m in bl["mods"]:
			ms.append(CharModel.mod_text(m))
		t += "[color=#ff7366]Backlash on ending: %s for %ss (%s)[/color]\n" % [bl["name"], Stats._g(float(bl["duration"])), ", ".join(ms)]
	return t


func _combat() -> void:
	var s := player.sheet
	var k := clampf(Combat.REF_ATTACK_SPEED / maxf(0.1, s.v("attack_speed")), 0.5, 1.5)
	_body.add_child(_label("MOVES AT YOUR ATTACK SPEED (%.2f/s)   startup / active / recovery in ms" % s.v("attack_speed"), 15, EDGE))
	for id in Combat.move_ids():
		var m := Combat.move_data(id)
		var line := "%-15s %-26s %4d / %3d / %4d ms   dmg x%.2f   impact %.1f   %d stamina" % [m.get("name", id), m.get("input", ""),
			roundi(float(m["startup"]) * k * 1000.0), roundi(float(m["active"]) * 1000.0), roundi(float(m["recovery"]) * k * 1000.0),
			float(m["mult"]), float(m["impact"]) * s.v("power"), int(m["stamina"])]
		_body.add_child(_label(line, 13, TEXT))
	for ln in [
		"Directional strikes: move the mouse as you click (up = uppercut, down = body blow, left / right = hooks); E kicks the same way.",
		"Frame-perfect chain: after a hit, the gold ring closes - strike again on the beat (+-50 ms) for PERFECT: faster, x1.3 damage and impact.",
		"Three perfects in a row, then E: finisher.",
		"Guard (RMB) takes 20% and costs stamina; tap RMB as the blow lands to PARRY (staggers them).",
		"Dodge (Alt) has i-frames; dodging just as a blow comes in is a PERFECT DODGE: Flurry (x1.3 attack speed, the attacker slows).",
		"Impact x your Power vs their Tenacity: flinch (< 1.2), stagger (>= 1.2), knockdown (>= 3.5, or >= 2 on a staggered target)."]:
		_body.add_child(_label(ln, 13, DIM))


func _controls() -> void:
	for ln in ["WASD move   Shift sprint   Space jump (side-hop when locked on)   C crouch   Alt dodge",
		"Mouse: look   LMB strike (hold: charge)   RMB guard / parry   MMB lock on   wheel: next target (zoom when not locked)",
		"E: pick up item / hold to carry a prop / kick when fighting   LMB while carrying: throw",
		"1-4: Boons   Ctrl+1-4: page 2   G: ragdoll   Tab / I: status window",
		"F1 rain   F2 restore   F3 debug HUD   F5 respawn   Esc: free the mouse"]:
		_body.add_child(_label(ln, 14, TEXT))


# ---- widgets ---------------------------------------------------------------------------------

func _show_tip(t: String) -> void:
	_tip_text.text = t
	_tip.visible = true
	_tip.reset_size()


func _hide_tip() -> void:
	_tip.visible = false


static func _box(bg: Color, edge: Color, w: int) -> StyleBoxFlat:
	var b := StyleBoxFlat.new()
	b.bg_color = bg
	b.border_color = edge
	b.set_border_width_all(w)
	b.set_corner_radius_all(6)
	b.set_content_margin_all(10)
	return b


func _style_button(b: Button) -> void:
	b.add_theme_stylebox_override("normal", _box(Color(0.06, 0.16, 0.3, 0.9), Color(0.3, 0.6, 0.9, 0.6), 1))
	b.add_theme_stylebox_override("hover", _box(Color(0.1, 0.25, 0.45, 0.95), EDGE, 1))
	b.add_theme_stylebox_override("pressed", _box(Color(0.15, 0.35, 0.6, 0.95), GOLD, 1))
	b.add_theme_color_override("font_color", TEXT)
	b.add_theme_color_override("font_pressed_color", GOLD)
	b.add_theme_font_size_override("font_size", 13)


func _stat_button(text: String) -> Button:
	var b := Button.new()
	b.text = text
	b.alignment = HORIZONTAL_ALIGNMENT_LEFT
	b.flat = true
	b.add_theme_color_override("font_color", TEXT)
	b.add_theme_color_override("font_hover_color", GOLD)
	b.add_theme_font_size_override("font_size", 13)
	return b


func _label(text: String, size: int, col: Color) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", size)
	l.add_theme_color_override("font_color", col)
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	return l


func _rich(size: int) -> RichTextLabel:
	var r := RichTextLabel.new()
	r.bbcode_enabled = true
	r.scroll_active = false
	r.add_theme_font_size_override("normal_font_size", size)
	r.add_theme_font_size_override("bold_font_size", size)
	r.add_theme_color_override("default_color", TEXT)
	return r
