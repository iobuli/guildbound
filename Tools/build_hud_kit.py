"""HUD backplate/frame kit (the L1/L2/L7 parts behind content), drawn from signed distance fields at exact sizes.
python -I tools/build_hud_kit.py  -> Assets/Game/UI/Kit/Sprites/*.png + kit.json
Sizes are 1080p reference pixels; textures are rendered at SCALE x with pixelsPerUnit 100*SCALE, so Unity shows
them at the reference size with SCALE x detail. 9-slice borders are recorded in texture pixels (L, B, R, T).
Colours and alpha floors: GDD v5.1 AMEND-UI-01 (plate #0B1A3A >= 0.70 alpha, edge #5FD7FF, text #EAF6FF)."""
import json, os
import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
OUT = os.path.join(ROOT, 'Assets', 'Game', 'UI', 'Kit', 'Sprites')
SCALE = 2
PLATE = (0x0B, 0x1A, 0x3A)
PLATE_TOP = (0x13, 0x2A, 0x55)
EDGE = (0x5F, 0xD7, 0xFF)
WHITE = (255, 255, 255)
PLATE_ALPHA = 0.80
KIT = {}


def sd_chamfer_rect(x, y, x0, y0, x1, y1, c):
    cx, cy, hx, hy = (x0 + x1) / 2, (y0 + y1) / 2, (x1 - x0) / 2, (y1 - y0) / 2
    px, py = np.abs(x - cx), np.abs(y - cy)
    return np.maximum(np.maximum(px - hx, py - hy), (px + py - (hx + hy - c)) / np.sqrt(2))


def sd_diamond(x, y, cx, cy, r):
    return (np.abs(x - cx) + np.abs(y - cy) - r) / np.sqrt(2)


def sd_circle(x, y, cx, cy, r):
    return np.hypot(x - cx, y - cy) - r


def sd_segment(x, y, ax, ay, bx, by, w):
    px, py, vx, vy = x - ax, y - ay, bx - ax, by - ay
    t = np.clip((px * vx + py * vy) / (vx * vx + vy * vy), 0, 1)
    return np.hypot(px - vx * t, py - vy * t) - w


def cov(d):
    return np.clip(0.5 - d * SCALE, 0, 1)


def line(d, width):
    return cov(np.abs(d) - width / 2)


def halo(d, radius):
    return np.exp(-np.maximum(d, 0) / radius) * (d > 0)


class Canvas:
    def __init__(self, w, h):
        self.w, self.h = w, h
        y, x = np.mgrid[0:h * SCALE, 0:w * SCALE].astype(np.float32)
        self.x, self.y = (x + 0.5) / SCALE, (y + 0.5) / SCALE
        self.rgb = np.zeros((h * SCALE, w * SCALE, 3), np.float32)
        self.a = np.zeros((h * SCALE, w * SCALE), np.float32)

    def _over(self, alpha, c):
        a = np.clip(alpha, 0, 1)[..., None]
        out = a[..., 0] + self.a * (1 - a[..., 0])
        num = c * a + self.rgb * self.a[..., None] * (1 - a)
        self.rgb = np.where(out[..., None] > 1e-6, num / np.maximum(out[..., None], 1e-6), self.rgb)
        self.a = out

    def paint(self, alpha, color):
        self._over(alpha, np.array(color, np.float32)[None, None, :] / 255)

    def gradient(self, alpha, top, bottom):
        t = (self.y / self.h)[..., None]
        self._over(alpha, (np.array(top, np.float32) * (1 - t) + np.array(bottom, np.float32) * t) / 255)

    def save(self, name):
        img = np.dstack([self.rgb, self.a]) * 255
        Image.fromarray(np.clip(img + 0.5, 0, 255).astype(np.uint8), 'RGBA').save(os.path.join(OUT, name + '.png'))


def record(name, w, h, border=None, kind='Sliced', layer='L1', note=''):
    KIT[name] = {'file': name + '.png', 'ref_size': [w, h], 'tex_size': [w * SCALE, h * SCALE],
                 'border_ref_px_LBRT': border or [0, 0, 0, 0],
                 'border_tex_px_LBRT': [b * SCALE for b in border] if border else [0, 0, 0, 0],
                 'image_type': kind, 'layer': layer, 'note': note}


