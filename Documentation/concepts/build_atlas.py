"""Export the reviewed GDD and kit lexicon as a self-contained illustrated atlas."""
from pathlib import Path
import base64
import html
import json
import re
import markdown
from bs4 import BeautifulSoup

ROOT = Path(__file__).resolve().parents[2]
DOC = ROOT / "Documentation"
LOG = (ROOT / "TANDEM_REASONING.log").read_text(encoding="utf-8-sig")

def block(name):
    found = re.findall(r"^BEGIN_" + name + r"\s*\n(.*?)^END_" + name + r"\s*$", LOG, re.M | re.S)
    if not found:
        raise ValueError("Missing source block: " + name)
    return found[-1].strip()

gdd, kit = block("MASTER_GDD"), block("KIT_LEXICON")
version = re.search(r"^Status: (v\d+(?:\.\d+)?)", gdd, re.M)
version = version.group(1) if version else "latest"
gdd = re.sub(r"^Status:.*$", "Status: Reviewed design baseline " + version + ", including the accepted amendments in the latest source block. P0 remains unchanged. Exported for the boss on 11 October 2026. Numbers marked (H) are tuning hypotheses, not verified results. UNKNOWN means not yet established. Section 12 is the reusable master build prompt.", gdd, flags=re.M)
def render(text, prefix):
    text = re.sub(r"^={5,}\s*$", "", text, flags=re.M)
    source_lines = text.splitlines()
    normalized = []
    for line in source_lines:
        if re.match(r"^(?:- |\d+\. )", line) and normalized and normalized[-1].strip() and not re.match(r"^(?:- |\d+\. )", normalized[-1]):
            normalized.append("")
        normalized.append(line)
    soup = BeautifulSoup(markdown.markdown("\n".join(normalized), extensions=["tables", "fenced_code", "toc", "sane_lists", "nl2br"]), "html.parser")
    links = []
    for tag in soup.find_all(re.compile("^h[1-6]$")):
        tag["id"] = prefix + "-" + tag.get("id", re.sub(r"[^a-z0-9]+", "-", tag.get_text().lower()).strip("-"))
        if tag.name == "h2":
            links.append((tag["id"], tag.get_text()))
    for table in soup.find_all("table"):
        wrap = soup.new_tag("div", attrs={"class": "table-wrap"})
        table.wrap(wrap)
    return str(soup), links

gdd_html, gdd_links = render(gdd, "gdd")
kit_html, kit_links = render(kit, "kit")

def img(path):
    path = Path(path)
    if not path.is_file():
        raise FileNotFoundError(path)
    mime = "image/png" if path.suffix.lower() == ".png" else "image/jpeg"
    return "data:" + mime + ";base64," + base64.b64encode(path.read_bytes()).decode("ascii")

concepts = [
    ("CS-OBJ-02-A", "hall-carved-a.png", "Carved folk / SD-A", "Hall exterior in the faceted, matte direction. Repeated timber bays, framed doorway, red/bone cloth and a clear sparring courtyard."),
    ("CS-OBJ-02-B", "hall-painted-b.png", "Painted soft / SD-B", "Alternative material and foliage treatment. This remains a style proposal; the source-piece construction vocabulary stays the base."),
    ("CS-OBJ-01", "hall-exploded.png", "The hall as a kit", "An exploded assembly study: posts, wall/window/door infill, floor and roof runs, with an interior service edge. Connections and dimensions need in-engine validation."),
    ("CS-OBJ-04", "gate-states.png", "Gate / damaged to restored", "The same gate grammar grows by aligned planks, a cleared passage and a restored marker. It changes the available route."),
    ("CS-OBJ-05", "stall-states.png", "Preparation / dormant to active", "Crates and shed pieces become an ordered service beneath a canopy. The growth offers an optional preparation tool."),
]
cards = []
for ident, file, title, caption in concepts:
    url = img(DOC / "concepts" / "objects" / file)
    cards.append(f'<figure class="concept"><a href="{url}" target="_blank" aria-label="Enlarge {html.escape(title)}"><img src="{url}" alt="{html.escape(title)}" loading="lazy"></a><figcaption><span class="eyebrow">{ident} / DRAFT CONCEPT</span><h3>{title}</h3><p>{caption}</p></figcaption></figure>')

