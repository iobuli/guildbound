"""Measure generated alpha and write explicit Unity layout/sprite metadata. No image edits."""
from pathlib import Path
from PIL import Image
import hashlib, json

folder = Path(__file__).resolve().parent
source = folder / 'frames-sheet-v1.png'
image = Image.open(source).convert('RGBA')
assert image.size == (1536, 1024)
alpha = image.getchannel('A')
# Reviewed alpha>=120 perimeter components, plus six pixels of local glow.
frames = [
    ('modal_frame', [37, 39, 483, 455]),
    ('card_frame', [542, 49, 453, 433]),
    ('status_frame', [1027, 50, 470, 431]),
    ('button_frame', [37, 496, 484, 454]),
    ('inset_frame', [541, 505, 455, 435]),
    ('focus_frame', [1018, 501, 485, 445]),
]
sprites = []
for name, rect in frames:
    x, y, w, h = rect
    tile = alpha.crop((x, y, x + w, y + h))
    centre = tile.crop((128, 128, w - 128, h - 128))
    histogram = tile.histogram()
    row = {'id': name, 'top_left_rect_px': rect,
        'unity_rect_px': [x, 1024 - y - h, w, h],
        'nine_slice_lbrt_px': [128, 128, 128, 128],
        'pixels_per_unit': 400, 'canvas_reference_pixels_per_unit': 100,
        'corner_size_canvas_units': 32,
        'centre_alpha_max': centre.getextrema()[1],
        'transparent_pixels': histogram[0], 'soft_alpha_pixels': sum(histogram[1:255])}
    assert row['centre_alpha_max'] <= 3, row
    assert row['transparent_pixels'] > 0 and row['soft_alpha_pixels'] > 0
    sprites.append(row)

def surface(name, frame, width, height, pad_x, pad_y):
    return {'id': name, 'frame': frame, 'size': [width, height],
        'padding': [pad_x, pad_y], 'content_size': [width - pad_x * 2, height - pad_y * 2]}

report = {'schema': 1, 'source': 'frames-sheet-v1.png',
    'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
    'size_px': list(image.size), 'reference_resolution': [1920, 1080],
    'canvas_scale_mode': 'ScaleWithScreenSize', 'match_width_or_height': 0.5,
    'sprites': sprites,
    'surfaces': [surface('StatusPanel', 'status_frame', 360, 144, 40, 28),
        surface('FirePanel', 'status_frame', 320, 112, 32, 24),
        surface('ObjectivePanel', 'status_frame', 400, 128, 32, 24),
        surface('GuildBoard', 'modal_frame', 1120, 760, 48, 48),
        surface('ContractCard', 'card_frame', 448, 520, 32, 32),
        surface('ActionButton', 'button_frame', 288, 80, 32, 20),
        surface('ContentWell', 'inset_frame', 480, 360, 32, 32)],
    'interactive_controls': {'button_size': [288, 80], 'slider_size': [320, 64],
        'minimum_hit_size': [48, 48], 'navigation': 'automatic selectable navigation',
        'decorative_images_raycast_target': False},
    'layers': ['Backplate', 'Frame', 'ScriptedAccent', 'Content'],
    'alpha_floors': {'backplate': 0.70, 'text': 0.85, 'critical_alert': 0.90},
    'limits': ['Reference sizes are authored layout choices, not boss-approved final placement.',
        'Measured rects replace the imprecise generation grid; this is not the earlier 64-cell master sheet.',
        'Tiny nonzero centre alpha <=3 is retained as generated; Image.fillCenter=false excludes it in Unity.',
        'Baked perimeter glints stay in frame art; dynamic hover/focus/flash is a separate scripted layer.',
        'No game-state, save, contract or combat bindings implemented by this kit.']}
(folder / 'panel-kit-manifest.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print('Six explicit slices / transparent centres / soft alpha: PASS')
print('Reference surfaces:', len(report['surfaces']))
