"""Review supplied HUD pieces unchanged, with hashes and native-size previews."""
from pathlib import Path
from PIL import Image
import base64, hashlib, html, json

root = Path(__file__).resolve().parents[3]
hud = Path(__file__).resolve().parent
source = hud / 'HUD/segments'
fire_paths = {
    'plain': 'diamond-ability-icon/diamond-ability-icon-13.png',
    'ready': 'diamond-ability-icon/diamond-ability-icon-13.png',
    'locked': 'diamond-ability-icon/diamond-ability-icon-20.png',
    'charging': 'circular-ability-icon/circular-ability-icon-3.png',
    'release': 'diamond-ability-icon/diamond-ability-icon-15.png',
    'cooldown': 'circular-ability-icon/circular-ability-icon-1.png',
    'interrupted': 'diamond-ability-icon/diamond-ability-icon-12.png',
}

def data(path):
    kind = 'jpeg' if path.suffix.lower() in ('.jpg', '.jpeg') else 'png'
    return 'data:image/' + kind + ';base64,' + base64.b64encode(path.read_bytes()).decode()

def main():
    groups = {}
    for path in sorted(source.rglob('*.png')):
        sha = hashlib.sha256(path.read_bytes()).hexdigest()
        with Image.open(path) as image:
            rgba = image.convert('RGBA')
            # Compare visible pixels, independent of differing transparent canvas padding.
            visible = rgba.crop(rgba.getbbox())
            pixels = bytes(channel for pixel in visible.getdata() for channel in (pixel if pixel[3] else (0, 0, 0, 0)))
            pixel_sha = hashlib.sha256(str(visible.size).encode() + pixels).hexdigest()
            if pixel_sha in groups:
                groups[pixel_sha]['duplicates'].append(path.relative_to(source).as_posix())
                continue
            alpha = list(rgba.getchannel('A').getdata())
            groups[pixel_sha] = {'path': path.relative_to(source).as_posix(), 'sha256': sha, 'pixel_sha256': pixel_sha,
                'size': list(image.size), 'mode': image.mode,
                'soft_alpha_pixels': sum(0 < value < 255 for value in alpha),
                'duplicates': []}
    report = {'source': source.relative_to(root).as_posix(),
        'provenance': 'Boss-supplied concepts; generator/licensing provenance not established by this inspection.',
        'unique_images': len(groups), 'files': sum(1 + len(row['duplicates']) for row in groups.values()),
        'fire_state_candidates': fire_paths, 'assets': list(groups.values())}
    (hud / 'library-review.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    cards = []
    for row in groups.values():
        w, h = row['size']
        path = source / row['path']
        cards.append('<article><div class="tile"><img src="' + data(path) + '" width="' + str(w) + '" height="' + str(h) + '" alt=""></div><b>' + html.escape(row['path']) + '</b><p>' + str(w) + ' x ' + str(h) + ' px / ' + str(len(row['duplicates'])) + ' duplicate(s)<br>Soft alpha: ' + str(row['soft_alpha_pixels']) + ' pixels</p></article>')
    state_cards = ''.join('<article><div class="tile"><img src="' + data(source / name) + '" alt=""></div><b>' + state.upper() + '</b><p>Shape candidate / text remains separate</p></article>' for state, name in fire_paths.items() if state != 'plain')
    page = '''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>FreeGuild HUD library review</title>
<style>body{margin:0;background:#091426;color:#eaf6ff;font:16px/1.5 system-ui}main{max-width:1440px;margin:auto;padding:24px}h1,h2,a{color:#74dfff}.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(190px,1fr));gap:12px}article{background:#10223b;padding:12px;border:1px solid #305475;overflow-wrap:anywhere}.tile{height:145px;background:var(--back,#071329);display:flex;align-items:center;justify-content:center}.tile img{max-width:100%;object-fit:contain;opacity:var(--alpha,1)}.board{width:100%;height:auto}p{color:#bdd0e2}.controls{position:sticky;top:0;background:#10223b;padding:12px;z-index:1;display:flex;gap:24px;flex-wrap:wrap}select,input{font:inherit}section{margin:30px 0}</style><main>
<h1>HUD parts / supplied library</h1><p>Source files are unchanged. Inspect the cut edges at native size; the controls affect this preview only. These pieces are concept ingredients, not approved Unity sprites.</p><p><a href="FreeGuild-HUD-Lab.html">Try the fire icons in the layered HUD</a> | <a href="FreeGuild-Design-Atlas.html">Design atlas</a></p>
<div class="controls"><label>Background <select id="back"><option value="#071329">Dark navy</option><option value="#c9c2ae">Bright stone</option><option value="#5d694c">Grass</option><option value="#8b5c43">Warm timber</option></select></label><label>Art opacity <input id="alpha" type="range" min="0" max="1" step=".01" value="1"></label><label><input id="gray" type="checkbox"> Grayscale</label></div>
<section><h2>First selection: fire state shapes</h2><p>Blue cooldown, orange ready/release, a ring for charging, lock and crossed interruption. The state label remains visible independently of colour and art opacity. Locked means the held-charge technique; plain fire stays available.</p>STATES</section>
<section><h2>Direction board</h2><p>Use the frame language and compact layout. Build text and widgets separately. Contract board and access options fit P0; appearance preview belongs to P1.</p><img class="board" src="BOARD" alt="Supplied FreeGuild HUD direction board"></section>
<section><h2>Native-size parts</h2><p>COUNTS. Category names contain duplicates and are not a reliable description of the picture. Most item/shop imagery is outside the current first-playable scope.</p>CARDS</section>
<p>Production work remaining: larger source renders with soft alpha, measured 9-slice borders, separate clean typography and in-game contrast checks. Production readability floors: plates behind text 0.70 alpha, text 0.85, critical alerts 0.90. This HUD fixture protects text with its own opaque backing; decorative art may fade independently.</p>
<script>document.getElementById('back').onchange=e=>document.documentElement.style.setProperty('--back',e.target.value);document.getElementById('alpha').oninput=e=>document.documentElement.style.setProperty('--alpha',e.target.value);document.getElementById('gray').onchange=e=>document.querySelectorAll('.tile').forEach(t=>t.style.filter=e.target.checked?'grayscale(1)':'none');</script></main></html>'''
    import re
    values = {'STATES': '<div class="grid">' + state_cards + '</div>', 'BOARD': data(source / 'fullimage.jpg'),
        'COUNTS': str(report['files']) + ' cut-outs, ' + str(report['unique_images']) + ' distinct visible images (transparent padding ignored)',
        'CARDS': '<div class="grid">' + ''.join(cards) + '</div>'}
    page = re.sub(r'\b(STATES|BOARD|COUNTS|CARDS)\b', lambda m: values[m.group(1)], page)
    target = root / 'Documentation/FreeGuild-HUD-Library.html'
    target.write_text(page, encoding='utf-8')
    print(target, report['files'], 'files /', report['unique_images'], 'unique')

if __name__ == '__main__':
    main()
