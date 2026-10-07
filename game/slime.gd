class_name Slime
extends CharacterBody3D
## A slime: a bouncy blob (data/monsters.json "slime"). It wanders around its home in little hops,
## and when the player comes near it hops after them and lunges; the lunge hurts on contact (dodge,
## guard and parry work as against a fighter). Squash and stretch on every hop and landing. Blows
## from swords, arrows and spells land on it like on a fighter (Combat.targets). On death it drops
## its loot (Mobs) and comes back home after `respawn` seconds.

static var all: Array[Slime] = []

const GRAVITY := 20.0

var kind := "slime"
var team := "slime"
var data := {}
var home := Vector3.ZERO
var hp := 100.0
var max_hp := 100.0
var radius := 0.55
var height := 1.0
var dead := false
var power := 1.0           # its rank's power (Rank): health and lunge damage
var slow := 0.0            # chill: share of its speed lost
var slow_t := 0.0
var _respawn_t := 0.0
var _hop_t := 0.0
var _lunge := false
var _lunge_hit := false
var _cool := 0.0
var _stun := 0.0
var _squash := 0.0          # >0 squashed (landing), <0 stretched (take-off)
var _was_floor := true
var _body: MeshInstance3D
var _mat: StandardMaterial3D
var rng := RandomNumberGenerator.new()


static func make(p_data: Dictionary, p_home: Vector3, seed_: int) -> Slime:
	var s := Slime.new()
	s.data = p_data
	s.home = p_home
	s.rng.seed = seed_
	return s


func _ready() -> void:
	all.append(self)
	radius = float(data.get("radius", 0.55))
	height = radius * 1.8
	power = Rank.power(Rank.parse(str(data.get("rank", "F-"))))
	max_hp = float(data.get("hp", 100.0)) * power
	hp = max_hp
	collision_layer = Layers.PLAYER
	collision_mask = Layers.WORLD | Layers.PROPS | Layers.PLAYER
	var cs := CollisionShape3D.new()
	var sh := SphereShape3D.new()
	sh.radius = radius * 0.9
	cs.shape = sh
	cs.position = Vector3.UP * radius * 0.9
	add_child(cs)
	_body = MeshInstance3D.new()
	var sm := SphereMesh.new()
	sm.radius = radius
	sm.height = height
	_body.mesh = sm
	_mat = StandardMaterial3D.new()
	var c: Array = data.get("color", [0.35, 0.85, 0.45])
	_mat.albedo_color = Color(c[0], c[1], c[2], 0.82)
	_mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_mat.roughness = 0.15
	_mat.rim_enabled = true
	_body.material_override = _mat
	_body.position = Vector3.UP * height * 0.5
	add_child(_body)
	for side in [-1.0, 1.0]:  # eyes
		var eye := MeshInstance3D.new()
		var em := SphereMesh.new()
		em.radius = radius * 0.12
		em.height = radius * 0.24
		eye.mesh = em
		var m := StandardMaterial3D.new()
		m.albedo_color = Color(0.05, 0.05, 0.06)
		eye.material_override = m
		eye.position = Vector3(side * radius * 0.32, height * 0.18, -radius * 0.85)
		_body.add_child(eye)
	global_position = home
	_hop_t = rng.randf_range(0.2, 1.5)


func _exit_tree() -> void:
	all.erase(self)


func alive() -> bool:
	return not dead and is_inside_tree()


func _foe() -> Player:
	var best: Player = null
	var best_d := float(data.get("aggro", 9.0))
	for f in Player.fighters:
		if f.is_ai or not LockOn.alive(f):
			continue
		var d := f.global_position.distance_to(global_position)
		if d < best_d and f.global_position.distance_to(home) < best_d * 2.5:
			best_d = d
			best = f
	return best


