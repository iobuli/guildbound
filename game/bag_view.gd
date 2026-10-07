class_name BagView
extends Control
## Draws a GridBag and lets you rearrange it: drag an item to move it (R turns it while dragging; a
## stack dropped on the same item merges), left click uses it, right click opens the item menu.
## The drop target lights green where the item fits and red where it does not.

signal hovered(id: String)
signal unhovered
signal clicked(id: String)
signal menu(id: String)
signal changed

const GRID_LINE := Color(0.47, 0.40, 0.29, 0.55)
const CELL_BG := Color(0.12, 0.11, 0.09, 0.9)
const ITEM_BG := Color(0.26, 0.23, 0.17, 0.95)
const OK_TINT := Color(0.3, 0.9, 0.35, 0.35)
const BAD_TINT := Color(0.95, 0.25, 0.2, 0.35)

var bag: GridBag
var cell := 36.0
var _drop := Rect2i()          # the cells the dragged item would take (empty: not dragging over us)
var _drop_ok := false
var _drag: Dictionary = {}      # the drag in progress (shared with the viewport's drag data)
var _press := Vector2(-1, -1)
var _hover_id := ""


func setup(p_bag: GridBag, p_cell: float) -> void:
	bag = p_bag
	cell = p_cell
	custom_minimum_size = Vector2(bag.w, bag.h) * cell
	mouse_filter = Control.MOUSE_FILTER_STOP
	refresh()


func refresh() -> void:
	for c in get_children():
		c.queue_free()
	for e in bag.entries:
		var f := GridBag.footprint(str(e["id"]), bool(e["rot"]))
		var t := TextureRect.new()
		t.texture = Icons.item(str(e["id"]))
		t.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		t.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
		t.position = Vector2(int(e["x"]), int(e["y"])) * cell + Vector2(3, 3)
		t.size = Vector2(f) * cell - Vector2(6, 6)
		t.mouse_filter = Control.MOUSE_FILTER_IGNORE
		add_child(t)
		if int(e["n"]) > 1:
			var l := Label.new()
			l.text = str(e["n"])
			l.add_theme_color_override("font_color", Color(1, 1, 0))
			l.add_theme_font_size_override("font_size", int(cell * 0.34))
			l.position = Vector2(int(e["x"]), int(e["y"])) * cell + Vector2(3, 0)
			l.mouse_filter = Control.MOUSE_FILTER_IGNORE
			add_child(l)
	queue_redraw()


func _cell_at(pos: Vector2) -> Vector2i:
	return Vector2i(floori(pos.x / cell), floori(pos.y / cell))


func _draw() -> void:
	for y in bag.h:
		for x in bag.w:
			draw_rect(Rect2(Vector2(x, y) * cell, Vector2(cell, cell)), CELL_BG)
	for e in bag.entries:
		var f := GridBag.footprint(str(e["id"]), bool(e["rot"]))
		draw_rect(Rect2(Vector2(int(e["x"]), int(e["y"])) * cell + Vector2(1, 1), Vector2(f) * cell - Vector2(2, 2)), ITEM_BG)
	for x in bag.w + 1:
		draw_line(Vector2(x * cell, 0), Vector2(x * cell, bag.h * cell), GRID_LINE)
	for y in bag.h + 1:
		draw_line(Vector2(0, y * cell), Vector2(bag.w * cell, y * cell), GRID_LINE)
	if _drop.size != Vector2i.ZERO:
		draw_rect(Rect2(Vector2(_drop.position) * cell, Vector2(_drop.size) * cell), OK_TINT if _drop_ok else BAD_TINT)


func _gui_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion:
		var e: Variant = bag.at(_cell_at(event.position).x, _cell_at(event.position).y)
		var id: String = str(e["id"]) if e != null else ""
		if id != _hover_id:
			_hover_id = id
			if id != "":
				hovered.emit(id)
			else:
				unhovered.emit()
	elif event is InputEventMouseButton and event.pressed:
		var e: Variant = bag.at(_cell_at(event.position).x, _cell_at(event.position).y)
		if event.button_index == MOUSE_BUTTON_LEFT:
			_press = event.position
		elif event.button_index == MOUSE_BUTTON_RIGHT and e != null:
			menu.emit(str(e["id"]))
	elif event is InputEventMouseButton and not event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		if _press.x >= 0.0 and event.position.distance_to(_press) < 6.0:
			var e: Variant = bag.at(_cell_at(event.position).x, _cell_at(event.position).y)
			if e != null:
				clicked.emit(str(e["id"]))
		_press = Vector2(-1, -1)


func _notification(what: int) -> void:
	if what == NOTIFICATION_MOUSE_EXIT and _hover_id != "":
		_hover_id = ""
		unhovered.emit()
	elif what == NOTIFICATION_DRAG_END:
		_drag = {}
		_drop = Rect2i()
		queue_redraw()


func _get_drag_data(at_position: Vector2) -> Variant:
	var c := _cell_at(at_position)
	var e: Variant = bag.at(c.x, c.y)
	if e == null:
		return null
	_press = Vector2(-1, -1)
	_drag = {"bag_entry": e, "rot": bool(e["rot"]), "grab": c - Vector2i(int(e["x"]), int(e["y"]))}
	_preview()
	unhovered.emit()
	return _drag


func _preview() -> void:
	var e: Dictionary = _drag["bag_entry"]
	var f := GridBag.footprint(str(e["id"]), bool(_drag["rot"]))
	var holder := Control.new()
	var t := TextureRect.new()
	t.texture = Icons.item(str(e["id"]))
	t.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	t.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	t.size = Vector2(f) * cell
	t.position = -(Vector2(_drag["grab"]) + Vector2(0.5, 0.5)) * cell
	t.modulate = Color(1, 1, 1, 0.8)
	holder.add_child(t)
	set_drag_preview(holder)


## R while dragging turns the item (its footprint swaps w and h).
func _input(event: InputEvent) -> void:
	if _drag.is_empty() or not (event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_R):
		return
	_drag["rot"] = not bool(_drag["rot"])
	_drag["grab"] = Vector2i.ZERO
	_preview()
	get_viewport().set_input_as_handled()


func _target(at_position: Vector2, data: Dictionary) -> Vector2i:
	return _cell_at(at_position) - Vector2i(data["grab"])


func _can_drop_data(at_position: Vector2, data: Variant) -> bool:
	if not (data is Dictionary and (data as Dictionary).has("bag_entry")):
		return false
	var e: Dictionary = data["bag_entry"]
	var at := _target(at_position, data)
	var f := GridBag.footprint(str(e["id"]), bool(data["rot"]))
	var other: Variant = bag.at(_cell_at(at_position).x, _cell_at(at_position).y)
	var merge: bool = other != null and other != e and other["id"] == e["id"] and GridBag.max_stack(str(e["id"])) > 1
	_drop = Rect2i(at, f)
	_drop_ok = merge or bag.fits(str(e["id"]), at.x, at.y, bool(data["rot"]), e)
	queue_redraw()
	return _drop_ok


func _drop_data(at_position: Vector2, data: Variant) -> void:
	var e: Dictionary = data["bag_entry"]
	var other: Variant = bag.at(_cell_at(at_position).x, _cell_at(at_position).y)
	var ok: bool
	if other != null and other != e and other["id"] == e["id"]:
		ok = bag.move(e, int(other["x"]), int(other["y"]), bool(other["rot"]))
	else:
		var at := _target(at_position, data)
		ok = bag.move(e, at.x, at.y, bool(data["rot"]))
	_drop = Rect2i()
	if ok:
		refresh()
		changed.emit()