character_concepts = [
    ("CS-CHR-01", "hero-turnaround.png", "The field member", "Front, side and back exploration of the hero: a compact work-clothing silhouette, wrapped forearms and a red/bone guild marker. Derived outfit construction and the final rig remain to be proven."),
    ("CS-CHR-02", "opponent-silhouettes.png", "Pressure and defensive opponents", "One body family, two readable patterns. Forward-loaded ochre/charcoal pressure versus broad, guarded slate/moss defence. The black silhouettes test role clarity without colour."),
    ("CS-CHR-03", "guild-roster.png", "The people of the hall", "Hero, clerk, stall keeper, repair crew and training partner. Practical clothing and stance identify each role within one shared character vocabulary."),
    ("CS-CHR-04", "combat-poses.png", "Weight, preparation and contact", "Eight aesthetic targets: home stance, light contact, heavy wind-up/contact, loaded guard, guard break, lateral step and throw reach-in. These images do not prove collision, support or animation correctness."),
    ("CS-CHR-05", "casting-poses.png", "Elemental growth on the body", "Ready, plain release, held charge and charged release. Effects are deliberately absent so the stance, hands and planted support communicate the action."),
]
character_cards = []
for ident, file, title, caption in character_concepts:
    url = img(DOC / "concepts" / "characters" / file)
    character_cards.append(f'<figure class="concept"><a href="{url}" target="_blank" aria-label="Enlarge {html.escape(title)}"><img src="{url}" alt="{html.escape(title)}" loading="lazy"></a><figcaption><span class="eyebrow">{ident} / DRAFT CHARACTER CONCEPT</span><h3>{title}</h3><p>{caption}</p></figcaption></figure>')

sheets = []
for file, title, desc in [
    ("tavern", "Hall source pieces", "Actual owned modular walls, supports, roof tiles, floor, furniture and props. Grey infill needs a material derivative."),
    ("town", "Village source pieces", "Actual owned houses, fence/gate pieces, awnings, boardwalks, banners, terrain, rocks and service props."),
    ("nature", "Foliage source pieces", "Actual owned painterly trees, bushes and grass. Their soft detail must be reconciled with the faceted architecture."),
    ("characters", "Owned bodies and rigs", "Actual source bodies for the rig proof. This is asset evidence, not approved final character art; missing materials are visible."),
]:
    url = img(ROOT / "Logs" / "sheets" / (file + ".png"))
    sheets.append(f'<details class="source-sheet"><summary>{title}</summary><figure><img src="{url}" alt="{title}" loading="lazy"><figcaption>{desc}<br><code>Logs/sheets/{file}.png</code> + <code>{file}.txt</code></figcaption></figure></details>')