def panel(name, w=96, h=96, chamfer=12, pad=6, bracket=True, fill_alpha=PLATE_ALPHA, layer='L1'):
    """Glass plate: gradient fill (alpha >= floor), 1.5 px cyan edge, soft outer halo, bright corner brackets."""
    k = Canvas(w, h)
    d = sd_chamfer_rect(k.x, k.y, pad, pad, w - pad, h - pad, chamfer)
    k.paint(halo(d, 2.5) * 0.35, EDGE)
    k.gradient(cov(d) * fill_alpha, PLATE_TOP, PLATE)
    inner = sd_chamfer_rect(k.x, k.y, pad + 3, pad + 3, w - pad - 3, h - pad - 3, max(chamfer - 2, 1))
    k.paint(line(inner, 0.75) * 0.18, EDGE)
    k.paint(line(d, 1.5) * 0.95, EDGE)
    if bracket:
        arm, t = 14, 2.6
        for sx in (1, -1):
            for sy in (1, -1):
                cx = pad if sx == 1 else w - pad
                cy = pad if sy == 1 else h - pad
                # bracket follows the chamfer, then runs arm px along both edges
                ax, ay = cx, cy + sy * chamfer
                bx, by = cx + sx * chamfer, cy
                b = np.minimum(np.minimum(sd_segment(k.x, k.y, ax, ay, bx, by, t / 2),
                                          sd_segment(k.x, k.y, ax, ay, ax, ay + sy * arm, t / 2)),
                               sd_segment(k.x, k.y, bx, by, bx + sx * arm, by, t / 2))
                k.paint(halo(b, 2.0) * 0.45, EDGE)
                k.paint(cov(b), WHITE)
    k.save(name)
    bd = pad + chamfer + (arm + 2 if bracket else 2)
    record(name, w, h, [bd, bd, bd, bd], 'Sliced', layer, 'glass plate; content safe inset %d px' % (pad + 8))


def header(name, w=128, h=40):
    k = Canvas(w, h)
    d = sd_chamfer_rect(k.x, k.y, 2, 4, w - 2, h - 4, 8)
    k.gradient(cov(d) * 0.88, (0x16, 0x33, 0x66), (0x0A, 0x16, 0x32))
    k.paint(line(d, 1.0) * 0.55, EDGE)
    u = np.maximum(np.abs(k.y - (h - 6)) - 0.9, np.abs(k.x - w / 2) - (w / 2 - 10))
    k.paint(halo(u, 3) * 0.45, EDGE)
    k.paint(cov(u), EDGE)
    k.paint(cov(sd_diamond(k.x, k.y, 16, h / 2, 7)) * 0.95, EDGE)
    k.paint(cov(sd_diamond(k.x, k.y, 16, h / 2, 3)), WHITE)
    k.save(name)
    record(name, w, h, [30, 8, 14, 8], 'Sliced', 'L1', 'title strip; text starts at x=34')


def bar_frame(name, w=64, h=24):
    k = Canvas(w, h)
    d = sd_chamfer_rect(k.x, k.y, 2, 3, w - 2, h - 3, 6)
    k.paint(cov(d) * 0.85, (0x05, 0x0C, 0x1C))
    k.paint(line(d, 1.2) * 0.9, EDGE)
    k.save(name)
    record(name, w, h, [10, 8, 10, 8], 'Sliced', 'L2', 'meter slot; fill inset 4 px')


def fill_mask(name, w=64, h=16):
    k = Canvas(w, h)
    k.paint(cov(sd_chamfer_rect(k.x, k.y, 0, 0, w, h, 4)), WHITE)
    k.save(name)
    record(name, w, h, None, 'Filled', 'L2', 'white; tint in code; fillMethod Horizontal')


def button(name, w=160, h=44, active=False):
    k = Canvas(w, h)
    d = sd_chamfer_rect(k.x, k.y, 5, 5, w - 5, h - 5, 9)
    k.paint(halo(d, 3) * (0.55 if active else 0.25), EDGE)
    top, bot = ((0x1E, 0x55, 0x9E), (0x0E, 0x2A, 0x5C)) if active else ((0x14, 0x30, 0x60), (0x0A, 0x18, 0x38))
    k.gradient(cov(d) * 0.92, top, bot)
    k.paint(line(d, 1.5), EDGE)
    hl = np.maximum(np.abs(k.y - 10) - 0.6, np.abs(k.x - w / 2) - (w / 2 - 22))
    k.paint(cov(hl) * 0.35, WHITE)
    k.save(name)
    record(name, w, h, [24, 16, 24, 16], 'Sliced', 'L7', 'uGUI Button; label centred')


