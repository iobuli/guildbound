class_name BodyParts
extends RefCounted
## The body as capsules between bones (slim, neutral proportions): what the prototype mannequin is made
## of, what armour shells are sized over, and the hurtboxes a melee blow is tested against.
##
## Rest space = the skeleton's own space in the bind pose: upright, facing +Z, +X is the character's
## left, the floor at y = 0. Every part is a rest-space shape riding ONE bone, so it follows that bone
## exactly (the clips rotate bones, they do not stretch them).
##
## A point is a bone (its rest origin), [bone, offset] (rest space), or [bone, from_bone, distance]:
## past the bone along from_bone -> bone. The radius is k x a reference length: the segment's own
## length, or the rest distance between the two "ref" bones (torso pieces are wider than they are long).

const ZONE_MULT := {"head": 1.2, "torso": 1.0, "arm": 0.85, "leg": 0.85}

static var _table: Array = []


## Every segment: {n, bone, a, b, k, zone, ref?}. Limbs come in pairs, "_l" and "_r".
static func table() -> Array:
	if not _table.is_empty():
		return _table
	_table = [
		{"n": "head", "bone": "Head", "a": ["Head", Vector3(0, 0.06, 0.02)], "b": ["Head", Vector3(0, 0.12, 0.01)], "k": 1.22, "ref": ["neck_01", "Head"], "zone": "head"},
		{"n": "neck", "bone": "neck_01", "a": ["neck_01", Vector3(0, -0.02, 0)], "b": "Head", "k": 0.5, "zone": "torso"},
		{"n": "chest", "bone": "spine_03", "a": ["spine_03", Vector3(0.085, 0.1, -0.015)], "b": ["spine_03", Vector3(-0.085, 0.1, -0.015)], "k": 0.56, "ref": ["spine_03", "neck_01"], "zone": "torso"},
		{"n": "abdomen", "bone": "spine_02", "a": ["spine_02", Vector3(0, -0.02, 0)], "b": ["spine_02", Vector3(0, 0.08, -0.005)], "k": 0.8, "ref": ["spine_02", "spine_03"], "zone": "torso"},
		{"n": "pelvis", "bone": "pelvis", "a": ["pelvis", Vector3(0.07, 0, 0)], "b": ["pelvis", Vector3(-0.07, 0, 0)], "k": 0.95, "ref": ["pelvis", "spine_01"], "zone": "torso"},
	]
	for s in ["l", "r"]:
		_table.append_array([
			{"n": "upperarm_" + s, "bone": "upperarm_" + s, "a": "upperarm_" + s, "b": "lowerarm_" + s, "k": 0.2, "zone": "arm"},
			{"n": "forearm_" + s, "bone": "lowerarm_" + s, "a": "lowerarm_" + s, "b": "hand_" + s, "k": 0.165, "zone": "arm"},
			{"n": "hand_" + s, "bone": "hand_" + s, "a": ["hand_" + s, "lowerarm_" + s, 0.03], "b": ["hand_" + s, "lowerarm_" + s, 0.13], "k": 0.4, "zone": "arm"},
			{"n": "thigh_" + s, "bone": "thigh_" + s, "a": "thigh_" + s, "b": "calf_" + s, "k": 0.155, "zone": "leg"},
			{"n": "shin_" + s, "bone": "calf_" + s, "a": "calf_" + s, "b": "foot_" + s, "k": 0.11, "zone": "leg"},
			{"n": "foot_" + s, "bone": "foot_" + s, "a": ["foot_" + s, Vector3(0, -0.035, -0.025)], "b": ["ball_leaf_" + s, Vector3(0, 0.02, -0.035)], "k": 0.2, "zone": "leg"},
		])
	return _table


## The darker joint spheres of the mannequin: [bone, segment whose radius they scale, factor].
static func joints() -> Array:
	var out: Array = [["spine_01", "abdomen", 0.9], ["neck_01", "neck", 1.15]]
	for s in ["l", "r"]:
		out.append_array([["upperarm_" + s, "upperarm_" + s, 1.18], ["lowerarm_" + s, "forearm_" + s, 1.2],
			["hand_" + s, "hand_" + s, 0.95], ["thigh_" + s, "thigh_" + s, 1.05], ["calf_" + s, "shin_" + s, 1.25],
			["foot_" + s, "shin_" + s, 0.95]])
	return out


static func names() -> Array:
	var out: Array = []
	for e: Dictionary in table():
		out.append(e["n"])
	return out


