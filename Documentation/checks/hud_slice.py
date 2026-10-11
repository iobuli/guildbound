"""Measured slicer for a candidate HUD sheet (no fake PASS, no upscaling).
python -I Documentation/checks/hud_slice.py <sheet.png> [--out <dir>]
1. find opaque cores (alpha >= 128), 2. assign each to a manifest cell by 1-D k-means of its real row/column
position (initialised from the manifest grid scaled to the sheet), 3. merge fragments per cell,
4. crop with a glow margin masked against neighbouring cells, 5. write per-sprite PNGs, a measured manifest,
and a canonical 2048 sheet with each sprite pasted at NATIVE size centred in its 256 px cell (scale 1.0).
Statuses: ok | empty_expected | missing | unexpected | fragmented(merged n) | not_white_mask."""
import json, os, sys
import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
man = json.load(open(os.path.join(ROOT, "Documentation", "concepts", "hud", "hud_sheet_manifest.json"), encoding="utf-8"))
sheet = sys.argv[1]
out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else os.path.join(os.path.dirname(sheet), "qa", "slices")
os.makedirs(out, exist_ok=True)
im = Image.open(sheet).convert("RGBA")
arr = np.array(im)
H, W = arr.shape[:2]
a = arr[:, :, 3]
G = man["sheet"]["grid"][0]
pitch = W / G


def components(mask):
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


lab, n = components(a >= 128)
comps = []
for i in range(1, n + 1):
    ys, xs = np.nonzero(lab == i)
    if len(ys) < 40:
        continue
    comps.append({"box": [int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1], "area": int(len(ys)),
                  "cx": float(xs.mean()), "cy": float(ys.mean())})


def merge_fragments(cs, pad=8):
    """Merge components whose boxes (expanded by pad) touch, so one sprite = one unit before cell assignment."""
    parent = list(range(len(cs)))
    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]; x = parent[x]
        return x
    for i in range(len(cs)):
        for j in range(i + 1, len(cs)):
            a_, b_ = cs[i]["box"], cs[j]["box"]
            if a_[0] - pad < b_[2] and b_[0] - pad < a_[2] and a_[1] - pad < b_[3] and b_[1] - pad < a_[3]:
                parent[find(i)] = find(j)
    groups = {}
    for i, c in enumerate(cs):
        groups.setdefault(find(i), []).append(c)
    out = []
    for g in groups.values():
        box = [min(c["box"][0] for c in g), min(c["box"][1] for c in g), max(c["box"][2] for c in g), max(c["box"][3] for c in g)]
        tot = sum(c["area"] for c in g)
        out.append({"box": box, "area": tot, "cx": sum(c["cx"] * c["area"] for c in g) / tot,
                    "cy": sum(c["cy"] * c["area"] for c in g) / tot, "fragments": len(g)})
    return out


expected_sprites = sum(1 for c in man["cells"] if not (c["layer"] == "reserved" or c["id"].startswith(("reserved_", "empty_", "icon_element_reserved"))))
chosen_pad = None
for pad_ in (8,):  # wider pads fuse neighbouring real sprites (tested); assignment is made robust below instead
    merged = merge_fragments(comps, pad_)
    chosen_pad = pad_
    if len(merged) <= expected_sprites:
        break
comps = merged


def kmeans1d(vals, init, iters=8):
    c = np.array(init, float)
    for _ in range(iters):
        idx = np.abs(np.array(vals)[:, None] - c[None, :]).argmin(1)
        for k in range(len(c)):
            sel = np.array(vals)[idx == k]
            if len(sel): c[k] = sel.mean()
    return c, np.abs(np.array(vals)[:, None] - c[None, :]).argmin(1)


# initialise centres: first row/col from the data's own extent, spaced by the scaled pitch
# centres are fitted on the LARGE sprites only, then every piece (including edge strips) is assigned to its nearest centre
big = [c for c in comps if c["area"] >= 800] or comps
y0 = min(c["cy"] for c in big); x0 = min(c["cx"] for c in big)
rows_c, _ = kmeans1d([c["cy"] for c in big], [y0 + pitch * k for k in range(G)])
cols_c, _ = kmeans1d([c["cx"] for c in big], [x0 + pitch * k for k in range(G)])
ri = np.abs(np.array([c["cy"] for c in comps])[:, None] - rows_c[None, :]).argmin(1)
ci = np.abs(np.array([c["cx"] for c in comps])[:, None] - cols_c[None, :]).argmin(1)
cells = {}
for c, r, k in zip(comps, ri, ci):
    cells.setdefault((int(r), int(k)), []).append(c)

report = {"sheet": os.path.basename(sheet), "size_px": [W, H], "components": len(comps), "merge_pad_px": chosen_pad, "expected_sprites": expected_sprites,
          "row_centres_px": [round(float(v), 1) for v in rows_c], "col_centres_px": [round(float(v), 1) for v in cols_c], "cells": []}
union = {}
for key, cs in cells.items():
    union[key] = [min(c["box"][0] for c in cs), min(c["box"][1] for c in cs), max(c["box"][2] for c in cs), max(c["box"][3] for c in cs)]

