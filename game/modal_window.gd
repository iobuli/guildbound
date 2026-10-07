class_name ModalWindow
extends Control
## A centred window over the dimmed, paused game (the quest board, the shop). Subclasses set `title`
## in _init and build their rows in refresh(); E, Esc or Close shut it. Sized by hand when it opens:
## anchors are not resolved for a Control straight under a CanvasLayer here (the character sheet
## sizes itself the same way).

const STONE := Color(0.24, 0.21, 0.17, 0.97)
const STONE_DARK := Color(0.15, 0.13, 0.10, 0.98)
const EDGE := Color(0.47, 0.40, 0.29)
const GOLD := Color(1.0, 0.85, 0.35)
const TEXT := Color(0.95, 0.88, 0.7)

var player: Player
var title := ""
var win_size := Vector2(620, 520)
var _list: VBoxContainer      # the scrolling body
var _head: Label              # one line under the title
var _foot: VBoxContainer      # under the list, above Close
var _title_label: Label
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
	_panel.custom_minimum_size = win_size
	add_child(_panel)
	var v := VBoxContainer.new()
	v.add_theme_constant_override("separation", 8)
	_panel.add_child(v)
	_title_label = Label.new()
	_title_label.text = title
	_title_label.add_theme_color_override("font_color", GOLD)
	_title_label.add_theme_font_size_override("font_size", 22)
	_title_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	v.add_child(_title_label)
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
	_foot = VBoxContainer.new()
	v.add_child(_foot)
	var close := Button.new()
	close.text = "Close (E / Esc)"
	close.pressed.connect(close_window)
	v.add_child(close)


func _fit() -> void:
	position = Vector2.ZERO
	size = get_viewport_rect().size
	_dim.position = Vector2.ZERO
	_dim.size = size
	_panel.size = _panel.custom_minimum_size
	_panel.position = ((size - _panel.size) * 0.5).floor()


func open_window() -> void:
	_fit()
	visible = true
	get_tree().paused = true
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	refresh()


func close_window() -> void:
	visible = false
	get_tree().paused = false
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED


func _unhandled_input(event: InputEvent) -> void:
	if visible and (event.is_action_pressed("ui_cancel") or event.is_action_pressed("interact")):
		close_window()
		get_viewport().set_input_as_handled()


## Rebuild the rows (subclasses).
func refresh() -> void:
	pass


func clear_list() -> void:
	for c in _list.get_children():
		c.queue_free()


func section(t: String) -> void:
	var l := Label.new()
	l.text = t
	l.add_theme_color_override("font_color", GOLD)
	l.add_theme_font_size_override("font_size", 17)
	_list.add_child(l)


func note(t: String) -> void:
	var l := Label.new()
	l.text = t
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	l.add_theme_color_override("font_color", TEXT.darkened(0.3))
	_list.add_child(l)


## A dark framed box in the list; returns its inner column.
func card() -> VBoxContainer:
	var box := PanelContainer.new()
	box.add_theme_stylebox_override("panel", SidePanel._box(STONE_DARK, EDGE.darkened(0.3), 1, 5))
	_list.add_child(box)
	var v := VBoxContainer.new()
	box.add_child(v)
	return v


func rich(parent: Node, txt: String) -> RichTextLabel:
	var t := RichTextLabel.new()
	t.bbcode_enabled = true
	t.fit_content = true
	t.scroll_active = false
	t.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	t.text = txt
	parent.add_child(t)
	return t


## Show what an action said (toast + sound) and redraw.
func act(r: Array) -> void:
	player.toast.emit(str(r[1]))
	Sfx.ui(self, "equip" if bool(r[0]) else "drop")
	refresh()
