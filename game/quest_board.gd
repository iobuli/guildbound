class_name QuestBoard
extends ModalWindow
## The guild's quest board (E at the board on the plaza): your rank, the quests you carry with their
## progress (turn in / abandon), the ones resting before they come back, and the offers for your rank.


func _init() -> void:
	title = "Adventurers' Guild - Quest Board"


func open_board() -> void:
	open_window()


func close_board() -> void:
	close_window()


func refresh() -> void:
	var q := player.quests
	clear_list()
	var nr := q.next_rank()
	_head.text = "Rank %s   -   %d rank points%s   -   %d gold" % [q.rank(), q.rank_points,
		("   (%s at %d)" % [nr[2], nr[1]]) if not nr.is_empty() else "   (top rank)", player.sheet.gold]
	section("Your quests  (%d / %d)" % [q.active.size(), int(q.data.get("max_active", 3))])
	if q.active.is_empty():
		note("None yet - take one from the board below.")
	for id: String in q.active:
		var col := _quest_card(id)
		var btns := HBoxContainer.new()
		var done := q.complete(id)
		var turn := Button.new()
		turn.text = "Turn in" if done else "Not finished"
		turn.disabled = not done
		turn.pressed.connect(func() -> void: act(q.turn_in(id)))
		btns.add_child(turn)
		var drop := Button.new()
		drop.text = "Abandon"
		drop.pressed.connect(func() -> void:
			q.abandon(id)
			refresh())
		btns.add_child(drop)
		col.add_child(btns)
	section("On the board")
	var offer := q.available()
	if offer.is_empty():
		note("Nothing new for your rank right now.")
	for id: String in offer:
		var col := _quest_card(id)
		var take := Button.new()
		take.text = "Accept"
		take.pressed.connect(func() -> void: act(q.accept(id)))
		col.add_child(take)
	var resting: Array = q.cooling()
	if not resting.is_empty():
		var parts: Array = []
		for r in resting:
			var secs := int(ceil(float(r[1])))
			parts.append("%s (%d:%02d)" % [q.quest(str(r[0]))["name"], secs / 60, secs % 60])
		note("Back on the board soon: " + ", ".join(parts))


func _quest_card(id: String) -> VBoxContainer:
	var q := player.quests
	var d := q.quest(id)
	var col := card()
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
	rich(col, txt).custom_minimum_size = Vector2(560, 0)
	return col