static func rest_point(sk: Skeleton3D, spec: Variant) -> Vector3:
	if spec is String:
		var bi := sk.find_bone(spec)
		return sk.get_bone_global_rest(bi).origin if bi >= 0 else Vector3.ZERO
	var arr: Array = spec
	var base := rest_point(sk, arr[0])
	if arr.size() == 2:
		return base + (arr[1] as Vector3)
	var from := rest_point(sk, arr[1])
	return base + (base - from).normalized() * float(arr[2])


## A segment bound to this skeleton: its bone index, both ends in that bone's frame, its radius (and the
## rest-space ends, for building meshes). Empty if the skeleton lacks the bone.
static func bind(sk: Skeleton3D, e: Dictionary) -> Dictionary:
	var bi := sk.find_bone(str(e["bone"]))
	if bi < 0:
		return {}
	var inv := sk.get_bone_global_rest(bi).affine_inverse()
	var pa := rest_point(sk, e["a"])
	var pb := rest_point(sk, e["b"])
	var ref_len := pa.distance_to(pb)
	if e.has("ref"):
		var rf: Array = e["ref"]
		ref_len = rest_point(sk, rf[0]).distance_to(rest_point(sk, rf[1]))
	return {"n": e["n"], "zone": e["zone"], "bone": bi, "a": inv * pa, "b": inv * pb, "r": float(e["k"]) * ref_len,
		"rest_a": pa, "rest_b": pb}


static func bind_all(sk: Skeleton3D) -> Array:
	var out: Array = []
	for e: Dictionary in table():
		var b := bind(sk, e)
		if not b.is_empty():
			out.append(b)
	return out


## A bound segment in the world, posed: [a, b, radius, zone, name].
static func to_world(sk: Skeleton3D, h: Dictionary) -> Array:
	var g := sk.global_transform * sk.get_bone_global_pose(int(h["bone"]))
	return [g * (h["a"] as Vector3), g * (h["b"] as Vector3), float(h["r"]) * sk.global_basis.get_scale().x, h["zone"], h["n"]]


## A segment's world endpoints and radius on this skeleton as it is posed now: [a, b, radius] ([] if unknown).
static func endpoints(sk: Skeleton3D, seg_name: String) -> Array:
	for e: Dictionary in table():
		if e["n"] == seg_name:
			var h := bind(sk, e)
			return [] if h.is_empty() else to_world(sk, h).slice(0, 3)
	return []


## The capsule a segment s0-s1 comes closest to (by surface gap): [gap, point on s0-s1, zone, name].
static func nearest(caps: Array, s0: Vector3, s1: Vector3) -> Array:
	var best: Array = [INF, Vector3.INF, "", ""]
	for c: Array in caps:
		var pts := Geometry3D.get_closest_points_between_segments(s0, s1, c[0], c[1])
		var gap := (pts[0] as Vector3).distance_to(pts[1]) - float(c[2])
		if gap < float(best[0]):
			best = [gap, pts[0], c[3], c[4]]
	return best


## A capsule's rest-space transform: centred between p and q, its axis (+Y) along p -> q.
static func along(p: Vector3, q: Vector3) -> Transform3D:
	var d := (q - p).normalized()
	var b := Basis(Vector3.RIGHT, PI) if d.dot(Vector3.UP) < -0.999 else Basis(Quaternion(Vector3.UP, d))
	return Transform3D(b, (p + q) * 0.5)


# ---- armour ------------------------------------------------------------------------------------

const ARMOUR_COL := {"leather": Color(0.46, 0.29, 0.15), "iron": Color(0.45, 0.46, 0.48), "steel": Color(0.84, 0.86, 0.9),
	"gold": Color(0.93, 0.72, 0.26), "cloth": Color(0.55, 0.5, 0.42)}
const MATERIAL_WORDS := [["gold", "gold"], ["steel", "steel"], ["mithril", "steel"], ["plate", "steel"], ["iron", "iron"],
	["chain", "iron"], ["mail", "iron"], ["leather", "leather"], ["hide", "leather"], ["jerkin", "leather"], ["cap", "leather"]]
static var _armour_mats := {}


## What an item is made of, from its id / name: leather, iron, steel, gold, else cloth.
static func material_of(text: String) -> String:
	var t := text.to_lower()
	for w: Array in MATERIAL_WORDS:
		if t.contains(str(w[0])):
			return w[1]
	return "cloth"


static func armour_material(kind: String) -> StandardMaterial3D:
	if _armour_mats.has(kind):
		return _armour_mats[kind]
	var m := StandardMaterial3D.new()
	m.albedo_color = ARMOUR_COL.get(kind, ARMOUR_COL["cloth"])
	var metal := kind == "iron" or kind == "steel" or kind == "gold"
	m.metallic = 0.75 if metal else 0.0
	m.roughness = (0.32 if kind != "iron" else 0.5) if metal else 0.85
	_armour_mats[kind] = m
	return m