def tab(name, w=120, h=36, active=False):
    k = Canvas(w, h)
    d = sd_chamfer_rect(k.x, k.y, 3, 3, w - 3, h - 3, 7)
    if active:
        k.paint(halo(d, 2.5) * 0.5, EDGE)
        k.gradient(cov(d) * 0.95, (0x23, 0x68, 0xB8), (0x10, 0x34, 0x70))
    else:
        k.paint(cov(d) * 0.75, PLATE)
    k.paint(line(d, 1.2) * (1.0 if active else 0.6), EDGE)
    k.save(name)
    record(name, w, h, [14, 12, 14, 12], 'Sliced', 'L7', 'tab; Toggle in a ToggleGroup')


def slider_track(name, w=96, h=12):
    k = Canvas(w, h)
    d = sd_chamfer_rect(k.x, k.y, 2, 3, w - 2, h - 3, 3)
    k.paint(cov(d) * 0.9, (0x05, 0x0C, 0x1C))
    k.paint(line(d, 1.0) * 0.7, EDGE)
    k.save(name)
    record(name, w, h, [6, 5, 6, 5], 'Sliced', 'L7', 'Slider background; fill = fill_mask tinted')


def knob_diamond(name, s=28):
    k = Canvas(s, s)
    d = sd_diamond(k.x, k.y, s / 2, s / 2, s / 2 - 6)
    k.paint(halo(d, 2.5) * 0.7, EDGE)
    k.paint(cov(d), (0x9F, 0xEC, 0xFF))
    k.paint(cov(sd_diamond(k.x, k.y, s / 2, s / 2, s / 2 - 11)), WHITE)
    k.paint(line(d, 1.2), EDGE)
    k.save(name)
    record(name, s, s, None, 'Simple', 'L7', 'Slider handle')


def toggle_track(name, w=64, h=28):
    k = Canvas(w, h)
    r = h / 2 - 3
    d = np.minimum(np.minimum(sd_circle(k.x, k.y, 3 + r, h / 2, r), sd_circle(k.x, k.y, w - 3 - r, h / 2, r)),
                   np.maximum(np.abs(k.y - h / 2) - r, np.abs(k.x - w / 2) - (w / 2 - 3 - r)))
    k.paint(cov(d) * 0.9, (0x05, 0x0C, 0x1C))
    k.paint(line(d, 1.2), EDGE)
    k.save(name)
    record(name, w, h, None, 'Simple', 'L7', 'switch background; knob slides in code')


def disc(name, s=24, layer='L2'):
    k = Canvas(s, s)
    k.paint(cov(sd_circle(k.x, k.y, s / 2, s / 2, s / 2 - 2)), WHITE)
    k.save(name)
    record(name, s, s, None, 'Filled', layer, 'white; tint in code')


def check_box(name, s=28, mark=False):
    k = Canvas(s, s)
    if mark:
        m = np.minimum(sd_segment(k.x, k.y, 8, 14, 12.5, 19, 1.6), sd_segment(k.x, k.y, 12.5, 19, 20.5, 9, 1.6))
        k.paint(halo(m, 2) * 0.6, EDGE)
        k.paint(cov(m), WHITE)
    else:
        d = sd_chamfer_rect(k.x, k.y, 3, 3, s - 3, s - 3, 4)
        k.paint(cov(d) * 0.85, (0x05, 0x0C, 0x1C))
        k.paint(line(d, 1.3), EDGE)
    k.save(name)
    record(name, s, s, None, 'Simple', 'L7', 'Toggle graphic' if mark else 'Toggle background')


def icon_slot(name, s=96, shape='diamond'):
    k = Canvas(s, s)
    if shape == 'diamond':
        d, inner = sd_diamond(k.x, k.y, s / 2, s / 2, s / 2 - 6), sd_diamond(k.x, k.y, s / 2, s / 2, s / 2 - 11)
    else:
        d, inner = sd_circle(k.x, k.y, s / 2, s / 2, s / 2 - 5), sd_circle(k.x, k.y, s / 2, s / 2, s / 2 - 10)
    k.paint(halo(d, 3) * 0.4, EDGE)
    k.gradient(cov(d) * PLATE_ALPHA, PLATE_TOP, PLATE)
    k.paint(line(inner, 0.8) * 0.35, EDGE)
    k.paint(line(d, 1.6), EDGE)
    k.save(name)
    record(name, s, s, None, 'Simple', 'L1', 'icon centred, %d px' % int(s * 2 / 3))


