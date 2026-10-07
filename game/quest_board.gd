class_name QuestBoard
extends Control
## The guild's quest board (E at the board on the plaza): your rank, the quests you carry with their
## progress (turn in / abandon), and the quests on offer for your rank (accept). The game pauses
## while it is open; E, Esc or Close shut it.

const STONE := Color(0.24, 0.21, 0.17, 0.97)
const STONE_DARK := Color(0.15, 0.13, 0.10, 0.98)
const EDGE := Color(0.47, 0.40, 0.29)
const GOLD := Color(1.0, 0.85, 0.35)
const TEXT := Color(0.95, 0.88, 0.7)
const GOOD := Color(0.55, 0.95, 0.5)

var player: Player
var _list: VBoxContainer
var _head: Label
var _panel: PanelContainer
var _dim: ColorRect


func _ready() -> void:
	process_mode = Node.PROCESS_MODE_ALWAYS
	visible = false
	mouse_filter = Control.MOUSE_FILTER_STOP
	_dim = ColorRect.new()
	_dim.color = Color(0, 0, 0, 0.45)
	add_child(_dim)
	_panel = PanelContainer.new()
	_panel.add_theme_stylebox_override("panel", SidePanel._box(STONE, EDGE, 3, 8))
	_panel.custom_minimum_size = Vector2(620, 520)
	add_child(_panel)
	var v := VBoxContainer.new()
	v.add_theme_constant_override("separation", 8)
	_panel.add_child(v)
	var title := Label.new()
	title.text = "Adventurers' Guild - Quest Board"
	title.add_theme_color_override("font_color", GOLD)
	title.add_theme_font_size_override("font_size", 22)
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	v.add_child(title)
	_head = Label.new()
	_head.add_theme_color_override("font_color", TEXT)
	_head.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	v.add_child(_head)
	var scroll := ScrollContainer.new()
	scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	v.add_child(scroll)
	_list = VBoxContainer.new()
	_list.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_list.add_theme_constant_override("separation", 6)
	scroll.add_child(_list)
	var close := Button.new()
	close.text = "Close (E / Esc)"
	close.pressed.connect(close_board)
	v.add_child(close)


## Fill the screen and centre the window (anchors are not resolved under a CanvasLayer here; the
## other windows size themselves the same way).
func _fit() -> void:
	position = Vector2.ZERO
	size = get_viewport_rect().size
	_dim.position = Vector2.ZERO
	_dim.size = size
	_panel.size = _panel.custom_minimum_size
	_panel.position = ((size - _panel.size) * 0.5).floor()


func open_board() -> void:
	_fit()
	visible = true
	get_tree().paused = true
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	refresh()


func close_board() -> void:
	visible = false
	get_tree().paused = false
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED


func _unhandled_input(event: InputEvent) -> void:
	if visible and (event.is_action_pressed("ui_cancel") or event.is_action_pressed("interact")):
		close_board()
		get_viewport().set_input_as_handled()


func refresh() -> void:
	var q := player.quests
	for c in _list.get_children():
		c.queue_free()
	var nr := q.next_rank()
	_head.text = "Rank %s   -   %d rank points%s   -   %d gold" % [q.rank(), q.rank_points,
		("   (%s at %d)" % [nr[2], nr[1]]) if not nr.is_empty() else "   (top rank)", player.sheet.gold]
	_section("Your quests  (%d / %d)" % [q.active.size(), int(q.data.get("max_active", 3))])
	if q.active.is_empty():
		_note("None yet - take one from the board below.")
	for id: String in q.active:
		var row := _quest_card(id)
		var btns := HBoxContainer.new()
		var done := q.complete(id)
		var turn := Button.new()
		turn.text = "Turn in" if done else "Not finished"
		turn.disabled = not done
		turn.pressed.connect(func() -> void: _do(q.turn_in(id)))
		btns.add_child(turn)
		var drop := Button.new()
		drop.text = "Abandon"
		drop.pressed.connect(func() -> void:
			q.abandon(id)
			refresh())
		btns.add_child(drop)
		row.add_child(btns)
	_section("On the board")
	var offer := q.available()
	if offer.is_empty():
		_note("Nothing new for your rank right now.")
	for id: String in offer:
		var row := _quest_card(id)
		var take := Button.new()
		take.text = "Accept"
		take.pressed.connect(func() -> void: _do(q.accept(id)))
		row.add_child(take)


func _do(r: Array) -> void:
	player.toast.emit(str(r[1]))
	Sfx.ui(self, "equip" if bool(r[0]) else "drop")
	refresh()


func _section(t: String) -> void:
	var l := Label.new()
	l.text = t
	l.add_theme_color_override("font_color", GOLD)
	l.add_theme_font_size_override("font_size", 17)
	_list.add_child(l)


func _note(t: String) -> void:
	var l := Label.new()
	l.text = t
	l.add_theme_color_override("font_color", TEXT.darkened(0.3))
	_list.add_child(l)


func _quest_card(id: String) -> VBoxContainer:
	var q := player.quests
	var d := q.quest(id)
	var box := PanelContainer.new()
	box.add_theme_stylebox_override("panel", SidePanel._box(STONE_DARK, EDGE.darkened(0.3), 1, 5))
	_list.add_child(box)
	var v := VBoxContainer.new()
	box.add_child(v)
	var t := RichTextLabel.new()
	t.bbcode_enabled = true
	t.fit_content = true
	t.scroll_active = false
	t.custom_minimum_size = Vector2(560, 0)
	var txt := "[b][color=#ffd95a]%s[/color][/b]   [color=#c8b48c]rank %s - from %s%s[/color]\n%s\n" % [d["name"], d.get("rank", "F"),
		d.get("giver", "the guild"), "  (repeatable)" if bool(d.get("repeatable", false)) else "", d.get("desc", "")]
	if q.active.has(id):
		for row in q.progress(id):
			var ok := int(row[1]) >= int(row[2])
			txt += "  [color=%s]%s  %d / %d[/color]\n" % ["#8cf27f" if ok else "#f2e2b3", row[0], int(row[1]), int(row[2])]
	else:
		for o in d["objectives"]:
			var what: String = ("Defeat %d %s" % [int(o["count"]), str(o["target"])]) if o["type"] == "kill" else ("Bring %d %s" % [int(o["count"]), str(player.sheet.item_data(str(o["item"])).get("name", o["item"]))])
			txt += "  %s\n" % what
	var r: Dictionary = d.get("rewards", {})
	var items := ""
	for it in r.get("items", []):
		items += ", %s x%d" % [player.sheet.item_data(str(it[0])).get("name", it[0]), int(it[1])]
	txt += "[color=#ffd95a]Reward: %d gold, %d rank points%s[/color]" % [int(r.get("gold", 0)), int(r.get("rank_points", 0)), items]
	t.text = txt
	v.add_child(t)
	return v
