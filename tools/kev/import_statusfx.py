"""Import the small shader references used by the Bevy StatusFX adapter."""
import hashlib
import json
from pathlib import Path
import sys
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[2]
PREFIX = "StatusFXFree/assets/BinbunVFX_Vol2/StatusFX/"
FILES = ("license.txt", "shader/aura_sphere.gdshader", "shader/streaks.gdshader",
         "shader/ice_particle.gdshader", "effects/status/vfx_status_shatter.tscn")


def main():
    archive = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "assets/StatusFXFree.zip"
    destination = ROOT / "assets/vfx/statusfx/source"
    destination.mkdir(parents=True, exist_ok=True)
    manifest = []
    with ZipFile(archive) as bundle:
        for relative in FILES:
            data = bundle.read(PREFIX + relative)
            target = destination / Path(relative).name
            target.write_bytes(data)
            manifest.append({"source": PREFIX + relative, "file": target.name,
                             "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    (destination.parent / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"Imported {len(manifest)} shader/preset references ({sum(m['bytes'] for m in manifest)} bytes)")


if __name__ == "__main__":
    main()
