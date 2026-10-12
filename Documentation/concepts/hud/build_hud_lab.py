"""Embed prototype ingredients for offline HUD review; no Unity asset mutation."""
from pathlib import Path
import base64, json, re, sys
sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_library_review import fire_paths
root = Path(__file__).resolve().parents[3]
def image(p):
    return 'data:image/png;base64,' + base64.b64encode(p.read_bytes()).decode('ascii')
hud = root / 'Documentation/concepts/hud'
frame = hud / 'qa/slices_v1/panel_frame_l.png'
if not frame.exists():
    frame = hud / 'qa/slices_v1/_canonical_2048.png'
    if not frame.exists():
        frame = hud / 'candidates/hud-master-sheet-v1.png'
    atlas = True
else:
    atlas = False
text = (hud / 'hud_lab_template.html').read_text(encoding='utf-8')
values = {
    '__PORTRAITS__': json.dumps([image(root / 'Documentation/experiments/codex_character_lab/skin_v2' / (name+'_vest.png')) for name in ('base','fighter','caster','heavy')]),
    '__WORLD__': image(root / 'Documentation/concepts/objects/hall-carved-a.png'),
    '__SHEET__': image(frame),
    '__FIRE_ART__': json.dumps({state: image(hud / 'HUD/segments' / path) for state, path in fire_paths.items()}),
}
text = re.sub(r'__(PORTRAITS|WORLD|SHEET|FIRE_ART)__', lambda m: values[m.group(0)], text)
if not atlas:
    text = text.replace('0 0/800% 800%', 'center/100% 100%')
text = text.replace('<style>', '<style>.modal-content{background:#071329ee;padding:12px}.modal-content h2,.modal-content p,.modal-content label{color:#edf7ff}')
(root / 'Documentation/FreeGuild-HUD-Lab.html').write_text(text, encoding='utf-8')
print('HUD lab built; frame source:', frame)
