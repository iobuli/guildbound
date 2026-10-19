"""Kev context: turn the project's architecture docs and code headers into components, and retrieve them.

    python3 tools/kev/context/build.py index  [--out tools/kev/context/components.json]
    python3 tools/kev/context/build.py ask "change the arm clearance"  [--k 5]
    python3 tools/kev/context/build.py eval  tools/kev/context/gold.json

Components come from three sources:
  - docs/HANDOFF.md, split by its own tables and lists: module map rows (kind "module"), the
    ownership table (kind "owner"), data files (kind "data"), monitor invariants (kind "invariant"),
    the frame order (kind "frame") and the rules (kind "rule").
  - every Rust file under crates/game/src: its `//!` header (kind "code"), plus its public items.
  - every Python tool under tools/model and tools/anim: its module docstring (kind "tool").

Retrieval is BM25 over each component's name, file paths, and summary. Nothing is learned; the
index is rebuilt from the source files, so it cannot drift from them except between rebuilds.
"""
import argparse
import ast
import json
import math
import os
import re
import sys
from collections import Counter

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
HANDOFF = os.path.join(REPO, "docs", "HANDOFF.md")
STOP = set("the a an and or of to in on for with by at is are be as it this that from into per each any not no do does".split())


def tokens(text):
    out = []
    for w in re.findall(r"[a-z0-9]+", text.lower()):
        if len(w) < 2 or w in STOP:
            continue
        out.append(w[:-1] if len(w) > 3 and w.endswith("s") else w)
    return out


def table_rows(lines, start_pattern):
    """Rows of the first markdown table after a heading line that matches start_pattern."""
    i = next((n for n, l in enumerate(lines) if re.match(start_pattern, l)), None)
    if i is None:
        return []
    rows, started = [], False
    for l in lines[i + 1:]:
        if l.startswith("|"):
            started = True
            cells = [c.strip() for c in l.strip().strip("|").split("|")]
            if set("".join(cells)) <= set("-: "):
                continue
            rows.append(cells)
        elif started:
            break
    return rows[1:] if rows else []  # the first row is the header


def bullets(lines, start_pattern):
    i = next((n for n, l in enumerate(lines) if re.match(start_pattern, l)), None)
    if i is None:
        return []
    out = []
    for l in lines[i + 1:]:
        if l.startswith("## "):
            break
        if l.startswith("- ") or re.match(r"^\d+\. ", l):
            out.append(l)
    return out


def code_paths(text):
    paths = re.findall(r"`([^`]+\.(?:rs|py|wgsl|json|glb|md))`", text)
    # A bare module name in backticks (`input`, `ai`) resolves to the file that has that name.
    for name in re.findall(r"`([a-z_][a-z0-9_]*)`", text):
        hits = _module_files().get(name, [])
        paths.extend(h for h in hits if h not in paths and len(hits) <= 2)
    return paths


_MODULES = None


def _module_files():
    global _MODULES
    if _MODULES is None:
        _MODULES = {}
        for dirpath, _, files in os.walk(os.path.join(REPO, "crates", "game", "src")):
            for name in files:
                if name.endswith(".rs"):
                    rel = os.path.relpath(os.path.join(dirpath, name), REPO)
                    _MODULES.setdefault(name[:-3], []).append(rel)
    return _MODULES


def index_docs():
    lines = open(HANDOFF, encoding="utf-8").read().splitlines()
    comps = []
    for cells in table_rows(lines, r"^## Module map"):
        if len(cells) >= 2:
            files = code_paths(cells[0]) or [cells[0]]
            comps.append({"kind": "module", "name": cells[0], "files": files, "summary": cells[1]})
    for cells in table_rows(lines, r"^## Ownership"):
        if len(cells) >= 2:
            comps.append({"kind": "owner", "name": cells[0], "files": code_paths(" ".join(cells)), "summary": " ".join(cells[1:])})
    for b in bullets(lines, r"^## Data files"):
        comps.append({"kind": "data", "name": b[:60], "files": code_paths(b), "summary": b})
    for cells in table_rows(lines, r"^## Monitor invariants"):
        if len(cells) >= 2:
            comps.append({"kind": "invariant", "name": cells[0], "files": ["crates/game/src/play/monitor.rs"], "summary": " ".join(cells)})
    for b in bullets(lines, r"^## Frame order"):
        comps.append({"kind": "frame", "name": b[:60], "files": code_paths(b), "summary": b})
    for l in lines:
        if l.startswith("Rules:"):
            comps.append({"kind": "rule", "name": "rules", "files": [], "summary": l})
    return comps


