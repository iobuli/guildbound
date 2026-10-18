#!/usr/bin/env python3
"""calibrate_guildhouse.py - measure the guildhouse views against their LABELLED marks, not the grid.

The grids are not a reliable metre (they are uneven, and in G4/I1 a floor tile is about 0.64 m, not
1 m), so the ruler used here is the drawing's own called-out dimension: the 16 m podium width on the
front elevation and the plan, and the podium depth plus steps on the side elevation. That gives
px-per-metre from the drawing itself, and the grid pitch is then reported as a *derived* cell size so
the "how many metres is one cell" question is answered rather than assumed.

    ../venv/bin/python tools/model/calibrate_guildhouse.py --art ../../guild_raw --out ../../guild_measure

Each reading is compared with agent-1's hand reading; anything more than 10% out is flagged.
"""
from __future__ import annotations

import argparse
import json
import os
import sys

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from measure_guildhouse import line_pitch, structure_mask, load  # noqa: E402

# view -> (labelled anchor in metres, agent-1's reading of the same dimension, what the anchor is)
ANCHORS = {
    "G2_front_elevation": (16.0, 16.0, "podium width"),
    "G3_side_elevation": (15.9, 13.4, "podium depth 13.4 + steps 2.5 = 15.9"),
    "G4_ground_plan": (16.0, 16.0, "podium width"),
    "I1_upper_plan": (16.0, 16.0, "podium width"),
}
# dimensions to cross-check per view: name -> (agent-1's metres, axis to measure, how)
# Only overall extents can be measured from the structure mask without reading internal features, so
# the checks are box dimensions against agent-1's arithmetic rather than single parts like the door.
CHECKS = {
    "G2_front_elevation": [("overall height: podium 1.5 + ridge 10.7", 12.2, "y")],
    "G3_side_elevation": [("overall width: podium 13.4 + steps 2.5", 15.9, "x"),
                          ("overall height: podium 1.5 + ridge 10.7", 12.2, "y")],
    "G4_ground_plan": [("podium depth", 13.4, "y")],
    "I1_upper_plan": [("outer footprint depth (upper walls match the ground plan)", 13.4, "y")],
}
TOL = 0.10


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--art", default="../../guild_raw")
    ap.add_argument("--out", default="../../guild_measure")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    rows, doc = [], {"_method": "px/m from the drawing's labelled dimension; grid pitch reported as a derived cell size",
                     "cells": {"elevations": "agent-1: 36.7 px in a 1400 px copy, about 0.9 m by the building width, 0.8 m by the 1.75 m figure",
                               "plans": "agent-1: 46 px tiles, 71.9 px/m at the 16 m mark, so about 0.64 m per tile"}}
    for view, (anchor_m, ref_m, what) in ANCHORS.items():
        p = os.path.join(args.art, view + ".png")
        if not os.path.isfile(p):
            rows.append(f"{view}: missing")
            continue
        a = load(p)
        g = a.mean(2)
        m, _ = structure_mask(a)
        ys, xs = np.where(m)
        w_px, h_px = xs.max() - xs.min(), ys.max() - ys.min()
        ppm = w_px / anchor_m                      # px per metre, from the label
        px_x, sx = line_pitch(g, 0, 18, min(g.shape[0] // 3, 260))
        px_y, sy = line_pitch(g, 1, 18, min(g.shape[1] // 3, 260))
        cell_x, cell_y = px_x / ppm, px_y / ppm    # what one grid cell really spans
        rows.append(f"{view:22s} anchor {what} = {anchor_m} m -> {ppm:.1f} px/m | measured box "
                    f"{w_px / ppm:.2f} x {h_px / ppm:.2f} m | grid cell {px_x:.1f}/{px_y:.1f} px = "
                    f"{cell_x:.2f}/{cell_y:.2f} m (support {sx:.2f}/{sy:.2f})")
        entry = {"px_per_m_from_label": round(ppm, 2), "anchor": what, "measured_box_m": [round(w_px / ppm, 2), round(h_px / ppm, 2)],
                 "grid_cell_px": [round(px_x, 1), round(px_y, 1)], "grid_cell_m": [round(cell_x, 2), round(cell_y, 2)],
                 "grid_support": [round(sx, 3), round(sy, 3)], "checks": []}
        for name, ref, axis in CHECKS.get(view, []):
            span = h_px if axis == "y" else w_px
            got = span / ppm
            delta = (got - ref) / ref
            flag = "OK" if abs(delta) <= TOL else "CHECK THE OVERLAY"
            entry["checks"].append({"dimension": name, "agent1_m": ref, "measured_m": round(got, 2),
                                    "delta_pct": round(100 * delta, 1), "verdict": flag})
            rows.append(f"    {name:26s} agent-1 {ref:5.1f} m | measured {got:5.2f} m | {100 * delta:+6.1f}% {flag}")
        doc[view] = entry
        # overlay: label the calibrated ruler and the anchor
        im = Image.open(p).convert("RGB")
        d = ImageDraw.Draw(im)
        d.rectangle([xs.min(), ys.min(), xs.max(), ys.max()], outline=(255, 92, 122), width=4)
        for i in range(int(w_px / ppm) + 1):       # one line per real metre, from the label
            x = int(xs.min() + i * ppm)
            if x < im.width:
                d.line([x, 0, x, im.height], fill=(46, 230, 197), width=1)
        d.text((xs.min() + 6, max(0, ys.min() - 18)), f"{anchor_m:.0f} m label -> {ppm:.1f} px/m", fill=(255, 92, 122))
        im.thumbnail((1400, 1400))
        im.save(os.path.join(args.out, view + "_calibrated.jpg"), "JPEG", quality=88)
    with open(os.path.join(args.out, "calibration.json"), "w") as fh:
        json.dump(doc, fh, indent=2)
    with open(os.path.join(args.out, "calibration_report.txt"), "w") as fh:
        fh.write("\n".join(rows) + "\n")
    print("\n".join(rows))
    print(f"wrote {args.out}/calibration.json, calibration_report.txt and {len(doc)} calibrated overlays")


if __name__ == "__main__":
    main()
