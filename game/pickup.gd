class_name ItemPickup
extends Node3D
## An item lying in the world. Walk up and press E to put it in your bag (gear, food, potions).

static var all: Array[ItemPickup] = []

var item_id := ""
var count := 1
var _t := 0.0
var _gem: MeshInstance3D


static func make(id: String, n: int = 1) -> ItemPickup:
	var p := ItemPickup.new()
	p.item_id = id
	p.count = n
	return p


func _ready() -> void:
	all.append(self)
	var consumable := CharModel.is_consumable(id_or_empty())
	var data: Dictionary = DataStore.items().get(item_id, DataStore.consumables().get(item_id, {}))
	_gem = MeshInstance3D.new()
	var m: PrimitiveMesh
	if consumable:
		m = SphereMesh.new()
	else:
		m = BoxMesh.new()
	if m is BoxMesh:
		(m as BoxMesh).size = Vector3(0.35, 0.22, 0.35)
	else:
		(m as SphereMesh).radius = 0.14
		(m as SphereMesh).height = 0.28
	var mat := StandardMaterial3D.new()
	var tier := int(data.get("tier", 1))
	var col: Color = [Color(0.8, 0.8, 0.8), Color(0.45, 0.85, 0.45), Color(0.4, 0.6, 1.0), Color(0.75, 0.45, 1.0), Color(1.0, 0.7, 0.2)][clampi(tier - 1, 0, 4)]
	if consumable:
		col = Color(1.0, 0.45, 0.4)
	mat.albedo_color = col
	mat.emission_enabled = true
	mat.emission = col * 0.6
	m.material = mat
	_gem.mesh = m
	_gem.position.y = 0.45
	add_child(_gem)
	var label := Label3D.new()
	label.text = "%s%s\n[E] pick up" % [data.get("name", item_id), " x%d" % count if count > 1 else ""]
	label.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	label.font_size = 28
	label.outline_size = 6
	label.modulate = col.lightened(0.3)
	label.position.y = 1.0
	add_child(label)


func id_or_empty() -> String:
	return item_id


func _exit_tree() -> void:
	all.erase(self)


func _process(delta: float) -> void:
	_t += delta
	_gem.position.y = 0.45 + 0.06 * sin(_t * 2.5)
	_gem.rotation.y = _t * 1.2


## The pickup in reach of `pos` (within `reach` m), nearest first, or null.
static func nearest(pos: Vector3, reach: float = 1.8) -> ItemPickup:
	var best: ItemPickup = null
	var bd := reach
	for p in all:
		if not is_instance_valid(p):
			continue
		var d := Vector2(p.global_position.x - pos.x, p.global_position.z - pos.z).length()
		if d < bd and absf(p.global_position.y - pos.y) < 1.5:
			bd = d
			best = p
	return best
