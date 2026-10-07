class_name SidePanel
extends Control
## RuneScape-style side panel, docked bottom-right: icon tabs along the top, the world keeps running.
##   COMBAT     your strings, flick moves, kicks and defence; the next punch / kick string
##   STATS      skill-grid of every stat (icon + value); hover = the full breakdown
##   INVENTORY  a 6 x 8 Tarkov-style grid: items take their size, drag to move, R turns them. Left click: wear / eat / drink;
##              right click: Wear / Use / Drop / Examine. Hover = appraisal in your own numbers
##   WORN       paper doll of the equipment slots (click to take off) and the gear totals
##   MAGIC      spellbook: click to cast / dismiss; hover = cost, upkeep, every modifier
##   CARD       the Hunter's Card: guild rank, quests carried and their progress, monsters slain
## Tab / I toggles it (the mouse is free while it is open; click the world to go back to fighting).

const TABS := ["combat", "stats", "inventory", "worn", "magic", "card"]
const TAB_NAMES := ["Combat", "Stats", "Inventory", "Worn equipment", "Spellbook", "Hunter's Card"]
const STONE := Color(0.24, 0.21, 0.17, 0.97)
const STONE_DARK := Color(0.15, 0.13, 0.10, 0.98)
const STONE_EDGE := Color(0.47, 0.40, 0.29)
const SLOT_BG := Color(0.19, 0.17, 0.13, 0.9)
const ORANGE := Color(1.0, 0.6, 0.1)
const YELLOW := Color(1.0, 1.0, 0.0)
const TEXT := Color(0.95, 0.88, 0.7)
const GOOD := Color(0.55, 0.95, 0.5)
const DOLL := [["", "head", ""], ["", "accessory1", ""], ["weapon", "body", "offhand"], ["hands", "feet", "accessory2"]]
const STAT_GRID := ["str", "agi", "int", "attack_damage", "power", "attack_speed", "crit_chance", "crit_damage", "lifesteal",
	"max_hp", "armor", "tenacity", "hp_regen", "magic_resist", "stamina", "max_mana", "spell_power", "mana_regen",
	"move_speed", "jump_height", "carry"]

var player: Player
var status: StatusWindow
var tab := 2
var _k := 1.0                       # UI scale (bigger windows get bigger slots)
var _frame: PanelContainer
var _title: Label
var _content: VBoxContainer
var _tab_btns: Array[Button] = []
var _tip: PanelContainer
var _tip_text: RichTextLabel
var _menu: PopupMenu
var _menu_id := ""
var _sig := ""
var _sig_t := 0.0