def index_code():
    comps = []
    for base in ("crates/game/src", "tools/model", "tools/anim"):
        for dirpath, _, files in os.walk(os.path.join(REPO, base)):
            for name in sorted(files):
                full = os.path.join(dirpath, name)
                rel = os.path.relpath(full, REPO)
                if name.endswith(".rs"):
                    header = [l[3:].strip() for l in open(full, encoding="utf-8", errors="ignore") if l.startswith("//!")][:6]
                    src = open(full, encoding="utf-8", errors="ignore").read()
                    # Private functions count too: a routine's name is often the only place its purpose is written down.
                    items = re.findall(r"^\s*(?:pub(?:\([a-z]+\))? )?(?:fn|struct|enum|const) (\w+)", src, re.M)
                    comps.append({"kind": "code", "name": rel, "files": [rel], "summary": " ".join(header) + " " + " ".join(items[:80])})
                elif name.endswith(".py") and "__pycache__" not in rel:
                    try:
                        doc = ast.get_docstring(ast.parse(open(full, encoding="utf-8").read())) or ""
                    except SyntaxError:
                        doc = ""
                    if doc:
                        comps.append({"kind": "tool", "name": rel, "files": [rel], "summary": doc[:600]})
    return comps


def build():
    return index_docs() + index_code()


class Bm25:
    def __init__(self, comps, k1=1.2, b=0.75):
        self.comps, self.k1, self.b = comps, k1, b
        self.docs = []
        for c in comps:
            # Names and file paths weigh double: a query naming a file should find that file.
            text = " ".join([c["name"], " ".join(c["files"]), c["name"], " ".join(c["files"]), c["summary"]])
            self.docs.append(Counter(tokens(text)))
        self.len = [sum(d.values()) for d in self.docs]
        self.avg = sum(self.len) / max(len(self.len), 1)
        df = Counter()
        for d in self.docs:
            df.update(d.keys())
        n = len(self.docs)
        self.idf = {t: math.log(1 + (n - f + 0.5) / (f + 0.5)) for t, f in df.items()}

    def scores(self, query):
        q = tokens(query)
        out = []
        for d, L in zip(self.docs, self.len):
            s = 0.0
            for t in q:
                f = d.get(t, 0)
                if f:
                    s += self.idf[t] * f * (self.k1 + 1) / (f + self.k1 * (1 - self.b + self.b * L / self.avg))
            out.append(s)
        return out

    def top(self, query, k):
        s = self.scores(query)
        order = sorted(range(len(s)), key=lambda i: -s[i])
        return [(self.comps[i], s[i]) for i in order[:k] if s[i] > 0]


def files_of(hits):
    seen, out = set(), []
    for c, _ in hits:
        for f in c["files"]:
            if f not in seen:
                seen.add(f)
                out.append(f)
    return out


def evaluate(gold_path, k=5):
    gold = json.load(open(gold_path))
    comps = build()
    kev = Bm25(comps)
    # Baseline: the same ranking over file names alone, no docs and no summaries.
    names = [{"kind": "name", "name": c["name"], "files": c["files"], "summary": ""} for c in comps]
    base = Bm25(names)
    rows = []
    for g in gold:
        for label, idx in (("kev", kev), ("filename", base)):
            hits = idx.top(g["task"], 50)
            got = files_of(hits)[:k]
            rank = next((n + 1 for n, f in enumerate(files_of(hits)) if f in g["files"]), None)
            rows.append((label, g["task"], len(set(got) & set(g["files"])) / len(g["files"]), rank))
    for label in ("kev", "filename"):
        mine = [r for r in rows if r[0] == label]
        recall = sum(r[2] for r in mine) / len(mine)
        mrr = sum((1 / r[3]) if r[3] else 0 for r in mine) / len(mine)
        found = sum(1 for r in mine if r[3] and r[3] <= k)
        print(f"{label:<9} recall@{k} {recall:.2f}   MRR {mrr:.2f}   tasks with a gold file in top {k}: {found}/{len(mine)}")
    return rows


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("index")
    p.add_argument("--out", default=os.path.join(os.path.dirname(__file__), "components.json"))
    q = sub.add_parser("ask")
    q.add_argument("task")
    q.add_argument("--k", type=int, default=5)
    e = sub.add_parser("eval")
    e.add_argument("gold")
    args = ap.parse_args()
    if args.cmd == "index":
        comps = build()
        with open(args.out, "w") as f:
            json.dump(comps, f, indent=1)
        kinds = Counter(c["kind"] for c in comps)
        print(f"{len(comps)} components: " + ", ".join(f"{k} {v}" for k, v in sorted(kinds.items())))
    elif args.cmd == "ask":
        for c, s in Bm25(build()).top(args.task, args.k):
            print(f"{s:6.2f}  [{c['kind']}] {c['name'][:70]}")
            print(f"        {c['summary'][:160]}")
    else:
        evaluate(args.gold)


if __name__ == "__main__":
    sys.exit(main())
