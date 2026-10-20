"""Kev graph: run Kev's tools as nodes of a dependency graph, skipping the ones whose inputs did not change.

    python3 tools/kev/graph.py [tools/kev/pipeline.json] [--only STAGE ...] [--force] [--list] [--dry-run]

pipeline.json names the stages. Each stage has a command (`cmd` runs `python3 -I <cmd...>`, `exec` runs a
command as is), the files it reads (`inputs`, globs allowed), the stages it needs first (`after`) and what it
writes (`outputs`). A stage's key is a hash of its command, its input files (path, size, mtime) and the keys of
the stages it comes after; when the key matches the cache and every output exists, the stage is skipped. So a
change to roles.json reruns `nodes` and everything after it, and nothing else.

Stages run in topological order (Kahn's algorithm); a cycle or an unknown `after` name stops the run before
anything starts. `--only` runs the named stages and the stages they come after.
"""
import argparse
import glob
import hashlib
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))


def load(path):
    with open(path) as f:
        return json.load(f)


def order(stages):
    """Stage names in dependency order. Raises ValueError on a cycle or an unknown dependency."""
    for name, st in stages.items():
        for dep in st.get("after", []):
            if dep not in stages:
                raise ValueError(f"stage {name!r} comes after unknown stage {dep!r}")
    indegree = {n: len(st.get("after", [])) for n, st in stages.items()}
    users = {n: [] for n in stages}
    for n, st in stages.items():
        for dep in st.get("after", []):
            users[dep].append(n)
    ready = sorted(n for n, d in indegree.items() if d == 0)
    out = []
    while ready:
        n = ready.pop(0)
        out.append(n)
        for u in sorted(users[n]):
            indegree[u] -= 1
            if indegree[u] == 0:
                ready.append(u)
        ready.sort()
    if len(out) != len(stages):
        stuck = sorted(set(stages) - set(out))
        raise ValueError(f"dependency cycle among {stuck}")
    return out


def closure(stages, wanted):
    """`wanted` plus every stage they come after, transitively."""
    keep, todo = set(), list(wanted)
    while todo:
        n = todo.pop()
        if n in keep:
            continue
        if n not in stages:
            raise ValueError(f"unknown stage {n!r}")
        keep.add(n)
        todo.extend(stages[n].get("after", []))
    return keep


def files_of(patterns, root):
    found = set()
    for p in patterns:
        hits = glob.glob(os.path.join(root, p), recursive=True)
        found.update(h for h in hits if os.path.isfile(h))
    return sorted(found)


def stage_key(name, st, upstream_keys, root):
    """Hash of the command, the input files (path, size, mtime) and the upstream keys."""
    h = hashlib.sha1()
    h.update(json.dumps({"cmd": st.get("cmd"), "exec": st.get("exec")}, sort_keys=True).encode())
    for f in files_of(st.get("inputs", []), root):
        s = os.stat(f)
        h.update(f"{os.path.relpath(f, root)}:{s.st_size}:{int(s.st_mtime)}".encode())
    for dep in sorted(st.get("after", [])):
        h.update(f"{dep}={upstream_keys[dep]}".encode())
    return h.hexdigest()


def command(st):
    if "exec" in st:
        return list(st["exec"])
    return [sys.executable, "-I", *st["cmd"]]


def run(pipeline_path, only=None, force=False, dry_run=False, root=REPO, runner=None):
    """Run the pipeline. Returns [(stage, 'ran' | 'cached' | 'would run' | 'failed', seconds)]."""
    pipe = load(pipeline_path)
    stages = pipe["stages"]
    names = order(stages)
    if only:
        keep = closure(stages, only)
        names = [n for n in names if n in keep]
    cache_path = os.path.join(root, pipe.get("cache", "staging/kev/cache.json"))
    cache = load(cache_path) if os.path.exists(cache_path) else {}
    runner = runner or (lambda cmd: subprocess.run(cmd, cwd=root).returncode)
    keys, report = {}, []
    for n in names:
        st = stages[n]
        keys[n] = stage_key(n, st, keys, root)
        outputs_present = all(os.path.exists(os.path.join(root, o)) for o in st.get("outputs", []))
        if not force and cache.get(n) == keys[n] and outputs_present:
            report.append((n, "cached", 0.0))
            continue
        if dry_run:
            report.append((n, "would run", 0.0))
            continue
        t0 = time.perf_counter()
        code = runner(command(st))
        dt = round(time.perf_counter() - t0, 2)
        if code != 0:
            report.append((n, "failed", dt))
            break
        cache[n] = keys[n]
        report.append((n, "ran", dt))
        os.makedirs(os.path.dirname(cache_path), exist_ok=True)
        with open(cache_path, "w") as f:
            json.dump(cache, f, indent=1)
    return report


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pipeline", nargs="?", default=os.path.join(HERE, "pipeline.json"))
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--force", action="store_true")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    if args.list:
        stages = load(args.pipeline)["stages"]
        for n in order(stages):
            print(f"{n:<12} after {stages[n].get('after', []) or '-'}")
        return 0
    report = run(args.pipeline, args.only, args.force, args.dry_run)
    for n, status, dt in report:
        print(f"{n:<12} {status:<10} {dt:>7.2f} s")
    return 1 if any(s == "failed" for _, s, _ in report) else 0


if __name__ == "__main__":
    sys.exit(main())