func _ready() -> void:
	set_anchors_preset(Control.PRESET_TOP_LEFT)  # sized by _fit()
	get_viewport().size_changed.connect(_fit)
	_fit.call_deferred()
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST  # crisp pixel-art icons
	visible = false
	_k = clampf(get_viewport_rect().size.y / 820.0, 1.0, 1.6)
	_frame = PanelContainer.new()
	_frame.add_theme_stylebox_override("panel", _box(STONE, STONE_EDGE, 3, 6))
	_frame.anchor_left = 1.0
	_frame.anchor_right = 1.0
	_frame.anchor_top = 1.0
	_frame.anchor_bottom = 1.0
	var w := 262.0 * _k
	var h := 460.0 * _k
	_frame.offset_left = -w - 12.0
	_frame.offset_right = -12.0
	_frame.offset_top = -h - 12.0
	_frame.offset_bottom = -12.0
	add_child(_frame)
	var v := VBoxContainer.new()
	v.add_theme_constant_override("separation", int(4 * _k))
	_frame.add_child(v)
	var tabs := HBoxContainer.new()
	tabs.add_theme_constant_override("separation", int(3 * _k))
	tabs.alignment = BoxContainer.ALIGNMENT_CENTER
	v.add_child(tabs)
	for i in TABS.size():
		var b := Button.new()
		b.icon = Icons.tex(Icons.UI[TABS[i]])
		b.expand_icon = true
		b.icon_alignment = HORIZONTAL_ALIGNMENT_CENTER
		b.custom_minimum_size = Vector2(38, 40) * _k
		b.toggle_mode = true
		b.tooltip_text = TAB_NAMES[i]
		b.add_theme_stylebox_override("normal", _box(STONE_DARK, STONE_EDGE.darkened(0.3), 2, 4))
		b.add_theme_stylebox_override("hover", _box(STONE_DARK.lightened(0.12), STONE_EDGE, 2, 4))
		b.add_theme_stylebox_override("pressed", _box(Color(0.42, 0.18, 0.12), ORANGE, 2, 4))
		b.add_theme_stylebox_override("hover_pressed", _box(Color(0.48, 0.22, 0.14), ORANGE, 2, 4))
		b.add_theme_stylebox_override("focus", StyleBoxEmpty.new())
		b.pressed.connect(func() -> void:
			Sfx.ui(self, "page")
			show_tab(i))
		tabs.add_child(b)
		_tab_btns.append(b)
	_title = _label("", 15, ORANGE)
	_title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	v.add_child(_title)
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	v.add_child(scroll)
	_content = VBoxContainer.new()
	_content.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_content.add_theme_constant_override("separation", int(4 * _k))
	scroll.add_child(_content)
	_tip = PanelContainer.new()
	_tip.add_theme_stylebox_override("panel", _box(Color(0.08, 0.07, 0.05, 0.96), STONE_EDGE, 1, 3))
	_tip.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_tip.visible = false
	_tip.z_index = 20
	add_child(_tip)
	_tip_text = RichTextLabel.new()
	_tip_text.bbcode_enabled = true
	_tip_text.fit_content = true
	_tip_text.scroll_active = false
	_tip_text.custom_minimum_size = Vector2(300, 10)
	_tip_text.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_tip_text.add_theme_font_size_override("normal_font_size", 13)
	_tip_text.add_theme_font_size_override("bold_font_size", 14)
	_tip.add_child(_tip_text)
	_menu = PopupMenu.new()
	_menu.id_pressed.connect(_on_menu)
	add_child(_menu)


# ---- open / close ----------------------------------------------------------------------------

func _input(event: InputEvent) -> void:
	if status != null and status.visible:
		return
	if event.is_action_pressed("menu"):
		toggle()
		get_viewport().set_input_as_handled()
	elif visible and event.is_action_pressed("free_mouse"):
		close()
		get_viewport().set_input_as_handled()


func _unhandled_input(event: InputEvent) -> void:
	# a click on the world (not the panel): back to the fight
	if visible and event is InputEventMouseButton and event.pressed and not _menu.visible:
		close()


func toggle() -> void:
	if visible:
		close()
	else:
		open()


func open(which: int = -1) -> void:
	_fit()
	if not visible:
		Sfx.ui(self, "open")
	visible = true
	Intent.menu_focus = true
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	show_tab(tab if which < 0 else which)


func close() -> void:
	if visible:
		Sfx.ui(self, "close")
	visible = false
	Intent.menu_focus = false
	_tip.visible = false
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED


func _process(delta: float) -> void:
	if not visible:
		return
	if _tip.visible:
		var m := get_viewport().get_mouse_position() + Vector2(16, 16)
		var vs := get_viewport_rect().size
		_tip.position = Vector2(clampf(m.x, 4.0, vs.x - _tip.size.x - 4.0), clampf(m.y, 4.0, vs.y - _tip.size.y - 4.0))
		if m.x + _tip.size.x > vs.x - 4.0:  # flip to the left of the cursor near the right edge
			_tip.position.x = maxf(4.0, m.x - 32.0 - _tip.size.x)
	# rebuild when what it shows has changed (bag, gear, buffs, cooldowns)
	_sig_t -= delta
	if _sig_t <= 0.0:
		_sig_t = 0.2
		var sg := _signature()
		if sg != _sig:
			show_tab(tab)


