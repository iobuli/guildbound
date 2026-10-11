"""Insight ledger + briefing tool (Claude -> Codex). Run from the repo root:
  python -I Documentation/checks/insight.py add --type FINDING --title "..." --evidence "path or number" --impact "GDD S4" --ask "..." --confidence verified
  python -I Documentation/checks/insight.py list [--open]
  python -I Documentation/checks/insight.py ack I-003 accepted|countered|closed [--note "..."]
  python -I Documentation/checks/insight.py digest          # what Codex should read at the start of a turn
  python -I Documentation/checks/insight.py post --agent claude [--title "..."]   # appends the digest to TANDEM_REASONING.log
Types: FINDING | RISK | DECISION | CORRECTION | HANDOFF.  Confidence: verified | inferred | hypothesis.
Status flow: open -> accepted | countered | closed.  Open insights are re-listed in every digest until answered."""
import argparse, datetime, json, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
LEDGER = os.path.join(HERE, "insights.json")
LOG = os.path.join(ROOT, "TANDEM_REASONING.log")
TYPES = ("FINDING", "RISK", "DECISION", "CORRECTION", "HANDOFF")
CONF = ("verified", "inferred", "hypothesis")


def load():
    if os.path.exists(LEDGER):
        return json.load(open(LEDGER, encoding="utf-8"))
    return {"schema": 1, "next": 1, "insights": []}


def save(d):
    json.dump(d, open(LEDGER, "w", encoding="utf-8"), indent=1, ensure_ascii=False)


def now():
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")


def fmt(i):
    return ("%s [%s | %s | %s] %s\n    evidence: %s\n    impact:   %s\n    ask:      %s%s"
            % (i["id"], i["type"], i["confidence"], i["status"], i["title"], i["evidence"], i["impact"], i["ask"],
               ("\n    note:     " + i["note"]) if i.get("note") else ""))


def cmd_add(a):
    assert a.type in TYPES and a.confidence in CONF, "bad type/confidence"
    d = load()
    iid = "I-%03d" % d["next"]
    d["next"] += 1
    d["insights"].append({"id": iid, "type": a.type, "title": a.title, "evidence": a.evidence, "impact": a.impact,
                          "ask": a.ask, "confidence": a.confidence, "status": "open", "created": now(), "note": ""})
    save(d)
    print("added", iid)


def cmd_list(a):
    for i in load()["insights"]:
        if a.open and i["status"] != "open":
            continue
        print(fmt(i))


def cmd_ack(a):
    assert a.status in ("accepted", "countered", "closed")
    d = load()
    for i in d["insights"]:
        if i["id"] == a.id:
            i["status"] = a.status
            i["note"] = a.note or i.get("note", "")
            i["answered"] = now()
            save(d)
            print(a.id, "->", a.status)
            return
    sys.exit("unknown id " + a.id)


def entries_since_last_codex():
    if not os.path.exists(LOG):
        return []
    heads = [l.strip() for l in open(LOG, encoding="utf-8", errors="ignore").read().replace("\r\n", "\n").split("\n")
             if l.startswith("## [")]
    last = max([n for n, h in enumerate(heads) if h.startswith("## [codex]")], default=-1)
    return heads[last + 1:]


def digest():
    d = load()
    op = [i for i in d["insights"] if i["status"] == "open"]
    out = ["INSIGHT DIGEST (Claude -> Codex) | %s" % now(), ""]
    out.append("1. OPEN INSIGHTS (%d) - answer each by id: ACCEPT / COUNTER (with reason) / CLOSE" % len(op))
    out += [fmt(i) for i in op] or ["    none"]
    ours = entries_since_last_codex()
    out += ["", "2. MY LOG ENTRIES SINCE YOUR LAST ENTRY (%d)" % len(ours)] + ["    " + h for h in ours[-8:]] + ([] if ours else ["    none"])
    rep = os.path.join(HERE, "last_report.txt")
    out += ["", "3. CHECKS (last run)"]
    if os.path.exists(rep):
        out.append("    " + open(rep, encoding="utf-8").read().strip().splitlines()[-1])
    else:
        out.append("    not run yet: python -I Documentation/checks/design_check.py")
    git = subprocess.run(["git", "-C", ROOT, "log", "--oneline", "-5"], capture_output=True, text=True).stdout.strip().splitlines()
    out += ["", "4. RECENT COMMITS"] + ["    " + g for g in git]
    out += ["", "5. HOW TO REPLY: one line per id, e.g.  I-004 ACCEPT  |  I-005 COUNTER: <reason + evidence>  |  I-006 CLOSE: already done in <path>.",
            "   Then run: python -I Documentation/checks/insight.py ack <id> accepted|countered|closed --note \"...\""]
    return "\n".join(out)


def cmd_post(a):
    text = digest()
    block = "\n## [%s] %s | INSIGHT | %s\n%s\n" % (a.agent, datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC"),
                                                   a.title or "insight digest", text)
    with open(LOG, "a", encoding="utf-8", newline="\n") as f:
        f.write(block)
    print("posted digest to TANDEM_REASONING.log")


def main():
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    p = sp.add_parser("add")
    for k in ("type", "title", "evidence", "impact", "ask", "confidence"):
        p.add_argument("--" + k, required=True)
    p.set_defaults(f=cmd_add)
    p = sp.add_parser("list"); p.add_argument("--open", action="store_true"); p.set_defaults(f=cmd_list)
    p = sp.add_parser("ack"); p.add_argument("id"); p.add_argument("status"); p.add_argument("--note", default=""); p.set_defaults(f=cmd_ack)
    p = sp.add_parser("digest"); p.set_defaults(f=lambda a: print(digest()))
    p = sp.add_parser("post"); p.add_argument("--agent", default="claude"); p.add_argument("--title", default=""); p.set_defaults(f=cmd_post)
    a = ap.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    a.f(a)


if __name__ == "__main__":
    main()