canon = Image.new("RGBA", tuple(man["sheet"]["size_px"]), (0, 0, 0, 0))
cp = man["sheet"]["cell_px"]
MARGIN = 14
for cell in man["cells"]:
    r, k = cell["row"], cell["col"]
    entry = {"id": cell["id"], "row": r, "col": k, "layer": cell["layer"], "status": None, "notes": []}
    reserved = cell["layer"] == "reserved" or cell["id"].startswith(("reserved_", "empty_", "icon_element_reserved"))
    cs = cells.get((r, k), [])
    if not cs:
        entry["status"] = "empty_expected" if reserved else "missing"
        report["cells"].append(entry); continue
    if reserved:
        entry["notes"].append("expected empty but content found")
    ux0, uy0, ux1, uy1 = union[(r, k)]
    x0_, y0_ = max(0, ux0 - MARGIN), max(0, uy0 - MARGIN)
    x1_, y1_ = min(W, ux1 + MARGIN), min(H, uy1 + MARGIN)
    soft = cell["id"].startswith("fx_") or cell["id"] in ("glow_blob", "sparkle_accent")
    if soft:
        # soft-alpha sprites: extent from a very low alpha threshold inside this cell's own territory
        # (midway to the neighbouring cell centres), so feathered edges are kept, not clipped at a core box.
        def mid(arr_, i, lo, hi):
            a0 = lo if i == 0 else (arr_[i - 1] + arr_[i]) / 2
            a1 = hi if i == G - 1 else (arr_[i] + arr_[i + 1]) / 2
            return int(a0), int(a1)
        tx0, tx1 = mid(cols_c, k, 0, W)
        ty0, ty1 = mid(rows_c, r, 0, H)
        sub = a[ty0:ty1, tx0:tx1] >= 4
        for (kr, kc), ub in union.items():       # never claim pixels inside another cell's own sprite box
            if (kr, kc) == (r, k): continue
            bx0, by0, bx1, by1 = ub
            ix0, iy0 = max(bx0, tx0), max(by0, ty0)
            ix1, iy1 = min(bx1, tx1), min(by1, ty1)
            if ix0 < ix1 and iy0 < iy1:
                sub[iy0 - ty0:iy1 - ty0, ix0 - tx0:ix1 - tx0] = False
        if sub.any():
            ys_, xs_ = np.nonzero(sub)
            x0_, x1_ = tx0 + int(xs_.min()), tx0 + int(xs_.max()) + 1
            y0_, y1_ = ty0 + int(ys_.min()), ty0 + int(ys_.max()) + 1
            entry["notes"].append("soft sprite: extent from alpha>=4 inside territory")
    crop = arr[y0_:y1_, x0_:x1_].copy()
    for (kr, kc), ub in union.items():           # erase neighbours' cores that fall inside our margin box
        if soft or (kr, kc) == (r, k): continue
        bx0, by0, bx1, by1 = ub
        ix0, iy0, ix1, iy1 = max(bx0, x0_), max(by0, y0_), min(bx1, x1_), min(by1, y1_)
        if ix0 < ix1 and iy0 < iy1:
            crop[iy0 - y0_:iy1 - y0_, ix0 - x0_:ix1 - x0_, 3] = 0
    h, w = crop.shape[:2]
    frags = sum(c.get("fragments", 1) for c in cs)
    entry.update({"src_rect_px": [int(x0_), int(y0_), int(w), int(h)], "core_rect_px": [ux0, uy0, ux1 - ux0, uy1 - uy0],
                  "fragments": frags, "sprites_in_cell": len(cs)})
    fit = w <= cp and h <= cp
    entry["fits_256_native"] = bool(fit)
    entry["status"] = "unexpected" if reserved else "ok"
    if len(cs) > 1:
        entry["notes"].append("%d separate sprites landed in one cell (check spacing)" % len(cs))
        entry["status"] = "multi_sprite"
    elif frags > 1:
        entry["notes"].append("fragmented: merged %d components" % frags)
    if not fit:
        entry["notes"].append("larger than 256 px cell at native size (%dx%d): needs a larger cell or a regenerate" % (w, h))
        entry["status"] = "oversize"
    if cell.get("tintable_white_mask"):
        op = crop[:, :, 3] >= 200
        white = float(((crop[:, :, :3].min(axis=2) >= 200) & op).sum() / max(1, op.sum()))
        entry["white_mask_frac"] = round(white, 2)
        if white < 0.8:
            entry["notes"].append("tintable cell is not a white-on-alpha mask (white %.0f%%)" % (100 * white))
            if entry["status"] == "ok": entry["status"] = "not_white_mask"
    Image.fromarray(crop).save(os.path.join(out, cell["id"] + ".png"))
    if fit:
        ox = k * cp + (cp - w) // 2
        oy = r * cp + (cp - h) // 2
        canon.alpha_composite(Image.fromarray(crop), (ox, oy))
        entry["export_rect_px"] = [ox, oy, w, h]
        entry["export_scale"] = 1.0
    report["cells"].append(entry)

canon.save(os.path.join(out, "_canonical_2048.png"))
tally = {}
for e in report["cells"]:
    tally[e["status"]] = tally.get(e["status"], 0) + 1
report["tally"] = tally
report["verdict"] = ("CANDIDATE ASSEMBLED, NOT PRODUCTION: native %dx%d source, export is mechanical at scale 1.0 (no resampling). "
                     "Cells needing attention: %s" % (W, H, sorted(k for k in tally if k not in ("ok", "empty_expected"))))
json.dump(report, open(os.path.join(out, "_measured_manifest.json"), "w", encoding="utf-8"), indent=1)
sys.stdout.reconfigure(encoding="utf-8")
print(json.dumps({k: report[k] for k in ("sheet", "size_px", "components", "tally", "verdict")}, indent=1))
for e in report["cells"]:
    if e["status"] not in ("ok", "empty_expected"):
        print(e["id"], e["status"], e.get("notes"))
