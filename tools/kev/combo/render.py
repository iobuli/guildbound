"""Orthographic frame strips of a solved combo, so a person can see whether the limbs arc and the feet stay put."""
import os

import numpy as np

BONES = (("pelvis", "hip_l"), ("hip_l", "knee_l"), ("knee_l", "ankle_l"), ("ankle_l", "ball_l"),
         ("pelvis", "hip_r"), ("hip_r", "knee_r"), ("knee_r", "ankle_r"), ("ankle_r", "ball_r"),
         ("pelvis", "neck"), ("neck", "head_joint"), ("head_joint", "crown"), ("head_joint", "nose"),
         ("shoulder_l", "shoulder_r"), ("hip_l", "hip_r"),
         ("neck", "shoulder_l"), ("shoulder_l", "elbow_l"), ("elbow_l", "wrist_l"),
         ("neck", "shoulder_r"), ("shoulder_r", "elbow_r"), ("elbow_r", "wrist_r"))


def _unit(v):
    v = np.asarray(v, float)
    n = float(np.linalg.norm(v))
    return v / n if n > 1e-9 else v


def _axes(info):
    up = _unit(info["bm"]["body"]["up"])
    fwd = _unit(info["bm"]["body"]["forward"])
    left = _unit(np.cross(up, fwd))
    return left, up, fwd


def _frames(solved, samples, n=12):
    idxs = [int(round(i * (len(solved) - 1) / (n - 1))) for i in range(n)]
    hits = [i for i, s in enumerate(samples) if s["tag"].endswith(":hit") and (i == 0 or not samples[i - 1]["tag"].endswith(":hit"))]
    # one frame per hit, inserted in order, de-duplicated
    seen = set()
    out = []
    for i in idxs + hits:
        if i not in seen:
            seen.add(i)
            out.append(i)
    out.sort()
    return out


def _draw_pose(draw, lm, left, up, fwd, origin, scale, view):
    def xy(name):
        p = np.asarray(lm[name], float)
        if view == "side":
            x, y = float(np.dot(p, fwd)), float(np.dot(p, up))
        else:
            x, y = float(np.dot(p, left)), float(np.dot(p, up))
        return origin[0] + x * scale, origin[1] - y * scale

    for a, b in BONES:
        if a not in lm or b not in lm:
            continue
        col = (255, 150, 90) if a.endswith("_r") or b.endswith("_r") else (120, 190, 255)
        if a in ("pelvis", "neck", "head_joint") and not (a.endswith("_l") or b.endswith("_l") or a.endswith("_r")):
            col = (230, 228, 220)
        draw.line([xy(a), xy(b)], fill=col, width=3)
    for name in ("wrist_l", "wrist_r", "ankle_l", "ankle_r", "ball_l", "ball_r"):
        x, y = xy(name)
        r = 3
        draw.ellipse([x - r, y - r, x + r, y + r], fill=(250, 220, 90))


def render_strip(info, samples, solved, path, title):
    """Two rows (side, front) of the combo, plus the hit frames in the same strip."""
    from PIL import Image, ImageDraw
    left, up, fwd = _axes(info)
    idxs = _frames(solved, samples)
    cell_w, cell_h = 150, 210
    head = 28
    img = Image.new("RGB", (cell_w * len(idxs), head + cell_h * 2), (22, 24, 30))
    draw = ImageDraw.Draw(img)
    draw.text((8, 6), title, fill=(220, 220, 220))
    # scale from the character height
    H = float(info["bm"]["body"]["height"])
    scale = (cell_h - 36) / H
    for col, i in enumerate(idxs):
        lm = solved[i]["landmarks"]
        # centre each cell on that frame's pelvis horizontally, feet near the bottom
        pel = np.asarray(lm["pelvis"], float)
        for row, view in enumerate(("side", "front")):
            ox = col * cell_w + cell_w * 0.5 - (float(np.dot(pel, fwd if view == "side" else left)) * scale)
            oy = head + row * cell_h + cell_h - 16
            # floor line
            floor = float(info["bm"]["body"]["floor"])
            draw.line([(col * cell_w + 8, oy - floor * scale), (col * cell_w + cell_w - 8, oy - floor * scale)], fill=(60, 64, 74))
            _draw_pose(draw, lm, left, up, fwd, (ox, oy), scale, view)
        tag = samples[i]["tag"]
        mark = (255, 180, 80) if tag.endswith(":hit") else (160, 160, 160)
        draw.text((col * cell_w + 6, head + 2), f"{samples[i]['t']:.2f}s", fill=mark)
        draw.text((col * cell_w + 6, head + cell_h + 2), tag.split(":")[-1], fill=mark)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    img.save(path)
    return path


def render_sheet(paths, out):
    """Stack the per-combo strips into one contact sheet."""
    from PIL import Image
    ims = [Image.open(p).convert("RGB") for p in paths]
    w = max(i.width for i in ims)
    h = sum(i.height for i in ims) + 8 * (len(ims) - 1)
    sheet = Image.new("RGB", (w, h), (12, 12, 16))
    y = 0
    for im in ims:
        sheet.paste(im, (0, y))
        y += im.height + 8
    os.makedirs(os.path.dirname(out), exist_ok=True)
    sheet.save(out)
    return out
