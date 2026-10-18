"""Repair the guildhouse textures that came back wrong from Higgsfield, in place.

Seen in the first in-game render (2026-10-18):
- thin bright seam lines across some tiles (timber, plaster, roof slate) - in the albedo and in
  the normal map derived from it;
- `wall_stone` drawn as multicoloured bricks (the palette read as stone colours), `timber` green;
- `floor_planks` and `ceiling_boards` greyscale (a height panel taken for the colour);
- `interior_plaster` with its "smoke near the top" band repeating every tile.

Fixes: seam rows / columns are replaced by interpolating their neighbours; off-colour slots are
re-coloured by a gradient map of their luminance through an earthy ramp (the shapes stay, the
colours become the palette's); the plaster's row-by-row brightness is levelled. The proper fix is a
re-run of T2, T4, T6 and T11; this keeps the build usable until then.

    python3 tools/model/guild_tex_fix.py assets/textures/guildhouse
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

# Gradient ramps (dark, mid, light) in sRGB.
RAMPS = {
    "wall_stone": ((0x4A, 0x44, 0x3D), (0x8A, 0x80, 0x70), (0xC2, 0xB8, 0xA4)),
    "timber": ((0x2E, 0x21, 0x17), (0x5A, 0x40, 0x29), (0x8A, 0x68, 0x44)),
    "floor_planks": ((0x3B, 0x2A, 0x1C), (0x7A, 0x5A, 0x3A), (0xB0, 0x8A, 0x5C)),
    "ceiling_boards": ((0x2C, 0x21, 0x18), (0x5E, 0x46, 0x30), (0x8E, 0x6E, 0x4C)),
}
LEVEL: tuple[str, ...] = ()
# Slots rebuilt from a cleaner one: (source slot, RGB gain). The interior plaster's smoke band is
# drawn into its shapes, so levelling cannot hide it; it becomes the exterior plaster, darker.
DERIVED = {"interior_plaster": ("plaster", (0.86, 0.82, 0.76))}


def luminance(a: np.ndarray) -> np.ndarray:
    return a[..., 0] * 0.299 + a[..., 1] * 0.587 + a[..., 2] * 0.114


def seam_lines(profile: np.ndarray, thresh: float) -> np.ndarray:
    """Indices near the tile's midline or wrap edge (where the tiler's offset seam lies; real
    mortar and plank lines elsewhere are left alone) whose mean stands out from the median of a
    +-4 window (excluding +-1)."""
    n = len(profile)
    near = [i % n for c in (0, n // 2) for i in range(c - 8, c + 9)]
    bad = []
    for i in sorted(set(near)):
        idx = [(i + d) % n for d in (-4, -3, -2, 2, 3, 4)]
        if abs(profile[i] - np.median(profile[idx])) > thresh:
            bad.append(i)
    return np.array(bad, dtype=int)


def heal(a: np.ndarray, bad: np.ndarray, axis: int) -> np.ndarray:
    """Replace rows (axis 0) or columns (axis 1) in `bad` by interpolating the nearest good ones."""
    if len(bad) == 0:
        return a
    a = np.moveaxis(a.copy(), axis, 0)
    n = a.shape[0]
    good = np.setdiff1d(np.arange(n), bad)
    for i in bad:
        lo = good[good < i]
        hi = good[good > i]
        lo_i = lo[-1] if len(lo) else hi[0]
        hi_i = hi[0] if len(hi) else lo[-1]
        t = 0.0 if hi_i == lo_i else (i - lo_i) / (hi_i - lo_i)
        a[i] = a[lo_i] * (1 - t) + a[hi_i] * t
    return np.moveaxis(a, 0, axis)


def find_seams(albedo: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    lum = luminance(albedo) / 255.0
    return seam_lines(lum.mean(axis=1), 0.05), seam_lines(lum.mean(axis=0), 0.05)


def gradient_map(albedo: np.ndarray, ramp) -> np.ndarray:
    lum = luminance(albedo)
    lo, hi = np.percentile(lum, 2), np.percentile(lum, 98)
    t = np.clip((lum - lo) / max(hi - lo, 1e-3), 0, 1)[..., None]
    dark, mid, light = (np.array(c, dtype=np.float32) for c in ramp)
    low = dark + (mid - dark) * np.clip(t * 2, 0, 1)
    high = mid + (light - mid) * np.clip(t * 2 - 1, 0, 1)
    return np.where(t < 0.5, low, high)


def level_rows(albedo: np.ndarray, window: int = 48) -> np.ndarray:
    lum = luminance(albedo)
    rows = lum.mean(axis=1)
    pad = np.concatenate([rows[-window:], rows, rows[:window]])
    kernel = np.ones(window * 2 + 1) / (window * 2 + 1)
    smooth = np.convolve(pad, kernel, mode="same")[window:-window]
    gain = (rows.mean() / np.maximum(smooth, 1.0))[:, None, None]
    return albedo * gain


def fix(folder: Path, slot: str) -> str:
    ap, np_ = folder / f"{slot}_albedo.png", folder / f"{slot}_normal.png"
    albedo = np.asarray(Image.open(ap).convert("RGB"), dtype=np.float32)
    rows, cols = find_seams(albedo)
    albedo = heal(heal(albedo, rows, 0), cols, 1)
    notes = [f"seams rows {len(rows)} cols {len(cols)}"]
    if np_.exists() and (len(rows) or len(cols)):
        normal = np.asarray(Image.open(np_).convert("RGB"), dtype=np.float32)
        normal = heal(heal(normal, rows, 0), cols, 1)
        Image.fromarray(np.clip(normal, 0, 255).astype(np.uint8)).save(np_, optimize=True)
    if slot in RAMPS:
        albedo = gradient_map(albedo, RAMPS[slot])
        notes.append("recoloured")
    if slot in LEVEL:
        albedo = level_rows(albedo)
        notes.append("rows levelled")
    Image.fromarray(np.clip(albedo, 0, 255).astype(np.uint8)).save(ap, optimize=True)
    return f"{slot}: " + ", ".join(notes)


def derive(folder: Path, slot: str, source: str, gain) -> str:
    albedo = np.asarray(Image.open(folder / f"{source}_albedo.png").convert("RGB"), dtype=np.float32)
    Image.fromarray(np.clip(albedo * np.array(gain, dtype=np.float32), 0, 255).astype(np.uint8)).save(folder / f"{slot}_albedo.png", optimize=True)
    Image.open(folder / f"{source}_normal.png").save(folder / f"{slot}_normal.png", optimize=True)
    return f"{slot}: from {source} x {gain}"


def main() -> None:
    folder = Path(sys.argv[1])
    slots = sorted(p.name[: -len("_albedo.png")] for p in folder.glob("*_albedo.png"))
    for slot in slots:
        if slot not in DERIVED:
            print(fix(folder, slot))
    for slot, (source, gain) in DERIVED.items():
        print(derive(folder, slot, source, gain))


if __name__ == "__main__":
    main()