palette = [("Dark timber", "4A3122"), ("Mid wood", "7A5233"), ("Frame", "2A211C"), ("Stone", "8C8A85"), ("Thatch", "9C9268"), ("Guild red", "8E2F25"), ("Guild bone", "E6DAC2"), ("Grass", "6E9A4A"), ("Sky", "9FB2C8"), ("Fire core", "FFD27A"), ("Fire edge", "E2561F")]
chips = "".join(f'<div class="swatch"><i style="background:#{c}"></i><b>{n}</b><small>#{c}</small></div>' for n,c in palette)
nav = '<a href="#characters">Character concepts</a><a href="#visuals">World concepts</a><a href="#sources">Actual asset sheets</a><a href="#palette">Palette</a><a href="#design">Complete GDD</a>' + "".join(f'<a class="sub" href="#{i}">{html.escape(t)}</a>' for i,t in gdd_links) + '<a href="#lexicon">Complete kit lexicon</a><a href="#provenance">Concept provenance</a>'
prompt_file = DOC / "concepts" / "concept-prompts.json"
prompt_data = json.loads(prompt_file.read_text(encoding="utf-8"))
prompt_html = "".join(f'<details><summary>{html.escape(item["id"])} / prompt and review</summary><pre>{html.escape(json.dumps(item, indent=2))}</pre></details>' for item in prompt_data)
hero = img(DOC / "concepts" / "objects" / "hall-carved-a.png")
spike = img(DOC / "experiments" / "chr_spike" / "lineup.png")
creator_section = f'''<section class="section" id="creator"><span class="eyebrow">AMEND-CHR-01 / OWNED BODY EXPERIMENT + P1 DESIGN</span><h2>Your body, built from the kit.</h2><figure class="concept"><img src="{spike}" alt="Blender experiment: base, fighter, caster, heavy and posed fighter"><figcaption><h3>The actual Blender spike</h3><p>Left to right: base, fighter, caster, heavy, posed fighter. Claude's recorded experiment reduced HumanM_BodyMesh from 5,340 to 1,880 vertices on a copy, retained the 51-bone rig, created three build keys and exported an FBX. This is a Blender render, not generated concept art or a Unity capture. Fighter/caster differences need stronger reading; heavy chest/shoulder seams need repair. Unity import, clip compatibility, clothing and combat contact are still unverified. Only this body was tested; additional body families are proposed.</p><code>Documentation/experiments/chr_spike/lineup.png + spike_log.txt</code></figcaption></figure><h3>A dressing corner beside the sparring floor</h3><p>An owned bench, chest and clothes rack form the station, with a timber frame derived from the hall kit. A full-body preview is sufficient for the first proof; a reflective mirror is a later presentation option. Interact, rotate front/side/back, then choose Frame, Head or Colours. Apply saves once at the hall; Cancel restores the previous appearance. Free, revisitable and skippable.</p><table><thead><tr><th>Build</th><th>Reads at distance</th><th>Rewards close inspection</th></tr></thead><tbody><tr><td>Fighter</td><td>Compact loaded stance, upper-back mass, wider wrap bands</td><td>Repair stitching and compact shoulder patch</td></tr><tr><td>Caster</td><td>Tapered vest, composed braced stance, dark forearm bands</td><td>One then two short ember seams; glow only during ready/charge states</td></tr><tr><td>Heavy</td><td>Padded vest, fuller lower torso/thighs, stable grounded stance</td><td>Broad reinforced cloth bands</td></tr></tbody></table><div class="note"><b>Training is permanent; appearance stays yours.</b> Useful landed strikes, throws, casts and deliberate guards contribute to mastery. Taking damage earns none. A unique delivery grants at most 0.08 total across the axes (H). The station suggests a bounded cosmetic blend; keeping your current appearance always works. No combat stats, reach or hitboxes come from the mesh. First version uses canonical rig scale.</div><p><b>Depth recommendation:</b> optional local <a href="https://github.com/DepthAnything/Depth-Anything-V2">Depth Anything V2 Small</a> (Apache-2.0), not installed or tested here. Its view-relative estimates can guide sculpting or isolated cloth relief; Blender baking from the derived garment supplies actual surface detail. No AI depth becomes body dimensions, topology or collision geometry.</p><p><b>Scope:</b> one approved faceted hero is P0. Creator, trained body suggestions and marker tiers are P1. First repair seams and prove Unity import, deformation and visible contact across the approved morph envelope.</p></section>'''
nav = nav.replace('<a href="#visuals">', '<a href="#creator">Creator + body spike</a><a href="#visuals">')

