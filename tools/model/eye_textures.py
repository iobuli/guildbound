"""Rebuild only the hero eye textures (`assets/characters/hero/tex/eye_<colour>.png`).

The iris is painted where the eye mesh looks out (`textures.EYE_IRIS_CENTRE` / `EYE_IRIS_RADIUS`,
measured from body.glb), the same call `build.py` makes, without rebuilding the bodies.

    python3 tools/model/eye_textures.py
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import textures as tx  # noqa: E402

OUT = os.path.join(HERE, "..", "..", "assets", "characters", "hero", "tex")

for colour, srgb in tx.IRIS_COLOURS.items():
    path = os.path.join(OUT, f"eye_{colour}.png")
    tx.iris(srgb, tx.EYE_IRIS_CENTRE, tx.EYE_IRIS_RADIUS, size=tx.EYE_SIZE, seed=sum(map(ord, colour))).save(path, optimize=True)
    print("wrote", os.path.relpath(path, os.path.join(HERE, "..", "..")))
