"""Measure a candidate HUD sheet against hud_sheet_manifest.json. Never reports PASS from dimensions alone.
python -I Documentation/checks/hud_measure.py <sheet.png> [--out <dir>]
Reports: size/mode, alpha statistics, haze fraction, core components at several alpha thresholds,
grid-alignment of component centres against the manifest pitch scaled to the real sheet size."""
import json, os, sys
import numpy as np
from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
MAN = os.path.join(ROOT, "Documentation", "concepts", "hud", "hud_sheet_manifest.json")

sheet = sys.argv[1]
out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else os.path.join(os.path.dirname(sheet), "qa")
os.makedirs(out, exist_ok=True)
man = json.load(open(MAN, encoding="utf-8"))
want = tuple(man["sheet"]["size_px"])
im = Image.open(sheet).convert("RGBA")
a = np.array(im)[:, :, 3]
H, W = a.shape
rep = {"file": os.path.basename(sheet), "size_px": [W, H], "wanted_px": list(want), "size_ok": (W, H) == want}
rep["alpha"] = {"transparent_frac": float((a == 0).mean()), "opaque_frac": float((a >= 250).mean()),
                "partial_frac": float(((a > 0) & (a < 250)).mean())}


def label(mask):
    """4-connected components without scipy; returns (labels, count)."""
    try:
        from scipy import ndimage
        lab, n = ndimage.label(mask)
        return lab, n
    except ImportError:
        pass
    lab = np.zeros(mask.shape, np.int32); n = 0
    for y, x in zip(*np.nonzero(mask)):
        if lab[y, x]: continue
        n += 1; st = [(y, x)]; lab[y, x] = n
        while st:
            cy, cx = st.pop()
            for ny, nx in ((cy + 1, cx), (cy - 1, cx), (cy, cx + 1), (cy, cx - 1)):
                if 0 <= ny < mask.shape[0] and 0 <= nx < mask.shape[1] and mask[ny, nx] and not lab[ny, nx]:
                    lab[ny, nx] = n; st.append((ny, nx))
    return lab, n


def boxes(mask, min_area):
    lab, n = label(mask)
    res = []
    for i in range(1, n + 1):
        ys, xs = np.nonzero(lab == i)
        if len(ys) >= min_area:
            res.append((int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1, int(len(ys))))
    return res


rep["components"] = {}
best = None
for thr in (250, 235, 200, 128):
    m = a >= thr
    bx = boxes(m, 300)
    large = [b for b in bx if (b[2] - b[0]) * (b[3] - b[1]) > 0.25 * (W * H) / 64 * 0.25]
    rep["components"][str(thr)] = {"count": len(bx), "giant_merged(>20% sheet)": sum(1 for b in bx if (b[2]-b[0])*(b[3]-b[1]) > 0.2*W*H)}
    if best is None or abs(len(bx) - 64) < abs(len(best[1]) - 64):
        best = (thr, bx)
thr, bx = best
rep["best_threshold"] = thr
rep["best_count"] = len(bx)

# grid alignment: scale manifest pitch to the real sheet
pitch = W / man["sheet"]["grid"][0]
off = []
for x0, y0, x1, y1, area in bx:
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    gx, gy = (cx % pitch) / pitch, (cy % pitch) / pitch
    off.append((abs(gx - 0.5), abs(gy - 0.5)))
rep["grid_pitch_px_scaled"] = round(pitch, 2)
rep["median_centre_offset_from_cell_centre_frac"] = [round(float(np.median([o[0] for o in off])), 3), round(float(np.median([o[1] for o in off])), 3)] if off else None
cross = 0
for x0, y0, x1, y1, area in bx:
    if int(x0 // pitch) != int((x1 - 1) // pitch) or int(y0 // pitch) != int((y1 - 1) // pitch): cross += 1
rep["components_crossing_cell_borders"] = cross
rep["verdict"] = ("MECHANICAL SLICE NOT POSSIBLE: " if (W, H) != want or len(bx) != 64 or cross else "grid slice plausible: ") + \
    "size %s vs %s, %d cores at alpha>=%d (want 64), %d cross cell borders, partial-alpha haze %.0f%% of pixels" % (
        (W, H), want, len(bx), thr, cross, 100 * rep["alpha"]["partial_frac"])

ov = im.copy().convert("RGBA")
bg = Image.new("RGBA", ov.size, (24, 24, 28, 255)); bg.alpha_composite(ov); ov = bg
d = ImageDraw.Draw(ov)
for k in range(1, man["sheet"]["grid"][0]):
    d.line([(k * pitch, 0), (k * pitch, H)], fill=(255, 80, 80, 255), width=1)
    d.line([(0, k * pitch), (W, k * pitch)], fill=(255, 80, 80, 255), width=1)
for x0, y0, x1, y1, _ in bx:
    d.rectangle([x0, y0, x1, y1], outline=(80, 255, 120, 255), width=2)
ov.convert("RGB").save(os.path.join(out, "hud_measure_overlay.png"))
json.dump(rep, open(os.path.join(out, "hud_measure_report.json"), "w", encoding="utf-8"), indent=1)
sys.stdout.reconfigure(encoding="utf-8")
print(json.dumps(rep, indent=1))
