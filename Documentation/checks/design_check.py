"""FreeGuild design/repo checks. Run from the repo root:  python -I Documentation/checks/design_check.py
Exit 0 = all PASS (WARN allowed), 1 = at least one FAIL. Writes Documentation/checks/last_report.txt.
Checks are read-only. Each check: id, what it proves, PASS/WARN/FAIL + evidence."""
import hashlib, json, os, re, subprocess, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
LOG = os.path.join(ROOT, "TANDEM_REASONING.log")
DOC = os.path.join(ROOT, "Documentation")
IP_TERMS = ["zelda", "witcher", "baldur", "divinity", "warcraft", "oldcraft", "marvel", "sparking zero", "tenkaichi",
            "dragon ball", "runescape", "fallout", "final fantasy", "kingdom come", "skyrim", "solo leveling", "isekai",
            "mixamo", "lugaru", "illarion", "godot"]
results = []


def rec(cid, status, msg):
    results.append((cid, status, msg))


def read(p):
    with open(p, "rb") as f:
        return f.read().decode("utf-8", "ignore").replace("\r\n", "\n")


def git(*a):
    return subprocess.run(["git", "-C", ROOT, *a], capture_output=True, text=True).stdout


def last_block(text, name):
    blocks = re.findall(r"BEGIN_%s\n(.*?)\nEND_%s" % (name, name), text, re.S)
    return blocks[-1] if blocks else None


def ip_hits(text):
    t = text.lower()
    return {w: t.count(w) for w in IP_TERMS if w in t}


# C01 manifests identical
a, b = (os.path.join(ROOT, f) for f in ("TANDEM_CLAUDE.manifest.md", "TANDEM_CODEX.manifest.md"))
if os.path.exists(a) and os.path.exists(b):
    rec("C01 manifests identical", "PASS" if read(a) == read(b) else "FAIL", "TANDEM_CLAUDE/CODEX manifests")
else:
    rec("C01 manifests identical", "FAIL", "manifest missing")

log = read(LOG) if os.path.exists(LOG) else ""
# C02 master GDD present, 12 sections in order
gdd = last_block(log, "MASTER_GDD")
if gdd is None:
    rec("C02 master GDD", "FAIL", "no BEGIN_MASTER_GDD block")
else:
    secs = [int(x) for x in re.findall(r"^## (\d+)\. ", gdd, re.M)]
    ok = secs == list(range(1, 13))
    ver = re.search(r"Status: ([^\n]+)", gdd)
    rec("C02 master GDD", "PASS" if ok else "FAIL", "sections %s | %s" % (secs, ver.group(1)[:80] if ver else "no status"))
    # C03 IP scan of GDD
    hits = ip_hits(gdd)
    rec("C03 GDD free of external IP", "PASS" if not hits else "FAIL", str(hits) if hits else "clean")
    # C04 every (H) number section present; P0/P1 split stated
    rec("C04 P0/P1 split stated", "PASS" if "P0" in gdd and "P1" in gdd and "steps 1-9" in gdd else "FAIL", "P0 steps 1-9 + P1 present")

# C05 kit lexicon present with three libraries
kit = last_block(log, "KIT_LEXICON")
if kit:
    libs = [k for k in ("OBJECT KIT", "CHARACTER KIT", "EFFECTS KIT", "SCENE COMPOSITION") if k in kit]
    rec("C05 kit lexicon separated", "PASS" if len(libs) == 4 else "FAIL", "found " + ", ".join(libs))
    hits = ip_hits(kit)
    rec("C06 kit lexicon free of external IP", "PASS" if not hits else "FAIL", str(hits) if hits else "clean")
else:
    rec("C05 kit lexicon separated", "FAIL", "no KIT_LEXICON block")

# C07 super prompt + STATE
gsp = last_block(log, "SUPER_PROMPT")
state = last_block(log, "STATE")
rec("C07 super prompt + STATE", "PASS" if gsp and state else "WARN", "GSP %s, STATE %s" % (bool(gsp), bool(state)))

# C08 atlas present, IP-clean text, carries GDD sections
atlas = os.path.join(DOC, "FreeGuild-Design-Atlas.html")
if os.path.exists(atlas):
    t = re.sub(r"data:[^\"')\s]+", "", read(atlas))
    t = re.sub(r"<[^>]+>", " ", t)
    hits = ip_hits(t)
    need = ["Gold path", "Build prompt", "OBJECT KIT", "CHARACTER KIT", "EFFECTS KIT"]
    miss = [n for n in need if n.lower() not in t.lower()]
    status = "FAIL" if hits else ("WARN" if miss else "PASS")
    rec("C08 design atlas", status, "IP %s | missing %s | %.1f MB" % (hits or "clean", miss or "none", os.path.getsize(atlas) / 1e6))
    if gdd and "AMEND-CHR-01" in gdd and "AMEND-CHR-01" not in t and "dressing corner" not in t.lower():
        rec("C09 atlas matches latest GDD", "WARN", "atlas predates AMEND-CHR-01")
    else:
        rec("C09 atlas matches latest GDD", "PASS", "creator amendment present or not required")
else:
    rec("C08 design atlas", "WARN", "atlas not built")

# C10 concept images: every file listed in concept-prompts.json exists and has an approval_state
cp = os.path.join(DOC, "concepts", "concept-prompts.json")
if os.path.exists(cp):
    data = json.load(open(cp, encoding="utf-8"))
    items = data if isinstance(data, list) else data.get("images", data.get("items", []))
    missing, nostate = [], []
    for it in items if isinstance(items, list) else []:
        path = it.get("output") or it.get("path") or it.get("file")
        if path and not os.path.exists(os.path.join(ROOT, path)) and not os.path.exists(path):
            missing.append(path)
        if not (it.get("approval_state") or it.get("status")):
            nostate.append(it.get("id", path))
    rec("C10 concept provenance", "PASS" if not missing and not nostate else "WARN",
        "%d entries | missing files %s | no approval_state %s" % (len(items) if isinstance(items, list) else 0, missing[:5], nostate[:5]))
