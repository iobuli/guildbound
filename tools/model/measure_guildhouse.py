#!/usr/bin/env python3
"""measure_guildhouse.py - read the 1 m grids in the guildhouse art and draft guildhouse.json.

No vision model involved: the grid is a regular ruler, so it is found by autocorrelating the
line-response profile, and the building is found by masking what differs from the flat background.
Everything is reported with its confidence, and each measurement is drawn back onto the image as
an overlay so a human can confirm it in one glance.

    ../venv/bin/python tools/model/measure_guildhouse.py --art ../../guild_raw --out ../../guild_measure

Views measured: G2 front, G3 side, G4 ground plan, I1 upper plan, I2-I5 hall walls, I9 stairs, I10
furniture. G1/G7/I6-I8/I10-ish are perspective views and carry no ruler, so they are skipped.
"""
from __future__ import annotations

import argparse
import json
import os
import sys

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage

VIEWS = [("G2_front_elevation.png", "elevation", 16, 9), ("G3_side_elevation.png", "elevation", 16, 9),
         ("G4_ground_plan.png", "plan", 1, 1), ("I1_upper_plan.png", "plan", 1, 1),
         ("I2_wall_front.png", "elevation", 16, 9), ("I3_wall_back.png", "elevation", 16, 9),
         ("I4_wall_left.png", "elevation", 16, 9), ("I5_wall_right.png", "elevation", 16, 9),
         ("I9_stairs_tower.png", "elevation", 16, 9), ("I10_furniture.png", "elevation", 16, 9)]


def load(path):
    return np.asarray(Image.open(path).convert("RGB"), np.float32) / 255.0


def line_pitch(g, axis, lo, hi):
    """Grid pitch in px along `axis`, from the autocorrelation of the line-response profile."""
    d = np.abs(np.diff(g, axis=axis))
    prof = d.mean(1 if axis == 0 else 0)
    prof = prof - prof.mean()
    ac = np.correlate(prof, prof, "full")[len(prof) - 1:]
    ac /= max(ac[0], 1e-9)
    band = np.arange(lo, min(hi, len(ac) - 1))
    if len(band) == 0:
        return 0.0, 0.0
    k = band[int(np.argmax(ac[band]))]
    # Autocorrelation peaks just as hard at 2x the period, which halves the metre scale. If half the
    # winning lag is also a strong peak, that is the fundamental and it wins.
    half = k / 2.0
    if half >= lo:
        hk = int(round(half))
        if lo <= hk <= hi and ac[hk] >= 0.80 * ac[k]:
            k = hk
    peak = float(ac[k])
    # parabolic refinement, then accept only a real periodic signal
    if 0 < k < len(ac) - 1:
        denom = ac[k - 1] - 2 * ac[k] + ac[k + 1]
        off = 0.5 * (ac[k - 1] - ac[k + 1]) / denom if abs(denom) > 1e-9 else 0.0
        k = k + off
    return float(k), peak


def structure_mask(a, tol=0.10):
    """What differs from the flat background, with thin grid lines eroded away."""
    h, w, _ = a.shape
    border = np.concatenate([a[:6].reshape(-1, 3), a[-6:].reshape(-1, 3),
                             a[:, :6].reshape(-1, 3), a[:, -6:].reshape(-1, 3)])
    bg = np.median(border, axis=0)
    diff = np.abs(a - bg[None, None, :]).max(-1)
    m = diff > tol
    m = ndimage.binary_erosion(m, iterations=2)
    m = ndimage.binary_dilation(m, iterations=3)
    m = ndimage.binary_fill_holes(m)
    lab, n = ndimage.label(m)
    if n == 0:
        return m, None
    sizes = ndimage.sum(m, lab, range(1, n + 1))
    keep = np.argmax(sizes) + 1
    return lab == keep, bg


