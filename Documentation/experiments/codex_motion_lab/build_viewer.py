"""Create an offline, reviewable viewer from real Blender output. No image edits."""
import base64, json, html, re
from pathlib import Path
root=Path(__file__).resolve().parents[3]
out=Path(__file__).resolve().parent
r=json.loads((out/'results.json').read_text())
def data(filename):
    return 'data:image/png;base64,'+base64.b64encode((out/filename).read_bytes()).decode()
cards=[]
for clip in r['clips']:
    figures=''.join('<figure><img src="'+data(x['file'])+'"><figcaption>'+str(round(x['normalized_time']*100))+'% - source frame '+str(round(x['source_frame'],3))+'</figcaption></figure>' for x in clip['renders'])
    cards.append('<section><h2>'+html.escape(clip['label'].replace('_',' '))+'</h2><p>'+html.escape(clip['source'])+'</p><details><summary>Actual source key timestamps and bone mapping</summary><pre>'+html.escape(json.dumps({'key_frames':clip['source_key_frames'],'mapped_bones':clip['mapped_bones']},indent=2))+'</pre></details><div class="strip">'+figures+'</div></section>')
rows=''.join('<tr><td>'+html.escape(t['from']+' -> '+t['to'])+'</td><td>'+str(round(t['raw_endpoint_max_angle_deg'],1))+' deg</td><td>'+str(round(t['raw_endpoint_max_limb_jump_m']*100,2))+' cm</td></tr>' for t in r['transitions'])
frames=[{'src':data(x['file']),'stage':x['stage']} for x in r['combo_frames']]
unity=json.loads((out/'unity/captures.json').read_text())
unity_cards=[]
for role in dict.fromkeys(c['role'] for c in unity['captures']):
    captures=[c for c in unity['captures'] if c['role']==role]
    figures=''.join('<figure><img src="'+data('unity/'+c['file'])+'"><figcaption>'+str(round(c['time'],3))+' s</figcaption></figure>' for c in captures)
    unity_cards.append('<section><h2>Unity / '+html.escape(role)+'</h2><p>'+html.escape(captures[0]['asset'])+'</p><div class="strip">'+figures+'</div></section>')
