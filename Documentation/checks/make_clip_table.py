"""Curated clip table for the M0.5 spike, generated from Documentation/catalog-clips.txt (durations looked up, never typed).
python -I Documentation/checks/make_clip_table.py  -> Documentation/build/CLIP_TABLE.md + clip_table.json
Fit = clip duration / GDD window total (H). ok 0.8-1.25; retime if within 0.5-2.0; else retune the (H) window or pick another clip.
This is a PROPOSAL for M0.5: which clips are only known by name/duration; contact frames and rest poses are UNKNOWN until measured in the editor."""
import json, os, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
cat = {}
for line in open(os.path.join(ROOT, "Documentation", "catalog-clips.txt"), encoding="utf-8"):
    p = line.rstrip("\n").split("\t")
    if len(p) < 8:
        continue
    cat.setdefault((p[0], p[1]), []).append({"dur": float(p[2].rstrip("s")), "loop": p[3], "speed": p[5].replace("speed=", ""), "path": p[7]})

BR, FM, EX, KI = "Brawler Animations Sample", "FightingMotionsVolume1", "ExplosiveLLC", "Kevin Iglesias"
# role, GDD window total (s, H) or None, primary [(pack, clip)], alternates, note
ROLES = [
    ("stance (home guard)", None, [(BR, "Fight_Idle")], [(FM, "idle_A"), (EX, "Unarmed-Idle")], "GDD: calm guard; Fight_Idle is a loop to inertialize into/out of."),
    ("locomotion run", None, [(EX, "Unarmed-Run-Forward")], [(EX, "Unarmed-Run-Backward"), (EX, "Unarmed-Run-Forward-Left"), (EX, "Unarmed-Run-Forward-Right")], "root motion 3.6-4.2 m/s; motor is the only mover, so use the clip's measured speed to scale playback."),
    ("strafe / walk", None, [(EX, "Unarmed-Strafe-Forward")], [(EX, "Unarmed-Strafe-Backward"), (BR, "Walk_F"), (BR, "Walk_B")], "strafe ~2.1 m/s."),
    ("light strike (L)", 0.65, [(EX, "Unarmed-Attack-L1")], [(BR, "Atk_P_1"), (FM, "hp_hook_left_Tiramis"), (FM, "hp_straight_right_A")], "two-hit phrase = L1 then R1 (each its own action)."),
    ("light strike (R)", 0.65, [(EX, "Unarmed-Attack-R1")], [(BR, "Atk_P_2"), (FM, "hp_upper_right_A")], "short clips (hp_straight_A 0.20 s) are too fast to read; avoid below 0.45 s."),
    ("heavy strike", 1.15, [(BR, "Atk_K_1")], [(BR, "Atk_K_2"), (FM, "hk_rh_right_A"), (BR, "Atk_P_2")], "no clip reaches the 1.15 s window naturally."),
    ("guard hold", None, [(BR, "Guard_Idle")], [], "loop; capacity rules live in code."),
    ("guard hit / block reaction", None, [(BR, "Guard_Hit")], [(FM, "hb_front_A"), (FM, "bb_front_A"), (FM, "lb_front_A")], "height variants are presentation only (GDD)."),
    ("guard break", 1.0, [], [(BR, "Hit_F"), (FM, "knockdown_A2")], "GAP: no clip named for it; needs a capture decision (stagger + open arms via procedural layer)."),
    ("step (back/forward)", 0.35, [(BR, "Quickstep_B")], [(BR, "Quickstep_F")], "forward/back only."),
    ("step (lateral)", 0.35, [], [(EX, "Unarmed-Strafe-Forward"), (EX, "Unarmed-DiveRoll-Forward1")], "GAP: no lateral quickstep clip; options: mirrored Humanoid pose of a strafe start, or DiveRoll-Left/Right if present; decide at M0.5 by capture."),
    ("throw reach-in", 0.30, [(BR, "Grab_Start")], [(BR, "Grab_Idle_A"), (BR, "Grab_Walk_F_A")], "legal-staging check before the paired motion."),
    ("throw paired", None, [(BR, "Grab_Throw_A")], [(BR, "Grab_Throw_B")], "A/B pairing and relative staging UNKNOWN until viewed (sample scene stages them ~0.70 m apart)."),
    ("grab escape / mutual", None, [(BR, "Grab_Break_A")], [(BR, "Grab_Break_B")], "use for mutual-attempt neutral reset."),
    ("cast plain", 1.2, [(KI, "HumanM@MagicAttackDirect1H01_R")], [(KI, "HumanM@MagicAttackDirect1H01_L")], "commitment point inside the clip UNKNOWN."),
    ("cast charged hold", None, [(KI, "HumanM@MagicAttackDirect2H01 - Load")], [], "loop; held until release or interrupt."),
    ("cast charged release", None, [(KI, "HumanM@MagicAttackDirect2H01 - Cast")], [(KI, "HumanM@MagicAttackOmni01")], ""),
    ("cast stance in/idle/out", None, [(KI, "HumanM@CastingEnter01")], [(KI, "HumanM@CastingIdle01"), (KI, "HumanM@CastingExit01")], "optional; plain cast is available from the stance."),
    ("cast interrupted", None, [(KI, "HumanM@CastingDamage01")], [], ""),
    ("hit reaction head", None, [(FM, "hd_front_A")], [(FM, "hd_front_B"), (BR, "Hit_F")], ""),
    ("hit reaction body", None, [(FM, "bd_front_A")], [(BR, "Hit_B"), (EX, "Unarmed-GetHit-F1")], ""),
    ("hit reaction low", None, [(FM, "ld_front_A")], [(EX, "Unarmed-GetHit-L1")], ""),
    ("knockdown", None, [(FM, "knockdown_A")], [(FM, "knockdown_loop_A"), (EX, "Unarmed-Knockdown1")], "knockdown_loop_A is the lie-down hold."),
    ("get up", None, [(FM, "getup_A")], [(EX, "Unarmed-Getup1")], "Explosive getup is 4.0 s: too long; prefer FightingMotions."),
    ("jump / fall / land", None, [(EX, "Unarmed-Jump")], [(EX, "Unarmed-Fall"), (EX, "Unarmed-Land")], "traversal only, no attacks (GDD)."),
]


