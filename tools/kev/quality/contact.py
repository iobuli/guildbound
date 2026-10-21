"""Orthographic contact sheet of hands and feet, before and after the no-glitch pass.

    python3 -I -m quality contact attack1h01_r out.png

Red is the grabbed sample, green the cleaned pose. Each cell is a side view: forward to the right,
up to the top. A blue stub is the palm normal so a backward palm is visible even when the fingers
are short.
"""
import importlib.util
import json
import os

import numpy as np

try:
    from PIL import Image, ImageDraw
except ImportError:  # python -I hides the user site; Pillow lives there in this environment
    import site
    site.addsitedir(site.getusersitepackages())
    from PIL import Image, ImageDraw

import quality.core as quality

KEV = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPO = os.path.dirname(os.path.dirname(KEV))
REFDIR = os.path.join(REPO, "staging", "kev", "refs")

FOOT = (("hip", "knee"), ("knee", "ankle"), ("ankle", "ball"), ("ball", "toe_tip"), ("ankle", "heel"))
HAND = (("elbow", "wrist"), ("wrist", "knuckle"))
FINGERS = ("thumb", "index", "middle", "ring", "pinky")


def _chain(frame, side, links):
    segs = []
    for a, b in links:
        ka, kb = f"{a}_{side}", f"{b}_{side}"
        if ka in frame and kb in frame:
            segs.append((np.array(frame[ka], float), np.array(frame[kb], float)))
    return segs


def _fingers(hands, side):
    segs = []
    if not hands:
        return segs
    hand = hands.get(f"hand_{side}")
    prev0 = np.array(hand, float) if hand is not None else None
    for f in FINGERS:
        prev = prev0
        i = 1
        while f"{f}_{i:02d}_{side}" in hands:
            cur = np.array(hands[f"{f}_{i:02d}_{side}"], float)
            if prev is not None:
                segs.append((prev, cur))
            prev = cur
            i += 1
    return segs


def _palm(hands, side):
    if not hands:
        return None
    need = [f"hand_{side}", f"middle_01_{side}", f"index_01_{side}", f"pinky_01_{side}"]
    if not all(n in hands for n in need):
        return None
    pts = [np.array(hands[n], float) for n in need]
    nrm = quality.palm_normal(side, *pts)
    if nrm is None:
        return None
    origin = pts[0]
    return origin, origin + nrm * 0.08, nrm


def _box_project(pts, right, up, box, floor):
    xs = np.array([float(p @ right) for p in pts])
    ys = np.array([float(p @ up) for p in pts])
    lo_x, hi_x = float(xs.min()), float(xs.max())
    lo_y, hi_y = float(ys.min()), float(ys.max())
    if floor is not None:
        lo_y = min(lo_y, float(floor))
        hi_y = max(hi_y, float(floor))
    span_x = max(hi_x - lo_x, 0.08)
    span_y = max(hi_y - lo_y, 0.08)
    x0, y0, x1, y1 = box
    pad = 22
    scale = min((x1 - x0 - 2 * pad) / span_x, (y1 - y0 - 2 * pad) / span_y)
    cx = 0.5 * (lo_x + hi_x)
    cy = 0.5 * (lo_y + hi_y)
    mid_x = 0.5 * (x0 + x1)
    mid_y = 0.5 * (y0 + y1)

    def proj(p):
        return (mid_x + (float(p @ right) - cx) * scale,
                mid_y - (float(p @ up) - cy) * scale)

    floor_y = None if floor is None else mid_y - (float(floor) - cy) * scale
    return proj, floor_y


def _cell(draw, segs, right, up, box, color, floor, extra=(), width=3):
    x0, y0, x1, y1 = box
    draw.rectangle(box, outline=(220, 216, 208))
    pts = [p for seg in segs for p in seg]
    pts += [p for seg in extra for p in seg]
    if not pts:
        draw.text((x0 + 8, y0 + 24), "no joints", fill=(140, 140, 140))
        return
    proj, floor_y = _box_project(pts, right, up, box, floor)
    if floor_y is not None:
        draw.line((x0 + 10, floor_y, x1 - 10, floor_y), fill=(180, 170, 150), width=1)
    for a, b in segs:
        ax, ay = proj(a)
        bx, by = proj(b)
        draw.line((ax, ay, bx, by), fill=color, width=width)
        draw.ellipse((bx - 3, by - 3, bx + 3, by + 3), fill=color)
    for a, b in extra:
        ax, ay = proj(a)
        bx, by = proj(b)
        draw.line((ax, ay, bx, by), fill=(40, 90, 180), width=2)
        draw.ellipse((bx - 2, by - 2, bx + 2, by + 2), fill=(40, 90, 180))


