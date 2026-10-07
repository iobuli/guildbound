class_name Merchant
extends Node
## A shop (data/merchants.json). It sells its stock at the item's value (Economy.value) x markup and
## buys anything you carry at value x the rate for its kind (gear 40%, food and potions 30%, monster
## materials 60%, Magic Cores 80%). Sold-out goods come back one of each every `restock_s`.

var shop_id := "general"
var data := {}
var stock := {}        # item id -> how many are on the shelf
var _restock_t := 0.0


func setup(id: String) -> void:
	shop_id = id
	data = DataStore.load_json("merchants.json")[id]
	stock = (data["stock"] as Dictionary).duplicate()


func _process(delta: float) -> void:
	_restock_t += delta
	if _restock_t < float(data.get("restock_s", 90.0)):
		return
	_restock_t = 0.0
	for id: String in data["stock"]:
		if int(stock[id]) < int(data["stock"][id]):
			stock[id] = int(stock[id]) + 1


## What the merchant asks for one.
func price(id: String) -> int:
	return maxi(1, Economy.round_half_up(Economy.value(id) * float(data.get("markup", 1.0))))


## What the merchant pays for one.
func offer(id: String) -> int:
	var ratio := float((data.get("buy_ratio", {}) as Dictionary).get(Economy.category(id), 0.4))
	return maxi(1, Economy.round_half_up(Economy.value(id) * ratio))


func buy(sheet: CharModel, id: String) -> Array:
	var nm := str(sheet.item_data(id).get("name", id))
	if int(stock.get(id, 0)) <= 0:
		return [false, nm + " is sold out"]
	var p := price(id)
	if sheet.gold < p:
		return [false, "Not enough gold for %s (%d g)" % [nm, p]]
	if not sheet.add_item(id):
		return [false, "No room in the backpack for " + nm]
	sheet.gold -= p
	stock[id] = int(stock[id]) - 1
	return [true, "Bought %s for %d gold" % [nm, p]]


func sell(sheet: CharModel, id: String, n: int = 1) -> Array:
	var nm := str(sheet.item_data(id).get("name", id))
	if int(sheet.bag.get(id, 0)) < n or not sheet.remove_item(id, n):
		return [false, "You don't have %d %s" % [n, nm]]
	var got := offer(id) * n
	sheet.gold += got
	return [true, "Sold %s%s for %d gold" % [nm, " x%d" % n if n > 1 else "", got]]