func _signature() -> String:
	var s := player.sheet
	var parts := [tab, str(s.bag), str(s.equipment), s.gold, s.effects.size()]
	if player.quests != null:
		parts.append(str(player.quests.active) + str(player.quests.kills) + str(player.sheet.xp))
	if tab == 1:
		for st in STAT_GRID:
			parts.append(_stat_value(st))
	if tab == 4:
		for sid in s.cooldowns:
			parts.append(int(maxf(0.0, float(s.cooldowns[sid]) - s.clock)))
	if tab == 0:
		parts.append(player.combat.stance_id)
	return str(parts)


# ---- tabs ------------------------------------------------------------------------------------

func show_tab(i: int) -> void:
	tab = i
	_sig = _signature()
	for k in _tab_btns.size():
		_tab_btns[k].button_pressed = k == i
	for c in _content.get_children():
		_content.remove_child(c)
		c.queue_free()
	_title.text = TAB_NAMES[i]
	match TABS[i]:
		"combat":
			_combat()
		"stats":
			_stats()
		"inventory":
			_inventory()
		"worn":
			_worn()
		"magic":
			_magic()
		"card":
			_card()


func _inventory() -> void:
	var s := player.sheet
	var top := HBoxContainer.new()
	var coin := TextureRect.new()
	coin.texture = Icons.tex(Icons.UI["coins"])
	coin.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	coin.custom_minimum_size = Vector2(20, 20) * _k
	top.add_child(coin)
	top.add_child(_label("%s gold    %d / %d cells    %.1f kg" % [_rs_count(s.gold), s.grid.cells_used(), s.grid.w * s.grid.h, s.carried_weight()], int(12 * _k), YELLOW, false))
	_content.add_child(top)
	var view := BagView.new()
	view.name = "BagView"
	view.setup(s.grid, 36.0 * _k)
	view.hovered.connect(func(id: String) -> void: _show_tip(_item_tip(id)))
	view.unhovered.connect(_hide_tip)
	view.clicked.connect(_use)
	view.menu.connect(_open_menu)
	view.changed.connect(func() -> void: Sfx.ui(self, "equip"))
	_content.add_child(view)
	_content.add_child(_label("Drag to move (R turns it).  Left click: wear / eat.  Right click: more.", 11, TEXT.darkened(0.3)))


func _card() -> void:
	var q := player.quests
	if q == null:
		_content.add_child(_label("Not registered with the guild.", 12, TEXT))
		return
	var s := player.sheet
	_content.add_child(_label("%s   -   Rank %s" % [s.display_name, q.rank()], int(18 * _k), YELLOW, false))
	_content.add_child(_label(Rank.title(s, player.combat), int(14 * _k), TEXT, false))
	var nr := q.next_rank()
	_content.add_child(_label(("%d / %d XP to %s   (rank power x%s)" % [s.xp, nr[1], nr[2], Stats._g(Rank.power(s.rank_step()))]) if not nr.is_empty() else "%d XP (top rank)" % s.xp, int(12 * _k), TEXT))
	var nu := Rank.next_unlock(s.rank_step())
	if not nu.is_empty():
		_content.add_child(_label("Next: %s opens %s" % [nu[0], nu[1]], int(11 * _k), TEXT.darkened(0.2)))
	_content.add_child(_label("Quests", int(14 * _k), ORANGE, false))
	if q.active.is_empty():
		_content.add_child(_label("None - the Quest Board is on the plaza.", 11, TEXT.darkened(0.3)))
	for id: String in q.active:
		var lines := str(q.quest(id)["name"]) + (" - DONE, turn it in" if q.complete(id) else "")
		for row in q.progress(id):
			lines += "\n   %s  %d / %d" % [row[0], int(row[1]), int(row[2])]
		_content.add_child(_label(lines, int(12 * _k), GOOD if q.complete(id) else TEXT))
	_content.add_child(_label("Monsters slain", int(14 * _k), ORANGE, false))
	if q.kills.is_empty():
		_content.add_child(_label("None yet.", 11, TEXT.darkened(0.3)))
	for kind: String in q.kills:
		_content.add_child(_label("%s  x%d" % [str(kind).capitalize(), int(q.kills[kind])], int(12 * _k), TEXT))
	var done := 0
	for id in q.finished:
		done += int(q.finished[id])
	_content.add_child(_label("Quests completed: %d" % done, int(12 * _k), TEXT))