def _striking_side(ref):
    samples = ref["samples"]
    best, side = -1.0, "r"
    for s in "lr":
        key = f"wrist_{s}"
        if key not in samples[0]["landmarks"]:
            continue
        pts = np.array([fr["landmarks"][key] for fr in samples], float)
        travel = float(np.max(np.linalg.norm(pts - pts[0], axis=1)))
        if travel > best:
            best, side = travel, s
    return side


def _frames(ref, n, side):
    T = len(ref["samples"])
    if T <= n:
        return list(range(T))
    idxs = [int(round(x)) for x in np.linspace(0, T - 1, n)]
    key = f"wrist_{side}"
    if key not in ref["samples"][0]["landmarks"]:
        return idxs
    pts = np.array([fr["landmarks"][key] for fr in ref["samples"]], float)
    speed = np.linalg.norm(np.diff(pts, axis=0), axis=1)
    peak = int(np.argmax(speed))
    nearest = min(range(len(idxs)), key=lambda i: abs(idxs[i] - peak))
    idxs[nearest] = peak
    return sorted(set(idxs))


def render(raw, cleaned, body, out, side="r", frames=6):
    """Side-view feet and hands. `raw` and `cleaned` are reference dicts."""
    up = np.array(body["up"], float)
    forward = np.array(body["forward"], float)
    forward = forward - up * float(forward @ up)
    nrm = float(np.linalg.norm(forward)) or 1.0
    forward = forward / nrm
    cols = frames
    cw, ch, head = 220, 240, 36
    rows = 4
    img = Image.new("RGB", (cw * cols + 110, ch * rows + head), (248, 246, 242))
    draw = ImageDraw.Draw(img)
    title = f"{raw.get('name') or raw.get('clip') or 'clip'}  side {side}  red raw / green cleaned"
    draw.text((12, 10), title, fill=(20, 20, 20))
    labels = ("feet before", "feet after", "hand before", "hand after")
    T = len(raw["samples"])
    idxs = _frames(raw, cols, side)
    for r, label in enumerate(labels):
        draw.text((8, head + r * ch + 8), label, fill=(40, 40, 40))
    for c, t in enumerate(idxs):
        t = min(t, T - 1, len(cleaned["samples"]) - 1)
        raw_s, cln_s = raw["samples"][t], cleaned["samples"][t]
        other = "l" if side == "r" else "r"
        for row, sample, color in ((0, raw_s, (180, 60, 60)), (1, cln_s, (24, 120, 72))):
            segs = _chain(sample["landmarks"], side, FOOT) + _chain(sample["landmarks"], other, FOOT)
            box = (100 + c * cw, head + row * ch, 100 + (c + 1) * cw - 8, head + (row + 1) * ch - 8)
            _cell(draw, segs, forward, up, box, color, body.get("floor"))
            draw.text((box[0] + 6, box[1] + 6), f"f{t}", fill=(80, 80, 80))
        for row, sample, color in ((2, raw_s, (180, 60, 60)), (3, cln_s, (24, 120, 72))):
            hands = sample.get("hands") or {}
            segs = _chain(sample["landmarks"], side, HAND) + _fingers(hands, side)
            palm = _palm(hands, side)
            extra = [(palm[0], palm[1])] if palm else ()
            box = (100 + c * cw, head + row * ch, 100 + (c + 1) * cw - 8, head + (row + 1) * ch - 8)
            _cell(draw, segs, forward, up, box, color, None, extra)
            draw.text((box[0] + 6, box[1] + 6), f"f{t}", fill=(80, 80, 80))
            if palm:
                nf = float(palm[2] @ forward)
                draw.text((box[0] + 6, box[3] - 16), f"palm.f {nf:+.2f}", fill=(40, 70, 130))
    os.makedirs(os.path.dirname(os.path.abspath(out)) or ".", exist_ok=True)
    img.save(out)
    return out


def _studio():
    studio = os.path.join(KEV, "studio", "server.py")
    spec = importlib.util.spec_from_file_location("kev_studio_server_contact", studio)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def draw(name, out, side="auto"):
    raw_path = os.path.join(REFDIR, name + "__raw.json")
    cln_path = os.path.join(REFDIR, name + ".json")
    if os.path.isfile(raw_path):
        raw = json.load(open(raw_path))
        cleaned = json.load(open(cln_path)) if os.path.isfile(cln_path) else None
    elif os.path.isfile(cln_path):
        raw = json.load(open(cln_path))
        cleaned = None
    else:
        raise SystemExit(f"no reference at {raw_path}. Run catalog.py rebuild first.")
    info = _studio().model_info(raw["model"])
    body = info["bm"]["body"]
    if cleaned is None:
        names, L, hands = quality.ref_arrays(raw)
        lengths = quality.lengths_from_body(info["bm"])
        S, hands2, _locked = quality.clean(L, names, 60.0, raw["floor"], raw["height"], hands, body, lengths)
        cleaned = quality.apply_tracks(raw, names, S, hands2)
    if side == "auto":
        side = _striking_side(raw)
    return render(raw, cleaned, body, out, side=side)
