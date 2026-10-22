"""No-glitch report for catalogue clips.

    python3 -I tools/kev/quality/__main__.py check NAME [--raw]
    python3 -I tools/kev/quality/__main__.py summary
    python3 -I tools/kev/quality/__main__.py contact NAME OUT.png

Run from the repo root. `python3 -I` does not put the working directory on the path, so `-m quality` will not resolve.
`check` reads the smoothed reference in staging/kev/refs (catalog.py rebuild writes it) and prints
one line per issue with the frame ranges. `summary` prints the before/after counts stored on the index.
"""
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
KEV = os.path.dirname(HERE)
sys.path.insert(0, KEV)
import quality.core as quality  # noqa: E402

INDEX = os.path.join(KEV, "catalog", "index.json")
REPO = os.path.dirname(os.path.dirname(KEV))
REFDIR = os.path.join(REPO, "staging", "kev", "refs")
STUDIO = os.path.join(KEV, "studio", "server.py")


def studio():
    spec = importlib.util.spec_from_file_location("kev_studio_server", STUDIO)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def body_of(srv, model):
    info = srv.model_info(model)
    return info["bm"]["body"], quality.lengths_from_body(info["bm"])


def check(name, raw=False):
    path = os.path.join(REFDIR, name + ("__raw" if raw else "") + ".json")
    if not os.path.isfile(path):
        sys.exit(f"no reference at {path}. Run: python3 -I tools/kev/catalog/catalog.py rebuild")
    ref = json.load(open(path))
    body, lengths = body_of(studio(), ref["model"])
    names, L, hands = quality.ref_arrays(ref)
    fps = 60.0
    if len(ref["samples"]) > 1:
        dt = ref["samples"][1]["t"] - ref["samples"][0]["t"]
        fps = 1.0 / dt if dt > 1e-6 else 60.0
    issues = quality.validate(L, names, fps, ref["floor"], ref["height"], hands, body, lengths)
    ix = json.load(open(INDEX))["animations"].get(name, {}) if os.path.isfile(INDEX) else {}
    motion = (ix.get("motion") or {}).get("state")
    print(quality.format_issues(name + (" raw" if raw else ""), issues, motion))
    if ix.get("motion", {}).get("state") == "frozen":
        sys.exit(1)


def summary():
    ix = json.load(open(INDEX))["animations"]
    rows = []
    frozen = []
    print(f"{'name':<24} {'motion':<8} {'wrist':>7} {'before':>7} {'after':>7}  kinds still open")
    for name, e in sorted(ix.items()):
        motion = (e.get("motion") or {}).get("state", "-")
        q = e.get("quality") or {}
        before = sum((q.get("before") or {}).values())
        after = sum((q.get("after") or {}).values())
        kinds = ",".join(sorted((q.get("after") or {})))
        wrist = (e.get("metrics") or {}).get("wrist_speed_hs", "-")
        print(f"{name:<24} {motion:<8} {wrist:>7} {before:>7} {after:>7}  {kinds}")
        rows.append((before, after))
        if motion == "frozen":
            frozen.append(name)
    b, a = sum(r[0] for r in rows), sum(r[1] for r in rows)
    print(f"{'TOTAL':<24} {'':<8} {'':>7} {b:>7} {a:>7}  clips {len(rows)}  frozen {len(frozen)}")
    if frozen:
        print("frozen:", ", ".join(frozen))
        sys.exit(1)


def main():
    args = sys.argv[1:]
    if not args or args[0] not in ("check", "summary", "contact"):
        print(__doc__.strip())
        sys.exit(2)
    if args[0] == "summary":
        return summary()
    if args[0] == "contact":
        if len(args) < 3:
            sys.exit("usage: quality contact NAME OUT.png")
        import quality.contact as contact
        print(contact.draw(args[1], args[2]))
        return
    check(args[1], raw="--raw" in args)


if __name__ == "__main__":
    main()