func _worn() -> void:
	var s := player.sheet
	var grid := GridContainer.new()
	grid.columns = 3
	grid.add_theme_constant_override("h_separation", int(14 * _k))
	grid.add_theme_constant_override("v_separation", int(6 * _k))
	var cc := CenterContainer.new()
	cc.add_child(grid)
	_content.add_child(cc)
	for row in DOLL:
		for slot: String in row:
			if slot == "":
				var gap := Control.new()
				gap.custom_minimum_size = Vector2(52, 46) * _k
				grid.add_child(gap)
				continue
			var id: String = s.equipment.get(slot, "")
			var b: Button
			if id == "":
				b = _slot(Icons.tex(Icons.SLOT[slot]), "", true)
				b.modulate = Color(1, 1, 1, 0.28)
				b.mouse_entered.connect(func() -> void: _show_tip("[color=#c8b48c]%s slot: empty[/color]" % slot.capitalize().replace("1", " (neck)").replace("2", " (ring)")))
			else:
				b = _slot(Icons.item(id), "", true)
				b.mouse_entered.connect(func() -> void: _show_tip(status.item_tip(id, false) + "\n[color=#c8b48c]Click to take off[/color]"))
				b.pressed.connect(func() -> void: _take_off(slot))
			grid.add_child(b)
	# what all of it adds up to
	_content.add_child(_label("Equipment bonuses", 13, ORANGE))
	var totals := {}
	for slot in s.equipment:
		for m in s.item_data(s.equipment[slot]).get("mods", []):
			var key := "%s|%s" % [m["stat"], m["op"]]
			totals[key] = float(totals.get(key, 0.0 if m["op"] != "more" else 1.0)) + (float(m["value"]) if m["op"] != "more" else 0.0)
			if m["op"] == "more":
				totals[key] = float(totals[key]) * float(m["value"])
	var lines := ""
	for key: String in totals:
		var parts := key.split("|")
		lines += CharModel.mod_text({"stat": parts[0], "op": parts[1], "value": totals[key]}) + "\n"
	var w := s._weapon()
	if not w.is_empty():
		lines = "Weapon: %d damage, %.2f attacks/s\n" % [int(w.get("base_damage", 0)), float(w.get("aps", 0.0))] + lines
	var rt := _rich(12)
	rt.text = lines if lines != "" else "[color=#9a8a6a]Nothing worn yet: equip from the Inventory tab.[/color]"
	_content.add_child(rt)


func _stats() -> void:
	var s := player.sheet
	var grid := GridContainer.new()
	grid.columns = 3
	grid.add_theme_constant_override("h_separation", int(3 * _k))
	grid.add_theme_constant_override("v_separation", int(3 * _k))
	_content.add_child(grid)
	for st: String in STAT_GRID:
		var b := Button.new()
		b.custom_minimum_size = Vector2(80, 38) * _k
		b.icon = Icons.stat(st)
		b.expand_icon = false
		b.text = _stat_value(st)
		b.alignment = HORIZONTAL_ALIGNMENT_RIGHT
		b.add_theme_font_size_override("font_size", int(14 * _k))
		b.add_theme_color_override("font_color", YELLOW)
		b.add_theme_color_override("font_hover_color", YELLOW)
		b.add_theme_stylebox_override("normal", _box(SLOT_BG, STONE_EDGE.darkened(0.45), 1, 3))
		b.add_theme_stylebox_override("hover", _box(SLOT_BG.lightened(0.1), STONE_EDGE, 1, 3))
		b.add_theme_stylebox_override("pressed", _box(SLOT_BG.lightened(0.1), STONE_EDGE, 1, 3))
		b.add_theme_stylebox_override("focus", StyleBoxEmpty.new())
		b.mouse_entered.connect(func() -> void: _show_tip(_stat_tip(st)))
		b.mouse_exited.connect(_hide_tip)
		grid.add_child(b)
	var r: Dictionary = DataStore.races().get(s.race, {})
	var foot := _rich(12)
	foot.text = "[color=#ff9a1a]Level %d %s[/color]   HP %d/%d   %d gold\n[color=#9a8a6a]Hover a stat for its full breakdown. P: full character sheet[/color]" % [
		s.level, r.get("name", s.race), s.hp, s.v("max_hp"), s.gold]
	_content.add_child(foot)
	var full := _text_button("Character sheet (P)")
	full.pressed.connect(func() -> void:
		close()
		status.open())
	_content.add_child(full)


