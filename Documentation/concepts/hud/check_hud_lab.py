"""Exercise actual browser controls and capture preview states, not Unity gameplay."""
from pathlib import Path
import subprocess, tempfile, json
from bs4 import BeautifulSoup
root = Path(__file__).resolve().parents[3]
page = root / 'Documentation/FreeGuild-HUD-Lab.html'
qa = root / 'Documentation/concepts/qa'
qa.mkdir(exist_ok=True)
rows = []
for demo in ('combat', 'board', 'dress', 'faded', 'pause'):
    capture = qa / ('hud-layer-lab.png' if demo == 'combat' else 'hud-' + demo + '.png')
    command = [r'C:\Program Files\Google\Chrome\Application\chrome.exe', '--headless=new', '--disable-gpu', '--hide-scrollbars', '--user-data-dir='+tempfile.mkdtemp(prefix='freeguild-hud-qa-'), '--window-size=1600,1400', '--virtual-time-budget=1200', '--dump-dom', '--screenshot='+str(capture), page.as_uri()+'?demo='+demo]
    result = subprocess.run(command,capture_output=True,text=True,encoding='utf-8',timeout=40)
    dom = BeautifulSoup(result.stdout,'html.parser')
    passed = dom.html and dom.html.get('data-tests') == 'pass'
    checks = dom.find(id='checks').get_text() if dom.find(id='checks') else result.stderr[-500:]
    rows.append({'scenario':demo,'passed':bool(passed),'checks':checks,'capture':str(capture.relative_to(root))})
    print(demo, 'PASS' if passed else 'FAIL')
    assert passed, checks
(qa/'hud-browser-results.json').write_text(json.dumps(rows,indent=2),encoding='utf-8')