def measure(g, kind, a):
    h, w = g.shape
    px_x, cx = line_pitch(g, 0, 18, min(h // 3, 260))
    px_y, cy = line_pitch(g, 1, 18, min(w // 3, 260))
    m, bg = structure_mask(a)
    if m is None or m.sum() < 500:
        return None
    ys, xs = np.where(m)
    box = dict(x0=int(xs.min()), x1=int(xs.max()), y0=int(ys.min()), y1=int(ys.max()))
    out = {"size_px": [w, h], "grid_pitch_px": {"x": round(px_x, 2), "y": round(px_y, 2)},
           "grid_confidence": {"x": round(cx, 3), "y": round(cy, 3)},
           "support_x0": round(float(cx), 3), "support_y0": round(float(cy), 3),
           "structure_box_px": box}
    if cx > 0.25 and cy > 0.25:
        out["px_per_m"] = round((px_x + px_y) / 2, 3)
        out["structure_m"] = {"width": round((box["x1"] - box["x0"]) / px_x, 2),
                              "height": round((box["y1"] - box["y0"]) / px_y, 2),
                              "width_px_per_m": round(px_x, 2), "height_px_per_m": round(px_y, 2)}
        if kind == "elevation":
            # top edge per 10 columns, in metres above the ground line, so eaves/ridge/tower read out
            tops = []
            step = max(1, (box["x1"] - box["x0"]) // 20)
            for x in range(box["x0"], box["x1"] + 1, step):
                col = np.where(m[:, x])[0]
                if len(col):
                    tops.append(round((box["y1"] - int(col.min())) / px_y, 2))
            out["top_edge_m"] = tops
            out["ground_line_y"] = box["y1"]
        else:
            widths = []
            step = max(1, (box["y1"] - box["y0"]) // 20)
            for y in range(box["y0"], box["y1"] + 1, step):
                row = np.where(m[y, :])[0]
                if len(row):
                    widths.append(round((row.max() - row.min()) / px_x, 2))
            out["width_profile_m"] = widths
    return out


def overlay(path, res, out_path):
    im = Image.open(path).convert("RGB")
    d = ImageDraw.Draw(im)
    m, bg = structure_mask(load(path))
    b = res["structure_box_px"]
    d.rectangle([b["x0"], b["y0"], b["x1"], b["y1"]], outline=(255, 92, 122), width=4)
    if res.get("px_per_m"):
        p = res["grid_pitch_px"]
        x = b["x0"]
        while x < im.width:                        # redraw the detected ruler
            d.line([x, 0, x, im.height], fill=(46, 230, 197), width=1)
            x += p["x"]
        y = b["y1"]
        while y > 0:
            d.line([0, y, im.width, y], fill=(46, 230, 197), width=1)
            y -= p["y"]
    im.thumbnail((1400, 1400))
    im.save(out_path, "JPEG", quality=88)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--art", default="../../guild_raw")
    ap.add_argument("--out", default="../../guild_measure")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    doc = {"_draft": True,
           "_warning": ("Measured from the drawing, not surveyed: confirm against the overlays in "
                        "this folder before building. Values are metres, derived from the detected "
                        "1 m grid pitch."),
           "_method": ("grid pitch from autocorrelation of the line-response profile; structure from "
                       "the mask of what differs from the flat background, thin grid lines eroded away"),
           "views": {}}
    report = []
    for name, kind, _, _ in VIEWS:
        p = os.path.join(args.art, name)
        if not os.path.isfile(p):
            report.append(f"{name}: missing")
            continue
        a = load(p)
        res = measure(a.mean(2), kind, a)
        if res is None:
            report.append(f"{name}: no structure found")
            continue
        doc["views"][name[:-4]] = res
        overlay(p, res, os.path.join(args.out, name[:-4] + "_measured.jpg"))
        if res.get("px_per_m"):
            s = res["structure_m"]
            report.append(f"{name[:-4]:22s} grid {res['grid_pitch_px']['x']:.1f}/{res['grid_pitch_px']['y']:.1f} px/m "
                          f"(support {res['support_x0']:.2f}/{res['support_y0']:.2f}) | structure "
                          f"{s['width']:.2f} x {s['height']:.2f} m")
        else:
            report.append(f"{name[:-4]:22s} grid NOT detected (support {res['support_x0']:.2f}/"
                          f"{res['support_y0']:.2f}) - ruler missing or too faint to measure against")
    with open(os.path.join(args.out, "guildhouse.json"), "w") as fh:
        json.dump(doc, fh, indent=2)
    with open(os.path.join(args.out, "measure_report.txt"), "w") as fh:
        fh.write("\n".join(report) + "\n")
    print("\n".join(report))
    print(f"wrote {args.out}/guildhouse.json and {len(doc['views'])} overlays")


if __name__ == "__main__":
    main()
