"""Downscale a GLB's embedded textures to at most --max px (default 2048); every other byte is kept as is.

python -I Tools/shrink_glb.py FILE.glb [FILE.glb ...] [--max 2048]

The Quaternius outfits ship 4096 px base colour, normal and ORM maps: about 30 MB per outfit on disk and ~250 MB
of VRAM per model once decoded, for a character that covers a few hundred pixels on screen. Rewrites in place.
"""
import argparse
import io
import json
import struct

from PIL import Image


def shrink(path, limit):
    b = open(path, "rb").read()
    jlen = struct.unpack("<I", b[12:16])[0]
    j = json.loads(b[20:20 + jlen])
    blen = struct.unpack("<I", b[20 + jlen:24 + jlen])[0]
    blob = b[28 + jlen:28 + jlen + blen]
    views = [blob[v.get("byteOffset", 0):v.get("byteOffset", 0) + v["byteLength"]] for v in j["bufferViews"]]
    changed = 0
    for im in j.get("images", []):
        if "bufferView" not in im:
            continue
        img = Image.open(io.BytesIO(views[im["bufferView"]]))
        if max(img.size) <= limit:
            continue
        k = limit / max(img.size)
        img = img.resize((round(img.size[0] * k), round(img.size[1] * k)), Image.LANCZOS)
        out = io.BytesIO()
        img.save(out, format="PNG" if im.get("mimeType", "image/png") == "image/png" else "JPEG", optimize=True, quality=92)
        views[im["bufferView"]] = out.getvalue()
        changed += 1
    if not changed:
        return 0
    blob = bytearray()
    for v, data in zip(j["bufferViews"], views):
        blob += b"\0" * (-len(blob) % 4)
        v["byteOffset"] = len(blob)
        v["byteLength"] = len(data)
        blob += data
    blob += b"\0" * (-len(blob) % 4)
    j["buffers"][0]["byteLength"] = len(blob)
    js = json.dumps(j, separators=(",", ":")).encode()
    js += b" " * (-len(js) % 4)
    with open(path, "wb") as f:
        f.write(struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(js) + 8 + len(blob)))
        f.write(struct.pack("<I4s", len(js), b"JSON") + js)
        f.write(struct.pack("<I4s", len(blob), b"BIN\0") + blob)
    return changed


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("glb", nargs="+")
    p.add_argument("--max", type=int, default=2048)
    a = p.parse_args()
    for g in a.glb:
        print(f"SHRUNK {shrink(g, a.max)} images in {g}")
