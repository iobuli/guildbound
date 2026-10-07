class_name ShopWindow
extends ModalWindow
## The merchant's stall (E at the counter): what the merchant sells (price, how many are left) and
## everything in your backpack with what the merchant pays for it. Hover a row for the item card.

var merchant: Merchant
var status: StatusWindow
var _info: RichTextLabel


func _init() -> void:
	win_size = Vector2(700, 580)


func _ready() -> void:
	super._ready()
	_info = RichTextLabel.new()
	_info.bbcode_enabled = true
	_info.scroll_active = false
	_info.custom_minimum_size = Vector2(0, 96)
	_foot.add_child(_info)


func open_shop() -> void:
	open_window()


func refresh() -> void:
	var s := player.sheet
	clear_list()
	_title_label.text = str(merchant.data.get("name", "Merchant"))
	_head.text = "You have %d gold    -    backpack %d / %d cells" % [s.gold, s.grid.cells_used(), s.grid.w * s.grid.h]
	section("For sale")
	for id: String in merchant.stock:
		var left := int(merchant.stock[id])
		var cost := merchant.price(id)
		var b := Button.new()
		b.text = "Buy  %d g" % cost
		b.disabled = left <= 0 or s.gold < cost
		b.pressed.connect(func() -> void: act(merchant.buy(s, id)))
		_row(id, "%s   [color=#c8b48c]%s[/color]" % [_name(id), ("%d left" % left) if left > 0 else "sold out"], [b])
	section("Sell from your backpack")
	if s.bag.is_empty():
		note("Your backpack is empty.")
	var ids: Array = s.bag.keys()
	ids.sort_custom(func(a: String, b: String) -> bool: return _name(a) < _name(b))
	for id: String in ids:
		var n := int(s.bag[id])
		var each := merchant.offer(id)
		var one := Button.new()
		one.text = "Sell 1  (%d g)" % each
		one.pressed.connect(func() -> void: act(merchant.sell(s, id, 1)))
		var buttons: Array = [one]
		if n > 1:
			var all := Button.new()
			all.text = "Sell all  (%d g)" % (each * n)
			all.pressed.connect(func() -> void: act(merchant.sell(s, id, int(s.bag.get(id, 0)))))
			buttons.append(all)
		_row(id, "%s  x%d" % [_name(id), n], buttons)
	_info.text = "[color=#c8b48c]Hover an item for its card.  Gear sells for 40% of its value, food and potions 30%, monster materials 60%, Magic Cores 80%.[/color]"


func _name(id: String) -> String:
	return str(player.sheet.item_data(id).get("name", id))


func _row(id: String, text: String, buttons: Array) -> void:
	var h := HBoxContainer.new()
	h.add_theme_constant_override("separation", 8)
	var icon := TextureRect.new()
	icon.texture = Icons.item(id)
	icon.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	icon.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	icon.custom_minimum_size = Vector2(34, 34)
	h.add_child(icon)
	var l := rich(h, text)
	l.mouse_filter = Control.MOUSE_FILTER_PASS
	for b in buttons:
		h.add_child(b)
	h.mouse_entered.connect(func() -> void:
		if status != null:
			_info.text = status.item_tip(id, true))
	_list.add_child(h)