def ring(name, s=96, width=5):
    k = Canvas(s, s)
    k.paint(line(sd_circle(k.x, k.y, s / 2, s / 2, s / 2 - 4 - width / 2), width), WHITE)
    k.save(name)
    record(name, s, s, None, 'Filled', 'L2', 'fillMethod Radial360; tint in code')


def divider(name, w=96, h=12):
    k = Canvas(w, h)
    u = np.maximum(np.abs(k.y - h / 2) - 0.6, np.abs(k.x - w / 2) - (w / 2 - 2))
    k.paint(cov(u) * np.clip(np.minimum(k.x, w - k.x) / 24, 0, 1) * 0.8, EDGE)
    k.save(name)
    record(name, w, h, [26, 0, 26, 0], 'Sliced', 'L1', 'horizontal rule')


def ornament(name, s=24):
    k = Canvas(s, s)
    d = sd_diamond(k.x, k.y, s / 2, s / 2, s / 2 - 4)
    k.paint(halo(d, 2.5) * 0.6, EDGE)
    k.paint(cov(d) * 0.9, PLATE)
    k.paint(line(d, 1.4), EDGE)
    k.paint(cov(sd_diamond(k.x, k.y, s / 2, s / 2, 3.5)), WHITE)
    k.save(name)
    record(name, s, s, None, 'Simple', 'L1', 'centred on frame edges (cards, headers)')


def soft(name, s=128, kind='blob'):
    k = Canvas(s, s)
    r = np.hypot(k.x - s / 2, k.y - s / 2) / (s / 2)
    if kind == 'blob':
        k.paint(np.clip(1 - r, 0, 1) ** 2, WHITE)
        record(name, s, s, None, 'Simple', 'L0', 'glow; tint/alpha in code')
    else:
        k.paint(np.clip((r - 0.55) / 0.6, 0, 1) ** 1.6 * 0.85, (0, 0, 0))
        record(name, s, s, None, 'Simple', 'L0', 'stretch full screen')
    k.save(name)


def accent_frame(name, w=96, h=64):
    k = Canvas(w, h)
    d = sd_chamfer_rect(k.x, k.y, 4, 4, w - 4, h - 4, 8)
    k.paint(halo(d, 2.5) * 0.3, EDGE)
    k.gradient(cov(d) * 0.86, PLATE_TOP, PLATE)
    k.paint(line(d, 1.2), EDGE)
    k.save(name)
    record(name, w, h, [16, 16, 16, 16], 'Sliced', 'L6', 'alert / toast / prompt')


def keycap(name, s=36):
    k = Canvas(s, s)
    d = sd_chamfer_rect(k.x, k.y, 3, 3, s - 3, s - 3, 5)
    k.paint(cov(d) * 0.95, (0x18, 0x26, 0x40))
    k.paint(line(d, 1.3), (0xEA, 0xF6, 0xFF))
    k.save(name)
    record(name, s, s, [10, 10, 10, 10], 'Sliced', 'L5', 'input glyph background; key text in TMP')


def compass_strip(name, w=128, h=64):
    """Heading tape. Sliced: the diamond end caps and the plate are preserved, the middle stretches to any width."""
    k = Canvas(w, h)
    d = sd_chamfer_rect(k.x, k.y, 2, 4, w - 2, h - 4, 10)
    k.paint(halo(d, 3) * 0.3, EDGE)
    k.gradient(cov(d) * PLATE_ALPHA, PLATE_TOP, PLATE)
    k.paint(line(d, 1.2), EDGE)
    base = np.maximum(np.abs(k.y - (h - 14)) - 0.9, np.abs(k.x - w / 2) - (w / 2 - 16))
    k.paint(cov(base) * 0.45, EDGE)
    for cx in (14, w - 14):
        k.paint(halo(sd_diamond(k.x, k.y, cx, h / 2, 12), 2.5) * 0.5, EDGE)
        k.paint(cov(sd_diamond(k.x, k.y, cx, h / 2, 12)) * 0.9, PLATE)
        k.paint(line(sd_diamond(k.x, k.y, cx, h / 2, 12), 1.4), EDGE)
        k.paint(cov(sd_diamond(k.x, k.y, cx, h / 2, 4)), WHITE)
    k.save(name)
    record(name, w, h, [30, 8, 30, 8], 'Sliced', 'L1', 'heading tape behind N/E/S/W text; end caps preserved')


