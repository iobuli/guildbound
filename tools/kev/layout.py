"""Kev layout: place the imported packs on the bigger map as data, from each GLB's measured bounds.

    python3 tools/kev/layout.py [--out assets/data/regions.json]

Each GLB's world-space bounding box is measured by walking its scene graph (node TRS matrices applied to
the corners of every mesh primitive's POSITION min / max), so FBX root rotations and centimetre scales
are accounted for. Every placement in the output carries:

  path      GLB under assets/ (packs are local-only: the game skips files that are not there)
  at        [x, z] world position of the footprint centre; y comes from the terrain at load
  yaw       degrees about +Y
  scale     uniform scale so the object reaches its real size (`fit` heights below)
  half      collider half extents (m, after scale), or null for no collider (small clutter)
  lift      metres from the object's origin up to the bottom of its box (so it sits on the ground)
  offset    [x, z] from the origin to the box centre (m, after scale and yaw), for the collider

Regions (their pads are flattened by world.rs `PADS`; keep the centres in step):
  foundry  Mech_00 floor tiles in a grid, walls round three sides, ledges, the mech as a statue, its
           sword, hammer and attachments as scrap
  market   PropsLite stalls in rows (tables with food, plates, jugs, candles), barrels, crates,
           firewood and a fire pit, the FoodPack spread on the long table
  plaza    the company characters (elf, knight, mage, dragon) as statues round a ring

The mech and the characters are placed as statues: their bind pose baked into static meshes by
tools/kev/statue.py (`<name>_statue.glb`, the mech without its spare body sets), since a scaled skinned
Rigify mesh distorts in the game.
"""
import argparse
import json
import math
import os
import struct

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
ASSETS = os.path.join(REPO, "assets")

# Pad centres and radii (m); world.rs PADS must match.
PADS = {"foundry": ((80.0, -60.0), 24.0), "market": ((-80.0, 40.0), 20.0), "plaza": ((40.0, 85.0), 16.0)}
# Real heights (m) objects are fitted to, by name prefix; anything else keeps its authored size.
FIT = {
    "Mech_statue": 3.7, "elf": 2.2, "knight": 2.3, "mage": 2.2, "dragon": 6.0,
}
# Widths (m) objects are fitted to instead (a pack whose nodes scale it far past its real size).
FIT_W = {"food_pack": 2.0}
# Objects smaller than this (largest half extent, m) get no collider.
CLUTTER = 0.35


def header(path):
    with open(path, "rb") as f:
        f.seek(12)
        n, _ = struct.unpack("<II", f.read(8))
        return json.loads(f.read(n))


def trs(node):
    if "matrix" in node:
        return np.array(node["matrix"], float).reshape(4, 4).T
    t = np.array(node.get("translation", [0, 0, 0]), float)
    x, y, z, w = node.get("rotation", [0, 0, 0, 1])
    s = np.array(node.get("scale", [1, 1, 1]), float)
    r = np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])
    m = np.eye(4)
    m[:3, :3] = r * s
    m[:3, 3] = t
    return m


def bounds(path):
    """World-space (min, max) of every mesh in the GLB's default scene. A skinned mesh ignores its own
    node transform (glTF): its bind-space vertices go through joint world x inverse bind, so its box is
    mapped by the first joint's world matrix times that joint's inverse-bind matrix."""
    js = header(path)
    lo, hi = np.full(3, np.inf), np.full(3, -np.inf)
    scene = js["scenes"][js.get("scene", 0)]
    world = {}

    def walk(i, parent):
        world[i] = parent @ trs(js["nodes"][i])
        for c in js["nodes"][i].get("children", []):
            walk(c, world[i])

    for i in scene["nodes"]:
        walk(i, np.eye(4))
    with open(path, "rb") as f:
        f.seek(12)
        n, _ = struct.unpack("<II", f.read(8))
        f.seek(20 + n)
        blen, _ = struct.unpack("<II", f.read(8))
        blob = f.read(blen)

    def _ibm0(skin):
        acc = js["accessors"][skin["inverseBindMatrices"]]
        view = js["bufferViews"][acc["bufferView"]]
        off = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
        return np.frombuffer(blob, "<f4", 16, off).reshape(4, 4).T.astype(float)

    for i, m in world.items():
        node = js["nodes"][i]
        if "mesh" not in node:
            continue
        if "skin" in node:
            # Skinned: bind-space vertices go through joint world x inverse bind; at the bind pose that
            # product is the same for every joint, so the first joint's stands for the mesh.
            skin = js["skins"][node["skin"]]
            m = world.get(skin["joints"][0], np.eye(4)) @ _ibm0(skin)
        for prim in js["meshes"][node["mesh"]]["primitives"]:
            acc = js["accessors"][prim["attributes"]["POSITION"]]
            a_, b_ = np.array(acc["min"]), np.array(acc["max"])
            for corner in np.array(np.meshgrid(*zip(a_, b_))).T.reshape(-1, 3):
                q = m @ np.r_[corner, 1.0]
                lo[:] = np.minimum(lo, q[:3])
                hi[:] = np.maximum(hi, q[:3])
    return lo, hi