else:
    rec("C10 concept provenance", "WARN", "no concept-prompts.json")

# C11 HUD manifest valid; if sheet exists, check size and that text-free/alpha cells are sane
hm = os.path.join(DOC, "concepts", "hud", "hud_sheet_manifest.json")
if os.path.exists(hm):
    m = json.load(open(hm, encoding="utf-8"))
    ids = [c["id"] for c in m["cells"]]
    ok = len(ids) == 64 and len(set(ids)) == 64 and all(c["layer"] in m["layers"] or c["layer"] == "reserved" for c in m["cells"])
    rec("C11 HUD manifest", "PASS" if ok else "FAIL", "%d cells, unique %d" % (len(ids), len(set(ids))))
    hud_dir = os.path.dirname(hm)
    prod = [f for f in os.listdir(hud_dir) if f.lower().startswith("hud_sheet_production") and f.lower().endswith(".png")]
    if prod:
        try:
            from PIL import Image
            for f_ in prod:
                im = Image.open(os.path.join(hud_dir, f_))
                rec("C12 HUD production sheet " + f_, "PASS" if im.size == tuple(m["sheet"]["size_px"]) and im.mode == "RGBA" else "FAIL",
                    "size %s mode %s (want %s RGBA)" % (im.size, im.mode, m["sheet"]["size_px"]))
        except ImportError:
            rec("C12 HUD production sheet", "WARN", "PIL missing")
    else:
        qa = os.path.join(hud_dir, "qa")
        tallies = []
        if os.path.isdir(qa):
            for d_ in sorted(os.listdir(qa)):
                mm = os.path.join(qa, d_, "_measured_manifest.json")
                if d_.startswith("slices_") and os.path.exists(mm):
                    r_ = json.load(open(mm, encoding="utf-8"))
                    t_ = r_["tally"]
                    tallies.append("%s: %d ok, %d empty-expected, %d need attention" % (
                        r_["sheet"], t_.get("ok", 0), t_.get("empty_expected", 0), sum(v for k_, v in t_.items() if k_ not in ("ok", "empty_expected"))))
        rec("C12 HUD sheet", "WARN", "no production sheet yet (candidates only). " + ("; ".join(tallies) if tallies else "no measured candidates"))

# C13 licence guard: no pack-derived meshes or raw pack folders tracked in git
tracked = git("ls-files").splitlines()
bad = [f for f in tracked if f.lower().endswith((".fbx", ".blend")) and ("experiments/" in f or "Documentation/" in f)]
packs = ["Assets/ExplosiveLLC/", "Assets/Kevin Iglesias/", "Assets/FightingMotionsVolume1/", "Assets/Hovl Studio/", "Assets/MoCapCentral/", "Assets/RPGPP_LT/"]
bad += [f for f in tracked if any(f.startswith(p) for p in packs)][:5]
rec("C13 no pack/derived meshes in git", "PASS" if not bad else "FAIL", "offenders %s" % bad[:5] if bad else "clean")

# C14 no _OldGodot or Library tracked
bad = [f for f in tracked if f.startswith(("_OldGodot/", "Library/"))]
rec("C14 archives/Library not tracked", "PASS" if not bad else "FAIL", "%d offenders" % len(bad))

# C15 source dirs untouched during design (uncommitted changes under Assets/Packages/ProjectSettings)
st = git("status", "--porcelain")
src_changes = [l for l in st.splitlines() if re.match(r"..\s+\"?(Assets|Packages|ProjectSettings)/", l)]
rec("C15 source dirs unchanged (design phase)", "PASS" if not src_changes else "WARN",
    "%d uncommitted source changes (pre-existing WIP may exist)" % len(src_changes))

# C16 log append-only: every header well-formed, no edited earlier entries vs HEAD
heads = re.findall(r"^## \[(claude|codex|user)\]", log, re.M)
committed = subprocess.run(["git", "-C", ROOT, "show", "HEAD:TANDEM_REASONING.log"], capture_output=True).stdout.decode("utf-8", "ignore").replace("\r\n", "\n")
prefix_ok = log.startswith(committed) if committed else True
rec("C16 log append-only", "PASS" if prefix_ok else "FAIL", "%d entries; working log %s committed prefix" % (len(heads), "extends" if prefix_ok else "DIVERGES from"))

# C17 insight ledger: no pile-up of unanswered insights
led = os.path.join(os.path.dirname(__file__), "insights.json")
if os.path.exists(led):
    items = json.load(open(led, encoding="utf-8"))["insights"]
    op = [i["id"] for i in items if i["status"] == "open"]
    rec("C17 insights answered", "PASS" if len(op) <= 5 else "WARN", "%d open of %d: %s" % (len(op), len(items), ", ".join(op[:8])))

# report
width = max(len(r[0]) for r in results)
lines = ["%-*s  %-4s  %s" % (width, c, s, m) for c, s, m in results]
fails = sum(1 for r in results if r[1] == "FAIL")
warns = sum(1 for r in results if r[1] == "WARN")
lines.append("TOTAL %d checks, %d FAIL, %d WARN" % (len(results), fails, warns))
out = "\n".join(lines)
sys.stdout.reconfigure(encoding="utf-8")
print(out)
open(os.path.join(os.path.dirname(__file__), "last_report.txt"), "w", encoding="utf-8").write(out + "\n")
sys.exit(1 if fails else 0)