css = """
:root{--ink:#252824;--paper:#f5f1e8;--panel:#fffcf5;--red:#8e2f25;--line:#d8d0be;--muted:#66665c}*{box-sizing:border-box}html{scroll-behavior:smooth}body{margin:0;background:var(--paper);color:var(--ink);font:16px/1.7 'Segoe UI',Arial,sans-serif}a{color:var(--red)}nav{position:fixed;inset:0 auto 0 0;width:252px;background:#252824;color:#f5f1e8;overflow:auto;padding:30px 20px;z-index:10}nav b{font-size:24px;display:block;letter-spacing:.03em}nav small{color:#c8bb9d}nav a{color:#e5ddcc;text-decoration:none;display:block;font-size:14px;padding:6px 0}nav a:hover{color:#ffc98e}nav .sub{font-size:12px;padding-left:10px;color:#b8b4a8}main{margin-left:252px}header{position:relative;min-height:600px;color:white;display:flex;align-items:end;background:#333}header>img{position:absolute;inset:0;width:100%;height:100%;object-fit:cover}header:after{content:'';position:absolute;inset:0;background:linear-gradient(0deg,rgba(18,21,18,.92),transparent 80%)}.hero-copy{position:relative;z-index:1;padding:50px;max-width:1000px}h1{font:normal clamp(46px,6vw,88px)/1.04 Georgia,serif;margin:15px 0}.eyebrow{font-size:12px;letter-spacing:.16em;text-transform:uppercase;color:var(--red);font-weight:700}.hero-copy .eyebrow{color:#ffd5a0}.hero-copy p{font-size:19px;max-width:690px}.tags{display:flex;gap:10px;flex-wrap:wrap}.tags span{padding:5px 12px;border:1px solid #ffffff66;border-radius:30px;font-size:12px}.section{padding:50px;max-width:1500px;margin:auto}h2{font:normal 34px/1.2 Georgia,serif;margin:15px 0 25px;scroll-margin-top:20px}h3{line-height:1.3}p{margin:12px 0}.lead{font-size:20px;max-width:920px}.note{background:#e9dfcb;border-left:4px solid var(--red);padding:15px 20px;margin:24px 0}.pillars{display:grid;grid-template-columns:repeat(4,1fr);gap:15px;margin:30px 0}.pillar{border-top:3px solid var(--red);padding:20px;background:var(--panel)}.pillar b{display:block;font-size:18px}.pillar p{font-size:14px;color:var(--muted)}.concept{margin:0 0 36px;border:1px solid var(--line);background:var(--panel);break-inside:avoid}.concept img{display:block;width:100%;height:auto;max-height:900px;object-fit:contain;background:#e5e0d4}.concept figcaption{padding:22px 26px}.concept h3{font:normal 27px Georgia,serif;margin:8px 0}.concept p{color:var(--muted)}details{border:1px solid var(--line);padding:14px 18px;margin:15px 0;background:var(--panel)}summary{cursor:pointer;font-weight:600}.source-sheet img{max-width:100%;height:auto}.source-sheet figure{margin:20px 0 0}.source-sheet figcaption{padding:15px 0;color:var(--muted)}.palette{display:grid;grid-template-columns:repeat(auto-fit,minmax(105px,1fr));gap:12px}.swatch{background:var(--panel);padding:8px;border:1px solid var(--line)}.swatch i{display:block;height:70px;margin-bottom:10px}.swatch b,.swatch small{display:block;font-size:12px}.swatch small{color:var(--muted)}.doc{max-width:1100px}.doc h1{font-size:38px}.doc h2{border-top:1px solid var(--line);padding-top:28px;margin-top:46px}.doc h3{font-size:21px;color:var(--red)}.doc li{margin:6px 0}.table-wrap{overflow:auto}table{width:100%;border-collapse:collapse;font-size:14px;margin:20px 0}th{background:#252824;color:#fff;text-align:left}td,th{padding:12px;border:1px solid var(--line);vertical-align:top}tbody tr:nth-child(even){background:#eae5d9}pre{font-size:12px;line-height:1.5;white-space:pre-wrap;overflow-wrap:anywhere;background:#eae5d9;padding:18px}code{font-family:Consolas,monospace;font-size:.88em}button{border:0;background:var(--red);color:#fff;padding:12px 18px;font:inherit;cursor:pointer;border-radius:3px}.toolbar{display:flex;gap:12px;align-items:center;flex-wrap:wrap}.flow{display:flex;gap:10px;flex-wrap:wrap;margin:22px 0}.flow span{background:#252824;color:#f5f1e8;padding:13px 18px;font-size:14px}.flow span:after{content:' >';color:#e8b982}footer{padding:35px 50px;background:#252824;color:#c8bb9d}body.print-mode details>summary{margin-bottom:15px}@media(max-width:1000px){nav{display:none}main{margin:0}.section{padding:28px}.hero-copy{padding:28px}.pillars{grid-template-columns:repeat(2,1fr)}}@media(max-width:600px){.pillars{grid-template-columns:1fr}header{min-height:430px}.section{padding:20px}table{font-size:12px}}
@page{size:A4;margin:16mm}@media print{body{font:10pt/1.5 'Segoe UI',Arial,sans-serif;background:#fff;color:#111}nav,.toolbar{display:none}main{margin:0}header{min-height:90mm;break-after:auto}.hero-copy{padding:25px}header h1{font-size:45pt}header p{font-size:12pt}.section{padding:0;margin-bottom:24px;max-width:none}h2{font-size:22pt;break-after:avoid}h3{break-after:avoid}.doc h1{font-size:27pt}.doc h2{padding-top:18px;margin-top:30px;font-size:20pt}.eyebrow{font-size:8pt}.pillars{grid-template-columns:repeat(2,1fr)}.pillar{padding:12px}.concept img{max-height:180mm}.concept figcaption{padding:12px}.concept h3{font-size:18pt}.concept{break-inside:avoid;break-after:page}.source-sheet{break-before:page}.source-sheet img{max-height:210mm;object-fit:contain;width:100%}details{border:none;padding:0}table{font-size:8pt}td,th{padding:6px}tr{break-inside:avoid}.palette{grid-template-columns:repeat(6,1fr)}.swatch i{height:35px}pre{font-size:8pt}.table-wrap{overflow:visible}a{color:inherit;text-decoration:none}footer{background:white;color:#555;padding:15px 0}.note{padding:10px}*{-webkit-print-color-adjust:exact;print-color-adjust:exact}}
"""
out = f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>FreeGuild - Illustrated Design Atlas</title><style>{css}</style></head><body>
<nav><b>FREEGUILD</b><small>DESIGN ATLAS / 11 OCT 2026</small><hr>{nav}</nav><main>
<header><img src="{hero}" alt="Draft modular guild hall concept"><div class="hero-copy"><span class="eyebrow">UNITY / OWNED ASSET KIT / REVIEWED DESIGN</span><h1>A guild built<br>piece by piece.</h1><p>Read the body. Commit to the exchange. Bring useful work home, and watch the same kit grow into a place that can rely on you.</p><div class="tags"><span>Grounded combat</span><span>Procedural motion</span><span>Elemental growth</span><span>One guild hall</span></div></div></header>
<section class="section"><div class="toolbar"><button onclick="printAtlas()">Print / Save as PDF</button><a href="#design">Read the complete design</a><a href="#lexicon">Explore the three kits</a></div><p class="lead">The owned assets are the base. New forms grow through recombination, proportion changes, mesh and material derivatives, retimed clips and bounded procedural correction.</p><div class="pillars"><div class="pillar"><b>01 / Physical combat</b><p>Preparation, verified contact and recovery. The body tells you what happened.</p></div><div class="pillar"><b>02 / Procedural layer</b><p>Owned clips provide intent; bounded contact, support and transition correction give them weight.</p></div><div class="pillar"><b>03 / Elemental growth</b><p>Plain fire becomes a deliberate held charge after your first delivery. New behaviour, visible on the body.</p></div><div class="pillar"><b>04 / A physical home</b><p>One hall, a courtyard, two routes and two service changes built from the same modular vocabulary.</p></div></div><div class="note"><b>What is complete:</b> the reviewed GDD and reusable build prompt. <b>What these images are:</b> draft generated concepts anchored in the owned asset sheets. They are not Unity captures or approved production assets. Character appearance and pose concepts are included below. Final rig selection, derived outfit construction and motion correctness remain implementation gates.</div><div class="flow"><span>Choose work</span><span>Read and respond</span><span>Recover objective</span><span>Deliver once</span><span>Grow the hall</span></div></section>
<section class="section" id="characters"><span class="eyebrow">CHARACTER LIBRARY / OWNED BODY REFERENCES</span><h2>The field member and the people around them.</h2><p>Five draft character studies based on the actual owned humanoid body sheet. Outfits and role silhouettes are proposed mesh/material derivatives. These are concept images, not finished rigged models.</p>{''.join(character_cards)}</section>
<section class="section" id="visuals"><span class="eyebrow">01 / CONCEPT BOARD</span><h2>Five studies from the same building kit.</h2><p>SD-A and SD-B are alternatives for comparison. No style choice has been recorded. The object library stays separate from character and effects libraries.</p>{''.join(cards)}</section>
<section class="section" id="sources"><span class="eyebrow">02 / EVIDENCE</span><h2>The actual pieces we own.</h2><p>Expand a sheet to compare source geometry against the concepts. These are local catalog previews of the installed packs.</p>{''.join(sheets)}</section>
<section class="section" id="palette"><span class="eyebrow">03 / MATERIAL LANGUAGE</span><h2>One palette, three libraries.</h2><p>Proposed targets from the kit brief. Verify on actual materials under Unity lighting before approval.</p><div class="palette">{chips}</div></section>
<section class="section doc" id="design"><span class="eyebrow">04 / FULL DESIGN</span><h2>The complete master GDD.</h2><p>The following 12 sections reproduce the latest reviewed master block. Hypotheses marked H are starting values; runtime gates have not been run. Presentation metadata is updated for this export; design rules and scope are retained.</p>{gdd_html}</section>
<section class="section doc" id="lexicon"><span class="eyebrow">05 / FULL KIT SPECIFICATION</span><h2>Objects. Characters. Effects.</h2><p>Complete Kit Lexicon v2: IDs, recipes, motion families, effect stages, concept specifications and render order. Outfit/accessory concepts remain proposals pending verified source derivation and rig proof.</p>{kit_html}</section>
<section class="section doc" id="provenance"><span class="eyebrow">06 / TRACEABILITY</span><h2>Concept prompts and review.</h2><p>Built-in GPT image generation. Reference images: Logs/sheets/tavern.png, Logs/sheets/town.png and Logs/sheets/characters.png. All ten images remain draft proposals awaiting the boss's style selection and in-engine construction or rig checks.</p>{prompt_html}</section>
<footer>FreeGuild / Illustrated Design Atlas / 11 October 2026<br>Source: latest MASTER_GDD and KIT_LEXICON blocks in TANDEM_REASONING.log. Self-contained HTML; images embedded for offline viewing. This export contains no external example-game imagery.</footer></main>
<script>function printAtlas(){{document.querySelectorAll('details').forEach(e=>e.open=true);document.body.classList.add('print-mode');setTimeout(()=>window.print(),150)}}document.addEventListener('keydown',e=>{{if((e.ctrlKey||e.metaKey)&&e.key==='p'){{e.preventDefault();printAtlas()}}}});if(new URLSearchParams(location.search).has('print')){{document.querySelectorAll('details').forEach(e=>e.open=true);document.body.classList.add('print-mode')}};</script></body></html>'''
out = out.replace('<section class="section" id="visuals">', creator_section + '<section class="section" id="visuals">')
integration_source = (DOC / 'concepts/system-integration.md').read_text(encoding='utf-8')
integration_html, _ = render(integration_source, 'integration')
development_cards = []
for file, title, caption in [
    ('trained-builds-v2.png', 'Early to trained / revised silhouettes', 'Edited fighter diagonal straps and narrower waist; heavy padded horizontal hip/thigh mass; caster tapered coat. Art direction only: actual 10/20 m review and costume/contact checks remain pending.'),
    ('dressing-corner.png', 'A physical place to change', 'Bench, chest, clothes rack and derived timber frame from the owned kit. Mirror is proposed; omit the illustrative rug from the first assembly.'),
    ('playable-system-sheet.png', 'Nine connected gameplay studies', 'Hall, departure, contact, guard, throw, plain/held fire, return and service growth. These panels are art targets, not a chronological first trip: held charge unlocks after the first delivery and lesson. Reduce impact effects if they obscure contact.'),
    ('blue-gui-system-sheet-v2.png', 'Blue GUI system / edited pass', 'Original cyan frames and navy panes guided by the boss\'s references. Weapons, compass and armour-fit controls from v1 were removed. Text and meters are separate runtime layers; never ship this whole presentation board as the HUD.'),
]:
    development_cards.append(f'<figure class="concept"><img src="{img(DOC / "concepts/development" / file)}" alt="{html.escape(title)}"><figcaption><h3>{title}</h3><p>{html.escape(caption)}</p></figcaption></figure>')
lab = DOC / 'experiments/codex_character_lab'
report = json.loads((lab / 'results.json').read_text())
summary = {k:v for k,v in report.items() if k not in ('pair_records','samples')}
summary['sample_count'] = len(report['samples'])
summary['sample_maxima_m'] = {k:max(s[k] for s in report['samples']) for k in ('max_surface_delta_m','protected_surface_delta_m','bone_endpoint_delta_m','near_island_pair_gap_increase_m','shell_corresponding_offset_max_m')}
lab_pictures = ''.join(f'<figure><img style="width:100%" src="{img(lab / (file + ".png"))}" alt="{label}"><figcaption>{label}</figcaption></figure>' for file,label in [('base_vest','Base + shell'),('fighter_vest','Fighter'),('caster_vest','Caster'),('heavy_vest','Heavy'),('guard_diagnostic','Manual guard'),('cast_diagnostic','Manual cast'),('step_diagnostic','Manual step')])
hud_capture = DOC / 'concepts/qa/hud-layer-lab.png'
hud_picture = f'<img style="width:100%" src="{img(hud_capture)}" alt="Actual browser HUD layer prototype">' if hud_capture.exists() else ''
development_section = '<section class="section" id="development"><span class="eyebrow">NEW / COMBINED ART + ACTUAL TESTS</span><h2>From concept to visible system.</h2>'+''.join(development_cards)+f'<h3>Codex\'s Blender lab</h3><div style="display:grid;grid-template-columns:repeat(4,1fr);gap:12px">{lab_pictures}</div><p>Actual Blender captures: 35 manual pose/blend samples; {report["near_island_pairs"]} nearby cross-island pairs after correcting pair coverage. Derived torso shell follows the same keys. No Unity contact, clip, grounding or clothing-intersection pass is claimed. <a href="experiments/codex_character_lab/results.json">Full pair/sample measurements</a> and the first-pass failure remain available in the experiment folder.</p><details><summary>Measured Blender summary</summary><pre>{html.escape(json.dumps(summary,indent=2))}</pre></details></section>'
hud_document = (DOC / 'FreeGuild-HUD-Lab.html').read_text(encoding='utf-8')
hud_picture = hud_picture.replace('<img ', '<img class="hud-static" ')
hud_section = f'<section class="section" id="hud"><span class="eyebrow">EDITABLE / L0-L7</span><h2>HUD layer lab.</h2><p><a href="FreeGuild-HUD-Lab.html">Open the interactive HUD preview separately</a>, or use it below. <a href="FreeGuild-HUD-Library.html">Inspect the boss-supplied HUD library</a>; its fire-state shapes now form the preview icon layer. Change individual opacity, master fade, grayscale and effects. Try the combat, board, dresser and pause views. This is a browser design prototype over a concept background, not a Unity game capture.</p><iframe title="FreeGuild interactive HUD layer lab" srcdoc="{html.escape(hud_document,quote=True)}" style="width:100%;height:1100px;border:1px solid #327291" loading="lazy"></iframe>{hud_picture}</section>'
out = out.replace('</style>', '@media screen{.hud-static{display:none}}@media print{iframe{display:none}.hud-static{display:block!important}}</style>', 1)
merged = '<section class="section doc" id="integration"><span class="eyebrow">GDD + SYSTEMS + PRESENTATION</span><h2>One implementation map.</h2>'+integration_html+'</section>'
out = out.replace('<section class="section" id="characters">', development_section + hud_section + merged + '<section class="section" id="characters">')
out = out.replace('<a href="#characters">', '<a href="#development">New combined studies</a><a href="#hud">Editable HUD</a><a href="#integration">System integration</a><a href="#characters">', 1)
out = out.replace('All ten images remain draft proposals', 'Concept images remain draft proposals')
motion_directory = DOC / 'experiments' / 'codex_motion_lab'
if (motion_directory / 'results.json').exists():
    motion = json.loads((motion_directory / 'results.json').read_text(encoding='utf-8'))
    pictures = ''.join(f'<figure><img src="{img(motion_directory / name)}" alt="Owned clip pose"><figcaption>{label}</figcaption></figure>' for name,label in [
        ('brawler_light1_30.png', 'Light 1 - owned Brawler alternate'),
        ('brawler_light2_30.png', 'Light 2 - owned Brawler alternate'),
        ('brawler_guard_30.png', 'Held guard - separate from home stance'),
        ('plain_fire_30.png', 'Actual casting pose - corrected source basis')])
    motion_section = f'<section class="section" id="motion"><span class="eyebrow">BUILD / ACTUAL OWNED ANIMATION KEYS</span><h2>Motion before polish.</h2><p>{len(motion["clips"])} owned clips, 100 captures at authored key timestamps, a scrub-able two-hit montage and measured handover mismatches. Casting-axis correction is verified against the actual exported source/target joint samples. Valid Unity skin-v2 Humanoid import and 60 raw Mecanim poses are now in the motion viewer; contact, support, twist and garment gates remain pending. <a href="FreeGuild-Motion-Lab.html">Open the motion viewer, combo flow and recovery plan</a>.</p><div style="display:grid;grid-template-columns:repeat(4,1fr);gap:12px">{pictures}</div></section>'
    out = out.replace('<section class="section" id="development">', motion_section + '<section class="section" id="development">', 1)
    out = out.replace('<a href="#development">', '<a href="#motion">Owned motion lab</a><a href="#development">', 1)
(DOC / 'FreeGuild-Build-Spec.md').write_text('# FreeGuild - merged build specification\n\nGenerated export. Authoritative GDD: latest MASTER_GDD block in TANDEM_REASONING.log.\n\n' + gdd + '\n\n---\n\n' + integration_source + '\n\n---\n\n' + kit + '\n', encoding='utf-8')
out = out.replace('Hall exterior in the faceted, matte direction. Repeated timber bays, framed doorway, red/bone cloth and a clear sparring courtyard.', 'Hall exterior in the faceted, matte direction. Repeated timber bays, framed doorway, red/bone cloth and a clear sparring courtyard. Lake and steps are illustrative: no owned water asset is established; steps require an owned-rock derivation or removal.')
out = out.replace('Alternative material and foliage treatment. This remains a style proposal; the source-piece construction vocabulary stays the base.', 'Alternative material and foliage treatment. This remains a style proposal; the source-piece construction vocabulary stays the base. Lake and steps are illustrative and need removal or a verified owned-piece derivation.')
(DOC / "FreeGuild-Design-Atlas.html").write_text(out, encoding="utf-8")
print(json.dumps({"html":str(DOC / "FreeGuild-Design-Atlas.html"),"bytes":len(out.encode()),"gdd_sections":len(gdd_links),"kit_sections":len(kit_links),"concepts":len(cards)+len(character_cards),"source_sheets":len(sheets)}))