func _combat() -> void:
	var c := player.combat
	var st: Dictionary = c.stance
	var s := player.sheet
	var head := _rich(13)
	head.text = "[color=#ffd95a]%s[/color]
attack %d   speed %.2f/s   power %.2f" % [str(st.get("name", "")), int(s.v("attack_damage")),
		s.v("attack_speed"), s.v("power")]
	_content.add_child(head)
	var lights: Array = st.get("light", [])
	var combo := ""
	for k in lights.size():
		var m: Dictionary = lights[k]
		combo += "%d. x%.2f%s
" % [k + 1, float(m.get("dmg", 1.0)), "  (" + str(m["spell"]) + ")" if m.has("spell") else ""]
	_box_text("Combo  (LMB, LMB...)", combo.strip_edges())
	var hv: Dictionary = st.get("heavy", {})
	var sp: Dictionary = st.get("special", {})
	_box_text("Heavy (hold LMB) / Special (E)", "heavy x%.2f, breaks poise %d
special x%.2f%s" % [float(hv.get("dmg", 0.0)), int(hv.get("poise", 0)),
		float(sp.get("dmg", 0.0)), "  - breaks guards" if bool(sp.get("guard_break", false)) else ""])
	if c.ranged():
		_box_text("Aim  (hold RMB)", "LMB fires / casts at the crosshair
(or straight at a locked target)")
	else:
		_box_text("Guard  (hold RMB)", "blocks %d%% from the front
tap it as a blow lands: PARRY" % int(float(st.get("block", 0.6)) * 100.0))
	_box_text("Moving", "Alt: roll / dodge step (invulnerable)
Q: next weapon style")


func _magic() -> void:
	var s := player.sheet
	var sp := DataStore.spells()
	var grid := GridContainer.new()
	grid.columns = 4
	grid.add_theme_constant_override("h_separation", int(4 * _k))
	grid.add_theme_constant_override("v_separation", int(4 * _k))
	_content.add_child(grid)
	for sid: String in sp:
		var d: Dictionary = sp[sid]
		var key := ""
		var pi := Player.PAGE1.find(sid)
		if pi >= 0:
			key = str(pi + 1)
		elif Player.PAGE2.find(sid) >= 0:
			key = "^%d" % (Player.PAGE2.find(sid) + 1)
		var cd := float(s.cooldowns.get(sid, 0.0)) - s.clock
		var b := _slot(Icons.spell(sid), key, false)
		var active := s.find_effect(sid) != null
		if active:
			b.add_theme_stylebox_override("normal", _box(Color(0.35, 0.28, 0.08, 0.95), YELLOW, 2, 3))
		if cd > 0.0:
			b.modulate = Color(0.5, 0.5, 0.5)
		b.mouse_entered.connect(func() -> void: _show_tip(StatusWindow.spell_tip(d) + ("\n[color=#ffff00]ACTIVE: click to dismiss[/color]" if active else "\n[color=#c8b48c]Click to cast[/color]")))
		b.pressed.connect(func() -> void:
			player.cast_spell(sid)
			show_tab(tab))
		grid.add_child(b)
	_content.add_child(_label("Number = hotbar key (^ = Ctrl). Attunement %d / %d" % [s.slots_used(), s.v("attunement_slots")], 11, TEXT.darkened(0.3)))


# ---- actions ---------------------------------------------------------------------------------

func _use(id: String) -> void:
	Sfx.ui(self, "consume" if CharModel.is_consumable(id) else "equip")
	status.use_item(id)
	player.model.refresh_armour(player.sheet)  # the body wears what was just put on
	_hide_tip()
	show_tab(tab)


func _take_off(slot: String) -> void:
	var s := player.sheet
	var id: String = s.equipment.get(slot, "")
	if id != "" and s.unequip(slot):
		Sfx.ui(self, "equip")
		player.toast.emit("Took off " + s.item_data(id).get("name", id))
		player.model.refresh_armour(s)
	elif id != "":
		player.toast.emit("No room in the backpack for " + str(s.item_data(id).get("name", id)))
	_hide_tip()
	show_tab(tab)


func _open_menu(id: String) -> void:
	_menu_id = id
	_menu.clear()
	_menu.add_item(("Eat / drink " if CharModel.is_consumable(id) else "Wear ") + str(player.sheet.item_data(id).get("name", id)), 0)
	_menu.add_item("Drop", 1)
	_menu.add_item("Examine", 2)
	_menu.add_item("Cancel", 3)
	_menu.position = Vector2i(get_viewport().get_mouse_position())
	_menu.popup()
	_hide_tip()


func _on_menu(i: int) -> void:
	var id := _menu_id
	match i:
		0:
			_use(id)
		1:
			if player.sheet.remove_item(id):
				var pk := ItemPickup.make(id)
				player.get_parent().add_child(pk)
				pk.global_position = player.global_position + player.facing * 0.9
				player.toast.emit("Dropped " + str(player.sheet.item_data(id).get("name", id)))
				Sfx.ui(self, "drop")
				show_tab(tab)
		2:
			var d := player.sheet.item_data(id)
			player.toast.emit("%s: %s" % [d.get("name", id), d.get("desc", "tier %d %s" % [int(d.get("tier", 1)), d.get("slot", "item")])])


# ---- tooltips --------------------------------------------------------------------------------

func _item_tip(id: String) -> String:
	return status.item_tip(id, true) + "\n[color=#c8b48c]%s   (right click: more)[/color]" % ["Click to use" if CharModel.is_consumable(id) else "Click to wear"]


func _stat_value(st: String) -> String:
	var s := player.sheet
	if st in ["str", "agi", "int"]:
		return str(roundi(float(s.attribute(st)["value"])))
	return CharModel.fmt_stat(st, s.v(st))


func _stat_tip(st: String) -> String:
	var s := player.sheet
	if st in ["str", "agi", "int"]:
		var help := {"str": "Strength: attack damage, Power (how hard blows land), health, carry.",
			"agi": "Agility: attack speed, move speed, light-weapon damage.", "int": "Intellect: mana, spell power, perception."}
		return "[b][color=#ffd95a]%s %s[/color][/b]\n%s" % [st.to_upper(), _stat_value(st), help[st]]
	var t := "[b][color=#ffd95a]%s  %s[/color][/b]\n" % [CharModel.STAT_NAMES.get(st, st), _stat_value(st)]
	t += "[color=#c8b48c]%s[/color]\n" % StatusWindow.STAT_HELP.get(st, "")
	var ex := s.explain(st).split("\n")
	for k in range(1, ex.size()):
		t += "  " + ex[k] + "\n"
	return t


func _show_tip(t: String) -> void:
	_tip_text.text = t
	_tip.reset_size()
	_tip.visible = true


func _hide_tip() -> void:
	_tip.visible = false


# ---- widgets ---------------------------------------------------------------------------------

func _slot(icon: Texture2D, count: String, framed: bool) -> Button:
	var b := Button.new()
	b.custom_minimum_size = Vector2(52, 46) * _k
	b.icon = icon
	b.expand_icon = true
	b.icon_alignment = HORIZONTAL_ALIGNMENT_CENTER
	b.add_theme_constant_override("icon_max_width", int(40 * _k))
	var bg := SLOT_BG if framed else Color(0, 0, 0, 0.0)
	var edge := STONE_EDGE.darkened(0.4) if framed else Color(0, 0, 0, 0.0)
	b.add_theme_stylebox_override("normal", _box(bg, edge, 1, 3))
	b.add_theme_stylebox_override("hover", _box(Color(1, 1, 1, 0.08), STONE_EDGE, 1, 3))
	b.add_theme_stylebox_override("pressed", _box(Color(1, 1, 1, 0.15), ORANGE, 1, 3))
	b.add_theme_stylebox_override("focus", StyleBoxEmpty.new())
	b.add_theme_stylebox_override("disabled", _box(bg, edge, 1, 3))
	b.mouse_exited.connect(_hide_tip)
	if count != "":
		var l := _label(count, int(12 * _k), YELLOW, false)
		l.position = Vector2(2, 0)
		l.mouse_filter = Control.MOUSE_FILTER_IGNORE
		b.add_child(l)
	return b


func _box_text(title: String, body: String) -> void:
	var p := PanelContainer.new()
	p.add_theme_stylebox_override("panel", _box(SLOT_BG, STONE_EDGE.darkened(0.4), 1, 3))
	var rt := _rich(12)
	rt.text = "[color=#ff9a1a]%s[/color]\n%s" % [title, body]
	p.add_child(rt)
	_content.add_child(p)


func _text_button(t: String) -> Button:
	var b := Button.new()
	b.text = t
	b.add_theme_font_size_override("font_size", int(13 * _k))
	b.add_theme_color_override("font_color", ORANGE)
	b.add_theme_stylebox_override("normal", _box(STONE_DARK, STONE_EDGE, 1, 3))
	b.add_theme_stylebox_override("hover", _box(STONE_DARK.lightened(0.1), ORANGE, 1, 3))
	b.add_theme_stylebox_override("pressed", _box(STONE_DARK.lightened(0.1), ORANGE, 1, 3))
	b.add_theme_stylebox_override("focus", StyleBoxEmpty.new())
	return b


func _label(t: String, font_size: int, col: Color, wrapped := true) -> Label:
	var l := Label.new()
	l.text = t
	l.add_theme_font_size_override("font_size", font_size)
	l.add_theme_color_override("font_color", col)
	l.add_theme_color_override("font_shadow_color", Color(0, 0, 0, 0.9))
	l.add_theme_constant_override("shadow_offset_x", 1)
	l.add_theme_constant_override("shadow_offset_y", 1)
	if wrapped:
		l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	return l


func _rich(font_size: int) -> RichTextLabel:
	var r := RichTextLabel.new()
	r.bbcode_enabled = true
	r.fit_content = true
	r.scroll_active = false
	r.mouse_filter = Control.MOUSE_FILTER_IGNORE
	r.add_theme_font_size_override("normal_font_size", int(font_size * _k))
	r.add_theme_font_size_override("bold_font_size", int((font_size + 1) * _k))
	r.add_theme_color_override("default_color", TEXT)
	return r


static func _box(bg: Color, edge: Color, w: int, radius: int) -> StyleBoxFlat:
	var sb := StyleBoxFlat.new()
	sb.bg_color = bg
	sb.border_color = edge
	sb.set_border_width_all(w)
	sb.set_corner_radius_all(radius)
	sb.set_content_margin_all(4)
	return sb


## RuneScape stack numbers: 9999, 100K, 10M
static func _rs_count(n: int) -> String:
	if n >= 10000000:
		return "%dM" % floori(n / 1000000.0)
	if n >= 100000:
		return "%dK" % floori(n / 1000.0)
	return str(n)


## Cover the window (a top-level Control under a CanvasLayer can come up 0 x 0 and then lays its
## children out from the corner, off screen).
func _fit() -> void:
	position = Vector2.ZERO
	size = get_viewport_rect().size
