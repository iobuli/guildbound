"""Convert the locally owned Kevin Iglesias Unity package to preview-only GLBs.

Usage: python tools/model/kevin_spell_pack.py <Human Spellcasting Animations FREE.unitypackage>
Requires Blender on PATH or BLENDER_EXE. Output stays in the ignored local cache.
"""
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "assets/anim/kevin_spell_cache"


def rename_animation(path: Path, name: str) -> None:
    data = path.read_bytes()
    length, kind = struct.unpack_from("<II", data, 12)
    if kind != 0x4E4F534A:
        raise ValueError(f"{path}: missing glTF JSON")
    document = json.loads(data[20 : 20 + length])
    if len(document.get("animations", [])) != 1:
        raise ValueError(f"{path}: expected one animation")
    document["animations"][0]["name"] = name
    encoded = json.dumps(document, separators=(",", ":")).encode("utf-8")
    encoded += b" " * (-len(encoded) % 4)
    binary = data[20 + length :]
    result = struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(encoded) + len(binary))
    result += struct.pack("<II", len(encoded), kind) + encoded + binary
    path.write_bytes(result)


def main() -> None:
    archive = Path(sys.argv[1]).resolve()
    blender = os.environ.get("BLENDER_EXE") or shutil.which("blender") or "D:/blender.exe"
    OUTPUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="athorina-kevin-") as temporary:
        source = Path(temporary)
        items = []
        with tarfile.open(archive) as package:
            members = {m.name: m for m in package.getmembers()}
            for member in members.values():
                if not member.name.endswith("/pathname"):
                    continue
                original = package.extractfile(member).read().decode("utf-8", "ignore").splitlines()[0]
                if not original.endswith(".fbx") or "/Male/" not in original or "HumanM@" not in original:
                    continue
                raw = package.extractfile(members[member.name.rsplit("/", 1)[0] + "/asset"]).read()
                clip = Path(original).stem.split("@", 1)[-1].replace(" [RM]", "_RM")
                safe = re.sub(r"[^A-Za-z0-9_]+", "_", clip)
                fbx = source / (safe + ".fbx")
                fbx.write_bytes(raw)
                items.append({"name": clip, "source": str(fbx), "output": str(OUTPUT / (safe + ".glb"))})
        items.sort(key=lambda item: item["name"])
        manifest = source / "manifest.json"
        manifest.write_text(json.dumps(items), encoding="utf-8")
        script = Path(__file__).with_name("kevin_spell_blender.py")
        subprocess.run([str(blender), "-b", "--factory-startup", "--python", str(script), "--", str(manifest)], check=True)
        for item in items:
            rename_animation(Path(item["output"]), item["name"])
    print(f"Kevin Iglesias: {len(items)} local animations in {OUTPUT}")


if __name__ == "__main__":
    main()
