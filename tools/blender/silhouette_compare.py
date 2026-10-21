import sys, numpy as np
from PIL import Image
chart = np.asarray(Image.open('mask.png')) > 127
s = 1.8315/734; floor = 1083
def run_at(row, cx):
    # the run containing cx (or nearest), return (a,b)
    xs = np.where(row)[0]
    if not len(xs): return None
    runs=[]; a=xs[0]; p=xs[0]
    for x in xs[1:]:
        if x != p+1: runs.append((a,p)); a=x
        p=x
    runs.append((a,p))
    return min(runs, key=lambda r: 0 if r[0]<=cx<=r[1] else min(abs(r[0]-cx),abs(r[1]-cx)))
def ours_mask(path, bg):
    im = np.asarray(Image.open(path).convert('RGBA')).astype(int)
    if bg is None: return im[...,3] > 40
    return np.abs(im[...,:3]-np.array(bg)).sum(2) > 30
view = sys.argv[1]
if view == 'front':
    m = ours_mask(sys.argv[2], (249,230,238)); ccx, ocx = 1290, 350
    cx0, cx1 = 1110, 1490
else:
    m = ours_mask(sys.argv[2], None); ccx, ocx = 1640, 350
    cx0, cx1 = 1540, 1760
ppm = 820/2.4
xs_c = None
for z in [float(a) for a in sys.argv[3].split(',')]:
    cy = int(round(floor - z/s)); oy = int(round(410 - (z-0.95)*ppm))
    off = float(sys.argv[4]) if len(sys.argv)>4 else 0.0  # lateral sample offset (m) e.g. legs
    rc = run_at(chart[cy, cx0:cx1], ccx - cx0 + off/s); ro = run_at(m[oy], ocx + off*ppm)
    if rc is None or ro is None: print(z, 'none'); continue
    wc = (rc[1]-rc[0])*s; wo = (ro[1]-ro[0])/ppm
    print(f"z {z:.2f} chart {wc:.3f} ours {wo:.3f} ratio {wc/max(wo,1e-3):.2f}  centres chart {((rc[0]+rc[1])/2+cx0-ccx)*s:+.3f} ours {((ro[0]+ro[1])/2-ocx)/ppm:+.3f}")