class Layout:
    def __init__(self):
        self.items = []
        self.cache = {}

    def measure(self, rel):
        if rel not in self.cache:
            p = os.path.join(ASSETS, rel)
            self.cache[rel] = bounds(p) if os.path.exists(p) else None
        return self.cache[rel]

    def put(self, rel, x, z, yaw=0.0, scale=None, solid=True, region=""):
        b = self.measure(rel)
        if b is None:
            return
        lo, hi = b
        name = os.path.splitext(os.path.basename(rel))[0]
        if scale is None:
            fit = next((h for k, h in FIT.items() if name.startswith(k)), None)
            fit_w = next((w for k, w in FIT_W.items() if name.startswith(k)), None)
            if fit:
                scale = fit / max(hi[1] - lo[1], 1e-6)
            elif fit_w:
                scale = fit_w / max(hi[0] - lo[0], hi[2] - lo[2], 1e-6)
            else:
                scale = 1.0
        size = (hi - lo) * scale
        centre = (hi + lo) * 0.5 * scale
        c, s = math.cos(math.radians(yaw)), math.sin(math.radians(yaw))
        off = [centre[0] * c + centre[2] * s, -centre[0] * s + centre[2] * c]
        half = (size * 0.5).round(3).tolist()
        self.items.append({
            "region": region, "path": rel, "at": [round(x - off[0], 3), round(z - off[1], 3)], "yaw": round(yaw, 1),
            "scale": round(float(scale), 5), "lift": round(float(-lo[1] * scale), 3),
            "half": half if solid and max(half) > CLUTTER else None, "offset": [round(off[0], 3), round(off[1], 3)],
        })
        return size