def compass_pointer(name, s=28):
    """Fixed centre marker the heading tape scrolls behind."""
    k = Canvas(s, s)
    d = sd_diamond(k.x, k.y, s / 2, s / 2, s / 2 - 4)
    k.paint(halo(d, 3) * 0.7, EDGE)
    k.paint(cov(d), (0x9F, 0xEC, 0xFF))
    k.paint(cov(sd_diamond(k.x, k.y, s / 2, s / 2, s / 2 - 10)), WHITE)
    k.save(name)
    record(name, s, s, None, 'Simple', 'L3', 'fixed centre marker over the heading tape')


def minimap_frame(name, s=256):
    """Round map frame: outer ring, four cardinal spikes, diagonal notches. The centre stays transparent so the
    map renders through it; put the Blit inside a child Image(disc_mask) + Mask to get a circular window."""
    k = Canvas(s, s)
    c = s / 2
    r = c - 18
    outer = sd_circle(k.x, k.y, c, c, r)
    k.paint(halo(outer, 4) * 0.35, EDGE)
    k.paint(line(outer, 3.0), EDGE)
    k.paint(line(sd_circle(k.x, k.y, c, c, r - 5), 1.0) * 0.45, EDGE)
    for dx, dy in ((0, -1), (1, 0), (0, 1), (-1, 0)):
        ax, ay = c + dx * (r - 6), c + dy * (r - 6)
        bx, by = c + dx * (r + 14), c + dy * (r + 14)
        sp = sd_segment(k.x, k.y, ax, ay, bx, by, 1.2)
        k.paint(halo(sp, 2.5) * 0.5, EDGE)
        k.paint(cov(sp), WHITE)
    for sx in (-1, 1):
        for sy in (-1, 1):
            k.paint(cov(sd_diamond(k.x, k.y, c + sx * (r - 24), c + sy * (r - 24), 5)), EDGE)
    k.save(name)
    record(name, s, s, None, 'Simple', 'L1', 'round map frame; centre transparent so the map shows through')


if __name__ == '__main__':
    os.makedirs(OUT, exist_ok=True)
    panel('panel_frame')
    panel('panel_frame_plain', bracket=False)
    panel('card_frame', w=96, h=128, chamfer=10, fill_alpha=0.86)
    panel('modal_frame', w=128, h=128, chamfer=16)
    header('header_bar')
    bar_frame('bar_frame')
    fill_mask('fill_mask')
    button('button_frame'); button('button_frame_active', active=True)
    tab('tab_idle'); tab('tab_active', active=True)
    slider_track('slider_track'); knob_diamond('slider_knob')
    toggle_track('toggle_track'); disc('toggle_knob', 22, 'L7')
    check_box('checkbox_frame'); check_box('checkbox_mark', mark=True)
    icon_slot('icon_slot_diamond'); icon_slot('icon_slot_circle', shape='circle'); icon_slot('portrait_slot', s=128)
    ring('ring_mask'); disc('disc_mask', 96)
    divider('divider'); ornament('ornament_diamond')
    soft('glow_blob'); soft('vignette', 256, 'vignette')
    accent_frame('toast_frame'); keycap('keycap_frame')
    compass_strip('compass_strip'); compass_pointer('compass_pointer')
    minimap_frame('minimap_frame')
    json.dump({'scale': SCALE, 'pixels_per_unit': 100 * SCALE, 'reference_resolution': [1920, 1080],
               'palette': {'plate': '#0B1A3A', 'edge': '#5FD7FF', 'text': '#EAF6FF', 'warn': '#FFB347', 'critical': '#FF4A5A'},
               'plate_alpha_min': 0.70, 'sprites': KIT}, open(os.path.join(OUT, 'kit.json'), 'w'), indent=1)
    print('HUD_KIT', len(KIT), 'sprites ->', OUT)