page='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>FreeGuild - Owned Motion Lab</title><style>
:root{color-scheme:dark}body{font:16px/1.55 system-ui;background:#0b1425;color:#eaf6ff;margin:0 auto;padding:30px;max-width:1400px}h1,h2{color:#70d8ff}p{max-width:90ch}.notice{border-left:4px solid #ffc06b;padding:12px;background:#19263b}section{border-top:1px solid #33506b;margin-top:32px;padding-top:20px}.strip{display:flex;overflow:auto;gap:8px}.strip figure{margin:0;flex:1;min-width:180px}.strip img{width:100%}figcaption{font-size:13px}button,input{padding:9px;margin:5px;background:#213654;color:inherit;border:1px solid #5fd7ff}#film{max-height:540px;max-width:100%;display:block}table{border-collapse:collapse}td,th{text-align:left;border-bottom:1px solid #33506b;padding:10px}pre{white-space:pre-wrap;font-size:12px}.flow{padding:16px;border:1px solid #385777;background:#111e33}a{color:#70d8ff}@media(prefers-reduced-motion:reduce){*{scroll-behavior:auto}}
</style><h1>Owned motion, then choreography</h1>
<p class="notice">Actual owned animation curves on the derived body. Blender authored-key studies plus Unity 6000.3.26f1 Humanoid captures are below. Contact, foot support and frame-rate gates remain pending.</p>
<h2>Two-hit phrase -> return to guard</h2><p>Alternate Brawler clips at their imported duration, with separately added four-frame handovers. This review montage keeps full preparation and recovery; it is not the tuned gameplay action sequence. Scrub the hands, hips and feet. A smooth crossfade can still slide a planted foot.</p>
<img id="film"><div><button id="play">Play</button><button id="reset">Start</button><input id="scrub" aria-label="Animation frame" type="range" min="0" step="1"><span id="caption"></span></div>
<h2>Measured raw handovers</h2><p>Largest mapped-bone rotation difference and largest hand/foot endpoint difference. These describe an unblended mismatch; they are not unexplained-transition residuals or acceptance passes.</p><table><tr><th>Handover</th><th>Rotation mismatch</th><th>Limb mismatch</th></tr>ROWS</table>
<h2>How an exchange should feel</h2><div class="flow">Read the opponent's preparation -> choose a committed verb -> verify contact -> show hit / loaded block / uninterrupted miss -> finish recovery -> accept the next buffered intent.</div>
<p>Light 1 should feed light 2 through its follow-through, rather than reset to idle. Give each hit a distinct contact identity. Heavy keeps its readable wind-up and punishable recovery. Guard takes weight through the forearms; guard break opens the elbows and lowers the centre. Throw begins with interruptible reach-in and only enters paired clips after legal staging. Step changes support feet; it needs a grounded lateral variant, not a hurried roll.</p>
<p>For verified contact, test a local 25-45 ms pose hold, with logical cooldowns and input still advancing. Misses receive no hold or spark. Foot support and bounded pelvis fitting must preserve canonical limb lengths; actor movement belongs to the motor.</p>
<h2>Recovery belongs to the loop</h2><div class="flow">Commit to an exchange -> create space or finish the threat -> out-of-combat recovery -> return, deliver and prepare -> next obligation.</div>
<p>Confirmed design: defeat or abandon returns to the hall bench at full HP; recovery and retry are free. The optional preparation item shortens out-of-combat recovery and is never required. Claude is defining duration, interruption, threat checks and consumption rules. Passive regeneration and combat healing are not specified.</p>
<p>Proposed presentation: a stable breathing/settling pose, visible progress, then one completion accent and the HP update. On interruption, immediately show the relevant reaction. Consumption and HP changes follow the logical completion event, once; the animation never awards healing. This needs an agreed rule and an owned clip before implementation.</p>
<h2>Unity retarget evidence</h2><p>12 clips, 60 quarter-duration poses, including primary Explosive L1/R1. Fixed-rig skin-v2 Avatar is valid; CPU-baked meshes capture each evaluated pose independently. No foot IK, contact fitting or combat acceptance is claimed. These are duration samples, not authored-key timestamps.</p>UNITY
<h2>Blender clip evidence</h2><p>The five images per clip evaluate actual authored key timestamps nearest the quarter intervals; the details list the imported curve's key timestamps. Preparation/contact/recovery labels require curation, not a guessed midpoint. The cast container's animated FBX axis/scale basis is removed for this Blender study.</p>CARDS
<section><h2>Remaining gates</h2><ul>LIMITS</ul><p><a href="experiments/codex_motion_lab/results.json">Raw measurement report</a> | <a href="FreeGuild-Design-Atlas.html">Design atlas</a></p></section>
<script>const frames=FRAMES, fps=FPS; let i=0,running=false,last=0; const film=document.getElementById('film'),scrub=document.getElementById('scrub'),caption=document.getElementById('caption'),play=document.getElementById('play'); scrub.max=frames.length-1;
function show(){film.src=frames[i].src;film.alt=frames[i].stage+' frame '+i;scrub.value=i;caption.textContent=frames[i].stage+' | '+(i/fps).toFixed(2)+' s';}
play.onclick=()=>{running=!running;play.textContent=running?'Pause':'Play';last=performance.now()};document.getElementById('reset').onclick=()=>{i=0;show()};scrub.oninput=()=>{running=false;play.textContent='Play';i=+scrub.value;show()};
function tick(now){if(running&&now-last>=1000/fps){i=(i+1)%frames.length;show();last=now}requestAnimationFrame(tick)}show();requestAnimationFrame(tick);</script></html>'''
values={'ROWS':rows,'CARDS':''.join(cards),'UNITY':''.join(unity_cards),'LIMITS':''.join('<li>'+html.escape(x)+'</li>' for x in r['limits']),
        'FRAMES':json.dumps(frames),'FPS':str(r['review_fps'])}
# Substitute the original template once. Never run text replacements over
# injected base64 (a short token such as FPS can occur inside PNG payloads).
page=re.sub(r'\b(ROWS|CARDS|UNITY|LIMITS|FRAMES|FPS)\b',lambda m:values[m.group(1)],page)
target=root/'Documentation/FreeGuild-Motion-Lab.html'
target.write_text(page,encoding='utf-8')
print(target, 'clips',len(cards),'combo frames',len(frames))