func _physics_process(delta: float) -> void:
	if dead:
		_respawn_t -= delta
		if _respawn_t <= 0.0:
			_revive()
		return
	_cool -= delta
	_stun -= delta
	slow_t -= delta
	if slow_t <= 0.0:
		slow = 0.0
	velocity.y -= GRAVITY * delta
	var on_floor := is_on_floor()
	if on_floor:
		velocity.x = move_toward(velocity.x, 0.0, 12.0 * delta)
		velocity.z = move_toward(velocity.z, 0.0, 12.0 * delta)
		if not _was_floor:
			_squash = 0.35
			_lunge = false
		_hop_t -= delta
		if _hop_t <= 0.0 and _stun <= 0.0:
			_hop()
	_was_floor = on_floor
	# the lunge lands on whoever it touches, once
	if _lunge and not _lunge_hit:
		for f in Player.fighters:
			if not f.is_ai and LockOn.alive(f) and f.global_position.distance_to(global_position) < radius + 0.55:
				_lunge_hit = true
				f.combat.receive_monster(self, float(data.get("damage", 12)) * power, float(data.get("poise", 10)), global_position + Vector3.UP * radius)
				velocity = -velocity * 0.3 + Vector3.UP * 3.0
				break
	move_and_slide()
	# squash and stretch: volume-ish preserving
	_squash = move_toward(_squash, 0.0, delta * 2.5)
	var sy := 1.0 - _squash + (0.25 if not on_floor and velocity.y > 0.0 else 0.0)
	var sxz := 1.0 / sqrt(maxf(0.3, sy))
	_body.scale = Vector3(sxz, sy, sxz)
	_body.position.y = height * 0.5 * sy


func _hop() -> void:
	var foe := _foe()
	var dir: Vector3
	var speed := float(data.get("speed", 2.4)) * (1.0 - slow)
	var up := 5.0
	_lunge = false
	if foe != null:
		dir = foe.global_position - global_position
		dir.y = 0.0
		var d := dir.length()
		dir = dir.normalized()
		if d < 2.4 and _cool <= 0.0:  # close: lunge at them
			speed = d / 0.45 + 1.0
			up = 4.5
			_lunge = true
			_lunge_hit = false
			_cool = 1.6
		_hop_t = rng.randf_range(0.35, 0.7)
	else:
		var to_home := home - global_position
		to_home.y = 0.0
		dir = (to_home.normalized() if to_home.length() > 4.0 else Vector3(rng.randf_range(-1, 1), 0, rng.randf_range(-1, 1)).normalized())
		speed *= 0.6
		_hop_t = rng.randf_range(0.9, 2.2)
	if dir.length() > 0.1:
		look_at(global_position + dir, Vector3.UP)
	velocity = dir * speed + Vector3.UP * up
	_squash = -0.2


## A blow from a fighter (Combat.apply_hit). Returns what it did, like Combat.receive.
func receive_hit(att: Combat, raw: float, poise_dmg: float, dir: Vector3, at: Vector3, crit: bool) -> String:
	if dead:
		return "none"
	var armor := float(data.get("armor", 0.0))
	var dmg := raw * 100.0 / (100.0 + armor)
	hp -= dmg
	Vfx.number(att.p, at + Vector3.UP * 0.2, ("%d!" if crit else "%d") % roundi(dmg), Color(1.0, 0.4, 0.3) if crit else Color.WHITE)
	_squash = 0.45
	velocity = dir * (2.0 + poise_dmg * 0.04) + Vector3.UP * 2.5
	_stun = 0.4
	_lunge = false
	if hp <= 0.0:
		_die(att.p)
		return "dead"
	return "stagger" if poise_dmg >= float(data.get("poise", 10)) * 3.0 else "hit"


## Damage that is not a blow (a burn). `src` gets the kill.
func take_damage(d: float, src: Player) -> void:
	if dead:
		return
	hp -= d
	if hp <= 0.0:
		_die(src)


## The player's guard was tapped just before the lunge landed: bounced back and dazed.
func parried(by: Player) -> void:
	var away := global_position - by.global_position
	away.y = 0.0
	velocity = away.normalized() * 5.0 + Vector3.UP * 4.0
	_stun = 1.2
	_lunge = false


func _die(killer: Player) -> void:
	dead = true
	Vfx.ring(get_parent(), global_position + Vector3.UP * 0.2, _mat.albedo_color, 0.8)  # not on us: we hide
	visible = false
	collision_layer = 0
	velocity = Vector3.ZERO
	_respawn_t = float(data.get("respawn", 15.0))
	if Mobs.instance != null:
		Mobs.instance.reward(kind, global_position, killer)


func _revive() -> void:
	dead = false
	hp = max_hp
	global_position = home + Vector3(rng.randf_range(-2, 2), 0.3, rng.randf_range(-2, 2))
	velocity = Vector3.ZERO
	visible = true
	collision_layer = Layers.PLAYER
	_hop_t = 1.0