def look(pack, name):
    r = cat.get((pack, name))
    return r[0] if r else None


def fit(dur, win):
    if win is None or dur is None:
        return ""
    r = dur / win
    if 0.8 <= r <= 1.25:
        return "ok (x%.2f)" % r
    if 0.5 <= r <= 2.0:
        return "retime x%.2f" % (1 / r)
    return "RETUNE window or pick another clip (x%.2f)" % r


rows, md = [], ["# Curated clip table (M0.5 proposal)", "",
                "Generated by `Documentation/checks/make_clip_table.py` from `Documentation/catalog-clips.txt`. Durations are looked up from the catalogue.",
                "Windows are the GDD starting values (H). Contact frames, rest poses and pairing are UNKNOWN until measured in the editor. Clips are only used after the M0.5 capture review.", "",
                "| Role | Primary (pack / clip, s) | GDD window (H) | Fit | Alternates | Note |", "|---|---|---|---|---|---|"]
missing = []
for role, win, prim, alts, note in ROLES:
    p_txt, p_dur = [], None
    for pack, name in prim:
        c = look(pack, name)
        if c is None:
            missing.append((pack, name))
            p_txt.append("%s / %s (NOT IN CATALOG)" % (pack, name))
        else:
            p_dur = p_dur or c["dur"]
            p_txt.append("%s / %s (%.2f s, %s)" % (pack, name, c["dur"], c["loop"]))
    a_txt = []
    for pack, name in alts:
        c = look(pack, name)
        a_txt.append("%s (%.2f s)" % (name, c["dur"]) if c else "%s (NOT IN CATALOG)" % name)
        if c is None:
            missing.append((pack, name))
    f = fit(p_dur, win)
    md.append("| %s | %s | %s | %s | %s | %s |" % (role, "<br>".join(p_txt) or "GAP", ("%.2f s" % win) if win else "-", f or ("GAP" if not prim else "-"), "; ".join(a_txt) or "-", note))
    rows.append({"role": role, "gdd_window_s": win, "primary": [{"pack": a, "clip": b, "dur": (look(a, b) or {}).get("dur")} for a, b in prim],
                 "alternates": [{"pack": a, "clip": b, "dur": (look(a, b) or {}).get("dur")} for a, b in alts], "fit": f, "note": note})
md += ["", "## Findings", ""]
retune = [r for r in rows if r["fit"].startswith("RETUNE")]
retime = [r for r in rows if r["fit"].startswith("retime")]
gaps = [r["role"] for r in rows if not r["primary"]]
md.append("- Clips needing a RETUNE of the (H) window (clip far from the window): %s." % (", ".join("%s [%s]" % (r["role"], r["fit"]) for r in retune) or "none"))
md.append("- Clips needing a retime: %s." % (", ".join("%s [%s]" % (r["role"], r["fit"]) for r in retime) or "none"))
md.append("- Gaps with no primary clip: %s." % (", ".join(gaps) or "none"))
md.append("- Names not found in the catalogue: %s." % (", ".join("%s/%s" % m for m in missing) or "none"))
os.makedirs(os.path.join(ROOT, "Documentation", "build"), exist_ok=True)
open(os.path.join(ROOT, "Documentation", "build", "CLIP_TABLE.md"), "w", encoding="utf-8").write("\n".join(md) + "\n")
json.dump({"generated_from": "Documentation/catalog-clips.txt", "roles": rows, "missing": missing}, open(os.path.join(ROOT, "Documentation", "build", "clip_table.json"), "w", encoding="utf-8"), indent=1)
sys.stdout.reconfigure(encoding="utf-8")
print("\n".join(md[-6:]))
print("roles", len(rows), "missing names", len(missing))
