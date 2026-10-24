"""Blender helper called by kevin_spell_pack.py; each FBX exports one local GLB."""
import json
from pathlib import Path
import sys

import bpy


manifest = Path(sys.argv[sys.argv.index("--") + 1])
items = json.loads(manifest.read_text(encoding="utf-8"))
for item in items:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=item["source"])
    bpy.ops.export_scene.gltf(filepath=item["output"], export_format="GLB",
                              export_animations=True, export_animation_mode="ACTIVE_ACTIONS")
    print(f"KEVIN_EXPORTED {item['name']}")
