"""Crop the guildhouse banner and rug to their cloth for in-game use.

The Higgsfield banner (`banner_cloth.png`) and rug (`rug.png`) are drawn on a pale background with a
hanging rod / fringe. The game shows them as flat quads, so we keep only the coloured cloth: the
bounding box of saturated pixels, scaled to at most
1024 px on the long side.

    python3 tools/model/guild_cutouts.py <src_dir> assets/textures/guildhouse
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def cloth(path: Path, top_trim: float, inset: float) -> Image.Image:
    im = Image.open(path).convert("RGB")
    hsv = np.asarray(im.convert("HSV")).astype(np.float32) / 255.0
    sat = (hsv[..., 1] > 0.3) & (hsv[..., 2] > 0.15)
    rows = np.where(sat.mean(axis=1) > 0.2)[0]
    cols = np.where(sat.mean(axis=0) > 0.2)[0]
    y0, y1, x0, x1 = rows[0], rows[-1], cols[0], cols[-1]
    h, w = y1 - y0, x1 - x0
    y0 += int(h * top_trim)
    dx, dy = int(w * inset), int(h * inset)
    box = (x0 + dx, y0 + dy, x1 - dx, y1 - dy)
    out = im.crop(box)
    out.thumbnail((1024, 1024), Image.LANCZOS)
    print(f"{path.name}: crop {box} -> {out.size}")
    return out


def main() -> None:
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    cloth(src / "banner_cloth.png", 0.0, 0.005).save(dst / "banner_cloth_game.png", optimize=True)
    cloth(src / "rug.png", 0.0, 0.005).save(dst / "rug_game.png", optimize=True)


if __name__ == "__main__":
    main()