def foundry(L):
    (cx, cz), _ = PADS["foundry"]
    tile = "packs/mech_00/glb/Tile_1-1.glb"
    b = L.measure(tile)
    if b is None:
        return
    step = float(max(b[1][0] - b[0][0], b[1][2] - b[0][2]))
    n = 7
    for i in range(n):
        for j in range(n):
            kind = "packs/mech_00/glb/Tile_1-1.glb" if (i + j) % 3 else "packs/mech_00/glb/Tile_1-0.glb"
            L.put(kind, cx + (i - n // 2) * step, cz + (j - n // 2) * step, solid=False, region="foundry")
    edge = (n // 2 + 0.5) * step
    wall = "packs/mech_00/glb/Wall_1-1.glb"
    wb = L.measure(wall)
    wlen = float(max(wb[1][0] - wb[0][0], wb[1][2] - wb[0][2])) if wb else 8.0
    k = max(1, int(round(2 * edge / wlen)))
    for side in ("n", "e", "w"):
        for i in range(k):
            t = -edge + (i + 0.5) * (2 * edge / k)
            x, z = {"n": (cx + t, cz - edge), "e": (cx + edge, cz + t), "w": (cx - edge, cz + t)}[side]
            # Turn the wall's long side onto the edge it lines: along x for the north edge, along z for the sides.
            long_z = wb is not None and (wb[1][2] - wb[0][2]) > (wb[1][0] - wb[0][0])
            along_x = side == "n"
            L.put(wall, x, z, yaw=90.0 if along_x == long_z else 0.0, region="foundry")
    for i, ledge in enumerate(["Ledge_0.glb", "Ledge_0_2.glb", "Ledge_0_4.glb", "Ledge_OuterEdge.glb"]):
        L.put("packs/mech_00/glb/" + ledge, cx - edge * 0.55 + i * 5.0, cz + edge * 0.55, yaw=180.0, region="foundry")
    # The mech itself is a parts kit (its pieces lie side by side, assembled by the pack's Unity body
    # swapper), so it is not placed; its weapons and attachments lie about as scrap.
    scrap = ["Core_Sword", "Core_Hammer", "Attach_Gun_00", "Attach_Gun_01", "Attach_Gun_02", "Attach_Back_Missiles_00",
             "Attach_Back_Missiles_01", "Attach_Back_JetPack_00", "Attach_Module_00", "Attach_Module_01", "Attach_Module_02",
             "Attach_Armour_00", "Attach_Armour_01", "Attach_Armour_02", "Attach_Back_1Gun", "Attach_Back_2Gun", "Attach_Back_WeaponAttacher"]
    for i, name in enumerate(scrap):
        a = i * 2.399963
        r = 5.0 + (i % 5) * 1.6
        L.put(f"packs/mech_00/glb/{name}.glb", cx + math.cos(a) * r, cz + 2.0 + math.sin(a) * r, yaw=(i * 47) % 360, region="foundry")


def market(L):
    (cx, cz), _ = PADS["market"]
    P = "packs/props_lite/glb/"
    cm = 0.01
    table = P + "Furniture_01.glb"
    on_table = ["Food_01", "Food_02", "Food_03", "Food_04", "Food_05", "Food_06", "Plate_01", "Plate_02", "Jug_01", "Jug_02",
                "Cup_01", "Bottle_01", "Wineglass_01", "Candle_01", "Coin_01", "Coin_02"]
    k = 0
    for row in range(3):
        for col in range(4):
            x, z = cx - 9.0 + col * 6.0, cz - 6.0 + row * 6.0
            size = L.put(table, x, z, yaw=90.0 * (row % 2), scale=cm, region="market")
            L.put(P + "Carpet_01.glb", x, z + 2.2, scale=cm, solid=False, region="market")
            for d in (-0.5, 0.0, 0.5):
                L.put(P + on_table[k % len(on_table)] + ".glb", x + d, z, scale=cm, solid=False, region="market")
                k += 1
            if size is not None:
                # Table-top props ride on the table: mark their lift on top of it.
                for it in L.items[-3:]:
                    it["lift"] = round(it["lift"] + float(size[1]), 3)
    for i in range(10):
        a = i / 10 * math.tau
        L.put(P + ("Barrel_01.glb" if i % 2 else "Box_01.glb"), cx + math.cos(a) * 15.0, cz + math.sin(a) * 15.0, yaw=i * 33, scale=cm, region="market")
    L.put(P + "Fire_01.glb", cx, cz + 11.0, scale=cm, region="market")
    L.put(P + "Firewood_01.glb", cx + 1.6, cz + 11.0, scale=cm, region="market")
    L.put(P + "Stairs_01.glb", cx - 14.0, cz, yaw=90.0, scale=cm, region="market")
    for i, f in enumerate(["Fence_01", "Fence_02"] * 4):
        L.put(P + f + ".glb", cx - 12.0 + i * 3.0, cz + 15.5, scale=cm, region="market")
    for i, f in enumerate(["Bucket_01", "Bag_01", "Stone_01", "Axe_01", "WoodPlank_01", "WoodPlank_02", "Pointer_01", "Pointer_02"]):
        L.put(P + f + ".glb", cx + 11.0, cz - 8.0 + i * 1.6, yaw=i * 51, scale=cm, region="market")
    L.put("packs/food_newlua/glb/food_pack.glb", cx, cz - 12.0, region="market")


def plaza(L):
    (cx, cz), _ = PADS["plaza"]
    figs = ["elf", "knight", "mage", "dragon"]
    for i, f in enumerate(figs):
        a = i / len(figs) * math.tau + math.pi / 4
        r = 10.0 if f == "dragon" else 7.0
        x, z = cx + math.cos(a) * r, cz + math.sin(a) * r
        yaw = math.degrees(math.atan2(cx - x, cz - z))
        L.put(f"packs/company_characters/glb/{f}_statue.glb", x, z, yaw=yaw, region="plaza")


def build():
    L = Layout()
    foundry(L)
    market(L)
    plaza(L)
    return {"_doc": "Written by tools/kev/layout.py from the GLBs' measured bounds; world.rs regions reads it. "
                    "Pads (flattened ground) are world.rs PADS.", "pads": {k: {"at": list(v[0]), "radius": v[1]} for k, v in PADS.items()},
            "items": L.items}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=os.path.join(ASSETS, "data", "regions.json"))
    args = ap.parse_args()
    data = build()
    with open(args.out, "w") as f:
        json.dump(data, f, indent=1)
    by = {}
    for it in data["items"]:
        by[it["region"]] = by.get(it["region"], 0) + 1
    print(f"{len(data['items'])} placements: {by}")


if __name__ == "__main__":
    main()
