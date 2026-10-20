"""Point GLB image URIs that are absolute paths on the exporter's machine at local texture files.

    python3 tools/kev/fix_textures.py assets/packs/<pack>   [--map OLD_NAME=NEW_NAME ...]

FBX-to-glTF conversions keep the authoring path ("D:\\...\\SimplePalette0014.png"), which no other machine
has. For each GLB under <pack>/glb, every image whose URI is such a path (a drive letter or a backslash) is
looked up by file name under <pack>/src (case-insensitive; `--map` renames a misspelt one), copied next to
the GLB, and the URI is rewritten to the bare file name. The JSON chunk is rewritten in place (padded to 4
bytes); the binary chunk is untouched. Prints what was fixed and what was not found.
"""
import argparse
import json
import os
import re
import shutil
import struct


def index(src):
    found = {}
    for root, _, files in os.walk(src):
        for f in files:
            if f.lower().endswith((".png", ".jpg", ".jpeg", ".tga")):
                found.setdefault(f.lower(), os.path.join(root, f))
    return found


def rewrite(path, js):
    with open(path, "rb") as f:
        data = f.read()
    n, _ = struct.unpack("<II", data[12:20])
    rest = data[20 + n:]
    body = json.dumps(js, separators=(",", ":")).encode()
    body += b" " * ((4 - len(body) % 4) % 4)
    total = 12 + 8 + len(body) + len(rest)
    with open(path, "wb") as f:
        f.write(struct.pack("<III", 0x46546C67, 2, total))
        f.write(struct.pack("<II", len(body), 0x4E4F534A))
        f.write(body)
        f.write(rest)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pack")
    ap.add_argument("--map", nargs="*", default=[])
    args = ap.parse_args()
    renames = dict(m.lower().split("=", 1) for m in args.map)
    found = index(os.path.join(args.pack, "src"))
    gdir = os.path.join(args.pack, "glb")
    fixed, missing = 0, set()
    for g in sorted(os.listdir(gdir)):
        if not g.endswith(".glb"):
            continue
        path = os.path.join(gdir, g)
        with open(path, "rb") as f:
            f.seek(12)
            n, _ = struct.unpack("<II", f.read(8))
            js = json.loads(f.read(n))
        changed = False
        for im in js.get("images", []):
            uri = im.get("uri", "")
            if not (re.match(r"^[A-Za-z]:", uri) or "\\" in uri):
                continue
            name = re.split(r"[\\/]", uri)[-1].lower()
            local = found.get(renames.get(name, name))
            if not local:
                missing.add(name)
                continue
            dest = os.path.join(gdir, os.path.basename(local))
            if not os.path.exists(dest):
                shutil.copyfile(local, dest)
            im["uri"] = os.path.basename(local)
            changed = True
        if changed:
            rewrite(path, js)
            fixed += 1
    print(f"{args.pack}: {fixed} GLBs fixed; not found: {sorted(missing) or 'none'}")


if __name__ == "__main__":
    main()
